//! Bounded preparation of a complete human model record.
//!
//! Both passes read the same immutable contents. The first counts actual UTF-8
//! writes without allocating a record; exceeding the byte ceiling stops that
//! pass. The second reserves the admitted length once and reproduces those bytes.
//! No external sink is touched here. Time is two spelling/selection traversals;
//! retained record storage is its admitted length, subject to allocator rounding.
//! Compound spelling also uses fallible temporary frames proportional to depth;
//! the record byte ceiling does not bound those frames.

use std::{
    convert::Infallible,
    fmt,
    io::{self, Write},
};

use zetesis_core::ValueWriteError;
use zetesis_core::catalog::{AtomRef, TermRef};
use zetesis_cpu::{Cancellation, Stop};
use zetesis_objective::Score;
use zetesis_themelios::observation::ModelView;

use crate::{ColorMode, RunError};

#[derive(Clone, Copy)]
pub(crate) enum Contents<'a> {
    Atoms(&'a ModelView<'a>),
    Observed(&'a str),
}

pub(crate) struct Record(Vec<u8>);

impl Record {
    pub(crate) fn prepare(
        number: usize,
        contents: Contents<'_>,
        score: Option<&Score>,
        color: ColorMode,
        maximum: usize,
        cancellation: &Cancellation,
    ) -> Result<Self, RunError> {
        let mut length = Length {
            bytes: 0,
            maximum,
            refusal: None,
            stopped: None,
            cancellation,
        };
        if let Err(error) = write_record(&mut length, number, contents, score, color) {
            if let Some(observed) = length.refusal {
                return Err(RunError::ObservationOutputLimit {
                    observed,
                    limit: maximum,
                });
            }
            if let Some(stop) = length.stopped {
                return Err(RunError::PublicationStopped(stop));
            }
            return Err(RunError::Output(error));
        }
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(length.bytes)
            .map_err(|error| RunError::Output(io::Error::other(error)))?;
        write_record(&mut bytes, number, contents, score, color)?;
        debug_assert_eq!(bytes.len(), length.bytes);
        Ok(Self(bytes))
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        &self.0
    }
}

fn write_record(
    output: &mut impl Write,
    number: usize,
    contents: Contents<'_>,
    score: Option<&Score>,
    color: ColorMode,
) -> io::Result<()> {
    color.answer(output, number)?;
    match contents {
        Contents::Atoms(view) => {
            write_atoms(output, view.shown_atoms())?;
        }
        Contents::Observed(text) => writeln!(output, "{text}")?,
    }
    if let Some(score) = score {
        color.objective(output)?;
        for &(_, cost) in score.costs() {
            write!(output, " {cost}")?;
        }
        color.objective_end(output)?;
    }
    Ok(())
}

/// A bounded count of the attempted record prefix, not an output sink.
struct Length<'a> {
    bytes: usize,
    maximum: usize,
    refusal: Option<u128>,
    stopped: Option<Stop>,
    cancellation: &'a Cancellation,
}

impl Write for Length<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if let Err(stop) = self.cancellation.poll() {
            self.stopped = Some(stop);
            return Err(io::Error::other(stop));
        }
        let observed = self.bytes as u128 + bytes.len() as u128;
        if observed > self.maximum as u128 {
            self.refusal = Some(observed);
            return Err(io::Error::other("human model record byte ceiling"));
        }
        // The checked prefix is bounded by a usize ceiling, so this sum fits.
        self.bytes += bytes.len();
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn write_atoms<'a>(
    output: &mut impl Write,
    atoms: impl Iterator<Item = AtomRef<'a>>,
) -> io::Result<()> {
    for (index, atom) in atoms.enumerate() {
        if index != 0 {
            write!(output, " ")?;
        }
        if atom.predicate().sign() == zetesis_core::Sign::Negative {
            write!(output, "-")?;
        }
        write!(output, "{}", atom.predicate().name())?;
        if !atom.values().is_empty() {
            write!(output, "(")?;
            for (position, value) in atom.values().iter().enumerate() {
                if position != 0 {
                    write!(output, ",")?;
                }
                write_term(output, value)?;
            }
            write!(output, ")")?;
        }
    }
    writeln!(output)
}

