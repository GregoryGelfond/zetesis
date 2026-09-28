//! Cancellation between real base membership and original-answer qualification.

use super::TerminalSession;
use crate::execution_observation::Ignore;
use crate::phase_timing::Recorder;
use crate::{Backend, Completion, ExecutionResources, Grounder, Interruption, SolveConfig};
use zetesis_cpu::{Cancellation, Stop};
use zetesis_themelios::{
    AdmissionOptions, ExpansionLimits, FormulaLimits, FormulaMaterialization, prepare_formula,
};

#[test]
fn consumed_base_answer_cannot_escape_a_cancelled_reconstruction() {
    let FormulaMaterialization::Terminal(owner) = prepare_formula(
        "seed(1). receipt(X):-seed(X).".into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap()
    .ground_adaptive()
    .unwrap() else {
        panic!("a terminal definition is required");
    };
    let config = SolveConfig {
        backend: Backend::Cpu,
        grounder: Grounder::Auto,
        models: 0,
        ..SolveConfig::default()
    };
    let cancellation = Cancellation::default();
    let phases = Recorder::new(true);
    let resources = ExecutionResources::default();
    let mut session = TerminalSession::new(
        &owner,
        &config,
        &resources,
        &mut Ignore,
        &cancellation,
        &phases,
    )
    .unwrap();
    // Advance the actual membership engine, then invoke the same qualification
    // stage as next(). This fixes the cancellation boundary without a time race
    // or a fabricated completed base certificate.
    let consumed = session.statistics.base_answers.checked_add(1).unwrap();
    let (model, _) = session
        .base
        .next(&session.base_config, &mut Ignore, &cancellation, &phases)
        .unwrap()
        .unwrap();
    cancellation.cancel();
    assert!(
        session
            .reconstruct(&model, consumed, &cancellation, &phases)
            .is_none()
    );
    let outcome = session.outcome(&phases);
    assert_eq!(outcome.verified_models(), 0);
    assert_eq!(outcome.completion(), Some(Completion::Interrupted));
    assert_eq!(
        outcome.interruption(),
        Some(Interruption::Reconstruction(Stop::Cancelled))
    );
    let receipt = outcome.terminal_execution().unwrap();
    assert_eq!(
        (receipt.base_answers, receipt.reconstructed, receipt.pending),
        (1, 0, 1)
    );
    assert_eq!(receipt.reconstruction.completed, 0);
    assert_eq!(
        phases
            .snapshot()
            .unwrap()
            .get(crate::SolvePhase::AnswerReconstruction)
            .unwrap()
            .calls,
        1
    );
    assert!(
        session
            .next(&config, &mut Ignore, &cancellation, &phases)
            .is_none()
    );
    assert_eq!(session.outcome(&phases).terminal_execution(), Some(receipt));
}
