//! Bounded sequential effects around fixed cells and pure answer contracts.
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use super::super::{Capture, Error, Fault, Phase, capture};
use super::{
    Decision, NativeInvocation, Plan, Producer, RecordedPlan, Report, Request, Sample, Slot, Suite,
    Workload, outcome,
};
use crate::selected::{identity, publication};
use crate::{answers, examples, process};
use serde_json::Value;

/// Scheduling limits shared by metadata, timed and supervised memory children.
#[derive(Clone, Copy)]
struct Schedule<'a> {
    deadline: Instant,
    cancelled: &'a AtomicBool,
}
impl Schedule<'_> {
    fn stopped(self, report: &mut Report) -> bool {
        if self.cancelled.load(Ordering::Relaxed) {
            record_cancellation(report);
            true
        } else {
            !report.unresolved_children.is_empty()
        }
    }
}

pub(super) fn campaign(
    request: &Request<'_>,
    workloads: Option<&[Workload]>,
    invocation: NativeInvocation,
    cancelled: &AtomicBool,
) -> Result<Report, Error> {
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
    if request.plan.memory_runs() > 0 && !request.helper.is_some_and(Path::is_absolute) {
        return Err(Error::Configuration(
            "memory rounds require an absolute helper executable",
        ));
    }
    let corpus = examples::load(request.corpus, request.limits.corpus).map_err(Error::Corpus)?;
    let cases = prepare(&corpus, request, workloads)?;
    let sources: BTreeSet<_> = cases
        .iter()
        .flat_map(|case| case.input.corpus_source_paths())
        .collect();
    let before = seal_inputs(&corpus, &sources, request, workloads)?;
    let destination = publication::prepare(request.report, corpus.root(), &before)?;
    let directory = tempfile::tempdir()
        .map_err(|e| super::super::io(Path::new("private matrix sources"), e))?;
    let mut report = Report {
        schema: if workloads.is_some() { 2 } else { 1 },
        protocol: if workloads.is_some() {
            "instrumented_derived_workload_matrix_v2"
        } else {
            "instrumented_explicit_profile_matrix_v1"
        },
        manifest_sha256: examples::MANIFEST_SHA256,
        manifest_scope: "correctness_catalog_context; authored_and_generated_workloads_retain_independent_source_and_contract_identities",
        plan: RecordedPlan {
            plan: request.plan.clone(),
            reference_policy: request.reference.map(|reference| reference.policy),
        },
        limits: request.limits,
        native_normalization_limits: normalization_limits(request),
        cases: cases
            .iter()
            .map(|case| case.input.path().to_owned())
            .collect(),
        workloads: workloads.map(<[Workload]>::to_vec),
        started_unix_ns: started,
        finished_unix_ns: None,
        wall_scope: wall_scope(request.reference.is_some()),
        comparison_scope: comparison_scope(request.reference.is_some()),
        peak_rss: if request.plan.memory_runs() > 0 {
            "memory_rounds: separate_fresh_helper_RUSAGE_CHILDREN; excludes_helper; may_include_usage_propagated_by_waited_descendants; not_simultaneous_tree_RSS_or_device_memory; macOS_bytes_Linux_KiB_converted_to_bytes"
        } else {
            "unavailable: safe direct-child capture does not retain per-child rusage; no inference from logical allocation counters"
        },
        before,
        after: Vec::new(),
        metadata: Vec::new(),
        samples: Vec::new(),
        total_capture_bytes: 0,
        faults: Vec::new(),
        unresolved_children: Vec::new(),
        destination,
    };
    match materialize(&corpus, &sources, &cases, directory.path(), request) {
        Ok(sealed) => {
            report.before.extend(sealed);
            execute(
                request,
                &cases,
                directory.path(),
                Schedule {
                    deadline,
                    cancelled,
                },
                &mut report,
                invocation,
            )?;
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

/// What a sample's wall time covers; clingo's output only when it takes part.
const fn wall_scope(clingo: bool) -> &'static str {
    if clingo {
        "fresh_process_spawn_capture_reap; native_json_and_stats_included; reference_json_included; comparison_hashing_excluded; no_cold_cache_claim"
    } else {
        "fresh_process_spawn_capture_reap; native_json_and_stats_included; no_reference; comparison_hashing_excluded; no_cold_cache_claim"
    }
}

/// What qualification compares: clingo's census when it takes part, the
/// recorded contracts otherwise.
const fn comparison_scope(clingo: bool) -> &'static str {
    if clingo {
        "complete_selected_displays_with_symbol_and_model_multiplicities; final_optimum_ties_and_costs; full_native_atoms_and_shown_values_compared_across_profiles_and_repeats; hidden_reference_interpretations_unavailable"
    } else {
        "complete_selected_displays_checked_against_recorded_contracts; final_optimum_ties_and_costs; full_native_atoms_and_shown_values_compared_across_profiles_and_repeats; workloads_without_contracts_need_clingo"
    }
}

/// Preserve the native, reference (when one takes part) and catalog prefix used
/// by retained-report readers, then seal optional helpers and independent
/// authored sources in the same campaign.
fn seal_inputs(
    corpus: &examples::Corpus,
    sources: &BTreeSet<&str>,
    request: &Request<'_>,
    workloads: Option<&[Workload]>,
) -> Result<Vec<crate::selected::FileSeal>, Error> {
    let mut sealed = super::super::run::seals(
        corpus,
        sources,
        request.corpus,
        request.native,
        request.reference.map(|reference| reference.executable),
        request.limits,
    )?;
    if let Some(helper) = request.helper.filter(|_| request.plan.memory_runs() > 0) {
        sealed.push(identity::seal(helper, request.limits.max_executable_bytes)?);
    }
    if let Some(workloads) = workloads {
        sealed.extend(
            workloads
                .iter()
                .filter_map(Workload::authored_seal)
                .cloned(),
        );
    }
    Ok(sealed)
}

struct Prepared<'a> {
    input: Input<'a>,
    directory: PathBuf,
}

