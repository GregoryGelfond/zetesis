//! The fresh helper's no-clobber publication boundary.

use std::{
    fmt,
    io::{self, Write},
    path::Path,
};

/// Measuring the sole child or publishing its resource record failed.
#[derive(Debug)]
pub enum RecordError {
    /// The child did not yield a valid independent resource observation.
    Measurement(super::Error),
    /// The new record could not be created or flushed.
    Io(io::Error),
    /// The typed resource record could not be serialized.
    Json(serde_json::Error),
}
impl fmt::Display for RecordError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Measurement(error) => error.fmt(formatter),
            Self::Io(error) => error.fmt(formatter),
            Self::Json(error) => error.fmt(formatter),
        }
    }
}
impl std::error::Error for RecordError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Measurement(error) => error,
            Self::Io(error) => error,
            Self::Json(error) => error,
        })
    }
}

/// Measure the fresh helper's sole solver child and publish a new JSON record.
///
/// This must run only in the fresh supervised helper described by [`super::measure`].
/// It never launches another helper, replaces a record, or consumes global arguments.
/// The caller supplies the exact directory and unmodified child argument sequence.
///
/// # Errors
/// Returns the measurement failure, or a publication error after the child was
/// reaped. A partially written record is retained and is not successful evidence.
pub fn measure_to_file(
    invocation: crate::process::Invocation<'_>,
    path: &Path,
) -> Result<(), RecordError> {
    let record = super::measure(invocation).map_err(RecordError::Measurement)?;
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(RecordError::Io)?;
    serde_json::to_writer(&mut output, &record).map_err(RecordError::Json)?;
    output.flush().map_err(RecordError::Io)
}
