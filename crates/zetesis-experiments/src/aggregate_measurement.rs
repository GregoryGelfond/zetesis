//! Matched native aggregate reduction over actual original/frozen eligibility.
//!
//! Formula acquisition and numerical plan preparation have separate observations.
//! Sample clocks include exact reduction/result construction, and device uploads,
//! dispatch and readback where selected; no grounding or stable-membership claim.

mod checking;
mod config;
mod fixture;
mod run;
mod view;

pub use config::{Case, Configuration, Function, Options};
pub use run::{measure, measure_with_cancellation};
use std::{fmt, io};
pub use view::{
    Activity, DeviceWork, Evaluation, Event, Observation, Outcome, Phase, Preparation, Route,
    Sample, Value,
};

/// Incomplete experiment. Published prefixes remain evidence; no completion
/// event or partial successful sample follows a failure.
#[derive(Debug)]
pub enum Error {
    /// The finite fixture/schedule contract was exceeded.
    Configuration(&'static str),
    /// Caller control or a fallible result allocation stopped.
    Cpu(zetesis_cpu::Stop),
    /// The fixed formula fixture could not be admitted.
    Admission(zetesis_ferraris::AdmissionError),
    /// Native aggregate admission, acquisition or reduction stopped.
    Native(zetesis_ferraris::native_aggregate::Error),
    /// The independently owned Rayon pool could not be created.
    Pool(rayon::ThreadPoolBuildError),
    /// Physical device/pipeline setup failed; no fallback occurs.
    Device(zetesis_wgpu::GpuError),
    /// Numeric preparation or exact device reduction stopped.
    Gpu(zetesis_wgpu::AggregateGpuError),
    /// Ordered original/frozen measures or guard truth disagree.
    Parity,
    /// Completed accounting or declared device residency does not agree.
    Accounting,
    /// The synchronous event consumer refused publication.
    Output(io::Error),
}
impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Configuration(reason) => write!(formatter, "aggregate experiment: {reason}"),
            Self::Cpu(error) => error.fmt(formatter),
            Self::Admission(error) => error.fmt(formatter),
            Self::Native(error) => error.fmt(formatter),
            Self::Pool(error) => error.fmt(formatter),
            Self::Device(error) => error.fmt(formatter),
            Self::Gpu(error) => error.fmt(formatter),
            Self::Parity => formatter.write_str("aggregate measures or guards disagree"),
            Self::Accounting => formatter.write_str("aggregate activity or residency disagrees"),
            Self::Output(error) => error.fmt(formatter),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Cpu(error) => Some(error),
            Self::Admission(error) => Some(error),
            Self::Native(error) => Some(error),
            Self::Pool(error) => Some(error),
            Self::Device(error) => Some(error),
            Self::Gpu(error) => Some(error),
            Self::Output(error) => Some(error),
            Self::Configuration(_) | Self::Parity | Self::Accounting => None,
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

/// Compose options, bounded measurements and the JSON-lines event view.
///
/// # Errors
/// Returns configuration, incomplete operation, disagreement or writer failure.
pub fn run(options: &Options, output: &mut impl io::Write) -> Result<(), Error> {
    measure(&options.configuration()?, |event| {
        serde_json::to_writer(&mut *output, event).map_err(io::Error::other)?;
        writeln!(output)
    })
}
