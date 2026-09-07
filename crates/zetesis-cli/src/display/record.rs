//! Bounded preparation of a complete human model record.
//!
//! Both passes read the same immutable contents. The first counts actual UTF-8
//! writes without allocating a record; exceeding the byte ceiling stops that
//! pass. The second reserves the admitted length once and reproduces those bytes.
//! No external sink is touched here. Time is two spelling/selection traversals;
//! retained record storage is its admitted length, subject to allocator rounding.

use std::io::{self, Write};

use zetesis_core::Model;
use zetesis_cpu::{Control, Stop};
use zetesis_objective::Score;
use zetesis_themelios::OutputSelection;

use crate::{ColorMode, RunError};

#[derive(Clone, Copy)]
pub(super) enum Contents<'a> {
    Atoms(&'a Model, &'a OutputSelection),
    Observed(&'a str),
}

pub(super) struct Record(Vec<u8>);

impl Record {
    pub(super) fn prepare(
        number: usize,
        contents: Contents<'_>,
        score: Option<&Score>,
        color: ColorMode,
        maximum: usize,
        control: &Control,
    ) -> Result<Self, RunError> {
        let mut length = Length {
            bytes: 0,
            maximum,
            refusal: None,
            stopped: None,
            control,
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

    pub(super) fn bytes(&self) -> &[u8] {
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
        Contents::Atoms(model, selection) => {
            crate::driver::write_atoms(output, model, selection)?;
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
    control: &'a Control,
}

impl Write for Length<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if let Err(stop) = self.control.poll() {
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