/// Where a cell's program comes from: a sealed corpus entry, unchanged or
/// through a workload that amends it, or an independently sealed authored/generated program.
enum Input<'a> {
    Corpus {
        case: &'a examples::Case,
        workload: Option<&'a Workload>,
    },
    Independent(&'a Workload),
}
impl<'a> Input<'a> {
    fn path(&self) -> &str {
        match self {
            Self::Corpus { case, .. } => case.path(),
            Self::Independent(workload) => workload.entry(),
        }
    }
    /// Corpus files this input reads; a generated program reads none.
    fn corpus_source_paths(&self) -> impl Iterator<Item = &str> {
        match self {
            Self::Corpus { case, .. } => case.transitive_source_paths().iter(),
            Self::Independent(_) => [].iter(),
        }
        .map(String::as_str)
    }
    /// The workload the input runs through, when it runs through one.
    fn workload(&self) -> Option<&'a Workload> {
        match self {
            Self::Corpus { workload, .. } => *workload,
            Self::Independent(workload) => Some(workload),
        }
    }
    /// The contract the campaign checks: the corpus contract of an
    /// unchanged entry, and the workload's otherwise.
    fn contract(&self) -> Option<&'a examples::Contract> {
        match self {
            Self::Corpus {
                case,
                workload: None,
            } => Some(case.contract()),
            Self::Corpus {
                workload: Some(workload),
                ..
            }
            | Self::Independent(workload) => workload.contract(),
        }
    }
}

