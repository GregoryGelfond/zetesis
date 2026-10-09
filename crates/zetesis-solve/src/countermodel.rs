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
    /// How the key analysis that asked them ended.
    pub(crate) key_analysis: zetesis_themelios::KeyAnalysis,
    pub(crate) objectives: &'a zetesis_objective::ObjectiveProgram,
    /// Source analysis chooses attempt order; each plan checks the whole theory.
    pub(crate) certificate_order: zetesis_sat::CertificateOrder,
}

pub(crate) fn certificate_order(
    analysis: &zetesis_themelios::analysis::Analysis,
    basis: zetesis_themelios::AnalysisBasis,
) -> zetesis_sat::CertificateOrder {
    use zetesis_themelios::analysis::classify::{HornKind, Normality, Stratification};
    let classes = analysis.classes();
    if basis == zetesis_themelios::AnalysisBasis::NormalizedProgram
        && matches!(classes.horn(), HornKind::Horn)
    {
        zetesis_sat::CertificateOrder::PositiveFirst
    } else if basis == zetesis_themelios::AnalysisBasis::NormalizedProgram
        && matches!(classes.normality(), Normality::Normal)
        && matches!(classes.stratification(), Stratification::Stratified)
    {
        zetesis_sat::CertificateOrder::StratifiedFirst
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
    if options.oracle != crate::Oracle::Auto {
        return Ok(None);
    }
    let limits = certificate_limits(options);
    let eligibility = phases.measure(SolvePhase::CertificateSetup, || {
        if options.backend.is_gpu() {
            // Device execution currently implements tight support. Preparation
            // authenticates the complete theory and charges the same owner, but
            // does not install CPU membership checks behind a device request.
            return models
                .prepare_tight_certificate(limits.tight)
                .map(|plan| plan.is_some());
        }
        models.enable_class_checking(limits, order)
    });
    match eligibility {
        Ok(true) => match models.statistics().certified.and_then(|stats| stats.plan) {
            Some(zetesis_sat::CertificatePlanStatistics::Tight(_)) => {
                diagnostics.record(Event::TightMembership)?;
            }
            Some(zetesis_sat::CertificatePlanStatistics::Positive(_)) => {
                diagnostics.record(Event::PositiveMembership)?;
            }
            Some(zetesis_sat::CertificatePlanStatistics::Stratified(_)) => {
                diagnostics.record(Event::StratifiedMembership)?;
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

/// Certificate populations use checked representation ceilings. Each compiler
/// counts the actual program and checks its storage before allocating; these
/// ceilings are not reservation sizes. Preparation consumes the enumeration's
/// remaining search work, so ordinary execution adds no independent work cap.
fn certificate_limits(options: &SolveConfig) -> zetesis_sat::CertificateLimits {
    zetesis_sat::CertificateLimits {
        tight: zetesis_ferraris::TightPlanLimits {
            max_producers: usize::MAX,
            max_dependencies: usize::MAX,
            max_bytes: options.max_completion_scratch_bytes,
            max_work: options.max_search_work,
        },
        positive: zetesis_ferraris::PositivePlanLimits {
            max_dependencies: usize::MAX,
            max_bytes: usize::try_from(options.max_completion_scratch_bytes).unwrap_or(usize::MAX),
            max_work: options.max_search_work,
        },
        stratified: zetesis_ferraris::StratifiedPlanLimits {
            max_dependencies: usize::MAX,
            max_bytes: usize::try_from(options.max_completion_scratch_bytes).unwrap_or(usize::MAX),
            max_work: options.max_search_work,
        },
    }
}

pub(crate) fn search_limits(options: &SolveConfig) -> zetesis_sat::Limits {
    zetesis_sat::Limits {
        admission: cnf_admission(options.max_candidate_bytes as u128),
        reduct_admission: cnf_admission(u128::from(options.max_reduct_bytes)),
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
    }
}

// These independent logical populations derive from their existing named owner
// allowance. They do not estimate total RSS. The extent cap keeps doubling a
// literal population and the variable/sign index within checked host indices.
fn cnf_admission(bytes: u128) -> zetesis_sat::AdmissionLimits {
    let extent = usize::try_from(bytes.min(isize::MAX as u128)).unwrap_or(isize::MAX as usize);
    zetesis_sat::AdmissionLimits {
        max_variables: extent / 32,
        max_clauses: extent / (4 * size_of::<usize>()),
        max_literals: extent / size_of::<zetesis_sat::Literal>(),
    }
}

#[cfg(test)]
mod tests;
