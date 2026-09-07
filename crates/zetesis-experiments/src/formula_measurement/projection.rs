//! Paired gate-projection measurements share membership and residency checks.

use super::{Backend, FormulaBenchmarkError, FormulaCase, FormulaOptions, FormulaPool, validate};
use crate::FormulaFixture;
use std::io::Write;
use std::time::Instant;
use zetesis_wgpu::{GateProjection, GpuFormulaOracle, GpuOptions};

/// Compare both exact gate projections on the same immutable synthetic batches.
///
/// The command requires physical Metal, keeps separate resident oracles, and
/// checks their reported metadata for equality. Metadata equality is not a
/// physical-device identity certificate. Scalar and Rayon native membership
/// precede each pair; warm projection order alternates. Candidate construction,
/// fixture construction and device setup are outside membership intervals.
/// Each hybrid interval includes serial exact CPU completion of every residual.
/// CPU search limits are per candidate: this does not characterize ordinary
/// cumulative-budget execution, grounding, or full answer-set enumeration.
///
/// Space is bounded by the configured fixture/batch dimensions, two resident
/// devices, one candidate batch and native result vectors. No samples are
/// retained by this function; the caller owns the output writer. Work is bounded
/// by the supplied finite case lists, repetitions and per-query limits. Driver
/// device creation and output may block independently of those query limits.
///
/// # Errors
/// Refuses CPU-only selection, invalid dimensions, mismatching adapter metadata,
/// incomplete native checks, GPU failures, semantic or residency mismatches,
/// allocation failures and writer errors. Any failure stops the pair traversal;
/// earlier complete output remains a prefix and no final PASS is written.
pub fn run_formula_projection(
    options: &FormulaOptions,
    output: &mut impl Write,
) -> Result<(), FormulaBenchmarkError> {
    validate(options)?;
    if options.backend != Backend::Metal {
        return Err(FormulaBenchmarkError::ProjectionRequiresMetal);
    }
    header(options, output)?;
    let max_candidates = options
        .batches
        .iter()
        .map(|n| n.get())
        .max()
        .ok_or(FormulaBenchmarkError::Dimensions)?;
    let started = Instant::now();
    let pool = FormulaPool::new(options.cpu_workers, max_candidates)?;
    writeln!(
        output,
        "# cpu_pool=rayon requested_workers={} actual_workers={} pool_init_ns={}",
        options.cpu_workers,
        pool.workers(),
        started.elapsed().as_nanos()
    )?;
    let mut oracles = [
        create(GateProjection::Enumerated, output)?,
        create(GateProjection::Bitwise, output)?,
    ];
    if oracles[0].info().metadata() != oracles[1].info().metadata() {
        return Err(FormulaBenchmarkError::AdapterMismatch);
    }
    writeln!(
        output,
        "family\tatoms\tnodes\tworlds\tbackend\tphase\titeration\telapsed_ns\tdispatch_host_ns\tresidual_cpu_ns\tresiduals"
    )?;
    for &family in &options.families {
        for atoms in &options.atoms {
            let started = Instant::now();
            let fixture = FormulaFixture::new(family, atoms.get())?;
            writeln!(
                output,
                "# family={} fixture_build_ns={}",
                family.label(),
                started.elapsed().as_nanos()
            )?;
            for batch in &options.batches {
                let case = FormulaCase {
                    options,
                    family,
                    fixture: &fixture,
                    batch: batch.get(),
                };
                for iteration in 0..=options.repetitions.get() {
                    let candidates = fixture.candidates(batch.get(), iteration)?;
                    let phase = if iteration == 0 {
                        "initial-case"
                    } else {
                        "warm"
                    };
                    let expected =
                        case.reference(output, &pool, &candidates, (phase, iteration))?;
                    for index in order(iteration) {
                        case.hybrid(
                            output,
                            &mut oracles[index],
                            &candidates,
                            &expected,
                            (phase, iteration),
                        )?;
                    }
                }
            }
        }
    }
    writeln!(
        output,
        "# status=PASS scope=paired-synthetic-membership all_completed_results_match_native=true"
    )?;
    Ok(())
}

fn header(options: &FormulaOptions, output: &mut impl Write) -> Result<(), FormulaBenchmarkError> {
    writeln!(
        output,
        "# profile=formula-projection source_grounding=excluded outer_search=excluded"
    )?;
    writeln!(
        output,
        "# projection_enumerated=metal-with-cpu-residuals projection_bitwise=metal-bitwise-with-cpu-residuals"
    )?;
    writeln!(
        output,
        "# timing_order=cpu-native,cpu-rayon,rotating-projection-pair even=enumerated,bitwise odd=bitwise,enumerated candidate_stream=synthetic cpu_workers={} residual_cpu_workers=1 cpu_work_limit_scope=per_candidate",
        options.cpu_workers
    )?;
    writeln!(
        output,
        "# version={} max_work_per_candidate={} max_batch_bytes={} gpu_max_rounds={} gpu_max_work_per_candidate={} repetitions={} gpu_readback_timeout_ns={}",
        env!("CARGO_PKG_VERSION"),
        options.max_work,
        options.max_batch_bytes,
        options.gpu_max_rounds,
        options.gpu_max_work,
        options.repetitions,
        zetesis_wgpu::FormulaLimits::default().timeout.as_nanos()
    )?;
    let native = zetesis_sat::Limits::default();
    writeln!(
        output,
        "# native_limits_scope=per_candidate max_variables={} max_clauses={} max_literals={} max_search_work={} max_decisions={} max_candidates={} max_verification_work={} cancellation=unset deadline=none",
        native.admission.max_variables,
        native.admission.max_clauses,
        native.admission.max_literals,
        options.max_work,
        native.search.max_decisions,
        native.max_candidates,
        options.max_work
    )?;
    let admission = zetesis_ferraris::AdmissionLimits::default();
    writeln!(
        output,
        "# fixture_limits max_atoms={} max_nodes={} max_roots={} require_physical_gpu=true",
        admission.max_atoms, admission.max_nodes, admission.max_roots
    )?;
    Ok(())
}

fn create(
    projection: GateProjection,
    output: &mut impl Write,
) -> Result<GpuFormulaOracle, FormulaBenchmarkError> {
    let started = Instant::now();
    let oracle = GpuFormulaOracle::new_metal_with_projection(GpuOptions::default(), projection)
        .map_err(FormulaBenchmarkError::Gpu)?;
    let metadata = oracle.info().metadata();
    writeln!(
        output,
        "# projection={} adapter={} backend={} category={} vendor_id={} device_id={} device_init_ns={}",
        projection.label(),
        metadata.name,
        metadata.backend,
        metadata.category,
        metadata.vendor_id,
        metadata.device_id,
        started.elapsed().as_nanos()
    )?;
    Ok(oracle)
}

fn order(iteration: usize) -> [usize; 2] {
    if iteration.is_multiple_of(2) {
        [0, 1]
    } else {
        [1, 0]
    }
}

#[cfg(test)]
mod tests {
    use super::order;

    #[test]
    fn paired_order_alternates_without_omitting_a_projection() {
        for iteration in 0..=100 {
            assert_eq!(
                order(iteration),
                if iteration % 2 == 0 { [0, 1] } else { [1, 0] }
            );
        }
    }
}
