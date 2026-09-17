//! Formula input, checked membership planning and search limits.

use crate::ExecutionObservation as Event;
use crate::execution_observation::ExecutionSink;
use crate::phase_timing::{Recorder, SolvePhase};
use crate::{SolveConfig, SolveError};
use zetesis_core::AtomCatalog;
use zetesis_ferraris::Theory;

#[derive(Clone, Copy)]
pub(crate) struct Input<'a> {
    pub(crate) theory: &'a Theory,
    pub(crate) atoms: &'a AtomCatalog,
    pub(crate) gate_atoms: usize,
    /// Written constraints asked as the one atom their key admits.
    pub(crate) keyed_constraints: usize,
    pub(crate) objectives: &'a zetesis_objective::ObjectiveProgram,
    /// Source analysis chooses attempt order; each plan checks the whole theory.
    pub(crate) certificate_order: zetesis_sat::CertificateOrder,
}

pub(crate) fn certificate_order(
    analysis: &zetesis_themelios::analysis::Analysis,
    basis: zetesis_themelios::AnalysisBasis,
) -> zetesis_sat::CertificateOrder {
    if basis == zetesis_themelios::AnalysisBasis::NormalizedProgram
        && matches!(
            analysis.classes().horn(),
            zetesis_themelios::analysis::classify::HornKind::Horn
        )
    {
        zetesis_sat::CertificateOrder::PositiveFirst
    } else {
        zetesis_sat::CertificateOrder::TightFirst
    }
}

/// An optional setup interruption is retained by the same search report. A
/// diagnostic failure propagates separately and never erases the owned stream.
pub(crate) fn prepare_certificate(
    models: &mut zetesis_sat::StableModels,
    order: zetesis_sat::CertificateOrder,
    options: &SolveConfig,
    diagnostics: &mut impl ExecutionSink,
    phases: &Recorder,
) -> Result<Option<zetesis_sat::Incomplete>, SolveError> {
    if options.oracle != crate::Oracle::Auto
        || !matches!(options.backend, crate::Backend::Auto | crate::Backend::Cpu)
    {
        return Ok(None);
    }
    let eligibility = phases.measure(SolvePhase::CertificateSetup, || {
        models.enable_class_checking(
            zetesis_sat::CertificateLimits {
                tight: zetesis_ferraris::TightPlanLimits {
                    max_bytes: options.max_completion_scratch_bytes,
                    ..Default::default()
                },
                positive: zetesis_ferraris::PositivePlanLimits {
                    max_bytes: usize::try_from(options.max_completion_scratch_bytes)
                        .unwrap_or(usize::MAX),
                    ..Default::default()
                },
            },
            order,
        )
    });
    match eligibility {
        Ok(true) => match models.statistics().certified.and_then(|stats| stats.plan) {
            Some(zetesis_sat::CertificatePlanStatistics::Tight(_)) => {
                diagnostics.record(Event::TightMembership)?;
            }
            Some(zetesis_sat::CertificatePlanStatistics::Positive(_)) => {
                diagnostics.record(Event::PositiveMembership)?;
            }
            None => return Ok(Some(zetesis_sat::Incomplete::InvalidWitness)),
        },
        Ok(false) => diagnostics.record(Event::GeneralMembership(
            models
                .statistics()
                .certified
                .and_then(|s| s.refusal)
                .expect("refused certificate records its reason"),
        ))?,
        Err(error) => return Ok(Some(error)),
    }
    Ok(None)
}

pub(crate) fn search_limits(options: &SolveConfig) -> zetesis_sat::Limits {
    zetesis_sat::Limits {
        search: zetesis_sat::SearchLimits {
            max_work: options.max_search_work,
            max_decisions: options.max_search_decisions,
        },
        projections: zetesis_sat::ProjectionLimits {
            max_entries: options.max_projection_entries,
            max_nodes: options.max_projection_nodes,
            max_bytes: options.max_projection_bytes,
        },
        max_candidates: options.max_candidates,
        max_reduct_bytes: options.max_reduct_bytes,
        max_verification_work: options.max_work,
        ..Default::default()
    }
}

#[cfg(test)]
#[path = "../tests/support/formula_harness.rs"]
mod test_harness;

#[cfg(test)]
#[path = "../tests/support/certificate_order.rs"]
mod certificate_order_tests;

#[cfg(test)]
#[path = "../tests/support/batch_orchestration.rs"]
mod batch_orchestration_tests;

#[cfg(test)]
#[path = "../tests/support/partial_batch.rs"]
mod partial_batch_tests;
