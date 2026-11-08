//! Opt-in driver measurements; unavailable execution counters are never inferred.

use std::io::{self, Write};
use std::time::Duration;

use crate::{Completion, Options, PublicationFailure, Report};

fn header(
    sink: &mut impl Write,
    options: &Options,
    config: &crate::SolveConfig,
    elapsed: Duration,
) -> io::Result<()> {
    writeln!(
        sink,
        "Statistics: zetesis {}; GPU compiled={}",
        env!("CARGO_PKG_VERSION"),
        cfg!(feature = "gpu")
    )?;
    writeln!(
        sink,
        "  executable: {}",
        running_executable().map_or_else(
            || "unreported".to_owned(),
            |path| path.display().to_string()
        )
    )?;
    writeln!(
        sink,
        "  requested: backend={}; oracle={}; grounder={}",
        options.backend.label(),
        options.oracle.label(),
        options.grounder.label()
    )?;
    writeln!(
        sink,
        "  configured: workers={}; batch={}; displayed models={} (0=all)",
        options.workers, config.batch_size, options.models
    )?;
    writeln!(
        sink,
        "  memory allowance: {} bytes (memory available to the process {}); shared policy for named input, grounding, solving and output capacities; not resident memory",
        options.memory,
        crate::options::host_memory()
            .map_or_else(|| "unreported".to_owned(), |bytes| bytes.to_string())
    )?;
    writeln!(
        sink,
        "  formula joins: requested={}; scope=completed_eager_support",
        match options.formula_joins {
            zetesis_themelios::JoinStrategy::Indexed => "indexed",
            zetesis_themelios::JoinStrategy::Table => "table",
        }
    )?;
    limits(sink, options, config)?;
    writeln!(
        sink,
        "  driver wall time: {:.3} ms (admission, search and output; excludes source loading and statistics)",
        elapsed.as_secs_f64() * 1000.0
    )?;
    Ok(())
}

pub(crate) fn write_progress(
    sink: &mut impl Write,
    options: &Options,
    result: Result<&crate::failure::Progress, &PublicationFailure>,
    elapsed: Duration,
) -> io::Result<()> {
    let progress = match result {
        Ok(progress) => progress,
        Err(failure) => return write_detailed(sink, options, Err(failure), elapsed),
    };
    let semantic = progress
        .semantic()
        .ok_or_else(|| io::Error::other("statistics require semantic progress"))?;
    let config = crate::SolveConfig::from(options);
    header(sink, options, &config, elapsed)?;
    if let Some(stop) = &progress.stop {
        writeln!(
            sink,
            "  publication: incomplete; {stop}; search completion={:?}",
            semantic.completion()
        )?;
    } else {
        writeln!(
            sink,
            "  completion: {}",
            match progress.completion().map_err(io::Error::other)? {
                Completion::Exhausted => "exhausted",
                Completion::RequestedModels => "requested models reached (partial coverage)",
                Completion::Interrupted => "interrupted (partial coverage)",
            }
        )?;
    }
    details(
        sink,
        &config,
        &Details {
            models: progress.publication.models,
            checked: semantic.candidate_progress(),
            optimum_proved: semantic.optimum_proved(),
            interruption: semantic.interruption(),
            countermodel_statistics: semantic.countermodel_statistics(),
            formula_execution: semantic.formula_execution(),
            hybrid_execution: semantic.hybrid_execution(),
            terminal_execution: semantic.terminal_execution(),
            source_evaluation: semantic.source_evaluation(),
            model_construction: semantic.model_construction(),
            optimization: semantic.incumbent(),
        },
    )
}

pub(crate) fn write_detailed(
    sink: &mut impl Write,
    options: &Options,
    result: Result<&Report, &PublicationFailure>,
    elapsed: Duration,
) -> io::Result<()> {
    let config = crate::SolveConfig::from(options);
    header(sink, options, &config, elapsed)?;
    match result {
        Ok(report) => completed(sink, &config, report),
        Err(failure) => {
            writeln!(sink, "  status: failed; completion=unavailable")?;
            if let Some(partial) = &failure.partial_report {
                writeln!(
                    sink,
                    "  partial execution: published models={}; verified models={}; search completion={:?}; summary published={}",
                    partial.published_models,
                    partial.verified_models,
                    partial.completion,
                    partial.summary_published,
                )?;
                details(sink, &config, &Details::from(partial.as_ref()))?;
            } else {
                writeln!(
                    sink,
                    "  effective execution: unavailable; counters=unavailable"
                )?;
            }
            writeln!(sink, "  failure: {}", failure.cause)
        }
    }
}

