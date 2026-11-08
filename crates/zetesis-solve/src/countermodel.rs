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
    /// Written constraints asked as the one atom their key admits.
    pub(crate) keyed_constraints: usize,
    /// How the key analysis that asked them ended.
    pub(crate) key_analysis: zetesis_themelios::KeyAnalysis,
    pub(crate) objectives: &'a zetesis_objective::ObjectiveProgram,
    /// Optional necessary choice facts authenticated to this original theory.
    pub(crate) required_choices: Option<&'a zetesis_themelios::RequiredChoices>,
    /// Source analysis chooses attempt order; each plan checks the whole theory.
    pub(crate) certificate_order: zetesis_stable::CertificateOrder,
}

pub(crate) fn certificate_order(
    classes: &zetesis_themelios::analysis::classify::Classes,
    basis: zetesis_themelios::AnalysisBasis,
) -> zetesis_stable::CertificateOrder {
    use zetesis_themelios::analysis::classify::{HornKind, Normality, Stratification};
    if basis == zetesis_themelios::AnalysisBasis::NormalizedProgram
        && matches!(classes.horn(), HornKind::Horn)
    {
        zetesis_stable::CertificateOrder::PositiveFirst
    } else if basis == zetesis_themelios::AnalysisBasis::NormalizedProgram
        && matches!(classes.normality(), Normality::Normal)
        && matches!(classes.stratification(), Stratification::Stratified)
    {
        zetesis_stable::CertificateOrder::StratifiedFirst
    } else {
        zetesis_stable::CertificateOrder::TightFirst
    }
}

/// An optional setup interruption is retained by the same search report. A
/// diagnostic failure propagates separately and never erases the owned stream.
pub(crate) fn prepare_certificate(
    models: &mut zetesis_stable::StableModels,
    order: zetesis_stable::CertificateOrder,
    options: &SolveConfig,
    diagnostics: &mut impl ExecutionSink,
    phases: &Recorder,
) -> Result<Option<zetesis_stable::Incomplete>, SolveError> {
    if options.oracle != crate::Oracle::Auto || models.root_refuted() {
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
            Some(zetesis_stable::CertificatePlanStatistics::Tight(_)) => {
                diagnostics.record(Event::TightMembership)?;
            }
            Some(zetesis_stable::CertificatePlanStatistics::Positive(_)) => {
                diagnostics.record(Event::PositiveMembership)?;
            }
            Some(zetesis_stable::CertificatePlanStatistics::Stratified(_)) => {
                diagnostics.record(Event::StratifiedMembership)?;
            }
            None => return Ok(Some(zetesis_stable::Incomplete::InvalidWitness)),
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
fn certificate_limits(options: &SolveConfig) -> zetesis_stable::CertificateLimits {
    zetesis_stable::CertificateLimits {
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

pub(crate) fn search_limits(options: &SolveConfig) -> zetesis_stable::Limits {
    zetesis_stable::Limits {
        search: zetesis_stable::SearchLimits {
            max_work: options.max_search_work,
            max_decisions: options.max_search_decisions,
        },
        max_candidates: options.max_candidates,
        max_reduct_bytes: options.max_reduct_bytes,
        // Candidate-index storage uses the existing candidate memory allowance,
        // independently of the reduct query workspace. No additional CLI control.
        max_native_narrowing_bytes: options.max_candidate_bytes as u64,
        max_verification_work: options.max_work,
        // Learned rows are candidate-narrowing state: their tiers grow with
        // the same candidate memory allowance.
        learning: zetesis_ferraris::LearningLimits::for_allowance(options.max_candidate_bytes),
    }
}

#[cfg(test)]
mod tests;
