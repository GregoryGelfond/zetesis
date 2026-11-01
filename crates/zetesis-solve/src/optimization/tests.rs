//! Retention failure preserves the last completely admitted incumbent.

use zetesis_algebra::Cancellation;
use zetesis_core::Model;
use zetesis_reference_support::formula;
use zetesis_themelios::AdmittedFormula;

use super::{Incumbents, OptimizationStop, ScoreValue};
use crate::{Backend, Interruption, PreparedInput, Session, SolveConfig};

fn fixture() -> (AdmittedFormula, Model, Model) {
    checked_answers("a | b. c :- b. #minimize {2,k:a;1,k:b}.")
}

fn checked_answers(source: &str) -> (AdmittedFormula, Model, Model) {
    let owner = formula(source);
    // Use genuinely checked answers; order the two verified models explicitly
    // so this retention test does not assume a SAT enumeration order.
    let mut answers = Session::enumerate(
        PreparedInput::formula(&owner),
        config(),
        Cancellation::default(),
    )
    .unwrap()
    .map(|answer| answer.unwrap().into_interpretation())
    .collect::<Vec<_>>();
    answers.sort_by_key(|model| model.atoms().len());
    assert_eq!(
        answers.iter().map(|m| m.atoms().len()).collect::<Vec<_>>(),
        [1, 2]
    );
    let second = answers.pop().unwrap();
    let first = answers.pop().unwrap();
    (owner, first, second)
}

fn config() -> SolveConfig {
    SolveConfig {
        backend: Backend::Cpu,
        models: 0,
        ..SolveConfig::default()
    }
}

#[test]
fn refused_improvement_preserves_the_old_incumbent() {
    let (owner, first, second) = fixture();
    // Three nullary catalog records, the larger two-position selection, and
    // one score record. This refuses the improvement by exactly one byte.
    let required = 8 + 3 * 18 + 8 + 2 * 8 + 21;
    let options = SolveConfig {
        max_optimal_bytes: required - 1,
        ..config()
    };
    let mut retained = Incumbents::default();
    assert!(
        retained
            .consider(
                owner.objectives(),
                first.clone(),
                &options,
                &Cancellation::default(),
                None,
            )
            .unwrap()
    );
    let before = retained.retention.payload();
    assert!(matches!(
        retained.consider(
            owner.objectives(),
            second,
            &options,
            &Cancellation::default(),
            None,
        ),
        Err(Interruption::Incumbent(OptimizationStop::Bytes)),
    ));
    assert_eq!(retained.models, [first]);
    assert_eq!(retained.retention.payload(), before);
    let best = retained.metadata().unwrap();
    assert_eq!(best.score.costs(), [(0, 2)]);
    assert_eq!(best.tied_models, 1);
    assert_eq!(best.scored_models, 2);
}

#[test]
fn improvement_replaces_the_complete_charge() {
    let (owner, first, second) = fixture();
    let required = 8 + 3 * 18 + 8 + 2 * 8 + 21;
    let options = SolveConfig {
        max_optimal_bytes: required,
        ..config()
    };
    let mut retained = Incumbents::default();
    retained
        .consider(
            owner.objectives(),
            first,
            &options,
            &Cancellation::default(),
            None,
        )
        .unwrap();
    assert!(
        retained
            .consider(
                owner.objectives(),
                second.clone(),
                &options,
                &Cancellation::default(),
                None,
            )
            .unwrap()
    );
    assert_eq!(retained.models, [second]);
    assert_eq!(retained.retention.payload().bytes, required);
    assert_eq!(retained.retention.payload().catalogs, 1);
    assert_eq!(retained.retention.payload().associated_bytes, 21);
    let best = retained.metadata().unwrap();
    assert_eq!(best.score.costs(), [(0, 1)]);
    assert_eq!(best.tied_models, 1);
    assert_eq!(best.scored_models, 2);
}

#[test]
fn scored_overflow_preserves_the_admitted_incumbent_and_its_count() {
    let (owner, first, second) = fixture();
    let options = config();
    let mut retained = Incumbents::default();
    retained
        .consider(
            owner.objectives(),
            first.clone(),
            &options,
            &Cancellation::default(),
            None,
        )
        .unwrap();
    let before = retained.retention.payload();
    retained.scored = u64::MAX;
    assert!(matches!(
        retained.consider(
            owner.objectives(),
            second,
            &options,
            &Cancellation::default(),
            None,
        ),
        Err(Interruption::Incumbent(OptimizationStop::Overflow))
    ));
    assert_eq!(retained.scored, u64::MAX);
    assert_eq!(retained.models, [first]);
    assert_eq!(retained.retention.payload(), before);
    let best = retained.metadata().unwrap();
    assert_eq!(best.score.costs(), [(0, 2)]);
    assert_eq!(best.tied_models, 1);
    assert_eq!(best.scored_models, 1);
}