/// The public policy and the actual storage capacities derived from it.
fn limits(sink: &mut impl Write, o: &Options, c: &crate::SolveConfig) -> io::Result<()> {
    writeln!(
        sink,
        "  work policy: cooperative cancellation; no cumulative operation budget"
    )?;
    if let Some(seconds) = o.time_limit {
        writeln!(
            sink,
            "  requested process time limit: {seconds} s (cooperative; begins after input loading)"
        )?;
    }
    let resources = o.resources();
    let source = resources.bundle_limits();
    let formula = resources.formula_limits();
    writeln!(
        sink,
        "  configured storage: source bytes/file={}; source bytes/graph={}; support bytes={}; formula nodes={}; formula operands={}",
        source.max_file_bytes,
        source.max_total_bytes,
        formula.max_support_bytes,
        formula.theory.max_nodes,
        formula.theory.max_operands
    )?;
    writeln!(
        sink,
        "  configured execution storage: candidate bytes={}; reduct bytes/owner={}; completion bytes={}; batch bytes={}; tight batch bytes={}",
        c.max_candidate_bytes,
        c.max_reduct_bytes,
        c.max_completion_scratch_bytes,
        c.max_batch_bytes,
        c.max_tight_batch_bytes
    )?;
    writeln!(
        sink,
        "  configured publication storage: model bytes={}; incumbent bytes={}; observation bytes={}; JSON record bytes={}",
        c.max_model_bytes,
        c.max_optimal_bytes,
        resources.observation_limits().max_output_bytes,
        resources.json_record_bytes()
    )
}

fn completed(
    sink: &mut impl Write,
    config: &crate::SolveConfig,
    report: &Report,
) -> io::Result<()> {
    let status = match report.completion {
        Completion::Exhausted => "exhausted",
        Completion::RequestedModels => "requested models reached (partial coverage)",
        Completion::Interrupted => "interrupted (partial coverage)",
    };
    writeln!(sink, "  completion: {status}")?;
    details(sink, config, &Details::from(report))
}

/// Borrow the shared statistics fields without inventing a successful report.
/// An absent completion remains absent even when execution has retained work.
struct Details<'a> {
    models: usize,
    checked: u64,
    optimum_proved: bool,
    interruption: Option<crate::Interruption>,

    countermodel_statistics: Option<&'a zetesis_stable::Statistics>,
    formula_execution: Option<&'a crate::FormulaExecutionStatistics>,
    hybrid_execution: Option<&'a zetesis_solve::HybridExecutionStatistics>,
    terminal_execution: Option<&'a zetesis_solve::TerminalExecutionStatistics>,
    source_evaluation: Option<&'a zetesis_themelios::SourceEvaluationStatistics>,
    model_construction: Option<&'a zetesis_solve::ModelConstructionStatistics>,

    optimization: Option<&'a crate::Optimization>,
}

impl<'a> From<&'a Report> for Details<'a> {
    fn from(report: &'a Report) -> Self {
        Self {
            models: report.models,
            checked: report.checked,
            optimum_proved: report.optimum_proved,
            interruption: report.interruption,
            countermodel_statistics: report.countermodel_statistics.as_ref(),
            formula_execution: report.formula_execution.as_ref(),
            hybrid_execution: report.hybrid_execution.as_ref(),
            terminal_execution: report.terminal_execution.as_ref(),
            source_evaluation: report.source_evaluation.as_ref(),
            model_construction: report.model_construction.as_ref(),
            optimization: report.optimization.as_ref(),
        }
    }
}

impl<'a> From<&'a crate::PartialReport> for Details<'a> {
    fn from(report: &'a crate::PartialReport) -> Self {
        Self {
            models: report.published_models,
            checked: report.checked,
            optimum_proved: report.optimum_proved,
            interruption: report.interruption,
            countermodel_statistics: report.countermodel_statistics.as_ref(),
            formula_execution: report.formula_execution.as_ref(),
            hybrid_execution: report.hybrid_execution.as_ref(),
            terminal_execution: report.terminal_execution.as_ref(),
            source_evaluation: report.source_evaluation.as_ref(),
            model_construction: report.model_construction.as_ref(),
            optimization: report.optimization.as_ref(),
        }
    }
}

