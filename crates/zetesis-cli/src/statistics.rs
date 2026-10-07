//! Opt-in driver measurements; unavailable execution counters are never inferred.

use std::io::{self, Write};
use std::time::Duration;

use crate::{Backend, Completion, Grounder, Options, PublicationFailure, Report};

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
        "  requested: backend={}; oracle={}; grounder={}; search={}",
        options.backend.label(),
        options.oracle.label(),
        options.grounder.label(),
        options.search.label()
    )?;
    writeln!(
        sink,
        "  configured: workers={}; batch={}; displayed models={} (0=all)",
        options.workers, config.batch_size, options.models
    )?;
    writeln!(
        sink,
        "  memory allowance: {} bytes (host physical memory {}); shared policy for named input, grounding, solving and output capacities; not resident memory",
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
        options,
        &config,
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
            hybrid_execution: semantic.hybrid_execution(),
            terminal_execution: semantic.terminal_execution(),
            model_construction: semantic.model_construction(),
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
    let config = crate::SolveConfig::from(options);
    header(sink, options, &config, elapsed)?;
    match result {
        Ok(report) => completed(sink, options, &config, report),
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
                details(sink, options, &config, &Details::from(partial.as_ref()))?;
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
        "  configured execution storage: candidate bytes={}; reduct bytes/owner={}; completion bytes={}; closure bytes/worker={}; closure bytes/collective={}; batch bytes={}",
        c.max_candidate_bytes,
        c.max_reduct_bytes,
        c.max_completion_scratch_bytes,
        c.max_closure_bytes,
        c.max_closure_batch_bytes,
        c.max_batch_bytes
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
    options: &Options,
    config: &crate::SolveConfig,
    report: &Report,
) -> io::Result<()> {
    let status = match report.completion {
        Completion::Exhausted => "exhausted",
        Completion::RequestedModels => "requested models reached (partial coverage)",
        Completion::Interrupted => "interrupted (partial coverage)",
    };
    writeln!(sink, "  completion: {status}")?;
    details(sink, options, config, &Details::from(report))
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
    hybrid_execution: Option<&'a zetesis_solve::HybridExecutionStatistics>,
    terminal_execution: Option<&'a zetesis_solve::TerminalExecutionStatistics>,
    model_construction: Option<&'a zetesis_solve::ModelConstructionStatistics>,
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
            hybrid_execution: report.hybrid_execution.as_ref(),
            terminal_execution: report.terminal_execution.as_ref(),
            model_construction: report.model_construction.as_ref(),
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
            hybrid_execution: report.hybrid_execution.as_ref(),
            terminal_execution: report.terminal_execution.as_ref(),
            model_construction: report.model_construction.as_ref(),
            lazy_execution: report.lazy_execution.as_ref(),
            shared_execution: report.shared_execution.as_ref(),
            closure_execution: report.closure_execution.as_ref(),
            query_execution: report.query_execution.as_ref(),
            optimization: report.optimization.as_ref(),
        }
    }
}

