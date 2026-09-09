//! Physical formula execution preserves unrestricted family and capture evidence.
#![cfg(feature = "gpu")]

use std::{collections::BTreeSet, num::NonZeroUsize};

use zetesis_cli::{
    AnswerSelection, AnswerSet, Backend, Completion, Grounder, Oracle, PreparedInput, SolveConfig,
    WorldView, WorldViewError, WorldViewLimits,
};
use zetesis_cpu::Control;
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionLimits, FormulaLimits, admit_formula,
};

fn formula(source: &str) -> AdmittedFormula {
    admit_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap()
}

fn config() -> SolveConfig {
    SolveConfig {
        backend: Backend::Metal,
        oracle: Oracle::Countermodel,
        grounder: Grounder::Eager,
        models: 0,
        batch_size: NonZeroUsize::new(4).unwrap(),
        max_optimal_models: 0,
        max_optimal_atoms: 0,
        max_optimal_bytes: 0,
        ..Default::default()
    }
}

fn record(answer: &AnswerSet) -> (Vec<String>, Vec<(i32, i64)>) {
    // These fixtures use positive nullary atoms, so names retain full identity.
    let atoms = answer
        .interpretation()
        .atoms()
        .iter()
        .map(|atom| atom.predicate().name().to_owned())
        .collect();
    let score = answer.score().expect("the original objective is active");
    assert!(score.is_present());
    (atoms, score.costs().to_vec())
}

#[test]
#[ignore = "requires actual Metal; unrestricted collection never substitutes CPU"]
fn metal_world_view_preserves_nonoptimal_answers() {
    let owner = formula("1 {a;b} 1. #minimize {1@2,a:a; 2@2,b:b}. #show.");
    let world_view = WorldView::collect(
        PreparedInput::formula(&owner),
        config(),
        WorldViewLimits::default(),
        Control::default(),
    )
    .unwrap();
    let answers: BTreeSet<_> = world_view.answer_sets().iter().map(record).collect();
    assert_eq!(
        answers,
        BTreeSet::from([
            (vec!["a".into()], vec![(2, 1)]),
            (vec!["b".into()], vec![(2, 2)]),
        ])
    );
    assert_eq!(world_view.len(), 2);
    let outcome = world_view.outcome();
    assert_eq!(outcome.selection(), Some(AnswerSelection::All));
    assert_eq!(outcome.completion(), Some(Completion::Exhausted));
    assert_eq!(outcome.verified_models(), 2);
    assert_eq!(outcome.scored_models(), 2);
    assert_eq!(outcome.retained_models(), 0);
    assert!(!outcome.optimum_proved());
    assert_eq!(
        outcome
            .countermodel_statistics()
            .unwrap()
            .candidate_restrictions,
        0
    );
    let execution = outcome
        .formula_execution()
        .expect("actual formula device execution");
    assert!(execution.adapter.contains("Metal"), "{}", execution.adapter);
    assert!(execution.gpu_batches > 0);
    assert!(execution.gpu_work > 0);
    assert_eq!(execution.gpu_candidates, 2);
    assert_eq!(execution.gpu_decided + execution.cpu_residuals, 2);
    assert_eq!(
        (execution.pending_candidates, execution.queued_models),
        (0, 0)
    );
}

#[test]
#[ignore = "requires actual Metal; collection refusal preserves completed and queued work"]
fn metal_collection_limit_retains_checked_accounting() {
    let owner = formula("{a;b}. #minimize {1,a:a; 2,b:b}.");
    let failure = WorldView::collect(
        PreparedInput::formula(&owner),
        config(),
        WorldViewLimits {
            max_answer_sets: 1,
            ..Default::default()
        },
        Control::default(),
    )
    .unwrap_err();
    assert!(
        matches!(failure.cause(), WorldViewError::AnswerSets),
        "{failure:?}"
    );
    assert_eq!(failure.answer_sets().len(), 1);
    let expected = BTreeSet::from([
        (vec![], vec![(0, 0)]),
        (vec!["a".into()], vec![(0, 1)]),
        (vec!["b".into()], vec![(0, 2)]),
        (vec!["a".into(), "b".into()], vec![(0, 3)]),
    ]);
    assert!(expected.contains(&record(&failure.answer_sets()[0])));
    assert!(
        failure.answer_sets()[0]
            .subject()
            .same_instance(failure.subject())
    );
    let outcome = failure.outcome().unwrap();
    assert_eq!(outcome.selection(), Some(AnswerSelection::All));
    assert_eq!(outcome.completion(), None);
    assert_eq!(outcome.verified_models(), 4);
    assert_eq!(outcome.scored_models(), 2);
    assert_eq!(outcome.retained_models(), 0);
    assert!(!outcome.unsatisfiable());
    assert!(!outcome.optimum_proved());
    let execution = outcome
        .formula_execution()
        .expect("actual formula device execution");
    assert!(execution.adapter.contains("Metal"), "{}", execution.adapter);
    assert_eq!(execution.gpu_batches, 1);
    assert!(execution.gpu_work > 0);
    assert_eq!(execution.gpu_candidates, 4);
    assert_eq!(execution.gpu_decided + execution.cpu_residuals, 4);
    assert_eq!(
        (execution.pending_candidates, execution.queued_models),
        (0, 2)
    );
}