fn prepare<'a>(
    corpus: &'a examples::Corpus,
    request: &Request<'_>,
    workloads: Option<&'a [Workload]>,
) -> Result<Vec<Prepared<'a>>, Error> {
    if request.plan.suite() == Suite::Scalability && workloads.is_none() {
        return Err(Error::Configuration(
            "scalability suite requires explicit authored workloads",
        ));
    }
    let allowed = cases(corpus, &request.plan)?;
    let Some(workloads) = workloads else {
        return Ok(allowed
            .into_iter()
            .map(|original| Prepared {
                input: Input::Corpus {
                    case: original,
                    workload: None,
                },
                directory: PathBuf::new(),
            })
            .collect());
    };
    if workloads.is_empty()
        || workloads.len() > request.limits.corpus.cases.min(super::config::MAX_CASES)
    {
        return Err(Error::Configuration(
            "explicit workload population must fit 1..=94",
        ));
    }
    let mut metadata = 0usize;
    let mut sources = 0usize;
    for (position, workload) in workloads.iter().enumerate() {
        if workloads[..position]
            .iter()
            .any(|prior| prior.identity() == workload.identity())
        {
            return Err(Error::Configuration(
                "explicit workloads repeat a content identity",
            ));
        }
        workload.validate(corpus, request.limits)?;
        metadata = metadata
            .checked_add(workload.metadata_bytes)
            .ok_or(Error::Configuration("workload metadata sum overflow"))?;
        sources = sources
            .checked_add(workload.source_bytes)
            .ok_or(Error::Configuration("workload source sum overflow"))?;
    }
    if metadata > request.limits.corpus.manifest_bytes
        || sources > request.limits.corpus.total_source_bytes
    {
        return Err(Error::Configuration(
            "combined workloads exceed retained input ceilings",
        ));
    }
    let mut prepared = Vec::new();
    prepared
        .try_reserve_exact(workloads.len())
        .map_err(|_| Error::Configuration("workload population allocation failed"))?;
    for (position, workload) in workloads.iter().enumerate() {
        let input = if workload.is_generated() || workload.is_authored() {
            Input::Independent(workload)
        } else {
            Input::Corpus {
                case: allowed
                    .iter()
                    .find(|case| case.path() == workload.entry())
                    .ok_or(Error::Configuration(
                        "workload is outside the plan's allowed suite",
                    ))?,
                workload: Some(workload),
            }
        };
        prepared.push(Prepared {
            input,
            directory: format!("workload-{position:02}").into(),
        });
    }
    Ok(prepared)
}

fn materialize(
    corpus: &examples::Corpus,
    sources: &BTreeSet<&str>,
    cases: &[Prepared<'_>],
    directory: &Path,
    request: &Request<'_>,
) -> Result<Vec<crate::selected::FileSeal>, Error> {
    if cases.iter().all(|case| case.input.workload().is_none()) {
        return super::super::run::copy_sources(
            corpus,
            sources,
            directory,
            request.limits.corpus.source_bytes,
        );
    }
    let mut sealed = Vec::new();
    for case in cases {
        let workload = case
            .input
            .workload()
            .ok_or(Error::Configuration("mixed workload preparation"))?;
        sealed.extend(workload.materialize(
            corpus,
            &directory.join(&case.directory),
            request.limits.corpus.source_bytes,
        )?);
    }
    Ok(sealed)
}
/// The plan's cases: its selection when it has one, each a case of its suite,
/// and the whole suite otherwise.
fn cases<'a>(corpus: &'a examples::Corpus, plan: &Plan) -> Result<Vec<&'a examples::Case>, Error> {
    let suite: Vec<&examples::Case> = match plan.suite {
        Suite::Corpus => corpus.cases().iter().collect(),
        Suite::Baseline | Suite::Scalability => {
            members(corpus, super::super::Suite::Baseline.cases())?
        }
        Suite::Queens => members(corpus, super::super::Suite::Queens.cases())?,
        Suite::Series => members(corpus, &super::super::series::CORPUS_CASES)?,
    };
    let Some(selection) = &plan.selection else {
        return Ok(suite);
    };
    selection
        .iter()
        .map(|path| {
            suite
                .iter()
                .copied()
                .find(|case| case.path() == path)
                .ok_or(Error::Configuration(
                    "a selected case is not a case of the plan's suite",
                ))
        })
        .collect()
}

