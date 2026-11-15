//! Every admitted source owner selects projection without losing exact families.

use crate::support::models::ScoredFamily;
use std::{convert::Infallible, num::NonZeroUsize};
use zetesis_algebra::Cancellation;
use zetesis_reference_support::{BundleFixture, formula, hybrid};
use zetesis_solve::{
    AnswerSelection, Backend, Completion, ExecutionObservation, ExecutionObserver, Grounder,
    PreparedInput, Session, SolveConfig,
};
use zetesis_themelios::objective_bound::{
    ObjectiveBoundErrorKind, ObjectiveBoundResource, ObjectiveProjectionError,
    ObjectiveProjectionErrorKind,
};
use zetesis_themelios::{BundleAdmissionOptions, ExpansionLimits, FormulaLimits};

#[derive(Default)]
struct Refusals {
    projected: Vec<ObjectiveProjectionError>,
    legacy: usize,
}
impl ExecutionObserver for Refusals {
    type Error = Infallible;
    fn observe(&mut self, event: ExecutionObservation<'_>) -> Result<(), Self::Error> {
        match event {
            ExecutionObservation::ObjectiveProjectionUnavailable(error) => {
                self.projected.push(error);
            }
            ExecutionObservation::ObjectiveUnavailable(_) => self.legacy += 1,
            _ => {}
        }
        Ok(())
    }
}

fn config(grounder: Grounder) -> SolveConfig {
    SolveConfig {
        backend: Backend::Cpu,
        grounder,
        workers: NonZeroUsize::MIN,
        completion_workers: NonZeroUsize::MIN,
        batch_size: NonZeroUsize::MIN,
        models: 0,
        max_objective_bound_work: 0,
        ..Default::default()
    }
}

