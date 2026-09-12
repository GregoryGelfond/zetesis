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
    pub(crate) objectives: &'a zetesis_objective::ObjectiveProgram,
}

/// An optional setup interruption is retained by the same search report. A
/// diagnostic failure propagates separately and never erases the owned stream.
pub(crate) fn prepare_certificate(
    models: &mut zetesis_sat::StableModels,
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
        models.enable_certified_checking(zetesis_ferraris::TightPlanLimits {
            max_bytes: options.max_completion_scratch_bytes,
            ..Default::default()
        })
    });
    match eligibility {
        Ok(true) => diagnostics.record(Event::TightMembership)?,
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
        max_candidates: options.max_candidates,
        max_verification_work: options.max_work,
        ..Default::default()
    }
}

#[cfg(test)]
#[path = "../tests/support/formula_harness.rs"]
mod test_harness;

#[cfg(test)]
#[path = "../tests/support/batch_orchestration.rs"]
mod batch_orchestration_tests;

#[cfg(test)]
#[path = "../tests/support/partial_batch.rs"]
mod partial_batch_tests;