fn details(
    sink: &mut impl Write,
    config: &crate::SolveConfig,
    report: &Details<'_>,
) -> io::Result<()> {
    if let Some(stats) = report.hybrid_execution {
        hybrid(sink, config, stats)?;
    }
    if let Some(stats) = report.terminal_execution {
        terminal(sink, stats)?;
    }
    if let Some(stats) = report.model_construction {
        writeln!(
            sink,
            "  model construction: work={}; prepared bytes={}; peak bytes={}; constructed={}",
            stats.work, stats.prepared_bytes, stats.peak_bytes, stats.constructed
        )?;
    }
    let examined = if report.source_evaluation.is_some() {
        "completed source evaluations"
    } else {
        "candidates examined"
    };
    writeln!(
        sink,
        "  results: displayed models={}; {examined}={}",
        report.models, report.checked
    )?;
    if let Some(reason) = report.interruption {
        writeln!(sink, "  interruption: {reason}")?;
    }
    if let Some(stats) = report.source_evaluation {
        writeln!(
            sink,
            "  effective execution: backend=cpu; operation=direct source evaluation"
        )?;
        writeln!(
            sink,
            "  source evaluation: work={}; substitutions={}; scope=source_admission; includes support, arithmetic and exact activity",
            stats.work, stats.substitutions
        )?;
    } else if let Some(stats) = report.countermodel_statistics {
        formula(sink, config, report)?;
        countermodel(
            sink,
            config,
            stats,
            if report.terminal_execution.is_some() {
                "verified base models"
            } else if report.hybrid_execution.is_some() {
                "verified core models"
            } else {
                "verified stable models"
            },
        )?;
    } else {
        writeln!(
            sink,
            "  effective execution: unavailable (stopped before execution counters)"
        )?;
    }
    objective(sink, report)
}

fn objective(sink: &mut impl Write, report: &Details<'_>) -> io::Result<()> {
    if let Some(optimum) = report.optimization {
        let qualification = if report.optimum_proved {
            "optimal"
        } else {
            "incumbent only"
        };
        let ties = if optimum.ties_pruned {
            "unavailable".to_owned()
        } else {
            optimum.tied_models.to_string()
        };
        writeln!(
            sink,
            "  objective: {qualification}; costs(priority,value)={:?}; full ties={ties}; scored models={}; evaluation work={}",
            optimum.score.costs(),
            optimum.scored_models,
            optimum.work
        )?;
    } else {
        writeln!(
            sink,
            "  objective: no retained score; evaluation counters=unavailable"
        )?;
    }
    Ok(())
}

fn terminal(
    sink: &mut impl Write,
    stats: &zetesis_solve::TerminalExecutionStatistics,
) -> io::Result<()> {
    writeln!(
        sink,
        "  terminal definitions: {} base; full reconstruction before original membership; base answers={}; reconstructed={}; pending={}",
        match stats.base {
            zetesis_themelios::BaseKind::Eager => "eager",
            zetesis_themelios::BaseKind::Hybrid => "hybrid",
        },
        stats.base_answers,
        stats.reconstructed,
        stats.pending
    )?;
    writeln!(
        sink,
        "  answer reconstruction: attempts={}; completed={}; work={}; substitutions={}; source admission included; admission work={}; substitutions={}; per-answer allowance work={}; substitutions={}; latest answer work={}; substitutions={}; largest answer work={}; substitutions={}; base search work separate",
        stats.reconstruction.attempts,
        stats.reconstruction.completed,
        stats.reconstruction.work,
        stats.reconstruction.substitutions,
        stats.reconstruction.admission.work,
        stats.reconstruction.admission.substitutions,
        stats.reconstruction.allowance.work,
        stats.reconstruction.allowance.substitutions,
        stats.reconstruction.latest.work,
        stats.reconstruction.latest.substitutions,
        stats.reconstruction.peak.work,
        stats.reconstruction.peak.substitutions
    )
}

