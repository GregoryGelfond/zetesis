//! Sequential bounded effects around pure full-model comparisons.
mod compare;

use std::ffi::OsString;
use std::path::Path;

use serde::Serialize;

use super::{
    CampaignFault, Error, FileSeal, Limits, NativeExecution, Report, Request, identity, publication,
};
use crate::{curated, process};

/// Result of a case's process, enumeration and immutable model contracts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    /// Both complete producer reports agree with every recorded model occurrence.
    Pass,
    /// A child did not start, complete capture, or exit with an accepted code.
    InvocationFailure,
    /// A producer report is malformed, incomplete or exceeds normalization limits.
    InvalidReport,
    /// A complete report differs from the full-model or helper contract.
    ModelMismatch,
}

/// Structured producer failure classification, independent of diagnostic prose.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "stage", content = "operation", rename_all = "snake_case")]
pub enum InvocationFault {
    /// The bounded process implementation is unavailable.
    UnsupportedPlatform,
    /// An invocation path was relative.
    RelativePath,
    /// A requested deadline exceeds the platform's representable instant range.
    DeadlineOverflow,
    /// The operating system could not start the requested producer.
    Spawn,
    /// A specific capture or direct-child cleanup operation failed.
    Capture(process::Operation),
}

/// Owned failure evidence retaining its operation and diagnostic spelling.
#[derive(Debug, Serialize)]
pub struct InvocationFailure {
    kind: InvocationFault,
    detail: String,
}
impl InvocationFailure {
    /// Typed start/capture classification.
    #[must_use]
    pub const fn kind(&self) -> InvocationFault {
        self.kind
    }
    /// Original diagnostic rendering; never used to classify a result.
    #[must_use]
    pub fn detail(&self) -> &str {
        &self.detail
    }
    pub(crate) fn capture(failure: &process::Failure) -> Self {
        Self {
            kind: InvocationFault::Capture(failure.operation()),
            detail: failure.to_string(),
        }
    }
    pub(crate) fn start(failure: &process::StartError) -> Self {
        let kind = match failure {
            process::StartError::UnsupportedPlatform => InvocationFault::UnsupportedPlatform,
            process::StartError::RelativePath => InvocationFault::RelativePath,
            process::StartError::DeadlineOverflow => InvocationFault::DeadlineOverflow,
            process::StartError::Spawn(_) => InvocationFault::Spawn,
        };
        Self {
            kind,
            detail: failure.to_string(),
        }
    }
}

/// Exact invocation and capture evidence; raw bytes never pass through lossy UTF-8.
#[derive(Debug, Serialize)]
pub struct InvocationRecord {
    executable: std::path::PathBuf,
    arguments: Vec<OsString>,
    stop: Option<process::Stop>,
    exit: Option<process::Exit>,
    elapsed_ns: Option<u128>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    failure: Option<InvocationFailure>,
    cleanup_failure: Option<InvocationFailure>,
    unresolved_child: Option<u32>,
}
impl InvocationRecord {
    /// Absolute executable requested for this producer.
    #[must_use]
    pub fn executable(&self) -> &Path {
        &self.executable
    }
    /// Exact argument sequence, with no implicit extra solver options.
    #[must_use]
    pub fn arguments(&self) -> &[OsString] {
        &self.arguments
    }
    /// Measured capture duration in nanoseconds; absent before a child starts.
    #[must_use]
    pub const fn elapsed_ns(&self) -> Option<u128> {
        self.elapsed_ns
    }
    /// Start or capture diagnostic, independent of the typed case decision.
    #[must_use]
    pub const fn failure(&self) -> Option<&InvocationFailure> {
        self.failure.as_ref()
    }
    /// Cleanup diagnostic retained separately from the primary failure.
    #[must_use]
    pub const fn cleanup_failure(&self) -> Option<&InvocationFailure> {
        self.cleanup_failure.as_ref()
    }
    /// Captured standard output prefix, within the invocation and campaign limits.
    #[must_use]
    pub fn stdout(&self) -> &[u8] {
        &self.stdout
    }
    /// Captured standard error prefix, within the same combined allowance.
    #[must_use]
    pub fn stderr(&self) -> &[u8] {
        &self.stderr
    }
    /// Capture termination; absent only when invocation could not start.
    #[must_use]
    pub const fn stop(&self) -> Option<process::Stop> {
        self.stop
    }
    /// Reaped direct-child exit, absent when unavailable.
    #[must_use]
    pub const fn exit(&self) -> Option<process::Exit> {
        self.exit
    }