#[test]
fn tie_overflow_precedes_retention_and_keeps_the_last_exact_count() {
    let (owner, first, second) = checked_answers("a | b. c :- b. #minimize {1,k:a;1,k:b}.");
    let options = SolveConfig {
        max_optimal_models: 1,
        ..config()
    };
    let mut retained = Incumbents::default();
    retained
        .consider(
            owner.objectives(),
            first.clone(),
            &options,
            &Cancellation::default(),
            None,
        )
        .unwrap();
    let before = retained.retention.payload();
    retained.best.as_mut().unwrap().tied_models = u64::MAX;
    assert!(matches!(
        retained.consider(
            owner.objectives(),
            second,
            &options,
            &Cancellation::default(),
            None,
        ),
        Err(Interruption::Incumbent(OptimizationStop::Overflow))
    ));
    assert_eq!(retained.models, [first]);
    assert_eq!(retained.retention.payload(), before);
    let best = retained.metadata().unwrap();
    assert_eq!(best.tied_models, u64::MAX);
    assert_eq!(best.scored_models, 2);
}

#[test]
fn a_tie_is_counted_before_its_retention_is_refused() {
    let (owner, first, second) = checked_answers("a | b. c :- b. #minimize {1,k:a;1,k:b}.");
    let options = SolveConfig {
        max_optimal_models: 1,
        ..config()
    };
    let mut retained = Incumbents::default();
    retained
        .consider(
            owner.objectives(),
            first.clone(),
            &options,
            &Cancellation::default(),
            None,
        )
        .unwrap();
    assert!(matches!(
        retained.consider(
            owner.objectives(),
            second,
            &options,
            &Cancellation::default(),
            None,
        ),
        Err(Interruption::Incumbent(OptimizationStop::Models))
    ));
    assert_eq!(retained.models, [first]);
    let best = retained.metadata().unwrap();
    assert_eq!(best.tied_models, 2);
    assert_eq!(best.scored_models, 2);
}

mod prepared;

#[test]
fn semantic_cost_values_preserve_zero_slots_and_objective_presence() {
    let program = zetesis_objective::ObjectiveProgram::none();
    let model = Model::default();
    let absent = zetesis_objective::evaluate(
        &program,
        &model,
        zetesis_objective::Limits::default(),
        &Cancellation::default(),
    )
    .unwrap()
    .into_score();
    let present = zetesis_objective::Score::from_costs(Vec::new()).unwrap();
    let zero_slots = zetesis_objective::Score::from_costs(vec![(3, 0), (-2, 0)]).unwrap();
    assert!(!absent.is_present());
    assert!(present.is_present());
    assert_ne!(absent, present);
    assert_ne!(present, zero_slots);
    assert_eq!(ScoreValue(&absent), ScoreValue(&present));
    assert_eq!(ScoreValue(&present), ScoreValue(&zero_slots));
    let minimum = zetesis_algebra::reduction::Minimum::new();
    let tied = minimum.step(Some(ScoreValue(&zero_slots)), ScoreValue(&present));
    assert_eq!(tied.incoming, std::cmp::Ordering::Equal);
    assert!(std::ptr::eq(tied.value.0, &raw const zero_slots));
    assert_eq!(tied.value.0.costs(), [(3, 0), (-2, 0)]);
    assert_eq!(
        super::score_payload_bytes(Some(tied.value.0)).unwrap(),
        super::score_payload_bytes(Some(&zero_slots)).unwrap()
    );
}

#[test]
fn semantic_cost_order_uses_priorities_missing_zeros_and_numeric_extremes() {
    use std::cmp::Ordering;
    use zetesis_objective::Score;

    let highest = Score::from_costs(vec![(i32::MAX, i64::MIN), (i32::MIN, i64::MAX)]).unwrap();
    let lower_priority = Score::from_costs(vec![(i32::MIN, i64::MIN)]).unwrap();
    let zero = Score::from_costs(Vec::new()).unwrap();
    let positive = Score::from_costs(vec![(i32::MIN, i64::MAX)]).unwrap();
    let cases = [
        (&highest, &lower_priority, Ordering::Less),
        (&lower_priority, &zero, Ordering::Less),
        (&zero, &positive, Ordering::Less),
    ];
    for (left, right, expected) in cases {
        assert_eq!(ScoreValue(left).cmp(&ScoreValue(right)), expected);
        assert_eq!(ScoreValue(right).cmp(&ScoreValue(left)), expected.reverse());
        let selected = zetesis_algebra::reduction::Minimum::new()
            .step(Some(ScoreValue(right)), ScoreValue(left));
        assert_eq!(selected.incoming, expected);
        assert!(std::ptr::eq(selected.value.0, left));
    }
}