fn write_term(output: &mut impl Write, value: TermRef<'_>) -> io::Result<()> {
    let mut sink = TermSink {
        output,
        failure: None,
    };
    // The record ceiling bounds emitted bytes, independently of traversal
    // frames. Use the fallible spelling API so frame allocation failure remains
    // an output error instead of becoming an unreported formatting failure.
    match value.write_with(&mut sink, usize::MAX, |_| Ok::<_, Infallible>(())) {
        Ok(()) => Ok(()),
        Err(ValueWriteError::Storage(error)) => Err(io::Error::other(error)),
        Err(ValueWriteError::Stopped(never)) => match never {},
        Err(ValueWriteError::Writer(error)) => {
            Err(sink.failure.unwrap_or_else(|| io::Error::other(error)))
        }
    }
}

struct TermSink<'a, W> {
    output: &'a mut W,
    failure: Option<io::Error>,
}

impl<W: Write> fmt::Write for TermSink<'_, W> {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        self.output.write_all(text.as_bytes()).map_err(|error| {
            self.failure = Some(error);
            fmt::Error
        })
    }
}

#[cfg(test)]
mod value_output_tests {
    use super::write_atoms;
    use zetesis_core::{Atom, Model, Predicate, Value};

    #[test]
    fn extrema_and_their_quoted_spellings_print_as_distinct_terms() {
        let values = [
            Value::Infimum,
            Value::String("#inf".into()),
            Value::String("#sup".into()),
            Value::Supremum,
        ];
        let model = Model::new(
            values
                .into_iter()
                .map(|value| Atom::new(Predicate::new("p", 1).unwrap(), vec![value]).unwrap()),
        )
        .unwrap();
        let mut output = Vec::new();
        write_atoms(&mut output, model.atoms().iter()).unwrap();
        assert_eq!(
            String::from_utf8(output).unwrap(),
            "p(#inf) p(\"#inf\") p(\"#sup\") p(#sup)\n"
        );
    }

    #[test]
    fn complete_typed_atom_spelling_preserves_every_writer_prefix() {
        use crate::test_writer::BoundedWriter;
        use zetesis_core::{Sign, ValueLimits, ValueNode};

        let nested = Value::from_nodes(
            vec![
                ValueNode::Function {
                    name: "f".into(),
                    sign: Sign::Negative,
                    arity: 1,
                },
                ValueNode::Tuple { arity: 1 },
                ValueNode::Number(2),
            ],
            ValueLimits::default(),
        )
        .unwrap();
        let model = Model::new([
            Atom::new(Predicate::new("z", 0).unwrap(), vec![]).unwrap(),
            Atom::new(
                Predicate::with_sign("p", 6, Sign::Negative).unwrap(),
                vec![
                    Value::Infimum,
                    Value::Number(-7),
                    Value::String("quote\" backslash\\ newline\n tab\tλ".into()),
                    Value::Symbol("s".into()),
                    nested,
                    Value::Supremum,
                ],
            )
            .unwrap(),
            Atom::new(Predicate::new("a", 0).unwrap(), vec![]).unwrap(),
        ])
        .unwrap();
        let expected =
            "a -p(#inf,-7,\"quote\\\" backslash\\\\ newline\\n tab\tλ\",s,-f((2,)),#sup) z\n";
        let mut complete = Vec::new();
        write_atoms(&mut complete, model.atoms().iter()).unwrap();
        assert_eq!(complete, expected.as_bytes());
        for capacity in 0..expected.len() {
            let mut output = BoundedWriter::new(capacity);
            let error = write_atoms(&mut output, model.atoms().iter()).unwrap_err();
            assert_eq!(error.kind(), std::io::ErrorKind::BrokenPipe);
            assert_eq!(error.to_string(), "diagnostic sink closed");
            assert_eq!(output.bytes(), &expected.as_bytes()[..capacity]);
        }
        let mut output = BoundedWriter::new(expected.len());
        write_atoms(&mut output, model.atoms().iter()).unwrap();
        assert_eq!(output.bytes(), expected.as_bytes());
    }
}
