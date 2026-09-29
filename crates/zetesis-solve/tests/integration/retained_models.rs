//! Retained answers share immutable catalogs without changing logical outcomes.

use std::{collections::BTreeSet, num::NonZeroUsize};

use crate::support::models::atom;
use zetesis_core::{Model, Sign, Value};
use zetesis_cpu::Cancellation;
use zetesis_reference_support::formula;
use zetesis_solve::{
    Backend, Completion, Interruption, OptimizationStop, Oracle, PreparedInput, Session,
    SolveConfig,
};

fn config() -> SolveConfig {
    SolveConfig {
        backend: Backend::Cpu,
        oracle: Oracle::Countermodel,
        models: 0,
        batch_size: NonZeroUsize::new(3).unwrap(),
        ..SolveConfig::default()
    }
}

#[test]
fn formula_answers_retain_the_original_catalog() {
    let (answers, original_catalog) = {
        let owner = formula("tag(\"shared\").{p(1);p(\"1\");-q(1)}.");
        let original = owner.atom_catalog().clone();
        let mut session = Session::enumerate(
            PreparedInput::formula(&owner),
            config(),
            Cancellation::default(),
        )
        .unwrap();
        let answers = session.by_ref().collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(
            session.outcome().unwrap().completion(),
            Some(Completion::Exhausted)
        );
        assert_eq!(session.outcome().unwrap().verified_models(), 8);
        (answers, original)
    };
    for answer in &answers {
        assert!(
            answer
                .interpretation()
                .catalog()
                .same_owner(&original_catalog)
        );
    }
    // The returned models now retain the catalog without the source owner or
    // this independent identity witness keeping it alive.
    drop(original_catalog);
    let choices = [
        atom("p", Sign::Positive, vec![Value::Number(1)]),
        atom("p", Sign::Positive, vec![Value::String("1".into())]),
        atom("q", Sign::Negative, vec![Value::Number(1)]),
    ];
    let fact = atom("tag", Sign::Positive, vec![Value::String("shared".into())]);
    let expected: BTreeSet<_> = (0..8_usize)
        .map(|mask| {
            Model::new(
                std::iter::once(fact.clone()).chain(
                    choices
                        .iter()
                        .enumerate()
                        .filter(|(id, _)| mask & (1 << id) != 0)
                        .map(|(_, atom)| atom.clone()),
                ),
            )
            .unwrap()
        })
        .collect();
    let actual: BTreeSet<_> = answers
        .iter()
        .map(|answer| answer.interpretation().clone())
        .collect();
    assert_eq!(actual, expected);
}

#[test]
fn optimum_bytes_include_unselected_catalog_payload() {
    let hidden = "x".repeat(8_192);
    let owner = formula(&format!(
        "a | hidden(\"{hidden}\"). :-hidden(\"{hidden}\"). #minimize{{0:a}}."
    ));
    let a = atom("a", Sign::Positive, vec![]);
    let selected = owner.atoms().iter().position(|atom| atom == a).unwrap();
    let model = Model::from_positions(owner.atom_catalog(), [selected]).unwrap();
    assert!(
        owner
            .atoms()
            .iter()
            .any(|atom| atom.predicate().name() == "hidden")
    );
    // One best score, held beside the shared model catalog: option tag1,
    // priority-count8, then one i32/i64 priority/cost pair12.
    let required = model.retained_payload_bytes().unwrap() + 21;
    assert!(required > hidden.len());
    assert!(
        required
            > Model::new([a.clone()])
                .unwrap()
                .retained_payload_bytes()
                .unwrap()
    );

    let mut exact = Session::new(
        PreparedInput::formula(&owner),
        SolveConfig {
            max_optimal_bytes: required,
            ..config()
        },
        Cancellation::default(),
    )
    .unwrap();
    let answers = exact.by_ref().collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(answers.len(), 1);
    assert_eq!(answers[0].interpretation(), &Model::new([a]).unwrap());
    assert!(exact.outcome().unwrap().optimum_proved());

    let mut short = Session::new(
        PreparedInput::formula(&owner),
        SolveConfig {
            max_optimal_bytes: required - 1,
            ..config()
        },
        Cancellation::default(),
    )
    .unwrap();
    assert!(short.next().is_none());
    let outcome = short.outcome().unwrap();
    assert_eq!(outcome.completion(), Some(Completion::Interrupted));
    assert!(matches!(
        outcome.interruption(),
        Some(Interruption::Incumbent(OptimizationStop::Bytes))
    ));
    assert!(!outcome.unsatisfiable());
    assert!(!outcome.optimum_proved());
}
