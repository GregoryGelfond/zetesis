//! Lazy original acceptance shares the ordinary objective and retention semantics.

use std::{collections::BTreeMap, convert::Infallible, num::NonZeroUsize};

use zetesis_core::Model;
use zetesis_cpu::Cancellation;
use zetesis_objective::Score;
use zetesis_solve::{
    AnswerSelection, Backend, Completion, ExecutionObservation, ExecutionObserver, Grounder,
    Interruption, OptimizationStop, PreparedInput, SearchMethod, SemanticOutcome, Session,
    SolveConfig,
};
use zetesis_test_support::programs::model;
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionLimits, FormulaLimits, HybridFormula,
    admit_formula, prepare_formula,
};

const PRIORITIES: &str = include_str!("../fixtures/hybrid-objectives/priorities.lp");
const FALLBACK: &str = include_str!("../fixtures/hybrid-objectives/fallback.lp");
const REQUIRED: &str = include_str!("../fixtures/hybrid-objectives/required.lp");
const INCONSISTENT: &str = include_str!("../fixtures/hybrid-objectives/inconsistent.lp");

type Family = BTreeMap<Model, Option<Score>>;

fn config() -> SolveConfig {
    SolveConfig {
        backend: Backend::Cpu,
        grounder: Grounder::Lazy,
        workers: NonZeroUsize::MIN,
        completion_workers: NonZeroUsize::MIN,
        batch_size: NonZeroUsize::MIN,
        models: 0,
        ..SolveConfig::default()
    }
}

fn hybrid(source: &str) -> HybridFormula {
    prepare_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap()
    .ground_hybrid()
    .unwrap()
}

fn eager(source: &str) -> AdmittedFormula {
    admit_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap()
}

fn capture(
    input: PreparedInput<'_>,
    config: SolveConfig,
    selection: AnswerSelection,
) -> (Family, SemanticOutcome) {
    let mut session = Session::builder(input, config, Cancellation::default())
        .selection(selection)
        .start()
        .unwrap();
    let mut family = Family::new();
    for answer in session.by_ref() {
        let answer = answer.unwrap();
        assert!(
            family
                .insert(answer.interpretation().clone(), answer.score().cloned())
                .is_none()
        );
    }
    assert!(session.next().is_none());
    (family, session.outcome().unwrap())
}

fn routes() -> impl Iterator<Item = SolveConfig> {
    [
        (SearchMethod::Clauses, 1),
        (SearchMethod::Regions, 1),
        (SearchMethod::Regions, 4),
    ]
    .into_iter()
    .flat_map(|(search, workers)| {
        [0, 1_000_000].map(|max_objective_bound_work| SolveConfig {
            search,
            workers: NonZeroUsize::new(workers).unwrap(),
            max_objective_bound_work,
            ..config()
        })
    })
}

#[test]
fn lazy_objectives_preserve_complete_scored_answers() {
    let original = eager(PRIORITIES);
    let owner = hybrid(PRIORITIES);
    let (expected, _) = capture(
        PreparedInput::formula(&original),
        SolveConfig {
            grounder: Grounder::Eager,
            ..config()
        },
        AnswerSelection::All,
    );
    assert_eq!(expected.len(), 6);
    // The same priority/weight/tuple key occurs three times across both forms.
    assert_eq!(
        expected[&model(&["a"])].as_ref().unwrap().costs(),
        [(2, 2), (1, 0)]
    );
    for config in routes() {
        let (actual, outcome) =
            capture(PreparedInput::hybrid(&owner), config, AnswerSelection::All);
        assert_eq!(actual, expected);
        assert_eq!(outcome.completion(), Some(Completion::Exhausted));
        assert_eq!(outcome.verified_models(), 6);
        assert_eq!(outcome.scored_models(), 6);
        assert!(outcome.incumbent().is_none());
    }
}