fn hybrid(
    sink: &mut impl Write,
    config: &crate::SolveConfig,
    stats: &zetesis_solve::HybridExecutionStatistics,
) -> io::Result<()> {
    writeln!(
        sink,
        "  hybrid grounding: eager retained core; streamed source constraints: templates={}, instances={}; grounded in the core: templates={}, instances={}; core answers={}; accepted={}; rejected={}; pending={}",
        stats.streamed.templates,
        stats.streamed.instances,
        stats.streamed.retained_templates,
        stats.streamed.retained_instances,
        stats.core_answers,
        stats.accepted,
        stats.rejected,
        stats.pending
    )?;
    writeln!(
        sink,
        "  constraint checks: work={}; substitutions={}; scalar payload bytes={}; each check at most work={}, substitutions={}, scalar payload bytes={}; independent of admission and reduct work",
        stats.constraints.work,
        stats.constraints.substitutions,
        stats.constraints.scalar_bytes,
        config.constraints.max_work,
        config.constraints.max_substitutions,
        config.constraints.max_scalar_bytes
    )
}

fn countermodel(
    sink: &mut impl Write,
    config: &crate::SolveConfig,
    stats: &zetesis_stable::Statistics,
    membership: &str,
) -> io::Result<()> {
    if let Some(filter) = stats.region_filter {
        writeln!(
            sink,
            "  candidate region checks: preparations={}; checks={}; refuted={}; failed={}; overflowed={}; source work accounted separately; frozen reduct excluded",
            filter.preparations, filter.checks, filter.refuted, filter.failed, filter.overflowed
        )?;
    }
    if let Some(regions) = stats.regions.as_ref() {
        candidate_regions(sink, regions)?;
    }
    writeln!(
        sink,
        "  countermodel: search work={}; decisions={}; candidates={}; queries={}; witnesses={}; candidate restrictions={}; {membership}={}",
        stats.search.work,
        stats.search.decisions,
        stats.candidates,
        stats.countermodel_queries,
        stats.countermodels,
        stats.candidate_restrictions,
        stats.stable_models
    )?;
    if stats.reduct.frozen_positive_attempts > 0 {
        writeln!(
            sink,
            "  frozen positive reduct: attempts={}; completed={}; preparation work={}; work={} (included in search work)",
            stats.reduct.frozen_positive_attempts,
            stats.reduct.frozen_positive_completed,
            stats.reduct.frozen_positive_preparation_work,
            stats.reduct.frozen_positive_work,
        )?;
    }
    if stats.reduct.regions.regions > 0 || stats.reduct.regions.work > 0 {
        let regions = stats.reduct.regions;
        writeln!(
            sink,
            "  reduct query regions: visited={}; refuted={}; leaves={}; propagations={}; reading work={} (included in search work)",
            regions.regions, regions.refuted, regions.leaves, regions.propagations, regions.work,
        )?;
    }
    if let Some(certified) = stats.certified {
        certificate(sink, &certified, config.max_completion_scratch_bytes)?;
    }
    Ok(())
}