#[test]
fn all_admitted_owners_preserve_families_after_projection_refusal() {
    let source = include_str!("../fixtures/hybrid-objectives/fallback.lp");
    let eager = formula(source);
    let lazy = hybrid(source);
    let fixture = BundleFixture::new();
    fixture.write("entry.lp", "#include \"body.lp\".");
    fixture.write("body.lp", source);
    let bundle = zetesis_themelios::admit_bundle_formula(
        fixture.bundle(),
        BundleAdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    let expected: ScoredFamily = [(&[][..], 0), (&["a"][..], 1), (&["b"][..], 2)]
        .into_iter()
        .map(|(names, cost)| {
            let model = zetesis_test_support::programs::model(names);
            let score = zetesis_objective::evaluate(
                eager.objectives(),
                &model,
                zetesis_objective::Limits::default(),
                &Cancellation::default(),
            )
            .unwrap();
            assert_eq!(score.score().costs(), [(0, cost)]);
            let score = score.into_score();
            (model, Some(score))
        })
        .collect();
    for (input, grounder, objectives) in [
        (
            PreparedInput::formula(&eager),
            Grounder::Eager,
            eager.objectives(),
        ),
        (
            PreparedInput::formula_bundle(&bundle),
            Grounder::Eager,
            bundle.objectives(),
        ),
        (
            PreparedInput::hybrid(&lazy),
            Grounder::Lazy,
            lazy.objectives(),
        ),
    ] {
        let mut observations = Refusals::default();
        let mut session = Session::builder(
            input,
            SolveConfig {
                max_objective_keys: 1,
                ..config(grounder)
            },
            Cancellation::default(),
        )
        .selection(AnswerSelection::All)
        .start_observed(&mut observations)
        .unwrap();
        let mut actual = ScoredFamily::new();
        let mut scoring_work = 0;
        while let Some(answer) = session.next_observed(&mut observations) {
            let answer = answer.unwrap();
            // Equivalent admitted owners can retain different objective
            // representations. Recompute this owner's exact fallback work.
            let score = zetesis_objective::evaluate(
                objectives,
                answer.interpretation(),
                zetesis_objective::Limits::default(),
                &Cancellation::default(),
            )
            .unwrap();
            scoring_work += score.statistics().work;
            assert!(
                actual
                    .insert(answer.interpretation().clone(), answer.score().cloned())
                    .is_none()
            );
        }
        assert_eq!(observations.legacy, 0);
        let [refusal] = observations.projected.as_slice() else {
            panic!("one projected preparation attempt")
        };
        assert_eq!(
            refusal.kind(),
            &ObjectiveProjectionErrorKind::Existing(ObjectiveBoundErrorKind::Limit(
                ObjectiveBoundResource::Keys
            ))
        );
        assert!(refusal.statistics().work > 0);
        assert_eq!(actual, expected);
        let outcome = session.outcome().unwrap();
        assert_eq!(outcome.completion(), Some(Completion::Exhausted));
        assert_eq!(outcome.scored_models(), 3);
        assert_eq!(
            outcome.objective_work(),
            refusal.statistics().work + scoring_work
        );
    }
}

#[test]
fn projected_preparation_preserves_every_optimal_tie() {
    let source = "1 {a;b} 1. {hidden}. :- a, not not b. #minimize {1,a:a;1,b:b}.";
    let eager = formula(source);
    let lazy = hybrid(source);
    let expected: std::collections::BTreeSet<_> = [
        vec!["a"],
        vec!["b"],
        vec!["a", "hidden"],
        vec!["b", "hidden"],
    ]
    .iter()
    .map(|names| zetesis_test_support::programs::model(names))
    .collect();
    for (input, grounder) in [
        (PreparedInput::formula(&eager), Grounder::Eager),
        (PreparedInput::hybrid(&lazy), Grounder::Lazy),
    ] {
        let mut observations = Refusals::default();
        let mut session = Session::builder(
            input,
            SolveConfig {
                max_objective_bound_work: SolveConfig::default().max_objective_bound_work,
                ..config(grounder)
            },
            Cancellation::default(),
        )
        .selection(AnswerSelection::Optimal)
        .start_observed(&mut observations)
        .unwrap();
        let mut actual = std::collections::BTreeSet::new();
        while let Some(answer) = session.next_observed(&mut observations) {
            let answer = answer.unwrap();
            assert_eq!(answer.score().unwrap().costs(), [(0, 1)]);
            assert!(actual.insert(answer.interpretation().clone()));
        }
        assert!(observations.projected.is_empty());
        assert_eq!(observations.legacy, 0);
        assert_eq!(actual, expected);
        let outcome = session.outcome().unwrap();
        assert!(outcome.optimum_proved());
        assert_eq!(outcome.incumbent().unwrap().tied_models, 4);
        assert_eq!(outcome.completion(), Some(Completion::Exhausted));
    }
}

#[derive(Default)]
struct ChoiceUse {
    refusals: Refusals,
    choice_refusals: usize,
    installed: usize,
}

impl ExecutionObserver for ChoiceUse {
    type Error = Infallible;

    fn observe(&mut self, event: ExecutionObservation<'_>) -> Result<(), Self::Error> {
        match &event {
            ExecutionObservation::ChoiceObjectiveBoundUnavailable(_) => self.choice_refusals += 1,
            ExecutionObservation::ObjectiveBound { .. } => self.installed += 1,
            _ => {}
        }
        self.refusals.observe(event)
    }
}

fn assert_sparse_choice_bound(eager: &zetesis_themelios::AdmittedFormula) {
    use zetesis_ferraris::NodeView;
    use zetesis_themelios::objective_bound::{
        ObjectiveBoundLimits, ObjectivePlan, ObjectivePlanLimits,
    };

    let selected: Vec<_> = (0..eager.atom_catalog().atoms().len())
        .filter(|&position| {
            matches!(
                eager
                    .atom_catalog()
                    .atoms()
                    .at(position)
                    .unwrap()
                    .predicate()
                    .name(),
                "z_a" | "z_b" | "z_c"
            )
        })
        .collect();
    assert_eq!(selected.len(), 3);
    assert!(selected.iter().any(|&position| position >= selected.len()));
    let cancellation = Cancellation::default();
    let mut plan = ObjectivePlan::new_projected(
        eager.objective_source(),
        ObjectivePlanLimits {
            max_atoms: selected.len(),
            ..ObjectivePlanLimits::default()
        },
        u128::MAX,
        &cancellation,
    )
    .into_parts()
    .0
    .unwrap();
    let incumbent = zetesis_objective::Score::from_costs(vec![(0, 1)]).unwrap();
    let exact = plan
        .bound(&incumbent, ObjectiveBoundLimits::default(), &cancellation)
        .unwrap();
    for (node, &position) in selected.iter().enumerate() {
        assert_eq!(
            exact.theory().view().node(node),
            Ok(NodeView::Atom(position))
        );
    }
    let preparation = plan
        .prepare_choice_bounds(
            eager.required_choices().expect("admitted mandatory choice"),
            ObjectivePlanLimits::default(),
            u128::MAX,
            &cancellation,
        )
        .unwrap();
    assert!(preparation.work > 0);
    assert_eq!(plan.choice_bound_groups(), 1);
    let strengthened = plan
        .bound(&incumbent, ObjectiveBoundLimits::default(), &cancellation)
        .unwrap();
    assert!(strengthened.choice_failure().is_none());
    assert!(strengthened.theory().view().len() > exact.theory().view().len());
    assert!(strengthened.statistics().work > exact.statistics().work);
}

#[test]
fn sparse_choice_bounds_preserve_every_optimal_tie() {
    // Unread facts separate compact objective-node positions from semantic atom
    // positions. The third alternative supplies a distinct, nonminimum tier.
    let source = include_str!("../fixtures/projected-choice-layers.lp");
    let eager = formula(source);
    let lazy = hybrid(source);
    assert_sparse_choice_bound(&eager);

    let facts: Vec<_> = (1..=16)
        .map(|value| zetesis_test_support::programs::unary("noise", value))
        .collect();
    let expected: std::collections::BTreeSet<_> = [
        &["z_a"][..],
        &["z_b"][..],
        &["z_a", "z_hidden"][..],
        &["z_b", "z_hidden"][..],
    ]
    .into_iter()
    .map(|choices| {
        zetesis_core::Model::new(
            facts.iter().cloned().chain(
                choices
                    .iter()
                    .map(|name| zetesis_test_support::programs::nullary(name)),
            ),
        )
        .unwrap()
    })
    .collect();
    for (input, grounder) in [
        (PreparedInput::formula(&eager), Grounder::Eager),
        (PreparedInput::hybrid(&lazy), Grounder::Lazy),
    ] {
        let mut observations = ChoiceUse::default();
        let mut session = Session::builder(
            input,
            SolveConfig {
                max_objective_bound_work: SolveConfig::default().max_objective_bound_work,
                ..config(grounder)
            },
            Cancellation::default(),
        )
        .selection(AnswerSelection::Optimal)
        .start_observed(&mut observations)
        .unwrap();
        let mut actual = std::collections::BTreeSet::new();
        while let Some(answer) = session.next_observed(&mut observations) {
            let answer = answer.unwrap();
            assert_eq!(answer.score().unwrap().costs(), [(0, 1)]);
            assert!(actual.insert(answer.interpretation().clone()));
        }
        assert!(observations.refusals.projected.is_empty());
        assert_eq!(observations.refusals.legacy, 0);
        assert_eq!(observations.choice_refusals, 0);
        assert!(observations.installed > 0);
        assert_eq!(actual, expected);
        let outcome = session.outcome().unwrap();
        assert!(outcome.optimum_proved());
        assert_eq!(outcome.incumbent().unwrap().tied_models, 4);
        assert_eq!(outcome.completion(), Some(Completion::Exhausted));
    }
}
