//! Sequential bounded processes around pure schedule and display comparisons.
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::Path;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use super::{
    Capture, Case, Decision, Error, Fault, Phase, Producer, Report, Request, Sample, Slot, timing,
};
use crate::selected::{FileSeal, InvocationFailure, identity, publication};
use crate::{answers, examples, process};

fn unix_ns() -> Option<u128> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|value| value.as_nanos())
}

pub(super) fn campaign(request: &Request<'_>) -> Result<Report, Error> {
    if !cfg!(any(target_os = "linux", target_os = "macos")) {
        return Err(Error::Configuration(
            "bounded process capture requires Linux or macOS",
        ));
    }
    let deadline = Instant::now()
        .checked_add(request.limits.campaign_timeout)
        .ok_or(Error::Configuration(
            "campaign deadline is not representable",
        ))?;
    let started_unix_ns =
        unix_ns().ok_or(Error::Configuration("UTC metadata precedes Unix epoch"))?;
    let corpus = examples::load(request.corpus, request.limits.corpus).map_err(Error::Corpus)?;
    let sources = source_paths(&corpus)?;
    let before = seals(&corpus, &sources, request)?;
    let destination = publication::prepare(request.report, corpus.root(), &before)?;
    let directory = tempfile::tempdir()
        .map_err(|source| super::io(Path::new("private performance sources"), source))?;
    let mut report = Report {
        schema: 1,
        manifest_sha256: examples::MANIFEST_SHA256,
        schedule: request.schedule,
        limits: request.limits,
        started_unix_ns,
        finished_unix_ns: None,
        wall_scope: "fresh_process_spawn_capture_reap; output_included; comparison_hashing_excluded; no_cold_cache_claim",
        comparison_scope: "complete_selected_displays_with_symbol_and_model_multiplicities; final_optimum_ties_and_costs; hidden_interpretations_unavailable",
        peak_rss: "unavailable: direct-child capture does not retain rusage",
        gpu_measurement: "unavailable: this fixed CPU campaign is not the full94 eager/lazy CPU/Metal matrix",
        before,
        after: Vec::new(),
        metadata: Vec::new(),
        samples: Vec::new(),
        total_capture_bytes: 0,
        faults: Vec::new(),
        unresolved_children: Vec::new(),
        destination,
    };
    match copy_sources(
        &corpus,
        &sources,
        directory.path(),
        request.limits.corpus.source_bytes,
    ) {
        Ok(private) => {
            report.before.extend(private);
            execute(request, &corpus, directory.path(), deadline, &mut report);
        }
        Err(error) => report.faults.push(Fault::InputWrite(error.to_string())),
    }
    report.after = report.before.iter().map(identity::recheck).collect();
    if let Err(error) = directory.close() {
        report.faults.push(Fault::InputCleanup(error.to_string()));
    }
    report.finished_unix_ns = unix_ns();
    if report.finished_unix_ns.is_none() {
        report.faults.push(Fault::Clock);
    }
    Ok(report)
}

fn source_paths(corpus: &examples::Corpus) -> Result<BTreeSet<&str>, Error> {
    let mut sources = BTreeSet::new();
    for selected in Case::ALL {
        let case = corpus
            .cases()
            .iter()
            .find(|case| case.path() == selected.path())
            .ok_or(Error::Configuration(
                "fixed performance case is missing from sealed corpus",
            ))?;
        sources.extend(case.transitive_source_paths().iter().map(String::as_str));
    }
    Ok(sources)
}

fn checked_seal(path: &Path, limit: usize, expected: &str) -> Result<FileSeal, Error> {
    let seal = identity::seal(path, limit)?;
    if seal.sha256() != expected {
        return Err(crate::selected::Error::Path {
            path: path.into(),
            detail: "input changed after corpus verification",
        }
        .into());
    }
    Ok(seal)
}