fn candidate_regions(
    sink: &mut impl Write,
    regions: &zetesis_stable::RegionSearchStatistics,
) -> io::Result<()> {
    let counts = regions.counts;
    writeln!(
        sink,
        "  candidate regions: visited={}; refuted={}; leaves={}; propagations={}; held={}; cut={}; support cut={}; reading work={} (included in search work)",
        counts.regions,
        counts.refuted,
        counts.leaves,
        counts.propagations,
        counts.held,
        counts.cut,
        if regions.producers {
            "applied"
        } else {
            "not applicable"
        },
        counts.work,
    )?;
    let generator = regions.generator_order;
    writeln!(
        sink,
        "  candidate generator order: status={}; preparation work={}; peak bytes={}; retained bytes={} (work included in reading work)",
        generator.status.label(),
        generator.work,
        generator.peak_bytes,
        generator.retained_bytes
    )?;
    if let Some(workers) = regions.workers {
        region_workers(sink, workers)?;
    }
    let parity = regions.parity;
    writeln!(
        sink,
        "  candidate parity: status={}; rows={}; rank={}; root refuted={}; units={}; region passes={}; forced={}; refuted={}; preparation work={}; preparation peak bytes={} (work included in reading work)",
        parity.status.label(),
        parity.rows,
        parity.rank,
        parity.root_refuted,
        parity.units,
        parity.region_passes,
        parity.forced,
        parity.refuted,
        parity.preparation_work,
        parity.preparation_bytes,
    )?;
    let cardinality = regions.cardinality;
    writeln!(
        sink,
        "  candidate cardinality: status={}; rows={}; families={}; root refuted={}; region passes={}; refuted={}; preparation work={}; preparation peak bytes={} (work included in reading work)",
        cardinality.status.label(),
        cardinality.rows,
        cardinality.families,
        cardinality.root_refuted,
        cardinality.region_passes,
        cardinality.refuted,
        cardinality.preparation_work,
        cardinality.preparation_bytes,
    )?;
    writeln!(
        sink,
        "  candidate support: producer work={}; possible-foundedness work={} (included in reading work)",
        counts.support_work, counts.founded_work,
    )?;
    let explanations = &counts.support_explanations;
    writeln!(
        sink,
        "  support explanations: attempts={}; certified={}; matched={}; cut={}; forced={}; refuted={}; declined={}; work={} (included in reading work); peak worker bytes={}",
        explanations.attempts,
        explanations.certified,
        explanations.matched,
        explanations.cut,
        explanations.forced,
        explanations.refuted,
        explanations.declined,
        explanations.work,
        explanations.peak_bytes,
    )?;
    Ok(())
}

/// The parallel walk's engagement record, one optional value per key: each
/// state's values are absent outside that state.
#[derive(Default)]
pub(crate) struct WorkerRecord {
    pub(crate) elapsed: Option<Duration>,
    pub(crate) walking: Option<Duration>,
    pub(crate) after: Option<Duration>,
    pub(crate) launched: Option<usize>,
    pub(crate) spawn: Option<Duration>,
}

impl From<zetesis_stable::RegionWalkEngagement> for WorkerRecord {
    fn from(engagement: zetesis_stable::RegionWalkEngagement) -> Self {
        use zetesis_stable::RegionWalkEngagement as State;
        match engagement {
            State::Unengaged { elapsed, walking } => Self {
                elapsed: Some(elapsed),
                walking: Some(walking),
                ..Self::default()
            },
            State::Engaged {
                after,
                walking,
                launched,
                spawn,
            } => Self {
                walking: Some(walking),
                after: Some(after),
                launched: Some(launched),
                spawn: Some(spawn),
                ..Self::default()
            },
            // A walk not begun has no values, nor has a state this build
            // does not name.
            _ => Self::default(),
        }
    }
}

/// The parallel walk's workers, under its candidate regions, with the JSON's
/// keys: `engagement_delay_ns=never` for a delay whose deadline overflows. The
/// queries the workers divided follow on their own line.
fn region_workers(
    sink: &mut impl Write,
    workers: zetesis_stable::RegionWorkerStatistics,
) -> io::Result<()> {
    let record = WorkerRecord::from(workers.engagement);
    write!(
        sink,
        "    workers: configured={}; engagement_delay_ns=",
        workers.configured
    )?;
    match workers.engagement_delay {
        Some(delay) => write!(sink, "{}", delay.as_nanos())?,
        None => write!(sink, "never")?,
    }
    write!(
        sink,
        "; engagement={}",
        workers.engagement.label().replace('_', " ")
    )?;
    if let Some(elapsed) = record.elapsed {
        write!(sink, "; elapsed_ns={}", elapsed.as_nanos())?;
    }
    if let Some(after) = record.after {
        write!(sink, "; engaged_after_ns={}", after.as_nanos())?;
    }
    if let Some(walking) = record.walking {
        write!(sink, "; walking_ns={}", walking.as_nanos())?;
    }
    if let Some(launched) = record.launched {
        write!(sink, "; launched={launched}")?;
    }
    if let Some(spawn) = record.spawn {
        write!(sink, "; spawn_ns={}", spawn.as_nanos())?;
    }
    writeln!(sink)?;
    let division = workers.query_division;
    writeln!(
        sink,
        "    query division: division={}; published={}; served={}; stolen={}; servers={}; peak served={}",
        division.division.label().replace('_', " "),
        division.published,
        division.served,
        division.stolen,
        division.servers,
        division.peak_served,
    )
}