#[test]
fn lazy_objectives_preserve_every_optimal_tie() {
    let original = eager(PRIORITIES);
    let owner = hybrid(PRIORITIES);
    let (expected, _) = capture(
        PreparedInput::formula(&original),
        SolveConfig {
            grounder: Grounder::Eager,
            ..config()
        },
        AnswerSelection::Optimal,
    );
    assert_eq!(
        expected.keys().cloned().collect::<Vec<_>>(),
        vec![model(&["b"]), model(&["b", "hidden"])]
    );
    for config in routes() {
        let (actual, outcome) = capture(
            PreparedInput::hybrid(&owner),
            config,
            AnswerSelection::Optimal,
        );
        assert_eq!(actual, expected);
        assert!(outcome.optimum_proved());
        assert_eq!(
            outcome.incumbent().unwrap().score.costs(),
            [(2, 0), (1, -3)]
        );
        assert_eq!(outcome.incumbent().unwrap().tied_models, 2);
        assert_eq!(outcome.retained_models(), 2);
        let counts = outcome.hybrid_execution().unwrap();
        assert_eq!(
            counts.core_answers,
            counts.accepted + counts.rejected + counts.pending
        );
        assert_eq!(counts.accepted, outcome.verified_models());
        assert_eq!(counts.pending, 0);
    }
}

#[test]
fn requested_optimal_models_limit_delivery() {
    let owner = hybrid(PRIORITIES);
    for config in routes() {
        let (answers, outcome) = capture(
            PreparedInput::hybrid(&owner),
            SolveConfig {
                models: 1,
                ..config
            },
            AnswerSelection::Optimal,
        );
        assert_eq!(answers.len(), 1);
        assert!(outcome.optimum_proved());
        assert_eq!(
            outcome.incumbent().unwrap().score.costs(),
            [(2, 0), (1, -3)]
        );
        assert_eq!(outcome.incumbent().unwrap().tied_models, 2);
        assert_eq!(outcome.retained_models(), 1);
    }
}

#[test]
fn requested_all_models_count_accepted_answers() {
    let owner = hybrid(REQUIRED);
    let (answers, outcome) = capture(
        PreparedInput::hybrid(&owner),
        SolveConfig {
            search: SearchMethod::Clauses,
            models: 1,
            ..config()
        },
        AnswerSelection::All,
    );
    assert_eq!(answers.len(), 1);
    assert_eq!(
        answers.values().next().unwrap().as_ref().unwrap().costs(),
        [(0, 1)]
    );
    assert_eq!(outcome.completion(), Some(Completion::RequestedModels));
    assert_eq!(outcome.verified_models(), 1);
    assert_eq!(outcome.scored_models(), 1);
    assert_eq!(outcome.hybrid_execution().unwrap().accepted, 1);
}

#[test]
fn lazy_objectives_preserve_admitted_presence() {
    for (source, present, costs) in [
        (
            include_str!("../fixtures/hybrid-objectives/positive-unit.lp"),
            false,
            vec![],
        ),
        // Empty source declarations contribute no admitted objective template.
        (
            include_str!("../fixtures/hybrid-objectives/absent-empty.lp"),
            false,
            vec![],
        ),
        (
            include_str!("../fixtures/hybrid-objectives/present-zero.lp"),
            true,
            vec![(4, 0)],
        ),
    ] {
        let owner = hybrid(source);
        let original = eager(source);
        assert_eq!(owner.objectives().is_present(), present, "{source}");
        assert_eq!(original.objectives().is_present(), present, "{source}");
        for selection in [AnswerSelection::All, AnswerSelection::Optimal] {
            let (expected, _) = capture(
                PreparedInput::formula(&original),
                SolveConfig {
                    grounder: Grounder::Eager,
                    ..config()
                },
                selection,
            );
            let (answers, outcome) = capture(PreparedInput::hybrid(&owner), config(), selection);
            assert_eq!(answers, expected, "{source}");
            let score = answers[&model(&[])].as_ref();
            assert_eq!(score.is_some(), present);
            if let Some(score) = score {
                assert!(score.is_present());
                assert_eq!(score.costs(), costs);
            }
            assert_eq!(
                outcome.selection(),
                Some(if present {
                    selection
                } else {
                    AnswerSelection::All
                })
            );
            assert_eq!(
                outcome.optimum_proved(),
                present && selection == AnswerSelection::Optimal
            );
        }
    }
}

#[test]
fn rejected_core_exhaustion_proves_original_inconsistency() {
    let owner = hybrid(INCONSISTENT);
    let (answers, outcome) = capture(
        PreparedInput::hybrid(&owner),
        SolveConfig {
            search: SearchMethod::Clauses,
            ..config()
        },
        AnswerSelection::Optimal,
    );
    assert!(answers.is_empty());
    assert!(outcome.unsatisfiable());
    assert_eq!(outcome.scored_models(), 0);
    assert!(outcome.incumbent().is_none());
    let receipt = outcome.hybrid_execution().unwrap();
    assert_eq!(receipt.core_answers, 2);
    assert_eq!(receipt.rejected, 2);
    assert_eq!(receipt.pending, 0);
}

