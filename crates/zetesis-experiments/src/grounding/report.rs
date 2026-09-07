//! Original-source identity and bounded report publication.

use std::{io::Write, path::PathBuf};

use serde::Serialize;
use sha2::{Digest, Sha256};
use zetesis_themelios::SourceBundle;

use super::{CaptureRefusal, Configuration, Error, Mode, Models, PhaseRecord, storage};

/// One original canonical file, hashed before admission timing.
#[derive(Debug, Serialize)]
pub struct SourceIdentity {
    /// Native source identifier used by phase locations.
    pub source: u32,
    /// Canonical file path from native source loading.
    pub path: PathBuf,
    /// Exact original UTF-8 source byte count.
    pub bytes: usize,
    /// SHA-256 of those original bytes, as lowercase hexadecimal.
    pub sha256: String,
}

/// One fresh admission attempt; comparison and solving occur after its timer.
#[derive(Debug, Serialize)]
pub struct Sample {
    /// Zero-based round. Mode order rotates with this number.
    pub round: usize,
    /// Callback instrumentation used for this admission.
    pub mode: Mode,
    /// Raising through completed admission, excluding loading/parsing and setup.
    pub admission_elapsed_ns: Option<u64>,
    /// Exact actual-ground callback span, unavailable for the unobserved mode.
    pub grounding_elapsed_ns: Option<u64>,
    /// Recorded sequential phase prefix; empty when detailed capture was disabled.
    pub phases: Vec<PhaseRecord>,
    /// Independent capture refusal, even if native admission also failed.
    pub capture_refusal: Option<CaptureRefusal>,
    /// Whether native admission returned a complete subject.
    pub admitted: bool,
    /// Exact ordered native subject/metadata/provenance comparison, if reached.
    pub subject_equal: Option<bool>,
    /// Native full-model verification, if reached, including a failed prefix.
    pub models: Option<Models>,
}

/// Reusable diagnostic result. The initial unmeasured native subject remains
/// live during all samples; only its catalog and complete model multiset survive
/// in this report. Source loading, hashing, setup, verification and serialization
/// are excluded from admission timing. Detailed timing includes instrumentation
/// overhead and does not isolate arithmetic from joins or emission.
#[derive(Debug, Serialize)]
pub struct Report {
    /// Report grammar version; currently one.
    pub schema_version: u32,
    /// Timer boundary identifier, fixed by this driver version.
    pub timing_scope: &'static str,
    /// Exact native and capture ceilings used throughout this run.
    pub configuration: Configuration,
    /// Original source catalog; files are loaded anew before each admission.
    pub sources: Vec<SourceIdentity>,
    /// Full signed atom identities in native theory index order, without #show.
    pub atoms: Vec<String>,
    /// Number of nodes in the qualified native formula DAG, if admission succeeded.
    pub nodes: Option<usize>,
    /// Number of roots in that DAG, if admission succeeded.
    pub roots: Option<usize>,
    /// Initial unmeasured native model enumeration, including any failed prefix.
    pub qualification: Option<Models>,
    /// Every started timed admission in execution order; no outliers are removed.
    pub samples: Vec<Sample>,
    /// First refusal; any measured and verified prefix remains available.
    pub failure: Option<Error>,
    /// True only if all requested admissions and complete-model checks succeeded.
    pub complete: bool,
}

impl Report {
    pub(super) fn new(configuration: &Configuration) -> Result<Self, Error> {
        Ok(Self {
            schema_version: 1,
            timing_scope: "fresh_bundle_formula_admission_excluding_load_parse_and_observer_setup",
            configuration: *configuration,
            sources: Vec::new(),
            atoms: Vec::new(),
            nodes: None,
            roots: None,
            qualification: None,
            samples: storage::reserve(configuration.repetitions * Mode::ALL.len())?,
            failure: None,
            complete: false,
        })
    }
}

pub(super) fn catalog(bundle: &SourceBundle, limit: usize) -> Result<Vec<SourceIdentity>, Error> {
    let mut sources = storage::reserve(bundle.sources().len())?;
    let mut path_bytes = 0_usize;
    for source in bundle.sources() {
        path_bytes = path_bytes
            .checked_add(source.path().as_os_str().as_encoded_bytes().len())
            .filter(|&bytes| bytes <= limit)
            .ok_or(Error::Limit {
                resource: "source_path_bytes",
                limit,
            })?;
        sources.push(SourceIdentity {
            source: source.id().get(),
            path: source.path().to_owned(),
            bytes: source.source().text().len(),
            sha256: format!("{:x}", Sha256::digest(source.source().text().as_bytes())),
        });
    }
    Ok(sources)
}

pub(super) fn same_sources(a: &SourceBundle, b: &SourceBundle) -> bool {
    a.entry() == b.entry()
        && a.working_directory() == b.working_directory()
        && a.roots() == b.roots()
        && a.sources().len() == b.sources().len()
        && a.sources().iter().zip(b.sources()).all(|(a, b)| {
            a.id() == b.id()
                && a.path() == b.path()
                && a.loaded_path() == b.loaded_path()
                && a.source().text() == b.source().text()
                && a.includes().len() == b.includes().len()
                && a.includes().iter().zip(b.includes()).all(|(a, b)| {
                    a.location() == b.location()
                        && a.target() == b.target()
                        && a.requested_path() == b.requested_path()
                        && a.resolved_path() == b.resolved_path()
                        && a.resolution() == b.resolution()
                })
        })
}

/// Serialize one complete JSON record before writing any external byte.
///
/// This performs work proportional to serialized data, bounded by
/// `configuration.capture.max_output_bytes`. The temporary byte buffer requests
/// at most that capacity (allocator overhead is excluded). A writer may retain a
/// byte prefix on I/O failure; success means the whole record and newline were
/// accepted, not that the writer has durably persisted or flushed them.
///
/// # Errors
/// Returns byte-cap, allocation, serialization or writer failure. The caller's
/// typed report remains available; preflight refusal publishes zero bytes.
pub fn write_report(report: &Report, output: &mut impl Write) -> Result<(), Error> {
    let mut bytes = storage::Bytes::new(
        report.configuration.capture.max_output_bytes,
        "output_bytes",
    );
    let serialized = serde_json::to_writer(&mut bytes, report);
    if let Some(error) = bytes.refusal.take() {
        return Err(error);
    }
    serialized.map_err(Error::Serialization)?;
    let newline = bytes.write_all(b"\n");
    if let Some(error) = bytes.refusal.take() {
        return Err(error);
    }
    newline.map_err(Error::Output)?;
    output.write_all(&bytes.data).map_err(Error::Output)
}