    fn complete(&self, reference: bool) -> bool {
        self.stop == Some(process::Stop::Completed)
            && self.failure.is_none()
            && self.cleanup_failure.is_none()
            && self.unresolved_child.is_none()
            && self.exit.is_some_and(|exit| {
                exit.signal.is_none()
                    && match exit.code {
                        Some(0) => true,
                        Some(10 | 20 | 30) => reference,
                        _ => false,
                    }
            })
    }
}

/// One original assertion's independent reference and native evidence.
#[derive(Debug, Serialize)]
pub struct CaseResult {
    id: String,
    source_sha256: String,
    decision: Decision,
    detail: Option<String>,
    reference: InvocationRecord,
    native: Option<InvocationRecord>,
}
impl CaseResult {
    /// Immutable assertion identity, unrelated to execution order or outcome.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }
    /// Classification, independent of diagnostic text.
    #[must_use]
    pub const fn decision(&self) -> Decision {
        self.decision
    }
    /// Explanation of a failed process or semantic contract.
    #[must_use]
    pub fn detail(&self) -> Option<&str> {
        self.detail.as_deref()
    }
    /// Independent reference invocation.
    #[must_use]
    pub const fn reference(&self) -> &InvocationRecord {
        &self.reference
    }
    /// Native invocation, absent if reference cleanup left a child unreaped.
    #[must_use]
    pub const fn native(&self) -> Option<&InvocationRecord> {
        self.native.as_ref()
    }
}

pub(super) fn campaign(request: &Request<'_>) -> Result<Report, Error> {
    if !cfg!(any(target_os = "linux", target_os = "macos")) {
        return Err(Error::UnsupportedPlatform);
    }
    let corpus = curated::open(request.corpus, request.limits.corpus).map_err(Error::Corpus)?;
    let before = seals(&corpus, request)?;
    let destination = publication::prepare(request.report, corpus.root(), &before)?;
    let directory = tempfile::tempdir()
        .map_err(|source| identity::io(Path::new("temporary campaign directory"), source))?;
    let mut report = Report {
        schema: 1,
        target: "clingo-5.8.2-selected-24",
        manifest_sha256: curated::MANIFEST_SHA256,
        requested_execution: request.execution,
        requested_limits: request.limits,
        full_model_comparison_scope: "pinned objective-free sources without output projection; native typed full atoms",
        physical_execution_qualified: false,
        before,
        after: Vec::new(),
        cases: Vec::new(),
        total_capture_bytes: 0,
        unresolved_children: Vec::new(),
        faults: Vec::new(),
        destination,
        max_report_bytes: request.limits.max_report_bytes,
    };
    let input = directory.path().join("input.lp");
    for case in corpus.cases() {
        if let Err(source) = std::fs::write(&input, case.source()) {
            report
                .faults
                .push(CampaignFault::InputWrite(source.to_string()));
            break;
        }
        let reference_arguments = vec![
            "--outf=2".into(),
            "--models=0".into(),
            input.as_os_str().to_owned(),
        ];
        let reference = invoke(
            request.reference,
            reference_arguments,
            directory.path(),
            request.limits,
            &mut report,
        );
        let native = report.unresolved_children.is_empty().then(|| {
            invoke(
                request.native,
                native_arguments(request.execution, &input),
                directory.path(),
                request.limits,
                &mut report,
            )
        });
        let (decision, detail) = compare::case(case, &reference, native.as_ref(), request.limits);
        report.cases.push(CaseResult {
            id: case.id().into(),
            source_sha256: case.source_sha256().into(),
            decision,
            detail,
            reference,
            native,
        });
        if !report.unresolved_children.is_empty() {
            break;
        }
    }
    report.after = report.before.iter().map(identity::recheck).collect();
    if let Err(error) = directory.close() {
        report
            .faults
            .push(CampaignFault::InputCleanup(error.to_string()));
    }
    Ok(report)
}