#[test]
fn a_refused_score_keeps_original_membership() {
    let owner = hybrid(REQUIRED);
    let (answers, outcome) = capture(
        PreparedInput::hybrid(&owner),
        SolveConfig {
            max_objective_work: 0,
            ..config()
        },
        AnswerSelection::Optimal,
    );
    assert!(answers.is_empty());
    assert!(matches!(
        outcome.interruption(),
        Some(Interruption::Objective(_))
    ));
    assert_eq!(outcome.verified_models(), 1);
    assert_eq!(outcome.scored_models(), 0);
    assert_eq!(outcome.retained_models(), 0);
    let receipt = outcome.hybrid_execution().unwrap();
    assert_eq!(receipt.accepted, 1);
    assert_eq!(receipt.pending, 0);
}

#[test]
fn refused_retention_keeps_completed_scoring() {
    let owner = hybrid(REQUIRED);
    let (answers, outcome) = capture(
        PreparedInput::hybrid(&owner),
        SolveConfig {
            max_optimal_models: 0,
            ..config()
        },
        AnswerSelection::Optimal,
    );
    assert!(answers.is_empty());
    assert_eq!(
        outcome.interruption(),
        Some(Interruption::Incumbent(OptimizationStop::Models))
    );
    assert_eq!(outcome.verified_models(), 1);
    assert_eq!(outcome.scored_models(), 1);
    assert_eq!(outcome.retained_models(), 0);
    assert!(outcome.incumbent().is_none());
}

#[derive(Default)]
struct PlanRefusals(Vec<zetesis_themelios::objective_bound::ObjectiveBoundErrorKind>);

impl ExecutionObserver for PlanRefusals {
    type Error = Infallible;

    fn observe(&mut self, event: ExecutionObservation<'_>) -> Result<(), Self::Error> {
        if let ExecutionObservation::ObjectiveUnavailable(error) = event {
            self.0.push(error.kind());
        }
        Ok(())
    }
}

#[test]
fn refused_preparation_preserves_lazy_scored_answers() {
    use zetesis_themelios::objective_bound::{ObjectiveBoundErrorKind, ObjectiveBoundResource};

    let owner = hybrid(FALLBACK);
    let original = eager(FALLBACK);
    let (expected, _) = capture(
        PreparedInput::formula(&original),
        SolveConfig {
            grounder: Grounder::Eager,
            ..config()
        },
        AnswerSelection::All,
    );
    let (prepared, _) = capture(
        PreparedInput::hybrid(&owner),
        config(),
        AnswerSelection::All,
    );
    assert_eq!(prepared, expected);
    assert_eq!(expected.len(), 3);
    for (atoms, cost) in [(vec![], 0), (vec!["a"], 1), (vec!["b"], 2)] {
        assert_eq!(
            expected[&model(&atoms)].as_ref().unwrap().costs(),
            [(0, cost)]
        );
    }
    // The possible relation has two keys; each original answer has at most
    // one. Rejected core {a,b} must never enter the fallback evaluator.
    let mut observations = PlanRefusals::default();
    let mut session = Session::builder(
        PreparedInput::hybrid(&owner),
        SolveConfig {
            search: SearchMethod::Clauses,
            max_objective_keys: 1,
            ..config()
        },
        Cancellation::default(),
    )
    .selection(AnswerSelection::All)
    .start_observed(&mut observations)
    .unwrap();
    let mut actual = Family::new();
    while let Some(answer) = session.next_observed(&mut observations) {
        let answer = answer.unwrap();
        assert!(
            actual
                .insert(answer.interpretation().clone(), answer.score().cloned())
                .is_none()
        );
    }
    assert_eq!(
        observations.0,
        [ObjectiveBoundErrorKind::Limit(ObjectiveBoundResource::Keys)]
    );
    assert_eq!(actual, expected);
    let outcome = session.outcome().unwrap();
    assert_eq!(outcome.completion(), Some(Completion::Exhausted));
    assert_eq!(outcome.scored_models(), 3);
    assert_eq!(outcome.hybrid_execution().unwrap().rejected, 1);
}
