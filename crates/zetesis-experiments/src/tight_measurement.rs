//! Matched complete-theory tight certificates with exact reduct completion.
//!
//! Classification and serial, occurrence-ordered residual completion have
//! directly measured host intervals, nested within the complete call interval.
//! References, witness verification, setup and JSON publication are outside those
//! intervals. This experiment measures neither grounding nor outer search.
//! Separate GPU instances keep fresh and resident transport populations distinct.

mod checking;
mod config;
mod fixture;
mod run;
mod view;

use std::{fmt, io};

pub use config::{Case, Configuration, Family, Options, Reference, Support};
pub use run::{measure, measure_with_cancellation};
pub use view::{
    Activity, Certificate, Decision, Device, DeviceWork, Event, Formula, Observation, Outcome,
    Phase, Producer, ProducerKind, Residency, Route, Sample,
};

/// An incomplete experiment, never an UNSAT result. Published prefixes remain
/// valid observations; no completion record follows an error.
#[derive(Debug)]
pub enum Error {
    /// The finite measurement scope was exceeded.
    Configuration(&'static str),
    /// A bounded fixture allocation or native evaluation stopped.
    Cpu(zetesis_cpu::Stop),
    /// The authored finite theory was not admitted.
    Admission(zetesis_ferraris::AdmissionError),
    /// Complete-theory certification or candidate classification stopped.
    Certificate(zetesis_ferraris::TightError),
    /// Exact general reduct checking did not complete.
    Residual(zetesis_sat::Incomplete),
    /// The independently owned Rayon pool could not be created.
    Pool(rayon::ThreadPoolBuildError),
    /// Physical setup failed; no fallback is used.
    Device(zetesis_wgpu::GpuError),
    /// Physical classification stopped or failed.
    Gpu(zetesis_wgpu::TightGpuError),
    /// Complete status, certificate witness or reduct counterexample disagrees.
    Parity,
    /// Successful device activity violates the declared route/cache contract.
    DeviceWork,
    /// The synchronous event consumer refused a record.
    Output(io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Configuration(reason) => write!(formatter, "tight experiment: {reason}"),
            Self::Cpu(error) => error.fmt(formatter),
            Self::Admission(error) => error.fmt(formatter),
            Self::Certificate(error) => error.fmt(formatter),
            Self::Residual(error) => error.fmt(formatter),
            Self::Pool(error) => error.fmt(formatter),
            Self::Device(error) => error.fmt(formatter),
            Self::Gpu(error) => error.fmt(formatter),
            Self::Parity => {
                formatter.write_str("tight checks disagree with complete reduct references")
            }
            Self::DeviceWork => {
                formatter.write_str("tight device activity violates the requested route")
            }
            Self::Output(error) => error.fmt(formatter),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Cpu(error) => Some(error),
            Self::Admission(error) => Some(error),
            Self::Certificate(error) => Some(error),
            Self::Residual(error) => Some(error),
            Self::Pool(error) => Some(error),
            Self::Device(error) => Some(error),
            Self::Gpu(error) => Some(error),
            Self::Output(error) => Some(error),
            Self::Configuration(_) | Self::Parity | Self::DeviceWork => None,
        }
    }
}

fn reserve<T>(count: usize) -> Result<Vec<T>, Error> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| Error::Cpu(zetesis_cpu::Stop::Allocation))?;
    Ok(values)
}

/// Compose the command view, bounded measurement and a JSON-lines event view.
///
/// # Errors
/// Returns configuration, incomplete checking, disagreement or writer failure.
pub fn run(options: &Options, output: &mut impl io::Write) -> Result<(), Error> {
    measure(&options.configuration()?, |event| {
        serde_json::to_writer(&mut *output, event).map_err(io::Error::other)?;
        writeln!(output)
    })
}