fn certificate(
    sink: &mut impl Write,
    certified: &zetesis_stable::CertifiedStatistics,
    max_bytes: u64,
) -> io::Result<()> {
    writeln!(
        sink,
        "  class certificate: eligible={}; refusal={:?}; storage limit={}; construction work={}; checks={}; stable decisions before commit={}; refuted by support={}; failed={}; checking work={}",
        certified.plan.is_some(),
        certified.refusal,
        max_bytes,
        certified.construction_work,
        certified.checks,
        certified.stable,
        certified.refuted,
        certified.failed,
        certified.checking_work
    )?;
    if let Some(plan) = certified.plan {
        match plan {
            zetesis_stable::CertificatePlanStatistics::Tight(plan) => writeln!(
                sink,
                "  tight certificate storage: construction logical bytes={}; resident logical bytes={}; dependencies={}",
                plan.construction_bytes, plan.resident_bytes, plan.dependencies
            )?,
            zetesis_stable::CertificatePlanStatistics::Positive(plan) => writeln!(
                sink,
                "  positive certificate: derived atoms={}; activated formula nodes={}; dependencies={}; propagated dependencies={}; construction peak bytes={}; retained bytes={}",
                plan.derived_atoms,
                plan.activated_nodes,
                plan.dependencies,
                plan.propagated_dependencies,
                plan.peak_bytes,
                plan.retained_bytes
            )?,
            zetesis_stable::CertificatePlanStatistics::Stratified(plan) => writeln!(
                sink,
                "  stratified certificate: components={}; derived atoms={}; activated formula nodes={}; dependencies={}; propagated dependencies={}; construction peak bytes={}; retained bytes={}",
                plan.components,
                plan.derived_atoms,
                plan.activated_nodes,
                plan.dependencies,
                plan.propagated_dependencies,
                plan.peak_bytes,
                plan.retained_bytes
            )?,
        }
    }
    writeln!(
        sink,
        "  class attempts: tight refusal={:?}; positive refusal={:?}; stratified refusal={:?}",
        certified.tight_refusal, certified.positive_refusal, certified.stratified_refusal
    )?;
    if let Some(peak) = certified.tight_check_peak_bytes {
        writeln!(
            sink,
            "  tight checking: peak plan and evaluation bytes={peak}"
        )?;
    }
    if let Some(peak) = certified.positive_check_peak_bytes {
        writeln!(
            sink,
            "  positive checking: peak plan and evaluation bytes={peak}"
        )?;
    }
    if !matches!(
        certified.plan,
        Some(zetesis_stable::CertificatePlanStatistics::Positive(_))
    ) && let Some(attempt) = certified.positive_attempt
    {
        writeln!(
            sink,
            "  positive preparation attempt: work={}; observed peak bytes={}; primitive retained bytes={}; positive plan not selected",
            attempt.work, attempt.peak_bytes, attempt.retained_bytes
        )?;
    }
    if let Some(peak) = certified.stratified_check_peak_bytes {
        writeln!(
            sink,
            "  stratified checking: peak plan and evaluation bytes={peak}"
        )?;
    }
    if !matches!(
        certified.plan,
        Some(zetesis_stable::CertificatePlanStatistics::Stratified(_))
    ) && let Some(attempt) = certified.stratified_attempt
    {
        writeln!(
            sink,
            "  stratified preparation attempt: work={}; observed peak bytes={}; primitive retained bytes={}; stratified plan not selected",
            attempt.work, attempt.peak_bytes, attempt.retained_bytes
        )?;
    }
    Ok(())
}

