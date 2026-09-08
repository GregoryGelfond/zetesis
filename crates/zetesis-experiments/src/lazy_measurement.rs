//! Matched relational programs and frozen candidate batches across CPU and physical GPUs.
//!
//! Source scans, complete reduct closure checking and result construction are
//! inside each sample. Fixture preparation, parity comparisons, pool/device
//! setup and publication are outside it. This is not outer answer-set search.
//! Scalar/Rayon limits apply per candidate; round-source limits apply per batch.
//! All successful routes must return the same complete ordered checks despite
//! those different schedules. Requested mask payload is not process peak RSS.

mod config;
mod fixture;
mod run;
mod view;

use std::{fmt, io};

pub use config::{Case, Configuration, Family, Options};
pub use run::measure;
pub use view::{DeviceWork, Event, Phase, Route, Sample, SourceWork};

/// Failed setup, incomplete checking, disagreement or publication failure.
/// A consumer may retain the event prefix; no completion event follows failure.
#[derive(Debug)]
pub enum Error {
    /// Dimensions exceed the experiment's finite scope.
    Configuration(&'static str),
    /// The authored relational fixture could not be admitted.
    Admission(zetesis_core::AdmissionError),
    /// A fixture atom or predicate violates the native construction contract.
    Construction(zetesis_core::ConstructionError),
    /// A fixture seed lies outside its admitted program's gate carrier.
    Seed(zetesis_core::SeedError),
    /// A scalar candidate exhausted its logical limits.
    Cpu(zetesis_cpu::Stop),
    /// Pool construction or bounded submission failed.
    Pool(zetesis_cpu::BatchError),
    /// An injected portable round evaluator did not complete.
    Source(zetesis_cpu::lazy::Failure<zetesis_cpu::Stop>),
    /// Physical device setup failed; no CPU fallback occurs.
    Device(zetesis_wgpu::GpuError),
    /// A physical round batch did not complete.
    Gpu(zetesis_cpu::lazy::Failure<zetesis_wgpu::GpuError>),
    /// Complete ordered closures or rejection reasons disagree.
    Parity,
    /// The physical route did not record its required submitted work.
    DeviceWork,
    /// The event consumer refused a record.
    Output(io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Configuration(reason) => write!(formatter, "lazy experiment: {reason}"),
            Self::Admission(error) => error.fmt(formatter),
            Self::Construction(error) => error.fmt(formatter),
            Self::Seed(error) => error.fmt(formatter),
            Self::Cpu(error) => error.fmt(formatter),
            Self::Pool(error) => error.fmt(formatter),
            Self::Source(error) => error.fmt(formatter),
            Self::Device(error) => error.fmt(formatter),
            Self::Gpu(error) => error.fmt(formatter),
            Self::Parity => {
                formatter.write_str("complete lazy checks disagree with scalar checking")
            }
            Self::DeviceWork => {
                formatter.write_str("lazy device work does not match completed chunks")
            }
            Self::Output(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Admission(error) => Some(error),
            Self::Construction(error) => Some(error),
            Self::Seed(error) => Some(error),
            Self::Cpu(error) => Some(error),
            Self::Pool(error) => Some(error),
            Self::Source(error) => Some(error),
            Self::Device(error) => Some(error),
            Self::Gpu(error) => Some(error),
            Self::Output(error) => Some(error),
            Self::Configuration(_) | Self::Parity | Self::DeviceWork => None,
        }
    }
}

/// Compose command configuration, measurement and a JSON-lines view.
/// Output includes preparation, warmups, timed observations and completion.
///
/// # Errors
/// Returns typed measurement or writer failure, preserving any written prefix.
pub fn run(options: &Options, output: &mut impl io::Write) -> Result<(), Error> {
    let configuration = options.configuration()?;
    measure(&configuration, |event| {
        serde_json::to_writer(&mut *output, event).map_err(io::Error::other)?;
        writeln!(output)
    })
}
