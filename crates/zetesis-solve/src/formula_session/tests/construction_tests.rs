//! Cancellation after real membership cannot publish a partial model.

use super::{FormulaSession, Input};
use crate::execution_observation::Ignore;
use crate::formula_execution::{Execution, Failure, MembershipExecution};
use crate::phase_timing::Recorder;
use crate::{
    AnswerSelection, Backend, Completion, FormulaExecutionStatistics, Interruption,
    ModelConstructionStop, SolveConfig, SolvePhase,
};
use zetesis_cpu::{Cancellation, Stop};
use zetesis_ferraris::Interpretation;
use zetesis_sat::StableModels;
use zetesis_themelios::{AdmissionOptions, ExpansionLimits, FormulaLimits, admit_formula};

struct CancelAfterMembership;

impl MembershipExecution for CancelAfterMembership {
    fn next(
        &mut self,
        models: &mut StableModels,
        config: &SolveConfig,
        cancellation: &Cancellation,
        phases: &Recorder,
    ) -> Option<Result<Interpretation, Failure>> {
        let next = Execution::Cpu.next(models, config, cancellation, phases);
        if matches!(next, Some(Ok(_))) {
            cancellation.cancel();
        }
        next
    }

    fn statistics(&self, _: &StableModels) -> Option<FormulaExecutionStatistics> {
        None
    }
}

#[test]
fn cancellation_after_membership_prevents_construction() {
    let owner = admit_formula(
        "a.".into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    let config = SolveConfig {
        backend: Backend::Cpu,
        workers: std::num::NonZeroUsize::MIN,
        models: 0,
        ..SolveConfig::default()
    };
    let cancellation = Cancellation::default();
    let phases = Recorder::new(true);
    let mut session = FormulaSession::with_selection(
        Input {
            theory: owner.theory(),
            atoms: owner.atom_catalog(),
            objectives: owner.objectives(),
            gate_atoms: 0,
            keyed_constraints: 0,
            key_analysis: zetesis_themelios::KeyAnalysis::Complete,
            certificate_order: zetesis_sat::CertificateOrder::TightFirst,
        },
        CancelAfterMembership,
        &config,
        &mut Ignore,
        &cancellation,
        &phases,
        AnswerSelection::All,
    );
    let prepared = *session.outcome(&phases).model_construction().unwrap();
    assert!(
        session
            .next(&config, &mut Ignore, &cancellation, &phases)
            .is_none()
    );
    let outcome = session.outcome(&phases);
    assert_eq!(outcome.verified_models(), 1);
    assert_eq!(outcome.completion(), Some(Completion::Interrupted));
    assert_eq!(
        outcome.interruption(),
        Some(Interruption::ModelConstruction(
            ModelConstructionStop::Control(Stop::Cancelled)
        ))
    );
    let receipt = outcome.model_construction().unwrap();
    assert_eq!(receipt.work, prepared.work);
    assert_eq!(receipt.prepared_bytes, prepared.prepared_bytes);
    assert_eq!(receipt.constructed, 0);
    assert!(!outcome.unsatisfiable());
    assert_eq!(
        phases
            .snapshot()
            .unwrap()
            .get(SolvePhase::ModelConstruction)
            .unwrap()
            .calls,
        2
    );
    assert!(
        session
            .next(&config, &mut Ignore, &cancellation, &phases)
            .is_none()
    );
}