fn formula(
    sink: &mut impl Write,
    config: &crate::SolveConfig,
    report: &Details<'_>,
) -> io::Result<()> {
    let grounder = if let Some(terminal) = report.terminal_execution {
        match terminal.base {
            zetesis_themelios::BaseKind::Eager => "eager_base_terminal_definitions",
            zetesis_themelios::BaseKind::Hybrid => "hybrid_base_terminal_definitions",
        }
    } else if report.hybrid_execution.is_some() {
        "hybrid"
    } else {
        "eager"
    };
    let oracle = match report
        .countermodel_statistics
        .and_then(|s| s.certified)
        .and_then(|s| s.plan)
    {
        Some(zetesis_stable::CertificatePlanStatistics::Tight(_)) => "tight-support",
        Some(zetesis_stable::CertificatePlanStatistics::Positive(_)) => "positive-consequences",
        Some(zetesis_stable::CertificatePlanStatistics::Stratified(_)) => "stratified-consequences",
        None => "countermodel",
    };
    if let Some(execution) = report.formula_execution {
        let backend = if execution.adapter.is_empty() {
            "cpu batched exact completion"
        } else if execution.tight_work_per_candidate.is_some() {
            "GPU tight support"
        } else {
            "hybrid GPU propagation + exact CPU residual search"
        };
        writeln!(
            sink,
            "  effective execution: backend={backend}; oracle={oracle}; grounder={grounder}; CPU completion requested workers={}; peak preflight workers={}; adapter={}",
            config.completion_workers, execution.completion.effective_workers, execution.adapter
        )?;
        writeln!(
            sink,
            "  formula completion: entered={}; residuals entered={}; completed before commit={}; failed={}; peak requested scratch bytes={}; peak shared owner, query and transient/result scratch bytes={}; scratch limit={}; counters overflowed={}",
            execution.completion.entered,
            execution.completion.residuals,
            execution.completion.completed,
            execution.completion.failed,
            execution.completion.requested_scratch_bytes,
            execution.completion.peak_scratch_bytes,
            config.max_completion_scratch_bytes,
            execution.completion.overflowed
        )?;
        writeln!(
            sink,
            "  formula residual completion: completed locally={}; failed={}",
            execution.completion.residual_completed, execution.completion.residual_failed
        )?;
        for (label, measurement) in [
            ("completion coordinator wall", execution.completion.wall),
            (
                "summed worker original validation",
                execution.completion.worker_original,
            ),
            ("summed worker reduct", execution.completion.worker_reduct),
        ] {
            if let Some(measurement) = measurement {
                writeln!(
                    sink,
                    "  {label}: calls={}; elapsed ns={}; overflowed={}",
                    measurement.calls,
                    measurement.elapsed.as_nanos(),
                    measurement.overflowed
                )?;
            }
        }
        writeln!(
            sink,
            "  formula accounting: pending candidates={}; queued verified models={}",
            execution.pending_candidates, execution.queued_models
        )?;
        if config.gpu_batching == zetesis_solve::GpuBatching::Adaptive
            && !execution.adapter.is_empty()
        {
            // Adaptive GPU slates have no ceiling; the batch size is where
            // they start and where bound feedback returns them.
            writeln!(
                sink,
                "  formula batch limits: adaptive slates; initial and reset slate size={}; pending bytes={}",
                config.batch_size, config.max_batch_bytes
            )?;
        } else {
            writeln!(
                sink,
                "  formula batch limits: candidates={}; pending bytes={}",
                config.batch_size, config.max_batch_bytes
            )?;
        }
        if !execution.adapter.is_empty() {
            formula_gpu(sink, execution)?;
        }
        Ok(())
    } else {
        writeln!(
            sink,
            "  effective execution: backend=cpu; oracle={oracle}; grounder={grounder}; native CPU search; batch-completion scratch limit=inapplicable"
        )
    }
}

/// The slates a GPU enumeration proposed ahead, and its time to first answer.
fn formula_gpu_lookahead(
    sink: &mut impl Write,
    execution: &crate::FormulaExecutionStatistics,
) -> io::Result<()> {
    if let Some(decision) = execution.gpu_lookahead {
        writeln!(
            sink,
            "  formula GPU lookahead: slates proposed ahead={}; last decision={}; overlapped device compute={}ns; finish waits={}ns; unmeasured submissions={}; device time hidden behind production is the compute less the waits, paired over the measured submissions",
            execution.gpu_lookahead_slates,
            decision.label(),
            execution.gpu_lookahead_device_ns,
            execution.gpu_lookahead_finish_wait_ns,
            execution.gpu_lookahead_unmeasured,
        )?;
    }
    if let Some(first) = execution.first_answer_ns {
        writeln!(
            sink,
            "  formula GPU first answer: {first}ns after the device execution began; reported, not judged"
        )?;
    }
    Ok(())
}

