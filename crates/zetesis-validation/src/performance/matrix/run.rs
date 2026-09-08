//! Bounded sequential effects around fixed cells and pure answer contracts.
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::Path;
use std::time::Instant;

use super::super::{Capture, Error, Fault, Phase, capture};
use super::{Decision, Plan, Producer, Report, Request, Sample, Slot, Suite, outcome, telemetry};
use crate::selected::{identity, publication};
use crate::{answers, examples, process};
use serde_json::Value;

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
    let started =
        capture::unix_ns().ok_or(Error::Configuration("UTC metadata precedes Unix epoch"))?;
    let corpus = examples::load(request.corpus, request.limits.corpus).map_err(Error::Corpus)?;
    let cases = cases(&corpus, &request.plan)?;
    let sources: BTreeSet<_> = cases
        .iter()
        .flat_map(|case| case.transitive_source_paths().iter().map(String::as_str))
        .collect();
    let before = super::super::run::seals(
        &corpus,
        &sources,
        request.corpus,
        request.native,
        request.reference,
        request.limits,
    )?;
    let destination = publication::prepare(request.report, corpus.root(), &before)?;
    let directory = tempfile::tempdir()
        .map_err(|e| super::super::io(Path::new("private matrix sources"), e))?;
    let mut report = Report {
        schema: 1,
        protocol: "instrumented_explicit_profile_matrix_v1",
        manifest_sha256: examples::MANIFEST_SHA256,
        plan: request.plan.clone(),
        limits: request.limits,
        native_normalization_limits: normalization_limits(request),
        cases: cases.iter().map(|case| case.path().to_owned()).collect(),
        started_unix_ns: started,
        finished_unix_ns: None,
        wall_scope: "fresh_process_spawn_capture_reap; native_json_and_stats_included; reference_json_included; comparison_hashing_excluded; no_cold_cache_claim",
        comparison_scope: "complete_selected_displays_with_symbol_and_model_multiplicities; final_optimum_ties_and_costs; native_full_records_retained; hidden_reference_interpretations_unavailable",
        peak_rss: "unavailable: safe direct-child capture does not retain per-child rusage; no inference from logical allocation counters",
        before,
        after: Vec::new(),
        metadata: Vec::new(),
        samples: Vec::new(),
        total_capture_bytes: 0,
        faults: Vec::new(),
        unresolved_children: Vec::new(),
        destination,
    };
    match super::super::run::copy_sources(
        &corpus,
        &sources,
        directory.path(),
        request.limits.corpus.source_bytes,
    ) {
        Ok(sealed) => {
            report.before.extend(sealed);
            execute(request, &cases, directory.path(), deadline, &mut report)?;
        }
        Err(error) => {
            report.faults.push(Fault::InputWrite(error.to_string()));
            fill_unattempted(&mut report)?;
        }
    }
    report.after = report.before.iter().map(identity::recheck).collect();
    if let Err(error) = directory.close() {
        report.faults.push(Fault::InputCleanup(error.to_string()));
    }
    report.finished_unix_ns = capture::unix_ns();
    if report.finished_unix_ns.is_none() {
        report.faults.push(Fault::Clock);
    }
    Ok(report)
}
fn cases<'a>(corpus: &'a examples::Corpus, plan: &Plan) -> Result<Vec<&'a examples::Case>, Error> {
    let suite = match plan.suite {
        Suite::Corpus => return Ok(corpus.cases().iter().collect()),
        Suite::Baseline => super::super::Suite::Baseline,
        Suite::Queens => super::super::Suite::Queens,
    };
    suite
        .cases()
        .iter()
        .map(|selected| {
            corpus
                .cases()
                .iter()
                .find(|case| case.path() == selected.path())
                .ok_or(Error::Configuration(
                    "selected case is missing from sealed corpus",
                ))
        })
        .collect()
}
fn unattempted(slot: Slot, blocked_by: Option<usize>, detail: &str) -> Sample {
    Sample {
        slot,
        capture: None,
        decision: Decision::NotAttempted,
        detail: Some(detail.into()),
        blocked_by,
        selected_models: None,
        cost: None,
        observation: None,
    }
}
fn fill_unattempted(report: &mut Report) -> Result<(), Error> {
    report.samples = report
        .plan
        .slots(report.cases.len())?
        .into_iter()
        .map(|slot| unattempted(slot, None, "campaign setup prevented execution"))
        .collect();
    Ok(())
}
fn execute(
    request: &Request<'_>,
    cases: &[&examples::Case],
    directory: &Path,
    deadline: Instant,
    report: &mut Report,
) -> Result<(), Error> {
    for (executable, argument) in [
        (request.native, "--version"),
        (request.native, "--help-all"),
        (request.reference, "--version"),
    ] {
        let Some(observed) = invoke(
            executable,
            vec![argument.into()],
            directory,
            deadline,
            report,
        ) else {
            fill_unattempted(report)?;
            return Ok(());
        };
        let completed = observed.complete(false);
        report.metadata.push(observed);
        if !completed {
            report.faults.push(Fault::Metadata);
            fill_unattempted(report)?;
            return Ok(());
        }
    }
    let mut references: Vec<Option<answers::ReportedAnswers>> =
        (0..cases.len()).map(|_| None).collect();
    let width = request.plan.profiles.len() + 1;
    let mut blocked: Vec<Option<usize>> = vec![None; cases.len() * width];
    let mut stopped = false;
    for slot in request.plan.slots(cases.len())? {
        let cell = slot.case * width + slot.producer.index();
        if stopped {
            report.samples.push(unattempted(
                slot,
                None,
                "campaign scheduling stopped; no replacement launches",
            ));
            continue;
        }
        if let Some(previous) = blocked[cell] {
            report.samples.push(unattempted(
                slot,
                Some(previous),
                "cell disabled by its first non-pass observation",
            ));
            continue;
        }
        if slot.phase != Phase::Qualification && references[slot.case].is_none() {
            report.samples.push(unattempted(
                slot,
                blocked[slot.case * width],
                "reference census did not establish a complete family",
            ));
            continue;
        }
        let (executable, arguments) =
            arguments(request, directory, cases[slot.case].path(), slot.producer);
        let Some(capture) = invoke(executable, arguments, directory, deadline, report) else {
            stopped = true;
            report.samples.push(unattempted(
                slot,
                None,
                "campaign scheduling budget or cleanup prevented launch",
            ));
            continue;
        };
        let mut sample = Sample {
            slot,
            capture: Some(capture),
            decision: Decision::Pass,
            detail: None,
            blocked_by: None,
            selected_models: None,
            cost: None,
            observation: None,
        };
        let result = qualify(
            &mut sample,
            cases[slot.case].contract(),
            references[slot.case].as_ref(),
            request,
        );
        match result {
            Ok(answer) => {
                if slot.phase == Phase::Qualification && slot.producer == Producer::Reference {
                    references[slot.case] = Some(answer);
                }
            }
            Err((decision, detail)) => {
                sample.decision = decision;
                sample.detail = Some(detail);
                blocked[cell] = Some(report.samples.len());
            }
        }
        stopped = !report.unresolved_children.is_empty();
        report.samples.push(sample);
    }
    Ok(())
}
fn arguments<'a>(
    request: &Request<'a>,
    directory: &Path,
    path: &str,
    producer: Producer,
) -> (&'a Path, Vec<OsString>) {
    let (executable, mut arguments): (_, Vec<OsString>) = match producer {
        Producer::Reference => (
            request.reference,
            vec![
                "--models=0".into(),
                "--outf=2".into(),
                "--opt-mode=optN".into(),
                format!("--parallel-mode={}", request.plan.reference_workers).into(),
                "--warn=none".into(),
            ],
        ),
        Producer::Native { profile } => {
            let profile = request.plan.profiles[profile];
            let values = [
                ("--backend", profile.backend.label().into()),
                ("--grounder", profile.grounder.label().into()),
                ("--oracle", profile.oracle.label().into()),
                ("--workers", profile.workers.to_string()),
                (
                    "--completion-workers",
                    profile.completion_workers.to_string(),
                ),
                ("--batch-size", profile.batch_size.to_string()),
                (
                    "--max-completion-scratch-bytes",
                    profile.max_completion_scratch_bytes.to_string(),
                ),
                ("--models", "0".into()),
                ("--color", "never".into()),
            ];
            (
                request.native,
                values
                    .into_iter()
                    .flat_map(|(flag, value)| [flag.into(), value.into()])
                    .chain(["--json".into(), "--stats".into()])
                    .collect(),
            )
        }
    };
    arguments.push(directory.join(path).into_os_string());
    (executable, arguments)
}
fn qualify(
    sample: &mut Sample,
    contract: &examples::Contract,
    reference: Option<&answers::ReportedAnswers>,
    request: &Request<'_>,
) -> Result<answers::ReportedAnswers, (Decision, String)> {
    let capture = sample
        .capture
        .as_ref()
        .expect("qualification follows a launched capture");
    if capture.stop() == Some(process::Stop::Deadline) {
        return Err((Decision::Timeout, "process deadline".into()));
    }
    if capture.stop() == Some(process::Stop::OutputLimit) {
        return Err((Decision::CaptureLimit, "capture byte ceiling".into()));
    }
    if capture.stop() != Some(process::Stop::Completed)
        || capture.failure().is_some()
        || capture.cleanup_failure().is_some()
    {
        return Err((
            Decision::InvocationFailure,
            "capture/start/cleanup did not complete".into(),
        ));
    }
    let parsed = match sample.slot.producer {
        Producer::Reference => {
            if !capture.complete(true) {
                return Err((
                    Decision::InvocationFailure,
                    "reference exit did not complete".into(),
                ));
            }
            answers::clingo_json(capture.stdout(), request.limits.answers)
                .map_err(|error| invalid(&error))?
        }
        Producer::Native { profile } => {
            let document: Value = serde_json::from_slice(capture.stdout())
                .map_err(|e| (Decision::InvalidReport, e.to_string()))?;
            outcome::check(&document, capture.exit())?;
            let native = answers::native_json::parse(capture.stdout(), request.native_answers)
                .map_err(|error| invalid(&error))?;
            let display = native
                .reported_displays(request.max_spelling_bytes)
                .map_err(|error| invalid(&error))?;
            sample.observation = Some(
                telemetry::observe(&document, capture.stderr(), request.plan.profiles[profile])
                    .map_err(|e| (Decision::InvalidTelemetry, e))?,
            );
            display
        }
    };
    sample.selected_models = Some(parsed.model_count());
    sample.cost = parsed.cost().map(<[i64]>::to_vec);
    contract
        .check(&parsed)
        .map_err(|e| (Decision::ParityMismatch, e.to_string()))?;
    if let Some(reference) = reference {
        if !answers::same_displays(reference, &parsed) {
            return Err((
                Decision::ParityMismatch,
                "complete selected displays/counts/costs differ from qualified reference".into(),
            ));
        }
    } else if sample.slot.producer != Producer::Reference
        || sample.slot.phase != Phase::Qualification
    {
        return Err((
            Decision::ReferenceUnavailable,
            "no complete qualified reference family".into(),
        ));
    }
    Ok(parsed)
}
fn invalid(error: &answers::Error) -> (Decision, String) {
    (Decision::InvalidReport, error.to_string())
}
fn invoke(
    executable: &Path,
    arguments: Vec<OsString>,
    directory: &Path,
    deadline: Instant,
    report: &mut Report,
) -> Option<Capture> {
    if !report.unresolved_children.is_empty() {
        return None;
    }
    let time = deadline.saturating_duration_since(Instant::now());
    if time.is_zero() {
        report.faults.push(Fault::Deadline);
        return None;
    }
    let bytes = report
        .limits
        .max_total_capture_bytes
        .saturating_sub(report.total_capture_bytes);
    if bytes == 0 {
        report.faults.push(Fault::CaptureBudget);
        return None;
    }
    let limits = process::Limits {
        timeout: report.limits.process.timeout.min(time),
        max_output_bytes: report.limits.process.max_output_bytes.min(bytes),
        ..report.limits.process
    };
    let (capture, fault) = capture::invoke(executable, arguments, directory, limits);
    report.total_capture_bytes += capture.stdout().len() + capture.stderr().len();
    if let Some(fault) = fault {
        report.faults.push(Fault::ChildCleanup(fault));
    }
    if let Some(id) = capture.unresolved_child {
        report.unresolved_children.push(id);
    }
    Some(capture)
}

fn normalization_limits(request: &Request<'_>) -> Value {
    let limits = request.native_answers;
    serde_json::json!({"input_bytes":limits.report.max_input_bytes,
        "witnesses":limits.report.max_witnesses,"symbols":limits.report.max_symbols,
        "cost_dimensions":limits.report.max_cost_dimensions,"atoms":limits.max_atoms,
        "value_nodes":limits.max_value_nodes,"value":{"nodes":limits.value.max_nodes,
        "depth":limits.value.max_depth,"bytes":limits.value.max_bytes},
        "spelling_bytes":request.max_spelling_bytes})
}

#[cfg(test)]
#[path = "../../../tests/support/matrix_comparison.rs"]
mod tests;