fn details(
    sink: &mut impl Write,
    options: &Options,
    config: &crate::SolveConfig,
    report: &Details<'_>,
) -> io::Result<()> {
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
        writeln!(
            sink,
            "  discovered gate tuples: inapplicable (complete semantic candidates)"
        )?;
    } else if report.checked > 0 {
        closure(sink, options, report)?;
    } else {
        if report
            .candidate_statistics
            .is_some_and(|stats| stats.root_refuted)
        {
            // The root's narrowing settled the program: no seed was offered
            // and no execution route ran; the carrier statistics above say
            // what refuted it.
            writeln!(
                sink,
                "  effective execution: none needed; the root narrowing refuted every seed"
            )?;
        } else {
            writeln!(
                sink,
                "  effective execution: unavailable (stopped before execution counters)"
            )?;
        }
        writeln!(
            sink,
            "  oracle work: unavailable; discovered gate tuples: {}",
            report.discovered_gate_atoms
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
    if stats.root_refuted {
        return writeln!(
            sink,
            "  carrier narrowing: refuted by a definite constraint after {} passes; no seed offered",
            stats.narrowing_passes
        );
    }
    writeln!(
        sink,
        "  carrier narrowing: passes={}; cut gate atoms={}; held gate atoms={}",
        stats.narrowing_passes, stats.cut_gate_atoms, stats.held_gate_atoms
    )?;
    if let Some(stop) = stats.narrowing_stop {
        writeln!(
            sink,
            "  carrier narrowing stop: {stop}; the completed passes' bounds were kept"
        )?;
    }
    if stats.regions > 0 {
        writeln!(
            sink,
            "  carrier regions: visited={}; refuted={}; leaves={}; counted={}; narrowing passes={}",
            stats.regions,
            stats.regions_refuted,
            stats.regions_leaves,
            stats.regions_counted,
            stats.region_passes
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
        "  hybrid grounding: eager retained core; streamed source constraints; core answers={}; accepted={}; rejected={}; pending={}",
        stats.core_answers, stats.accepted, stats.rejected, stats.pending
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
            "  prepared CPU queries: builds={}; adoptions={}; retained workspaces={}; active ranges={}; reused slots={}; retained bytes={}; reserved envelope bytes={}",
            stats.preparation_builds,
            stats.preparation_adoptions,
            stats.retained_workspaces,
            stats.active_workspaces,
            stats.reused_workspaces,
            stats.retained_bytes,
            stats.reserved_bytes
        )?;
        if let Some(preparation) = stats.preparation {
            writeln!(
                sink,
                "  query preparation: work={}; retained bytes={}; dense predicates={} of {}; separate from candidate work; capacities are not RSS",
                preparation.work,
                preparation.retained_bytes,
                preparation.dense_predicates,
                preparation.predicates
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
    config: &crate::SolveConfig,
    stats: &zetesis_sat::Statistics,
    membership: &str,
) -> io::Result<()> {
    if let Some(filter) = stats.region_filter {
        writeln!(
            sink,
            "  candidate region checks: preparations={}; checks={}; refuted={}; failed={}; overflowed={}; source work accounted separately; frozen reduct excluded",
            filter.preparations, filter.checks, filter.refuted, filter.failed, filter.overflowed
        )?;
    }
    if let Some(support) = stats.support {
        writeln!(
            sink,
            "  necessary disjunctive support: status={:?}; construction work={}; encoding work={} (included in search work)",
            support.status, support.construction_work, support.encoding_work
        )?;
    }
    if let Some(regions) = stats.regions {
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
    }
    writeln!(
        sink,
        "  countermodel: search work={}; decisions={}; candidates={}; queries={}; witnesses={}; candidate restrictions={}; classical queries={}; {membership}={}",
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
        Some(zetesis_sat::CertificatePlanStatistics::Tight(_)) => "tight-support",
        Some(zetesis_sat::CertificatePlanStatistics::Positive(_)) => "positive-consequences",
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
        writeln!(
            sink,
            "  formula batch limits: candidates={}; pending bytes={}",
            config.batch_size, config.max_batch_bytes
        )?;
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
            "  tight GPU limits: complete support scan work/candidate={work}; propagation sweeps=not applicable"
        )?;
    }
    if let Some(work) = execution.gpu_scheduled_work {
        writeln!(
            sink,
            "  tight GPU scheduled full-scan work={work}; includes submitted unreturned batches"
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
        "  formula GPU resources: peak authored GPU bytes={}; GPU kernel timing=unavailable",
        execution.peak_accounted_bytes
    )
}

fn closure(sink: &mut impl Write, options: &Options, report: &Details<'_>) -> io::Result<()> {
    // The statistics say which route ran; without one, the requested policy
    // says which would have.
    let grounder = report.closure_execution.map_or(
        if options.grounder == Grounder::Eager {
            "eager"
        } else {
            "lazy"
        },
        |closure| closure.route.label(),
    );
    let cpu = report.shared_execution.is_some() || options.backend == Backend::Cpu;
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
    let (rounds, units) = match closure.route {
        crate::ClosureRoute::Eager => ("rule passes", "eager scan units"),
        crate::ClosureRoute::Lazy(_) => ("source rounds", "join/copy units"),
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
    if let crate::ClosureRoute::Lazy(joins) = closure.route {
        writeln!(
            sink,
            "  closure joins: catalog work={} (within work); bindings={}; tuple probes={}; dense heads={} (recorded as bits); block steps={} (blocks joined by words); peak named closure bytes={} (admitted or reserved capacity, not RSS)",
            joins.catalog_work,
            joins.bindings,
            joins.tuple_probes,
            joins.dense_heads,
            joins.block_steps,
            joins.peak_closure_bytes
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