fn formula_gpu(
    sink: &mut impl Write,
    execution: &crate::FormulaExecutionStatistics,
) -> io::Result<()> {
    writeln!(
        sink,
        "  formula GPU decoded: batches={}; candidates={}; primitive work={}; completed sweeps={}; GPU-decided committed={}; CPU residuals completed={}",
        execution.gpu_batches,
        execution.gpu_candidates,
        execution.gpu_work,
        execution.gpu_rounds,
        execution.gpu_decided,
        execution.cpu_residuals
    )?;
    writeln!(
        sink,
        "  formula GPU submitted: batches={}; candidates={}; completed work/sweeps for unreturned results=unavailable",
        execution.gpu_submitted_batches, execution.gpu_submitted_candidates
    )?;
    writeln!(
        sink,
        "  formula GPU slates: committed={}; largest={}; feedback resets={}; each slate takes one or more submitted batches",
        execution.gpu_slates,
        execution
            .gpu_largest_slate
            .map_or_else(|| "none".to_owned(), |largest| largest.to_string()),
        execution.gpu_slate_resets
    )?;
    formula_gpu_lookahead(sink, execution)?;
    if let Some(residuals) = execution.gpu_residuals {
        writeln!(
            sink,
            "  formula GPU decoded residuals: fixed point={}; round limit={}; work limit={}; precede CPU completion and commit",
            residuals.fixed_point, residuals.round_limit, residuals.work_limit,
        )?;
    }
    if let Some(work) = execution.tight_work_per_candidate {
        writeln!(
            sink,
            "  tight GPU limits: work/candidate={work}; propagation sweeps=not applicable"
        )?;
    }
    if let Some(work) = execution.gpu_scheduled_work {
        writeln!(
            sink,
            "  tight GPU scheduled work={work}; performed by decoded batches, reserved by submitted unreturned batches"
        )?;
    }
    if let Some(routes) = execution.gpu_tight_routes {
        writeln!(
            sink,
            "  tight GPU routes: complete={}; dense reuse={} (beyond update threshold={}); update={}; nodes evaluated by update={}",
            routes.complete,
            routes.dense_reuse,
            routes.beyond_threshold,
            routes.update,
            routes.nodes_evaluated
        )?;
    }
    if let Some(updates) = execution.gpu_tight_updates {
        writeln!(
            sink,
            "  tight GPU update route: {}; count width={}; steady state={}; guard declined batches={}",
            updates.availability.label(),
            updates
                .count_bits
                .map_or_else(|| "unavailable".to_owned(), |bits| bits.to_string()),
            updates.steady_state,
            updates.guard_declined_batches
        )?;
    }
    if let Some(limits) = execution.gpu_limits {
        writeln!(
            sink,
            "  formula GPU limits: propagation sweeps/candidate={}; propagation work/candidate={}",
            limits.rounds_per_candidate, limits.work_per_candidate
        )?;
    }
    writeln!(
        sink,
        "  formula GPU resources: peak authored GPU bytes={}; workgroup size={}",
        execution.peak_accounted_bytes,
        execution
            .gpu_workgroup_size
            .map_or_else(|| "unavailable".to_owned(), |size| size.to_string())
    )?;
    let timing = &execution.gpu_compute_timing;
    writeln!(
        sink,
        "  GPU compute passes: status={}; elapsed_ns={}; measured batches={}; unavailable batches={}; unresolved batches={}; flush submissions={}; complete={}; host preparation, transfers and waits excluded",
        timing.status.label(),
        timing.elapsed.map_or_else(
            || "unavailable".to_owned(),
            |duration| duration.as_nanos().to_string()
        ),
        timing.measured_batches,
        timing.unavailable_batches,
        timing.unresolved_batches,
        timing.flush_submissions,
        timing.is_complete(),
    )
}

/// The executable doing this run's work, resolved through links, for evidence
/// that names the binary behind a measurement: after the default install's
/// front hands a GPU request to its sibling, the sibling. None when the host
/// does not report it. The run resolves it once, at its first use, so every
/// statistics view of the run names the same executable.
pub(crate) fn running_executable() -> Option<&'static std::path::Path> {
    static EXECUTABLE: std::sync::OnceLock<Option<std::path::PathBuf>> = std::sync::OnceLock::new();
    EXECUTABLE
        .get_or_init(|| crate::process::this_executable().ok())
        .as_deref()
}

#[cfg(test)]
mod tests;
