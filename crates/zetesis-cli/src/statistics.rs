//! Opt-in driver measurements; unavailable execution counters are never inferred.

use std::io::{self, Write};
use std::time::Duration;

use crate::{Backend, Completion, Grounder, Options, Report, RunFailure};

fn header(sink: &mut impl Write, options: &Options, elapsed: Duration) -> io::Result<()> {
    writeln!(
        sink,
        "Statistics: zetesis {}; GPU compiled={}",
        env!("CARGO_PKG_VERSION"),
        cfg!(feature = "gpu")
    )?;
    writeln!(
        sink,
        "  requested: backend={:?}; oracle={:?}; grounder={}",
        options.backend,
        options.oracle,
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
    result: Result<&Report, &RunFailure>,
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
                let report = Report {
                    models: partial.published_models,
                    checked: partial.checked,
                    completion: partial.completion.unwrap_or(Completion::Interrupted),
                    interruption: partial.interruption,
                    discovered_gate_atoms: partial.discovered_gate_atoms,
                    countermodel_statistics: partial.countermodel_statistics,
                    formula_execution: partial.formula_execution.clone(),
                    optimization: partial.optimization.clone(),
                    phase_timings: failure.phase_timings.as_deref().copied(),
                };
                details(sink, options, &report)?;
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
    writeln!(
        sink,
        "  search limits: candidates={}; countermodel work={}; decisions={}; per-candidate oracle work={}",
        o.max_candidates, o.max_search_work, o.max_search_decisions, o.max_work
    )?;
    writeln!(
        sink,
        "  grounding limits: atoms={}; carrier atoms={}; substitutions={}; ground rules={}; GPU batch bytes={}",
        o.max_atoms,
        o.max_carrier_atoms,
        o.max_substitutions,
        o.max_ground_rules,
        o.max_batch_bytes
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
        "  expansion limits: work={}; templates={}; values={}",
        o.max_expansion_work, o.max_expanded_templates, o.max_expansion_values
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
    details(sink, options, report)
}

fn details(sink: &mut impl Write, options: &Options, report: &Report) -> io::Result<()> {
    writeln!(
        sink,
        "  results: displayed models={}; candidates examined={}",
        report.models, report.checked
    )?;
    if let Some(reason) = report.interruption {
        writeln!(sink, "  interruption: {reason}")?;
    }
    if let Some(stats) = report.countermodel_statistics {
        formula(sink, options, report)?;
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
    if let Some(optimum) = &report.optimization {
        let qualification = if report.completion == Completion::Exhausted {
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

fn formula(sink: &mut impl Write, options: &Options, report: &Report) -> io::Result<()> {
    if let Some(execution) = &report.formula_execution {
        let backend = if execution.adapter.is_empty() {
            "cpu batched exact completion"
        } else {
            "hybrid GPU propagation + exact CPU residual search"
        };
        writeln!(
            sink,
            "  effective execution: backend={backend}; oracle=countermodel; grounder=eager; CPU completion requested workers={}; peak effective workers={}; adapter={}",
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
            "  effective execution: backend=cpu; oracle=countermodel; grounder=eager; search workers=1; completion scratch limit=inapplicable (scalar cursor)"
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

fn closure(sink: &mut impl Write, options: &Options, report: &Report) -> io::Result<()> {
    let cpu = options.backend == Backend::Cpu
        || (options.backend == Backend::Auto
            && (options.grounder == Grounder::Lazy || !cfg!(feature = "gpu")));
    if cpu {
        let grounder = if options.grounder == Grounder::Eager {
            "eager"
        } else {
            "lazy"
        };
        writeln!(
            sink,
            "  effective execution: backend=cpu; oracle=closure; grounder={grounder}; workers={}",
            options.workers
        )?;
    } else if options.backend == Backend::Auto {
        let grounder = if options.grounder == Grounder::Eager {
            "eager"
        } else {
            "untracked (lazy/eager possible)"
        };
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
            "  effective execution: oracle=closure; backend=requested GPU policy; grounder=eager (see backend diagnostics for actual adapter)"
        )?;
    }
    writeln!(
        sink,
        "  discovered gate tuples: {}; oracle work=unavailable (not accumulated by this driver)",
        report.discovered_gate_atoms
    )
}

#[cfg(test)]
#[path = "../tests/support/statistics_writer_contracts.rs"]
mod writer_contract_tests;
