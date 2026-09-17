//! Opt-in driver measurements; unavailable execution counters are never inferred.

use std::io::{self, Write};
use std::time::Duration;

use crate::{Backend, Completion, Grounder, Options, PublicationFailure, Report};

fn header(sink: &mut impl Write, options: &Options, elapsed: Duration) -> io::Result<()> {
    writeln!(
        sink,
        "Statistics: zetesis {}; GPU compiled={}",
        env!("CARGO_PKG_VERSION"),
        cfg!(feature = "gpu")
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
        options.workers, options.batch_size, options.models
    )?;
    writeln!(
        sink,
        "  formula joins: requested={}; scope=completed_eager_support",
        match options.formula_joins {
            zetesis_themelios::JoinStrategy::Indexed => "indexed",
            zetesis_themelios::JoinStrategy::Table => "table",
        }
    )?;
    limits(sink, options)?;
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
    header(sink, options, elapsed)?;
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
        options,
        &Details {
            models: progress.publication.models,
            checked: semantic.candidate_progress(),
            optimum_proved: semantic.optimum_proved(),
            interruption: semantic.interruption(),
            discovered_gate_atoms: semantic.discovered_gate_atoms(),
            expansion: progress.expansion,
            candidate_statistics: semantic.candidate_statistics(),
            countermodel_statistics: semantic.countermodel_statistics(),
            formula_execution: semantic.formula_execution(),
            lazy_execution: semantic.lazy_execution(),
            shared_execution: semantic.shared_execution(),
            closure_execution: semantic.closure_execution(),
            query_execution: semantic.query_execution(),
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
    header(sink, options, elapsed)?;
    match result {
        Ok(report) => completed(sink, options, report),
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
                details(sink, options, &Details::from(partial.as_ref()))?;
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

fn closure_limits(sink: &mut impl Write, o: &Options) -> io::Result<()> {
    let share = if o.max_closure_bytes.is_none() {
        format!(" (collective share of {} workers)", o.workers)
    } else {
        String::new()
    };
    writeln!(
        sink,
        "  independent CPU closure limits: named bytes/owner={}{share}; preparation/cache/collective reservation bytes={}; query preparation work={}; returned models and allocator overhead excluded",
        o.closure_allowance(),
        o.max_closure_batch_bytes,
        o.max_source_work
    )
}

fn limits(sink: &mut impl Write, o: &Options) -> io::Result<()> {
    if let Some(seconds) = o.time_limit {
        writeln!(
            sink,
            "  requested process time limit: {seconds} s (cooperative; begins after input loading)"
        )?;
    }
    if o.source_batching != crate::SourceBatching::Independent {
        writeln!(
            sink,
            "  shared CPU limits: source work/batch={}; record visits plus antecedent tests/world={}; collective catalog atoms={}; host payload bytes={}",
            o.max_source_work, o.max_work, o.max_atoms, o.max_batch_bytes
        )?;
    }
    writeln!(
        sink,
        "  search limits: candidates={}; candidate restriction/formula work={}; decisions={}; CPU candidate/lazy GPU source batch/formula verification call work={}",
        o.max_candidates, o.max_search_work, o.max_search_decisions, o.max_work
    )?;
    writeln!(
        sink,
        "  candidate restriction limits: copied payload bytes={}; atom occurrences={}; allocator/index overhead excluded",
        o.max_candidate_bytes, o.max_atoms
    )?;
    writeln!(
        sink,
        "  projection history limits: entries={}; nodes={}; named capacity/overlap bytes={}; work shares the search allowance",
        o.max_projection_entries, o.max_projection_nodes, o.max_projection_bytes
    )?;
    writeln!(
        sink,
        "  prepared reduct limits: cold preparation/each query bytes={}; collective owner/worker/result bytes={}; theory and allocator metadata excluded",
        o.max_reduct_bytes, o.max_completion_scratch_bytes
    )?;
    closure_limits(sink, o)?;
    writeln!(
        sink,
        "  requested grounding limits: atoms={}; carrier atoms={}; substitutions={}; ground rules={}; GPU batch bytes={}",
        o.max_atoms,
        o.max_carrier_atoms,
        o.max_substitutions,
        o.max_ground_rules,
        o.max_batch_bytes
    )?;
    let formula = crate::admission::formula_limits(o);
    writeln!(
        sink,
        "  formula profile ceilings: atoms={}; roots={}; nodes={}; source values={}; assignment values/operation={}; generated binding values={}; support rounds={}; work={} (applicable when formula admission is selected)",
        formula.theory.max_atoms,
        formula.theory.max_roots,
        formula.theory.max_nodes,
        formula.max_domain_values,
        formula.max_assignment_values,
        formula.max_generated_values,
        formula.max_support_rounds,
        formula.max_work
    )?;
    writeln!(
        sink,
        "  source limits: bytes/file={}; roots={}; files={}; total bytes={}; include depth={}",
        o.max_source_bytes,
        o.max_source_roots,
        o.max_source_files,
        o.max_total_source_bytes,
        o.max_include_depth
    )?;
    writeln!(
        sink,
        "  expansion limits: work={}; templates={}; values={}; scalar bytes={}; eager support bytes={}",
        crate::admission::expansion_limits(o).max_term_work,
        o.max_expanded_templates,
        o.max_expansion_values,
        o.max_expansion_bytes,
        o.max_support_bytes
    )?;
    writeln!(
        sink,
        "  objective limits: work={}; bound work={}; bindings/model={}; keys/model={}; key bytes/model={}",
        o.max_objective_work,
        o.max_objective_bound_work,
        o.max_objective_bindings,
        o.max_objective_keys,
        o.max_objective_key_bytes
    )?;
    writeln!(
        sink,
        "  incumbent limits: models={}; atoms={}; bytes={}",
        o.max_optimal_models, o.max_optimal_atoms, o.max_optimal_bytes
    )?;
    writeln!(
        sink,
        "  observation limits/model: work={}; bindings={}; terms={}; bytes={}",
        o.max_observation_work,
        o.max_observation_bindings,
        o.max_observation_terms,
        o.max_observation_bytes
    )
}

fn completed(sink: &mut impl Write, options: &Options, report: &Report) -> io::Result<()> {
    let status = match report.completion {
        Completion::Exhausted => "exhausted",
        Completion::RequestedModels => "requested models reached (partial coverage)",
        Completion::Interrupted => "interrupted (partial coverage)",
    };
    writeln!(sink, "  completion: {status}")?;
    details(sink, options, &Details::from(report))
}

/// Borrow the shared statistics fields without inventing a successful report.
/// An absent completion remains absent even when execution has retained work.
struct Details<'a> {
    models: usize,
    checked: u64,
    optimum_proved: bool,
    interruption: Option<crate::Interruption>,
    discovered_gate_atoms: usize,
    expansion: Option<zetesis_themelios::ExpansionUsage>,
    candidate_statistics: Option<zetesis_cpu::CandidateStatistics>,
    countermodel_statistics: Option<&'a zetesis_sat::Statistics>,
    formula_execution: Option<&'a crate::FormulaExecutionStatistics>,
    lazy_execution: Option<&'a crate::LazyExecutionStatistics>,
    shared_execution: Option<&'a crate::SharedExecutionStatistics>,
    closure_execution: Option<&'a crate::ClosureExecutionStatistics>,
    query_execution: Option<&'a crate::QueryExecutionObservation>,
    optimization: Option<&'a crate::Optimization>,
}

impl<'a> From<&'a Report> for Details<'a> {
    fn from(report: &'a Report) -> Self {
        Self {
            models: report.models,
            checked: report.checked,
            optimum_proved: report.optimum_proved,
            interruption: report.interruption,
            discovered_gate_atoms: report.discovered_gate_atoms,
            expansion: report.expansion,
            candidate_statistics: report.candidate_statistics,
            countermodel_statistics: report.countermodel_statistics.as_ref(),
            formula_execution: report.formula_execution.as_ref(),
            lazy_execution: report.lazy_execution.as_ref(),
            shared_execution: report.shared_execution.as_ref(),
            closure_execution: report.closure_execution.as_ref(),
            query_execution: report.query_execution.as_ref(),
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
            discovered_gate_atoms: report.discovered_gate_atoms,
            expansion: report.expansion,
            candidate_statistics: report.candidate_statistics,
            countermodel_statistics: report.countermodel_statistics.as_ref(),
            formula_execution: report.formula_execution.as_ref(),
            lazy_execution: report.lazy_execution.as_ref(),
            shared_execution: report.shared_execution.as_ref(),
            closure_execution: report.closure_execution.as_ref(),
            query_execution: report.query_execution.as_ref(),
            optimization: report.optimization.as_ref(),
        }
    }
}

fn details(sink: &mut impl Write, options: &Options, report: &Details<'_>) -> io::Result<()> {
    if let Some(usage) = report.expansion {
        let limits = crate::admission::expansion_limits(options);
        writeln!(
            sink,
            "  expansion used: term work={} of {}; templates={} of {}; values={} of {}; scalar bytes={} of {}; origins={} of {}",
            usage.term_work,
            limits.max_term_work,
            usage.templates,
            limits.max_templates,
            usage.values,
            limits.max_values,
            usage.scalar_bytes,
            limits.max_scalar_bytes,
            usage.origin_locations,
            limits.max_origin_locations
        )?;
    }
    if let Some(stats) = report.candidate_statistics {
        candidates(sink, stats)?;
    }
    if let Some(observation) = report.query_execution {
        query(sink, observation)?;
    }
    if let Some(stats) = report.shared_execution {
        shared(sink, stats)?;
    }
    if let Some(stats) = report.lazy_execution {
        lazy(sink, stats)?;
    }
    let examined = if report.shared_execution.is_some() {
        "closure result/control records examined"
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
    if let Some(stats) = report.countermodel_statistics {
        formula(sink, options, report)?;
        countermodel(sink, options, stats)?;
        writeln!(
            sink,
            "  discovered gate tuples: inapplicable (complete semantic candidates)"
        )?;
    } else if report.checked > 0 {
        closure(sink, options, report)?;
    } else {
        writeln!(
            sink,
            "  effective execution: unavailable (stopped before execution counters)"
        )?;
        writeln!(
            sink,
            "  oracle work: unavailable; discovered gate tuples: {}",
            report.discovered_gate_atoms
        )?;
    }
    if let Some(optimum) = report.optimization {
        let qualification = if report.optimum_proved {
            "optimal"
        } else {
            "incumbent only"
        };
        writeln!(
            sink,
            "  objective: {qualification}; costs(priority,value)={:?}; full ties={}; scored models={}; evaluation work={}",
            optimum.score.costs(),
            optimum.tied_models,
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

fn candidates(sink: &mut impl Write, stats: zetesis_cpu::CandidateStatistics) -> io::Result<()> {
    writeln!(
        sink,
        "  candidate restrictions: work={}; conjunctions={}; skipped impossible intervals={}; prepared atom occurrences={}; copied payload bytes={}; peak copied payload bytes={}; allocator/index overhead excluded",
        stats.restriction_work,
        stats.restriction_conjunctions,
        stats.conflicts,
        stats.restriction_atoms,
        stats.restriction_bytes,
        stats.restriction_peak_bytes
    )?;
    match stats.bounds_stop {
        None => writeln!(
            sink,
            "  carrier bounds: underivable gate atoms={}; necessary gate atoms={}",
            stats.underivable_gate_atoms, stats.necessary_gate_atoms
        ),
        Some(stop) => writeln!(
            sink,
            "  carrier bounds: unavailable ({stop}); the counter ran over the symbolic gate carrier"
        ),
    }
}

fn query(sink: &mut impl Write, observation: &crate::QueryExecutionObservation) -> io::Result<()> {
    if observation.fault.is_some() && observation.statistics.is_some() {
        writeln!(
            sink,
            "  query statistics: retaining last successful snapshot; current snapshot failed"
        )?;
    }
    if let Some(stats) = observation.statistics {
        writeln!(
            sink,
            "  prepared CPU queries: builds={}; retained workspaces={}; active ranges={}; reused slots={}; retained bytes={}; reserved envelope bytes={}",
            stats.preparation_builds,
            stats.retained_workspaces,
            stats.active_workspaces,
            stats.reused_workspaces,
            stats.retained_bytes,
            stats.reserved_bytes
        )?;
        if let Some(preparation) = stats.preparation {
            writeln!(
                sink,
                "  query preparation: work={}; retained bytes={}; separate from candidate work; capacities are not RSS",
                preparation.work, preparation.retained_bytes
            )?;
        }
    } else {
        writeln!(
            sink,
            "  prepared CPU queries: no successful ownership snapshot"
        )?;
    }
    if let Some(fault) = &observation.fault {
        writeln!(sink, "  query observation fault: {fault}")?;
    }
    Ok(())
}

fn shared(sink: &mut impl Write, stats: &crate::SharedExecutionStatistics) -> io::Result<()> {
    writeln!(
        sink,
        "  shared CPU: source={:?}; workers={}; batches={}; submitted={}; completed={}; stopped={}; queued={}",
        stats.selection,
        stats.workers,
        stats.batches,
        stats.submitted_candidates,
        stats.completed_candidates,
        stats.stopped_candidates,
        stats.queued_results
    )?;
    writeln!(
        sink,
        "  shared source: rounds={}; work={}; instances={}; peak catalog atoms={}; mask words={}; pruned prefixes={}; peak mask payload bytes={}",
        stats.source_rounds,
        stats.source_work,
        stats.source_instances,
        stats.peak_catalog_atoms,
        stats.mask_words,
        stats.pruned_prefixes,
        stats.peak_mask_bytes
    )?;
    writeln!(
        sink,
        "  CPU world evaluation: record visits plus antecedent tests={}; record visits={}",
        stats.world_work, stats.world_instances
    )?;
    if let Some(cause) = stats.last_stop {
        writeln!(sink, "  shared batch interruption: {cause}")?;
    }
    Ok(())
}

fn lazy(sink: &mut impl Write, stats: &crate::LazyExecutionStatistics) -> io::Result<()> {
    writeln!(
        sink,
        "  lazy device: requested={}; observed={}; adapter={}",
        stats.requested_backend.label(),
        stats.backend,
        stats.adapter
    )?;
    writeln!(
        sink,
        "  lazy candidates: submitted={}; completed={}; stopped={}; queued results={}",
        stats.submitted_candidates,
        stats.completed_candidates,
        stats.stopped_candidates,
        stats.queued_results
    )?;
    writeln!(
        sink,
        "  lazy source: batches={}; completed rounds={}; shared work={}; offered instances={}; peak catalog atoms={}",
        stats.batches,
        stats.source_rounds,
        stats.source_work,
        stats.source_instances,
        stats.peak_catalog_atoms
    )?;
    writeln!(
        sink,
        "  lazy GPU: dispatches={}; instance/world checks={}; uploaded bytes={}; decoded bytes={}; host wait={:.3} ms (kernel time unmeasured)",
        stats.dispatches,
        stats.world_instances,
        stats.uploaded_bytes,
        stats.downloaded_bytes,
        stats.host_wait.as_secs_f64() * 1000.0
    )?;
    writeln!(
        sink,
        "  lazy transport: chunks allocating buffers={}; complete-set reuses={}; peak requested device bytes={} (RSS unmeasured)",
        stats.transport_allocations, stats.transport_reuses, stats.peak_transport_bytes
    )?;
    let reasons = stats.transport_replacements;
    writeln!(
        sink,
        "  lazy replacements (overlapping): initial={}; offsets growth={}; records growth={}; snapshots growth={}; seeds growth={}; result shape={}; budget={}; accounting overflow={}",
        reasons.initial,
        reasons.offsets_growth,
        reasons.records_growth,
        reasons.snapshots_growth,
        reasons.seeds_growth,
        reasons.result_shape,
        reasons.budget,
        reasons.accounting_overflow
    )?;
    lazy_buffer_usage(sink, stats.transport_usage)
}

fn lazy_buffer_usage(sink: &mut impl Write, usage: crate::LazyTransportUsage) -> io::Result<()> {
    write!(sink, "  lazy buffer requests (allocated/reused):")?;
    for (name, binding) in [
        ("uniform", usage.uniform),
        ("offsets", usage.offsets),
        ("records", usage.records),
        ("snapshots", usage.snapshots),
        ("seeds", usage.seeds),
        ("output", usage.output),
        ("readback", usage.readback),
    ] {
        write!(sink, " {name}={}/{};", binding.allocations, binding.reuses)?;
    }
    writeln!(
        sink,
        " slack releases: budget={}; accounting overflow={}",
        usage.budget_releases, usage.accounting_overflow_releases
    )
}

fn countermodel(
    sink: &mut impl Write,
    options: &Options,
    stats: &zetesis_sat::Statistics,
) -> io::Result<()> {
    if let Some(support) = stats.support {
        writeln!(
            sink,
            "  necessary disjunctive support: status={:?}; construction work={}; encoding work={} (included in search work)",
            support.status, support.construction_work, support.encoding_work
        )?;
    }
    writeln!(
        sink,
        "  countermodel: search work={}; decisions={}; candidates={}; queries={}; witnesses={}; candidate restrictions={}; classical queries={}; verified stable models={}",
        stats.search.work,
        stats.search.decisions,
        stats.candidates,
        stats.countermodel_queries,
        stats.countermodels,
        stats.candidate_restrictions,
        stats.candidate_queries,
        stats.stable_models
    )?;
    writeln!(
        sink,
        "  projection history: entries={}; nodes={}; retained bytes={}; peak bytes={}; work={} (included in search work)",
        stats.projections.entries,
        stats.projections.nodes,
        stats.projections.retained_bytes,
        stats.projections.peak_bytes,
        stats.projections.work,
    )?;
    if let Some(preparation) = stats.reduct.preparation {
        writeln!(
            sink,
            "  prepared reduct: work={}; variables={}; clauses={}; literals={}; retained bytes={}; construction peak bytes={}; work included in search; capacities exclude allocator overhead",
            preparation.work,
            preparation.variables,
            preparation.clauses,
            preparation.literals,
            preparation.retained_bytes,
            preparation.peak_bytes,
        )?;
        writeln!(
            sink,
            "  prepared reduct queries: original evaluation work={}; parameter work={} (included in search); peak worker workspace bytes={}",
            stats.reduct.original_work,
            stats.reduct.parameter_work,
            stats.reduct.peak_workspace_bytes,
        )?;
    }
    if let Some(certified) = stats.certified {
        certificate(sink, &certified, options.max_completion_scratch_bytes)?;
    }
    Ok(())
}

fn certificate(
    sink: &mut impl Write,
    certified: &zetesis_sat::CertifiedStatistics,
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
            zetesis_sat::CertificatePlanStatistics::Tight(plan) => writeln!(
                sink,
                "  tight certificate storage: construction logical bytes={}; resident logical bytes={}; dependencies={}",
                plan.construction_bytes, plan.resident_bytes, plan.dependencies
            )?,
            zetesis_sat::CertificatePlanStatistics::Positive(plan) => writeln!(
                sink,
                "  positive certificate: derived atoms={}; activated formula nodes={}; dependencies={}; propagated dependencies={}; construction peak bytes={}; retained bytes={}",
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
        "  class attempts: tight refusal={:?}; positive refusal={:?}; restriction refusal={:?}; restriction work={}; committed restriction clauses={}",
        certified.tight_refusal,
        certified.positive_refusal,
        certified.restriction_refusal,
        certified.restriction_work,
        certified.restriction_clauses
    )?;
    if let Some(peak) = certified.positive_check_peak_bytes {
        writeln!(
            sink,
            "  positive checking: peak plan and evaluation bytes={peak}"
        )?;
    }
    if !matches!(
        certified.plan,
        Some(zetesis_sat::CertificatePlanStatistics::Positive(_))
    ) && let Some(attempt) = certified.positive_attempt
    {
        writeln!(
            sink,
            "  positive preparation attempt: work={}; observed peak bytes={}; primitive retained bytes={}; positive plan not selected",
            attempt.work, attempt.peak_bytes, attempt.retained_bytes
        )?;
    }
    Ok(())
}

fn formula(sink: &mut impl Write, options: &Options, report: &Details<'_>) -> io::Result<()> {
    let oracle = match report
        .countermodel_statistics
        .and_then(|s| s.certified)
        .and_then(|s| s.plan)
    {
        Some(zetesis_sat::CertificatePlanStatistics::Tight(_)) => "tight-support",
        Some(zetesis_sat::CertificatePlanStatistics::Positive(_)) => "positive-consequences",
        None => "countermodel",
    };
    if let Some(execution) = report.formula_execution {
        let backend = if execution.adapter.is_empty() {
            "cpu batched exact completion"
        } else {
            "hybrid GPU propagation + exact CPU residual search"
        };
        writeln!(
            sink,
            "  effective execution: backend={backend}; oracle={oracle}; grounder=eager; CPU completion requested workers={}; peak preflight workers={}; adapter={}",
            options.completion_workers, execution.completion.effective_workers, execution.adapter
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
            options.max_completion_scratch_bytes,
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
        writeln!(
            sink,
            "  formula batch limits: candidates={}; pending bytes={}",
            options.batch_size, options.max_batch_bytes
        )?;
        if !execution.adapter.is_empty() {
            formula_gpu(sink, execution)?;
        }
        Ok(())
    } else {
        writeln!(
            sink,
            "  effective execution: backend=cpu; oracle={oracle}; grounder=eager; search workers=1; completion scratch limit=inapplicable (scalar cursor)"
        )
    }
}

fn formula_gpu(
    sink: &mut impl Write,
    execution: &crate::FormulaExecutionStatistics,
) -> io::Result<()> {
    writeln!(
        sink,
        "  formula GPU decoded: batches={}; candidates={}; propagation work={}; completed sweeps={}; GPU-decided committed={}; CPU residuals completed={}",
        execution.gpu_batches,
        execution.gpu_candidates,
        execution.gpu_work,
        execution.gpu_rounds,
        execution.gpu_decided,
        execution.cpu_residuals
    )?;
    writeln!(
        sink,
        "  formula GPU submitted: batches={}; candidates={}; work/sweeps for unreturned results=unavailable",
        execution.gpu_submitted_batches, execution.gpu_submitted_candidates
    )?;
    if let Some(limits) = execution.gpu_limits {
        writeln!(
            sink,
            "  formula GPU limits: propagation sweeps/candidate={}; propagation work/candidate={}",
            limits.rounds_per_candidate, limits.work_per_candidate
        )?;
    }
    writeln!(
        sink,
        "  formula GPU resources: peak authored GPU bytes={}; GPU kernel timing=unavailable",
        execution.peak_accounted_bytes
    )
}

fn closure(sink: &mut impl Write, options: &Options, report: &Details<'_>) -> io::Result<()> {
    let grounder = if options.grounder == Grounder::Eager {
        "eager"
    } else {
        "lazy"
    };
    let cpu = report.shared_execution.is_some()
        || matches!(options.backend, Backend::Auto | Backend::Cpu);
    if cpu {
        writeln!(
            sink,
            "  effective execution: backend=cpu; oracle=closure; grounder={grounder}; workers={}",
            options.workers
        )?;
    } else {
        writeln!(
            sink,
            "  effective execution: oracle=closure; backend=requested GPU policy; grounder={grounder}; see backend diagnostics for actual adapter"
        )?;
    }
    if report.shared_execution.is_some() {
        writeln!(
            sink,
            "  discovered gate tuples: {}; source and world work reported separately above",
            report.discovered_gate_atoms
        )
    } else if let Some(closure) = report.closure_execution {
        writeln!(
            sink,
            "  discovered gate tuples: {}",
            report.discovered_gate_atoms
        )?;
        independent_closure(sink, closure)
    } else {
        writeln!(
            sink,
            "  discovered gate tuples: {}; oracle work=unavailable (not accumulated by this driver)",
            report.discovered_gate_atoms
        )
    }
}

fn independent_closure(
    sink: &mut impl Write,
    closure: &crate::ClosureExecutionStatistics,
) -> io::Result<()> {
    let (rounds, units) = match closure.grounder {
        Grounder::Eager => ("rule passes", "eager scan units"),
        Grounder::Lazy | Grounder::Auto => ("source rounds", "join/copy units"),
    };
    writeln!(
        sink,
        "  independent closure: checks completed={}; stopped={}; {rounds}={}; work={} ({units}); derived atoms={}; stopped checks return no counters",
        closure.completed_checks,
        closure.stopped_checks,
        closure.rounds,
        closure.work,
        closure.derived_atoms
    )?;
    if let Some(joins) = closure.joins {
        writeln!(
            sink,
            "  closure joins: catalog work={} (within work); bindings={}; tuple probes={}; peak named closure bytes={} (admitted or reserved capacity, not RSS)",
            joins.catalog_work, joins.bindings, joins.tuple_probes, joins.peak_closure_bytes
        )?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "../tests/support/statistics_writer_contracts.rs"]
mod writer_contract_tests;
