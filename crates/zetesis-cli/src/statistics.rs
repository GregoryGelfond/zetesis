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
    limits(sink, options)?;
    writeln!(
        sink,
        "  driver wall time: {:.3} ms (admission, search and output; excludes source loading and statistics)",
        elapsed.as_secs_f64() * 1000.0
    )?;
    Ok(())
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

fn limits(sink: &mut impl Write, o: &Options) -> io::Result<()> {
    if o.source_batching != crate::SourceBatching::Independent {
        writeln!(
            sink,
            "  shared CPU limits: source work/batch={}; record visits plus antecedent tests/world={}; collective catalog atoms={}; host payload bytes={}",
            o.max_source_work, o.max_work, o.max_atoms, o.max_batch_bytes
        )?;
    }
    writeln!(
        sink,
        "  search limits: candidates={}; candidate restriction/formula work={}; decisions={}; CPU candidate/lazy GPU batch source work={}",
        o.max_candidates, o.max_search_work, o.max_search_decisions, o.max_work
    )?;
    writeln!(
        sink,
        "  candidate restriction limits: copied payload bytes={}; atom occurrences={}; allocator/index overhead excluded",
        o.max_candidate_bytes, o.max_atoms
    )?;
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
        "  formula profile ceilings: atoms={}; roots={}; nodes={}; source values={}; support rounds={} (applicable when formula admission is selected)",
        formula.theory.max_atoms,
        formula.theory.max_roots,
        formula.theory.max_nodes,
        formula.max_domain_values,
        formula.max_support_rounds
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
        "  expansion limits: work={}; templates={}; values={}; eager support bytes={}",
        o.max_expansion_work, o.max_expanded_templates, o.max_expansion_values, o.max_support_bytes
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
    completion: Option<Completion>,
    interruption: Option<crate::Interruption>,
    discovered_gate_atoms: usize,
    candidate_statistics: Option<zetesis_cpu::CandidateStatistics>,
    countermodel_statistics: Option<&'a zetesis_sat::Statistics>,
    formula_execution: Option<&'a crate::FormulaExecutionStatistics>,
    lazy_execution: Option<&'a crate::LazyExecutionStatistics>,
    shared_execution: Option<&'a crate::SharedExecutionStatistics>,
    optimization: Option<&'a crate::Optimization>,
}

impl<'a> From<&'a Report> for Details<'a> {
    fn from(report: &'a Report) -> Self {
        Self {
            models: report.models,
            checked: report.checked,
            completion: Some(report.completion),
            interruption: report.interruption,
            discovered_gate_atoms: report.discovered_gate_atoms,
            candidate_statistics: report.candidate_statistics,
            countermodel_statistics: report.countermodel_statistics.as_ref(),
            formula_execution: report.formula_execution.as_ref(),
            lazy_execution: report.lazy_execution.as_ref(),
            shared_execution: report.shared_execution.as_ref(),
            optimization: report.optimization.as_ref(),
        }
    }
}

impl<'a> From<&'a crate::PartialReport> for Details<'a> {
    fn from(report: &'a crate::PartialReport) -> Self {
        Self {
            models: report.published_models,
            checked: report.checked,
            completion: report.completion,
            interruption: report.interruption,
            discovered_gate_atoms: report.discovered_gate_atoms,
            candidate_statistics: report.candidate_statistics,
            countermodel_statistics: report.countermodel_statistics.as_ref(),
            formula_execution: report.formula_execution.as_ref(),
            lazy_execution: report.lazy_execution.as_ref(),
            shared_execution: report.shared_execution.as_ref(),
            optimization: report.optimization.as_ref(),
        }
    }
}

fn details(sink: &mut impl Write, options: &Options, report: &Details<'_>) -> io::Result<()> {
    if let Some(stats) = report.candidate_statistics {
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
        let qualification = if report.completion == Some(Completion::Exhausted) {
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
    if let Some(certified) = stats.certified {
        writeln!(
            sink,
            "  tight certificate: eligible={}; refusal={:?}; storage limit={}; construction work={}; checks={}; stable decisions before commit={}; residuals={}; failed={}; checking work={}",
            certified.plan.is_some(),
            certified.refusal,
            options.max_completion_scratch_bytes,
            certified.construction_work,
            certified.checks,
            certified.stable,
            certified.residuals,
            certified.failed,
            certified.checking_work
        )?;
        if let Some(plan) = certified.plan {
            writeln!(
                sink,
                "  tight certificate storage: construction logical bytes={}; resident logical bytes={}; dependencies={}",
                plan.construction_bytes, plan.resident_bytes, plan.dependencies
            )?;
        }
    }
    Ok(())
}

fn formula(sink: &mut impl Write, options: &Options, report: &Details<'_>) -> io::Result<()> {
    let oracle = if report
        .countermodel_statistics
        .and_then(|s| s.certified)
        .is_some_and(|s| s.plan.is_some())
    {
        "tight-support"
    } else {
        "countermodel"
    };
    if let Some(execution) = report.formula_execution {
        let backend = if execution.adapter.is_empty() {
            "cpu batched exact completion"
        } else {
            "hybrid GPU propagation + exact CPU residual search"
        };
        writeln!(
            sink,
            "  effective execution: backend={backend}; oracle={oracle}; grounder=eager; CPU completion requested workers={}; peak effective workers={}; adapter={}",
            options.completion_workers, execution.completion.effective_workers, execution.adapter
        )?;
        writeln!(
            sink,
            "  formula completion: entered={}; residuals entered={}; completed before commit={}; failed={}; peak admitted logical scratch bytes={}; scratch limit={}; counters overflowed={}",
            execution.completion.entered,
            execution.completion.residuals,
            execution.completion.completed,
            execution.completion.failed,
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
            formula_gpu(sink, options, execution)?;
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
    options: &Options,
    execution: &crate::FormulaExecutionStatistics,
) -> io::Result<()> {
    writeln!(
        sink,
        "  formula GPU: batches={}; candidates={}; propagation work={}; completed sweeps={}; GPU-decided committed={}; CPU residuals completed={}",
        execution.gpu_batches,
        execution.gpu_candidates,
        execution.gpu_work,
        execution.gpu_rounds,
        execution.gpu_decided,
        execution.cpu_residuals
    )?;
    writeln!(
        sink,
        "  formula GPU limits: propagation sweeps=64; propagation work/candidate={} (u32 ceiling); peak authored GPU bytes={}; GPU kernel timing=unavailable",
        options.max_work.min(u64::from(u32::MAX)),
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
        || options.backend == Backend::Cpu
        || (options.backend == Backend::Auto && !cfg!(feature = "gpu"));
    if cpu {
        writeln!(
            sink,
            "  effective execution: backend=cpu; oracle=closure; grounder={grounder}; workers={}",
            options.workers
        )?;
    } else if options.backend == Backend::Auto {
        writeln!(
            sink,
            "  effective execution: oracle=closure; backend=untracked (CPU/GPU/mixed possible); grounder={grounder}"
        )?;
        writeln!(
            sink,
            "  auto selection: may change between batches; see backend diagnostics for actual adapter and fallback events"
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
    } else {
        writeln!(
            sink,
            "  discovered gate tuples: {}; oracle work=unavailable (not accumulated by this driver)",
            report.discovered_gate_atoms
        )
    }
}

#[cfg(test)]
#[path = "../tests/support/statistics_writer_contracts.rs"]
mod writer_contract_tests;
