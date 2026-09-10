//! Matched packed equality masks over one retained typed relation view.
//!
//! Construction, query resolution, pipeline creation and column upload are
//! separate from repeated batch observations. Scalar, Rayon and physical GPU
//! routes publish the same packed-mask representation and reconstruct typed rows
//! through the same core operation. This is neither full pattern matching nor a
//! grounding/solving benchmark; route operation counts can differ.

mod config;
mod run;
mod view;

use std::io::Write as _;
use std::{fmt, io};

pub use config::{Configuration, Options};
pub use run::measure;
pub use view::{DeviceWork, Event, Observation, Phase, Preparation, Route, Subject};

/// A bounded experiment did not complete; previously published events remain.
#[derive(Debug)]
pub enum Error {
    /// The finite experiment scope or simultaneous capacity was exceeded.
    Configuration(&'static str),
    /// Shared fixture construction or the independent row reference failed.
    Fixture(crate::relation_fixtures::Error),
    /// Checked relation construction, lookup or selection failed.
    Relation(zetesis_core::relation::Failure),
    /// Allocation, cancellation or deadline stopped the experiment.
    Stopped(zetesis_cpu::Stop),
    /// The independently owned Rayon pool could not be created.
    Pool(rayon::ThreadPoolBuildError),
    /// Real-device preparation, selection or decoding failed.
    Gpu(zetesis_wgpu::RelationGpuError),
    /// Complete masks or typed reconstructed rows disagree with their source.
    Parity,
    /// Actual device accounting does not match the requested execution route.
    DeviceWork,
    /// Publication or bounded subject serialization failed.
    Output(io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Configuration(reason) => write!(f, "relation experiment: {reason}"),
            Self::Fixture(error) => error.fmt(f),
            Self::Relation(error) => error.fmt(f),
            Self::Stopped(error) => error.fmt(f),
            Self::Pool(error) => error.fmt(f),
            Self::Gpu(error) => error.fmt(f),
            Self::Parity => f.write_str("relation masks disagree with original typed rows"),
            Self::DeviceWork => {
                f.write_str("relation device accounting differs from requested route")
            }
            Self::Output(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Fixture(error) => Some(error),
            Self::Relation(error) => Some(error),
            Self::Stopped(error) => Some(error),
            Self::Pool(error) => Some(error),
            Self::Gpu(error) => Some(error),
            Self::Output(error) => Some(error),
            Self::Configuration(_) | Self::Parity | Self::DeviceWork => None,
        }
    }
}

/// Compose bounded library measurement with a streaming JSON-lines view.
///
/// # Errors
/// Complete is offered only after all prior observations succeed. Sink failures
/// may retain partial bytes and return `Error::Output`. Physical selection never
/// falls back to CPU or another graphics API.
pub fn run(options: &Options, output: &mut impl io::Write) -> Result<(), Error> {
    let mut output = Output {
        writer: output,
        bytes: 0,
    };
    measure(
        options.configuration()?,
        &zetesis_cpu::Control::default(),
        |event| {
            serde_json::to_writer(&mut output, event).map_err(io::Error::other)?;
            writeln!(output)
        },
    )
}

const MAX_REPORT_BYTES: usize = 256 * 1024 * 1024;

struct Output<'a, W> {
    writer: &'a mut W,
    bytes: usize,
}
impl<W: io::Write> io::Write for Output<'_, W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > MAX_REPORT_BYTES - self.bytes {
            return Err(io::Error::other("relation report exceeds 256 MiB ceiling"));
        }
        let written = self.writer.write(bytes)?;
        self.bytes += written;
        Ok(written)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}
