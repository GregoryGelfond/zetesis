//! Actual collection and optimum consumers share canonical catalog accounting.

use std::{collections::BTreeSet, num::NonZeroUsize};
use zetesis_reference_support::formula;

use zetesis_algebra::Cancellation;
use zetesis_core::{Atom, Model, Predicate};
use zetesis_solve::{
    AnswerSet, Backend, Completion, Interruption, OptimizationStop, Oracle, PreparedInput, Session,
    SolveConfig, WorldView, WorldViewError, WorldViewLimits,
};
use zetesis_themelios::AdmittedFormula;

fn config() -> SolveConfig {
    SolveConfig {
        backend: Backend::Cpu,
        oracle: Oracle::Countermodel,
        models: 0,
        batch_size: NonZeroUsize::MIN,
        workers: NonZeroUsize::MIN,
        ..SolveConfig::default()
    }
}

fn fixture() -> AdmittedFormula {
    formula("{a;b}. #minimize {0@3,k:a}. #show.")
}

fn expected() -> BTreeSet<Model> {
    let atom = |name| Atom::new(Predicate::new(name, 0).unwrap(), vec![]).unwrap();
    BTreeSet::from([
        Model::new([]).unwrap(),
        Model::new([atom("a")]).unwrap(),
        Model::new([atom("b")]).unwrap(),
        Model::new([atom("a"), atom("b")]).unwrap(),
    ])
}

fn records(answers: &[AnswerSet]) -> Vec<(Model, Vec<(i32, i64)>)> {
    answers
        .iter()
        .map(|answer| {
            (
                answer.interpretation().clone(),
                answer.score().unwrap().costs().to_vec(),
            )
        })
        .collect()
}

fn limits(max_bytes: usize) -> WorldViewLimits {
    WorldViewLimits {
        max_bytes,
        ..WorldViewLimits::default()
    }
}

// Two nullary catalog atoms:8+18+18. Four selections:8+16+16+24.
// The complete family has four independent optional score records of21 bytes.
const CATALOG_BYTES: usize = 44;
const SELECTION_BYTES: usize = 64;
const SCORE_BYTES: usize = 21;

#[test]
fn complete_world_view_charges_one_shared_catalog() {
    let owner = fixture();
    let view = WorldView::collect(
        PreparedInput::formula(&owner),
        config(),
        limits(CATALOG_BYTES + SELECTION_BYTES + 4 * SCORE_BYTES),
        Cancellation::default(),
    )
    .unwrap();
    assert_eq!(
        view.answer_sets()
            .iter()
            .map(|answer| answer.interpretation().clone())
            .collect::<BTreeSet<_>>(),
        expected()
    );
    for answer in view.answer_sets() {
        assert!(
            answer
                .interpretation()
                .catalog()
                .same_owner(owner.atom_catalog())
        );
        assert_eq!(answer.score().unwrap().costs(), [(3, 0)]);
    }
    assert_eq!(view.outcome().completion(), Some(Completion::Exhausted));
    assert!(view.outcome().countermodel_statistics().is_some());
}

#[test]
fn world_view_byte_refusal_preserves_the_checked_prefix() {
    let owner = fixture();
    let complete = WorldView::collect(
        PreparedInput::formula(&owner),
        config(),
        WorldViewLimits::default(),
        Cancellation::default(),
    )
    .unwrap();
    let prefix = &complete.answer_sets()[..2];
    let selected = prefix
        .iter()
        .map(|a| 8 + 8 * a.interpretation().atoms().len())
        .sum::<usize>();
    let failure = WorldView::collect(
        PreparedInput::formula(&owner),
        config(),
        limits(CATALOG_BYTES + selected + 2 * SCORE_BYTES),
        Cancellation::default(),
    )
    .unwrap_err();
    assert!(matches!(failure.cause(), WorldViewError::Bytes));
    assert_eq!(records(failure.answer_sets()), records(prefix));
    assert_eq!(failure.outcome().unwrap().completion(), None);
    assert!(!failure.outcome().unwrap().unsatisfiable());
    for answer in failure.answer_sets() {
        assert!(answer.subject().same_instance(failure.subject()));
    }
}

#[test]
fn optimum_ties_share_one_catalog_and_one_score() {
    let owner = fixture();
    let mut session = Session::new(
        PreparedInput::formula(&owner),
        SolveConfig {
            max_optimal_bytes: CATALOG_BYTES + SELECTION_BYTES + SCORE_BYTES,
            ..config()
        },
        Cancellation::default(),
    )
    .unwrap();
    let answers = session.by_ref().collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(
        answers
            .iter()
            .map(|answer| answer.interpretation().clone())
            .collect::<BTreeSet<_>>(),
        expected()
    );
    for answer in &answers {
        assert!(
            answer
                .interpretation()
                .catalog()
                .same_owner(owner.atom_catalog())
        );
        assert_eq!(answer.score().unwrap().costs(), [(3, 0)]);
    }
    let outcome = session.outcome().unwrap();
    assert!(outcome.optimum_proved());
    assert_eq!(outcome.incumbent().unwrap().tied_models, 4);
    assert_eq!(outcome.retained_models(), 4);
}

#[test]
fn short_optimum_budget_preserves_verified_ties() {
    let owner = fixture();
    let complete = Session::new(
        PreparedInput::formula(&owner),
        config(),
        Cancellation::default(),
    )
    .unwrap()
    .collect::<Result<Vec<_>, _>>()
    .unwrap();
    let mut session = Session::new(
        PreparedInput::formula(&owner),
        SolveConfig {
            max_optimal_bytes: CATALOG_BYTES + SELECTION_BYTES + SCORE_BYTES - 1,
            ..config()
        },
        Cancellation::default(),
    )
    .unwrap();
    let answers = session.by_ref().collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(records(&answers), records(&complete[..3]));
    let outcome = session.outcome().unwrap();
    assert_eq!(outcome.completion(), Some(Completion::Interrupted));
    assert!(matches!(
        outcome.interruption(),
        Some(Interruption::Incumbent(OptimizationStop::Bytes))
    ));
    assert_eq!(outcome.incumbent().unwrap().tied_models, 4);
    assert_eq!(outcome.retained_models(), 3);
    assert!(!outcome.optimum_proved());
}