fn members<'a>(
    corpus: &'a examples::Corpus,
    cases: &[super::super::Case],
) -> Result<Vec<&'a examples::Case>, Error> {
    cases
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
        memory: None,
    }
}
/// A position a clingo-free campaign cannot qualify: without clingo, only a
/// recorded contract qualifies a family, and this workload has none.
fn needs_clingo(slot: Slot) -> Sample {
    Sample {
        decision: Decision::NeedsClingo,
        ..unattempted(
            slot,
            None,
            "no recorded contract; only clingo establishes this workload's family",
        )
    }
}
fn fill_unattempted(report: &mut Report) -> Result<(), Error> {
    report.samples = report
        .plan
        .plan
        .slots(report.cases.len(), report.plan.reference_policy)?
        .into_iter()
        .map(|slot| unattempted(slot, None, "campaign setup prevented execution"))
        .collect();
    Ok(())
}
fn capture_metadata(
    request: &Request<'_>,
    directory: &Path,
    schedule: Schedule<'_>,
    report: &mut Report,
    invocation: NativeInvocation,
) -> bool {
    let reference = request
        .reference
        .map(|reference| (reference.executable, vec!["--version".into()]));
    for (executable, arguments) in [
        (request.native, vec!["--version".into()]),
        (request.native, invocation.help_arguments()),
    ]
    .into_iter()
    .chain(reference)
    {
        let Some(observed) = invoke(executable, arguments, false, directory, schedule, report)
        else {
            return false;
        };
        let completed = observed.complete(false);
        report.metadata.push(observed);
        if !completed {
            report.faults.push(Fault::Metadata);
            return false;
        }
    }
    true
}
fn execute(
    request: &Request<'_>,
    cases: &[Prepared<'_>],
    directory: &Path,
    schedule: Schedule<'_>,
    report: &mut Report,
    invocation: NativeInvocation,
) -> Result<(), Error> {
    if !capture_metadata(request, directory, schedule, report, invocation) {
        fill_unattempted(report)?;
        return Ok(());
    }
    let mut censuses: Vec<_> = (0..cases.len()).map(|_| Census::default()).collect();
    let width = request.plan.profiles.len() + 1;
    let mut blocked: Vec<Option<usize>> = vec![None; cases.len() * width];
    let mut stopped = false;
    let policy = request.reference.map(|reference| reference.policy);
    for slot in request.plan.slots(cases.len(), policy)? {
        stopped |= schedule.stopped(report);
        let cell = slot.case * width + slot.producer.index();
        let skipped = if stopped {
            Some((None, "campaign scheduling stopped; no replacement launches"))
        } else if let Some(previous) = blocked[cell] {
            Some((
                Some(previous),
                "cell disabled by its first non-pass observation",
            ))
        } else if request.reference.is_some()
            && slot.phase != Phase::Qualification
            && censuses[slot.case].reference.is_none()
        {
            Some((
                blocked[slot.case * width],
                "reference census did not establish a complete family",
            ))
        } else {
            None
        };
        if let Some((blocker, detail)) = skipped {
            report.samples.push(unattempted(slot, blocker, detail));
            continue;
        }
        let selected = &cases[slot.case];
        if request.reference.is_none() && selected.input.contract().is_none() {
            blocked[cell] = Some(report.samples.len());
            report.samples.push(needs_clingo(slot));
            continue;
        }
        let case_directory = directory.join(&selected.directory);
        let (executable, arguments) = arguments(
            request,
            &case_directory,
            selected.input.path(),
            slot.producer,
            invocation,
        );
        let record = case_directory.join("child-rss.json");
        let launched = launch(
            request,
            slot.phase,
            (executable, arguments),
            &record,
            &case_directory,
            schedule,
            report,
        );
        let Some(capture) = launched else {
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
            memory: None,
        };
        if slot.phase == Phase::Memory
            && !cancelled_capture(sample.capture.as_ref().unwrap())
            && !measured(&mut sample, &record, report)
        {
            continue;
        }
        let contract = selected.input.contract();
        if let Err((decision, detail)) = censuses[slot.case].check(&mut sample, contract, request) {
            sample.decision = decision;
            sample.detail = Some(detail);
            blocked[cell] = Some(report.samples.len());
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
    invocation: NativeInvocation,
) -> (&'a Path, Vec<OsString>) {
    let (executable, mut arguments): (_, Vec<OsString>) = match producer {
        Producer::Reference => (
            request
                .reference
                .expect("the schedule has reference positions only when a reference takes part")
                .executable,
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
            (
                request.native,
                invocation
                    .arguments(&profile)
                    .into_iter()
                    .chain(["--color".into(), "never".into()])
                    .chain(["--json".into(), "--stats".into()])
                    .collect(),
            )
        }
    };
    arguments.push(directory.join(path).into_os_string());
    (executable, arguments)
}
#[derive(Debug)]
struct Qualified {
    display: answers::ReportedAnswers,
    native: Option<answers::native_json::NativeAnswers>,
}

/// One workload's completed answer baselines. Neither a failed reference nor a
/// mismatching native sample can replace the first accepted census it violates.
#[derive(Default)]
struct Census {
    reference: Option<answers::ReportedAnswers>,
    native: Option<answers::native_json::NativeAnswers>,
}

impl Census {
    fn check(
        &mut self,
        sample: &mut Sample,
        contract: Option<&examples::Contract>,
        request: &Request<'_>,
    ) -> Result<(), (Decision, String)> {
        let answer = qualify(sample, contract, self.reference.as_ref(), request)?;
        if let Some(native) = answer.native {
            super::native_family::accept(&mut self.native, native)?;
        }
        if sample.slot.phase == Phase::Qualification && sample.slot.producer == Producer::Reference
        {
            self.reference = Some(answer.display);
        }
        Ok(())
    }
}

fn qualify(
    sample: &mut Sample,
    contract: Option<&examples::Contract>,
    reference: Option<&answers::ReportedAnswers>,
    request: &Request<'_>,
) -> Result<Qualified, (Decision, String)> {
    let capture = sample
        .capture
        .as_ref()
        .expect("qualification follows a launched capture");
    if cancelled_capture(capture) {
        return Err((Decision::Cancelled, "campaign cancelled".into()));
    }
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
    let exit = solver_exit(sample)?;
    let (parsed, native) = match sample.slot.producer {
        Producer::Reference => {
            if !exit.is_some_and(|exit| {
                exit.signal.is_none() && matches!(exit.code, Some(0 | 10 | 20 | 30))
            }) {
                return Err((
                    Decision::InvocationFailure,
                    "reference exit did not complete".into(),
                ));
            }
            (
                answers::clingo_json(capture.stdout(), request.limits.answers)
                    .map_err(|error| invalid(&error))?,
                None,
            )
        }
        Producer::Native { profile } => {
            let document: Value = serde_json::from_slice(capture.stdout())
                .map_err(|e| (Decision::InvalidReport, e.to_string()))?;
            outcome::check(&document, exit)?;
            let native = answers::native_json::parse(capture.stdout(), request.native_answers)
                .map_err(|error| invalid(&error))?;
            let display = native
                .reported_displays(request.max_spelling_bytes)
                .map_err(|error| invalid(&error))?;
            sample.observation = Some(
                super::Observation::from_statistics(
                    &document,
                    capture.stderr(),
                    request.plan.profiles[profile],
                )
                .map_err(|e| (Decision::InvalidTelemetry, e))?,
            );
            (display, Some(native))
        }
    };
    sample.selected_models = Some(parsed.model_count());
    sample.cost = parsed.cost().map(<[i64]>::to_vec);
    if let Some(contract) = contract {
        contract
            .check(&parsed)
            .map_err(|e| (Decision::ParityMismatch, e.to_string()))?;
    }
    // With clingo taking part, its census is the authority; without it, the
    // recorded contract checked above is, and a family without one is refused.
    if let Some(reference) = reference {
        if !answers::same_displays(reference, &parsed) {
            return Err((
                Decision::ParityMismatch,
                "complete selected displays/counts/costs differ from qualified reference".into(),
            ));
        }
    } else if request.reference.is_none() {
        if contract.is_none() {
            return Err((
                Decision::NeedsClingo,
                "no recorded contract; only clingo establishes this workload's family".into(),
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
    Ok(Qualified {
        display: parsed,
        native,
    })
}

/// A memory capture belongs to the helper. Require its success separately,
/// then use the measured solver exit for the producer's outcome contract.
fn solver_exit(sample: &Sample) -> Result<Option<process::Exit>, (Decision, String)> {
    let capture = sample
        .capture
        .as_ref()
        .expect("a launched sample has a capture");
    if sample.slot.phase != Phase::Memory {
        return Ok(capture.exit());
    }
    if !capture.complete(false) {
        return Err((
            Decision::InvocationFailure,
            "memory helper did not complete successfully".into(),
        ));
    }
    let memory = sample
        .memory
        .filter(|memory| memory.valid())
        .ok_or_else(|| {
            (
                Decision::InvalidMemory,
                "memory round lacks a valid solver resource record".into(),
            )
        })?;
    Ok(Some(process::Exit {
        code: memory.exit_code,
        signal: memory.signal,
    }))
}
fn invalid(error: &answers::Error) -> (Decision, String) {
    (Decision::InvalidReport, error.to_string())
}
/// Launch a slot's solver: directly, or on a memory round as the helper's
/// child, the helper writing the child's resource record to `record`.
fn launch(
    request: &Request<'_>,
    phase: Phase,
    (executable, arguments): (&Path, Vec<OsString>),
    record: &Path,
    directory: &Path,
    schedule: Schedule<'_>,
    report: &mut Report,
) -> Option<Capture> {
    if phase != Phase::Memory {
        return invoke(executable, arguments, false, directory, schedule, report);
    }
    let helper = request
        .helper
        .expect("memory rounds admitted with a helper");
    let mut supervised: Vec<OsString> = vec![
        crate::process::memory::HELPER_COMMAND.into(),
        record.as_os_str().to_owned(),
        executable.as_os_str().to_owned(),
    ];
    supervised.extend(arguments);
    invoke(helper, supervised, true, directory, schedule, report)
}

/// Read a memory round's resource record into its sample. A missing or
/// contradictory record decides the sample and is retained with a fault;
/// the campaign goes on, since the rounds are independent invocations.
fn measured(sample: &mut Sample, record: &Path, report: &mut Report) -> bool {
    let helper_child = sample.capture.as_ref().and_then(Capture::helper_child_id);
    match super::super::run::memory::read(record, helper_child).1 {
        Ok(measurement) => {
            sample.memory = Some(measurement);
            true
        }
        Err(detail) => {
            sample.decision = Decision::InvalidMemory;
            sample.detail = Some(detail);
            report.faults.push(Fault::Observation);
            report.samples.push(std::mem::replace(
                sample,
                unattempted(sample.slot, None, ""),
            ));
            false
        }
    }
}

/// Launch one bounded process within the campaign's remaining time and
/// capture; a supervised launch is the memory helper's, whose child is
/// reaped separately.
fn invoke(
    executable: &Path,
    arguments: Vec<OsString>,
    supervised: bool,
    directory: &Path,
    schedule: Schedule<'_>,
    report: &mut Report,
) -> Option<Capture> {
    if schedule.stopped(report) {
        return None;
    }
    let time = schedule.deadline.saturating_duration_since(Instant::now());
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
    let (capture, fault) = if supervised {
        capture::supervised_with_cancellation(
            executable,
            arguments,
            directory,
            limits,
            schedule.cancelled,
        )
    } else {
        capture::invoke_with_cancellation(
            executable,
            arguments,
            directory,
            limits,
            schedule.cancelled,
        )
    };
    if cancelled_capture(&capture) {
        record_cancellation(report);
    }
    report.total_capture_bytes += capture.stdout().len() + capture.stderr().len();
    if let Some(fault) = fault {
        report.faults.push(Fault::ChildCleanup(fault));
    }
    if let Some(id) = capture.unresolved_child {
        report.unresolved_children.push(id);
    }
    Some(capture)
}

fn cancelled_capture(capture: &Capture) -> bool {
    capture.stop() == Some(process::Stop::Cancelled)
        || capture
            .failure()
            .is_some_and(|failure| failure.kind() == crate::selected::InvocationFault::Cancelled)
}

fn record_cancellation(report: &mut Report) {
    if !report
        .faults
        .iter()
        .any(|fault| matches!(fault, Fault::Cancelled))
    {
        report.faults.push(Fault::Cancelled);
    }
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
mod tests;