fn seals(corpus: &curated::Corpus, request: &Request<'_>) -> Result<Vec<FileSeal>, Error> {
    let mut seals = Vec::with_capacity(corpus.cases().len() + 4);
    for path in [request.reference, request.native] {
        if !path.is_absolute() {
            return Err(Error::Path {
                path: path.into(),
                detail: "solver executable must be absolute",
            });
        }
        seals.push(identity::seal(path, request.limits.max_executable_bytes)?);
    }
    if identity::aliases(&seals[0], &seals[1]) {
        return Err(Error::Path {
            path: request.native.into(),
            detail: "reference and native executables must have distinct file identities",
        });
    }
    let manifest = identity::seal(
        &request.corpus.join("manifest.json"),
        request.limits.corpus.manifest_bytes,
    )?;
    if manifest.sha256() != curated::MANIFEST_SHA256 {
        return Err(Error::Path {
            path: manifest.requested().into(),
            detail: "manifest changed after verification",
        });
    }
    seals.push(manifest);
    let license = identity::seal(
        &request.corpus.join("LICENSE.md"),
        request.limits.corpus.license_bytes,
    )?;
    if license.sha256() != curated::LICENSE_SHA256 {
        return Err(Error::Path {
            path: license.requested().into(),
            detail: "license changed after verification",
        });
    }
    seals.push(license);
    for case in corpus.cases() {
        let seal = identity::seal(
            &request.corpus.join(case.path()),
            request.limits.corpus.source_bytes,
        )?;
        if seal.sha256() != case.source_sha256() {
            return Err(Error::Path {
                path: seal.requested().into(),
                detail: "source changed after verification",
            });
        }
        seals.push(seal);
    }
    Ok(seals)
}

fn native_arguments(execution: NativeExecution, input: &Path) -> Vec<OsString> {
    let values = [
        ("--backend", execution.backend.label().into()),
        ("--oracle", execution.oracle.label().into()),
        ("--grounder", execution.grounder.label().into()),
        ("--workers", execution.workers.to_string()),
        (
            "--completion-workers",
            execution.completion_workers.to_string(),
        ),
        ("--batch-size", execution.batch_size.to_string()),
        (
            "--max-completion-scratch-bytes",
            execution.max_completion_scratch_bytes.to_string(),
        ),
        ("--models", "0".into()),
    ];
    values
        .into_iter()
        .flat_map(|(flag, value)| [flag.into(), value.into()])
        .chain([
            "--json".into(),
            "--stats".into(),
            input.as_os_str().to_owned(),
        ])
        .collect()
}

fn invoke(
    executable: &Path,
    arguments: Vec<OsString>,
    directory: &Path,
    limits: Limits,
    report: &mut Report,
) -> InvocationRecord {
    let remaining = limits
        .max_total_capture_bytes
        .saturating_sub(report.total_capture_bytes);
    let bounded = process::Limits {
        max_output_bytes: limits.process.max_output_bytes.min(remaining),
        ..limits.process
    };
    let mut record = InvocationRecord {
        executable: executable.into(),
        arguments,
        stop: None,
        exit: None,
        elapsed_ns: None,
        stdout: Vec::new(),
        stderr: Vec::new(),
        failure: None,
        cleanup_failure: None,
        unresolved_child: None,
    };
    match process::invoke(
        process::Invocation {
            executable,
            arguments: &record.arguments,
            directory,
        },
        bounded,
    ) {
        Err(error) => record.failure = Some(InvocationFailure::start(&error)),
        Ok(outcome) => {
            let (capture, pending) = outcome.into_parts();
            record.stop = Some(capture.stop());
            record.exit = capture.exit();
            record.elapsed_ns = Some(capture.elapsed().as_nanos());
            record.stdout = capture.stdout().to_vec();
            record.stderr = capture.stderr().to_vec();
            record.failure = capture.failure().map(InvocationFailure::capture);
            record.cleanup_failure = capture.cleanup_failure().map(InvocationFailure::capture);
            if let Some(pending) = pending {
                let cleanup = pending.retry(limits.process.cleanup_timeout);
                record.exit = cleanup.exit.or(record.exit);
                if let Some(failure) = cleanup.failure {
                    report
                        .faults
                        .push(CampaignFault::ChildCleanup(failure.to_string()));
                }
                if let Some(pending) = cleanup.pending {
                    let id = pending.abandon();
                    record.unresolved_child = Some(id);
                    report.unresolved_children.push(id);
                }
            }
            // The process runner already enforces this joint remaining-byte ceiling.
            report.total_capture_bytes += record.stdout.len() + record.stderr.len();
        }
    }
    record
}

#[cfg(test)]
#[path = "../../tests/support/selected_run.rs"]
mod tests;
