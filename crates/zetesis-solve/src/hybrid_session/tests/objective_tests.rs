//! Final source failures retain only already accepted objective evidence.

use super::*;
use crate::hybrid_session::accept;
use zetesis_core::Model;

fn owner() -> HybridFormula {
    prepare_formula(
        include_str!("../../../tests/fixtures/hybrid-objectives/stopped-incumbent.lp").into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap()
    .ground_hybrid()
    .unwrap()
}

fn config() -> SolveConfig {
    SolveConfig {
        backend: Backend::Cpu,
        grounder: Grounder::Lazy,
        search: SearchMethod::Clauses,
        workers: std::num::NonZeroUsize::MIN,
        models: 0,
        ..SolveConfig::default()
    }
}

#[test]
fn a_source_stop_retains_the_earlier_incumbent() {
    let owner = owner();
    let config = config();
    let cancellation = Cancellation::default();
    let phases = Recorder::new(false);
    let mut session = HybridSession::new(
        super::super::HybridInput {
            core: owner.core(),
            subject: crate::Subject::Hybrid(owner.clone()),
        },
        &config,
        &ExecutionResources::default(),
        &mut Ignore,
        &cancellation,
        &phases,
        AnswerSelection::Optimal,
    )
    .unwrap();
    let source_control = Cancellation::default();
    let error = {
        let HybridSession {
            core,
            checker,
            statistics,
            ..
        } = &mut session;
        core.next_with_acceptance(&config, &mut Ignore, &cancellation, &phases, &mut |model| {
            // Select the actual source-check boundary without racing the
            // native producer or cancelling its separate control token.
            if statistics.accepted == 1 {
                source_control.cancel();
            }
            accept(checker, statistics, model, &source_control, &phases)
        })
        .unwrap()
        .unwrap_err()
    };
    assert!(!session.core.finished());
    session.finish(None, Some(error), &phases).unwrap();
    let before = session.outcome(&phases);
    assert_eq!(
        before.interruption(),
        Some(Interruption::Constraint(Stop::Cancelled))
    );
    assert_eq!(before.verified_models(), 1);
    assert_eq!(before.scored_models(), 1);
    assert_eq!(before.retained_models(), 1);
    assert!(!before.optimum_proved());
    let receipt = *before.hybrid_execution().unwrap();
    assert_eq!(
        (receipt.core_answers, receipt.accepted, receipt.pending),
        (2, 1, 1)
    );
    assert_eq!(
        session.conclude(None, &phases).unwrap(),
        before.search_state()
    );
    assert_eq!(
        session.conclude(None, &phases).unwrap(),
        before.search_state()
    );
    assert_eq!(session.outcome(&phases).hybrid_execution(), Some(&receipt));
    session.core.stop(&phases).unwrap();
    session.core.conclude(before.search_state(), &phases);
    assert_eq!(session.core.outcome(&phases).retained_models(), 1);
    let answer = session
        .next(&config, &mut Ignore, &cancellation, &phases)
        .unwrap()
        .unwrap();
    assert_eq!(answer.1.unwrap().costs(), [(0, 0)]);
    assert!(
        session
            .next(&config, &mut Ignore, &cancellation, &phases)
            .is_none()
    );
    assert_eq!(session.outcome(&phases).hybrid_execution(), Some(&receipt));
    assert_eq!(session.outcome(&phases).retained_models(), 1);
}

#[test]
fn a_source_fault_does_not_publish_retained_answers() {
    let owner = owner();
    let config = config();
    let cancellation = Cancellation::default();
    let phases = Recorder::new(false);
    let mut session = HybridSession::new(
        super::super::HybridInput {
            core: owner.core(),
            subject: crate::Subject::Hybrid(owner.clone()),
        },
        &config,
        &ExecutionResources::default(),
        &mut Ignore,
        &cancellation,
        &phases,
        AnswerSelection::Optimal,
    )
    .unwrap();
    let error =
        {
            let HybridSession {
                core,
                checker,
                statistics,
                ..
            } = &mut session;
            core.next_with_acceptance(&config, &mut Ignore, &cancellation, &phases, &mut |model| {
                if statistics.accepted == 0 {
                    accept(checker, statistics, model, &cancellation, &phases)
                } else {
                    // Structural equality cannot authenticate a fresh catalog.
                    let foreign =
                        Model::new(model.atoms().iter().map(|atom| {
                            atom.to_atom(zetesis_core::ValueLimits::default()).unwrap()
                        }))
                        .unwrap();
                    accept(checker, statistics, &foreign, &cancellation, &phases)
                }
            })
            .unwrap()
            .unwrap_err()
        };
    assert!(matches!(session.finish(None, Some(error), &phases),
        Err(SolveError::Constraint(error)) if matches!(error.cause, ConstraintCheckCause::WrongProgram)));
    let outcome = session.outcome(&phases);
    assert_eq!(outcome.completion(), None);
    assert_eq!(outcome.verified_models(), 1);
    assert_eq!(outcome.scored_models(), 1);
    assert_eq!(outcome.retained_models(), 1);
    assert_eq!(outcome.hybrid_execution().unwrap().pending, 1);
    assert!(
        session
            .next(&config, &mut Ignore, &cancellation, &phases)
            .is_none()
    );
}
