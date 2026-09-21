//! Bounded preparation of a complete human model record.
//!
//! Both passes read the same immutable contents. The first counts actual UTF-8
//! writes without allocating a record; exceeding the byte ceiling stops that
//! pass. The second reserves the admitted length once and reproduces those bytes.
//! No external sink is touched here. Time is two spelling/selection traversals;
//! retained record storage is its admitted length, subject to allocator rounding.

use std::io::{self, Write};

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
    atoms: impl Iterator<Item = &'a zetesis_core::Atom>,
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
                match value {
                    zetesis_core::Value::Infimum => write!(output, "#inf")?,
                    zetesis_core::Value::Supremum => write!(output, "#sup")?,
                    zetesis_core::Value::Number(number) => write!(output, "{number}")?,
                    zetesis_core::Value::Symbol(symbol) => write!(output, "{symbol}")?,
                    zetesis_core::Value::String(string) => write_string(output, string)?,
                    zetesis_core::Value::Structured(value) => write!(output, "{value}")?,
                }
            }
            write!(output, ")")?;
        }
    }
    writeln!(output)
}

fn write_string(output: &mut impl Write, value: &str) -> io::Result<()> {
    // The admitted clingo string dialect has exactly these three escapes.
    // Other admitted characters, including literal tabs, retain their bytes;
    // Rust Debug's \t and \u{...} spellings are not clingo string escapes.
    write!(output, "\"")?;
    for character in value.chars() {
        match character {
            '"' => write!(output, "\\\"")?,
            '\\' => write!(output, "\\\\")?,
            '\n' => write!(output, "\\n")?,
            other => write!(output, "{other}")?,
        }
    }
    write!(output, "\"")
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
        );
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
        ]);
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
