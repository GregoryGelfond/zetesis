use std::{
    fmt,
    fs::File,
    io::{self, BufReader, Read},
    path::Path,
};

use super::{Comparison, Labelled, ViewError};

/// One published report selected for a comparison.
#[derive(Clone, Copy, Debug)]
pub struct ReportSource<'a> {
    /// Unique display label, independent of the file path.
    pub label: &'a str,
    /// Published matrix JSON document.
    pub path: &'a Path,
}

/// Bounded report loading or typed comparison failed.
#[derive(Debug)]
pub enum ReadError {
    /// Opening, inspecting or reading a report failed.
    Io(io::Error),
    /// One report exceeds the caller's byte ceiling.
    Bytes {
        /// Label of the refused report.
        label: String,
        /// Maximum source bytes allowed for this document.
        limit: u64,
    },
    /// The report is not a complete JSON document.
    Json(serde_json::Error),
    /// The typed comparison's identity or schema contract refused the reports.
    Compare(ViewError),
}

impl fmt::Display for ReadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => error.fmt(formatter),
            Self::Bytes { label, limit } => {
                write!(formatter, "report {label} exceeds its {limit}-byte ceiling")
            }
            Self::Json(error) => error.fmt(formatter),
            Self::Compare(error) => error.fmt(formatter),
        }
    }
}
impl std::error::Error for ReadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Json(error) => Some(error),
            Self::Compare(error) => Some(error),
            Self::Bytes { .. } => None,
        }
    }
}

/// Read bounded published reports and apply the maintained comparison contract.
///
/// At most `max_file_bytes` source bytes are decoded per report, plus a one-byte
/// over-limit probe. JSON-owned storage is separate from this source-byte bound.
/// All documents remain owned until comparison completes; the returned value
/// retains derived observations, not raw model streams. No files are written.
///
/// # Errors
/// Returns I/O, source byte-limit, JSON or comparison-identity failures.
pub fn read_compare(
    sources: &[ReportSource<'_>],
    max_file_bytes: u64,
) -> Result<Comparison, ReadError> {
    let mut documents = Vec::with_capacity(sources.len());
    for source in sources {
        let file = File::open(source.path).map_err(ReadError::Io)?;
        let too_large = || ReadError::Bytes {
            label: source.label.to_owned(),
            limit: max_file_bytes,
        };
        if file.metadata().map_err(ReadError::Io)?.len() > max_file_bytes {
            return Err(too_large());
        }
        let mut limited = BufReader::new(file.take(max_file_bytes));
        let parsed = serde_json::from_reader(&mut limited);
        // A file can grow after metadata was inspected. Check the actual stream
        // beyond its bound even when the bounded JSON prefix could be complete.
        let exhausted = limited.get_ref().limit() == 0;
        let mut reader = limited.into_inner().into_inner();
        let mut extra = [0];
        if exhausted && reader.read(&mut extra).map_err(ReadError::Io)? != 0 {
            return Err(too_large());
        }
        documents.push(parsed.map_err(ReadError::Json)?);
    }
    let labelled: Vec<_> = sources
        .iter()
        .zip(&documents)
        .map(|(source, report)| Labelled {
            label: source.label,
            report,
        })
        .collect();
    super::compare(&labelled).map_err(ReadError::Compare)
}
