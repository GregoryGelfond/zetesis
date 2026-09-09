//! Sequential bounded processes around pure schedule and display comparisons.
mod memory;
mod metadata;

use super::capture::unix_ns;
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::path::Path;
use std::time::Instant;

use super::{
    Capture, Case, Decision, Error, Fault, Phase, Producer, Report, Request, Sample, Slot, Suite,
    timing,
};
use crate::selected::{FileSeal, identity, publication};
use crate::{answers, examples, process};

#[cfg(test)]
#[path = "../../tests/support/performance_sources.rs"]
mod tests;

pub(super) fn campaign(request: &Request<'_>, helper: Option<&Path>) -> Result<Report, Error> {
    if !cfg!(any(target_os = "linux", target_os = "macos")) {
        return Err(Error::Configuration(
            "bounded process capture requires Linux or macOS",
        ));
    }
    if request.schedule.memory_runs() > 0 && helper.is_none() {
        return Err(Error::Configuration(
            "memory observations require run_with_runner and a fresh helper",
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
    let sources = match request.schedule.suite() {
        Some(suite) => source_paths(&corpus, suite)?,
        None => selected_sources(&corpus, request.schedule.cases())?,
    };
    let mut before = seals(
        &corpus,
        &sources,
        request.corpus,
        request.native,
        request.reference,
        request.limits,
    )?;
    if let Some(helper) = helper {
        if !helper.is_absolute() {
            return Err(Error::Configuration("memory helper must be absolute"));
        }
        before.push(identity::seal(helper, request.limits.max_executable_bytes)?);
    }
    let destination = publication::prepare(request.report, corpus.root(), &before)?;
    let directory = tempfile::tempdir()
        .map_err(|source| super::io(Path::new("private performance sources"), source))?;
    let mut report = Report {
        schema: if request.schedule.extended() { 2 } else { 1 },
        manifest_sha256: examples::MANIFEST_SHA256,
        schedule: request.schedule.clone(),
        limits: request.limits,
        started_unix_ns,
        finished_unix_ns: None,
        wall_scope: "fresh_process_spawn_capture_reap; output_included; comparison_hashing_excluded; no_cold_cache_claim",
        comparison_scope: "complete_selected_displays_with_symbol_and_model_multiplicities; final_optimum_ties_and_costs; hidden_interpretations_unavailable",
        peak_rss: if request.schedule.memory_runs() > 0 {
            "separate_fresh_helper_RUSAGE_CHILDREN; excludes_helper; may_include_usage_propagated_by_waited_descendants; not_simultaneous_tree_RSS_or_device_memory; macOS_bytes_Linux_KiB_converted_to_bytes"
        } else {
            "unavailable: direct-child capture does not retain rusage"
        },
        gpu_measurement: "unavailable: this fixed CPU campaign is not the full94 eager/lazy CPU/Metal matrix",
        before,
        after: Vec::new(),
        metadata: Vec::new(),
        metadata_complete: false,
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
            execute(
                request,
                &corpus,
                directory.path(),
                deadline,
                &mut report,
                helper,
            );
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

pub(super) fn source_paths(
    corpus: &examples::Corpus,
    suite: Suite,
) -> Result<BTreeSet<&str>, Error> {
    selected_sources(corpus, suite.cases())
}

fn selected_sources<'a>(
    corpus: &'a examples::Corpus,
    cases: &[Case],
) -> Result<BTreeSet<&'a str>, Error> {
    let mut sources = BTreeSet::new();
    for selected in cases {
        let case = corpus
            .cases()
            .iter()
            .find(|case| case.path() == selected.path())
            .ok_or(Error::Configuration(
                "selected performance case is missing from sealed corpus",
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

pub(super) fn seals(
    corpus: &examples::Corpus,
    sources: &BTreeSet<&str>,
    root: &Path,
    native: &Path,
    reference: &Path,
    limits: super::Limits,
) -> Result<Vec<FileSeal>, Error> {
    let mut sealed = Vec::new();
    for executable in [native, reference] {
        if !executable.is_absolute() {
            return Err(Error::Configuration(
                "native and reference executable paths must be absolute",
            ));
        }
        sealed.push(identity::seal(executable, limits.max_executable_bytes)?);
    }
    if identity::aliases(&sealed[0], &sealed[1]) {
        return Err(Error::Configuration(
            "native and reference executable identities must differ",
        ));
    }
    sealed.push(checked_seal(
        &root.join("manifest.json"),
        limits.corpus.manifest_bytes,
        examples::MANIFEST_SHA256,
    )?);
    sealed.push(checked_seal(
        &root.join("LICENSE"),
        limits.corpus.source_bytes,
        corpus.license_sha256(),
    )?);
    for source in corpus
        .files()
        .iter()
        .filter(|source| sources.contains(source.path()))
    {
        sealed.push(checked_seal(
            &root.join(source.path()),
            limits.corpus.source_bytes,
            source.source_sha256(),
        )?);
    }
    Ok(sealed)
}

pub(super) fn copy_sources(
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
    helper: Option<&Path>,
) {
    if !metadata::collect(request, directory, deadline, report) {
        return;
    }
    let mut references = BTreeMap::new();
    for slot in request.schedule.slots() {
        let case = corpus
            .cases()
            .iter()
            .find(|case| case.path() == slot.case.path())
            .expect("fixed source cases validated before execution");
        let (executable, arguments) = arguments(request, directory, &slot);
        let memory_path = directory.join("child-rss.json");
        let (capture, memory) = if slot.phase == Phase::Memory {
            let Some(capture) = memory::invoke(
                helper.expect("memory helper admitted"),
                process::Invocation {
                    executable,
                    arguments: &arguments,
                    directory,
                },
                &memory_path,
                deadline,
                report,
            ) else {
                break;
            };
            let memory = memory::read(&memory_path, capture.helper_child_id());
            (capture, Some(memory))
        } else {
            let Some(capture) = invoke(executable, arguments, directory, deadline, report) else {
                break;
            };
            (capture, None)
        };
        let mut sample = Sample {
            slot: slot.clone(),
            capture,
            decision: Decision::Pass,
            detail: None,
            selected_models: None,
            cost: None,
            diagnostics: None,
            memory: None,
            memory_record: None,
        };
        if let Some((raw, memory)) = memory {
            sample.memory_record = Some(raw);
            match memory {
                Ok(memory) => sample.memory = Some(memory),
                Err(detail) => {
                    sample.decision = if sample.capture.complete(false) {
                        Decision::InvalidMemory
                    } else {
                        Decision::InvocationFailure
                    };
                    sample.detail = Some(detail);
                    report.samples.push(sample);
                    report.faults.push(Fault::Observation);
                    break;
                }
            }
        }
        let result = qualify(
            &mut sample,
            case.contract(),
            references.get(slot.case.path()),
            request.limits.answers,
        );
        match result {
            Ok(answer) => {
                if slot.phase == Phase::Qualification && slot.producer == Producer::Reference {
                    references.insert(slot.case.path().to_owned(), answer);
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

fn arguments<'a>(
    request: &Request<'a>,
    directory: &Path,
    slot: &Slot,
) -> (&'a Path, Vec<OsString>) {
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
        .complete(sample.slot.phase != Phase::Memory && sample.slot.producer == Producer::Reference)
    {
        return Err((
            Decision::InvocationFailure,
            "capture/exit/cleanup did not complete successfully".into(),
        ));
    }
    if sample.slot.phase == Phase::Memory
        && !sample.memory.is_some_and(|memory| {
            memory.valid()
                && memory.signal.is_none()
                && match memory.exit_code {
                    Some(0) => true,
                    Some(10 | 20 | 30) => sample.slot.producer == Producer::Reference,
                    _ => false,
                }
        })
    {
        return Err((
            Decision::InvocationFailure,
            "child RSS record lacks an accepted solver exit".into(),
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
    launch(executable, arguments, directory, deadline, report, false)
}

fn launch(
    executable: &Path,
    arguments: Vec<OsString>,
    directory: &Path,
    deadline: Instant,
    report: &mut Report,
    supervised: bool,
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
    let (record, cleanup_fault) = if supervised {
        super::capture::supervised(executable, arguments, directory, limits)
    } else {
        super::capture::invoke(executable, arguments, directory, limits)
    };
    if let Some(fault) = cleanup_fault {
        report.faults.push(Fault::ChildCleanup(fault));
    }
    if let Some(id) = record.unresolved_child {
        report.unresolved_children.push(id);
    }
    report.total_capture_bytes += record.stdout.len() + record.stderr.len();
    Some(record)
}