fn seals(
    corpus: &examples::Corpus,
    sources: &BTreeSet<&str>,
    request: &Request<'_>,
) -> Result<Vec<FileSeal>, Error> {
    let mut sealed = Vec::new();
    for executable in [request.native, request.reference] {
        if !executable.is_absolute() {
            return Err(Error::Configuration(
                "native and reference executable paths must be absolute",
            ));
        }
        sealed.push(identity::seal(
            executable,
            request.limits.max_executable_bytes,
        )?);
    }
    if identity::aliases(&sealed[0], &sealed[1]) {
        return Err(Error::Configuration(
            "native and reference executable identities must differ",
        ));
    }
    sealed.push(checked_seal(
        &request.corpus.join("manifest.json"),
        request.limits.corpus.manifest_bytes,
        examples::MANIFEST_SHA256,
    )?);
    sealed.push(checked_seal(
        &request.corpus.join("LICENSE"),
        request.limits.corpus.source_bytes,
        corpus.license_sha256(),
    )?);
    for source in corpus
        .files()
        .iter()
        .filter(|source| sources.contains(source.path()))
    {
        sealed.push(checked_seal(
            &request.corpus.join(source.path()),
            request.limits.corpus.source_bytes,
            source.source_sha256(),
        )?);
    }
    Ok(sealed)
}

fn copy_sources(
    corpus: &examples::Corpus,
    sources: &BTreeSet<&str>,
    directory: &Path,
    limit: usize,
) -> Result<Vec<FileSeal>, Error> {
    let mut sealed = Vec::new();
    for source in corpus
        .files()
        .iter()
        .filter(|source| sources.contains(source.path()))
    {
        let path = directory.join(source.path());
        let parent = path
            .parent()
            .ok_or(Error::Configuration("source path lacks a parent"))?;
        std::fs::create_dir_all(parent).map_err(|error| super::io(parent, error))?;
        std::fs::write(&path, source.source()).map_err(|error| super::io(&path, error))?;
        sealed.push(checked_seal(&path, limit, source.source_sha256())?);
    }
    Ok(sealed)
}

fn execute(
    request: &Request<'_>,
    corpus: &examples::Corpus,
    directory: &Path,
    deadline: Instant,
    report: &mut Report,
) {
    for (executable, argument) in [
        (request.native, "--version"),
        (request.native, "--help-all"),
        (request.reference, "--version"),
    ] {
        let Some(capture) = invoke(
            executable,
            vec![argument.into()],
            directory,
            deadline,
            report,
        ) else {
            return;
        };
        let complete = capture.complete(false);
        report.metadata.push(capture);
        if !complete {
            report.faults.push(Fault::Metadata);
            return;
        }
    }
    let mut references: [Option<answers::ReportedAnswers>; 3] = std::array::from_fn(|_| None);
    for slot in request.schedule.slots() {
        let case = corpus
            .cases()
            .iter()
            .find(|case| case.path() == slot.case.path())
            .expect("fixed source cases validated before execution");
        let (executable, arguments) = arguments(request, directory, slot);
        let Some(capture) = invoke(executable, arguments, directory, deadline, report) else {
            break;
        };
        let mut sample = Sample {
            slot,
            capture,
            decision: Decision::Pass,
            detail: None,
            selected_models: None,
            cost: None,
            diagnostics: None,
        };
        let result = qualify(
            &mut sample,
            case.contract(),
            references[slot.case.index()].as_ref(),
            request.limits.answers,
        );
        match result {
            Ok(answer) => {
                if slot.phase == Phase::Qualification && slot.producer == Producer::Reference {
                    references[slot.case.index()] = Some(answer);
                }
            }
            Err((decision, detail)) => {
                sample.decision = decision;
                sample.detail = Some(detail);
            }
        }
        let passed = sample.decision == Decision::Pass;
        report.samples.push(sample);
        if !passed {
            report.faults.push(Fault::Observation);
            break;
        }
    }
}

fn arguments<'a>(request: &Request<'a>, directory: &Path, slot: Slot) -> (&'a Path, Vec<OsString>) {
    let (executable, flags): (_, &[&str]) = match slot.producer {
        Producer::Native => (
            request.native,
            &[
                "--backend",
                "cpu",
                "--grounder",
                "eager",
                "--oracle",
                "auto",
                "--workers",
                "1",
                "--completion-workers",
                "1",
                "--models",
                "0",
                "--color",
                "never",
            ],
        ),
        Producer::Reference => (
            request.reference,
            &[
                "--models=0",
                "--outf=2",
                "--opt-mode=optN",
                "--parallel-mode=1",
                "--warn=none",
            ],
        ),
    };
    let mut arguments: Vec<OsString> = flags.iter().map(OsString::from).collect();
    if slot.phase == Phase::Diagnostics {
        arguments.push("--stats".into());
    }
    arguments.push(directory.join(slot.case.path()).into_os_string());
    (executable, arguments)
}

fn qualify(
    sample: &mut Sample,
    contract: &examples::Contract,
    reference: Option<&answers::ReportedAnswers>,
    limits: answers::Limits,
) -> Result<answers::ReportedAnswers, (Decision, String)> {
    if !sample
        .capture
        .complete(sample.slot.producer == Producer::Reference)
    {
        return Err((
            Decision::InvocationFailure,
            "capture/exit/cleanup did not complete successfully".into(),
        ));
    }
    let parsed = match sample.slot.producer {
        Producer::Reference => answers::clingo_json(sample.capture.stdout(), limits),
        Producer::Native => answers::native_text(
            sample.capture.stdout(),
            contract.family() == examples::Family::Optimal,
            limits,
        ),
    }
    .map_err(|error| (Decision::InvalidReport, error.to_string()))?;
    sample.selected_models = Some(parsed.model_count());
    sample.cost = parsed.cost().map(<[i64]>::to_vec);
    contract
        .check(&parsed)
        .map_err(|error| (Decision::ModelMismatch, error.to_string()))?;
    if let Some(reference) = reference {
        if !answers::same_displays(reference, &parsed) {
            return Err((
                Decision::ModelMismatch,
                "complete selected displays/counts/costs differ from first qualified reference"
                    .into(),
            ));
        }
    } else if sample.slot.producer != Producer::Reference
        || sample.slot.phase != Phase::Qualification
    {
        return Err((
            Decision::ModelMismatch,
            "no complete qualified reference is available".into(),
        ));
    }
    if sample.slot.phase == Phase::Diagnostics {
        sample.diagnostics = Some(
            timing::parse(sample.capture.stderr())
                .map_err(|error| (Decision::InvalidDiagnostics, error))?,
        );
    }
    Ok(parsed)
}

fn invoke(
    executable: &Path,
    arguments: Vec<OsString>,
    directory: &Path,
    deadline: Instant,
    report: &mut Report,
) -> Option<Capture> {
    let remaining_time = deadline.saturating_duration_since(Instant::now());
    if remaining_time.is_zero() {
        report.faults.push(Fault::Deadline);
        return None;
    }
    let remaining_bytes = report
        .limits
        .max_total_capture_bytes
        .saturating_sub(report.total_capture_bytes);
    if remaining_bytes == 0 {
        report.faults.push(Fault::CaptureBudget);
        return None;
    }
    let limits = process::Limits {
        timeout: report.limits.process.timeout.min(remaining_time),
        max_output_bytes: report.limits.process.max_output_bytes.min(remaining_bytes),
        ..report.limits.process
    };
    let mut record = Capture {
        executable: executable.into(),
        arguments,
        directory: directory.into(),
        started_unix_ns: unix_ns(),
        elapsed_ns: None,
        stop: None,
        exit: None,
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
        limits,
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
            if let Some(child) = pending {
                let cleanup = child.retry(limits.cleanup_timeout);
                record.exit = cleanup.exit.or(record.exit);
                if let Some(failure) = cleanup.failure {
                    report.faults.push(Fault::ChildCleanup(failure.to_string()));
                }
                if let Some(child) = cleanup.pending {
                    let id = child.abandon();
                    record.unresolved_child = Some(id);
                    report.unresolved_children.push(id);
                }
            }
            report.total_capture_bytes += record.stdout.len() + record.stderr.len();
        }
    }
    Some(record)
}
