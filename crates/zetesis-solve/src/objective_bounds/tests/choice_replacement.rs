//! A failed optional strengthening preserves optimal ties after an earlier
//! strengthened bound has already narrowed the original search.

use std::cmp::Ordering;
use std::collections::BTreeSet;

use super::Bounds;
use crate::execution_observation::Observer;
use crate::{ExecutionObservation, ExecutionObserver, SolveConfig};
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{Interpretation, Theory};
use zetesis_objective::Score;
use zetesis_reference_support::formula;
use zetesis_sat::{Limits, SearchMethod, StableModels};
use zetesis_themelios::AdmittedFormula;
use zetesis_themelios::objective_bound::{
    ObjectiveBoundErrorKind, ObjectiveBoundLimits, ObjectiveBoundResource, ObjectivePlan,
    ObjectivePlanLimits,
};

fn named(owner: &AdmittedFormula, name: &str) -> Interpretation {
    let atom = owner
        .atoms()
        .iter()
        .position(|atom| atom.predicate().name() == name)
        .unwrap();
    Interpretation::new(owner.theory(), [atom]).unwrap()
}

fn score(plan: &ObjectivePlan, candidate: &Interpretation) -> Score {
    plan.score(
        candidate,
        zetesis_objective::Limits::default(),
        &Cancellation::default(),
    )
    .unwrap()
    .unwrap()
    .into_score()
}

fn holds(theory: &Theory, candidate: &Interpretation) -> bool {
    let candidate = Interpretation::new(theory, candidate.atoms()).unwrap();
    zetesis_ferraris::models(
        theory,
        &candidate,
        zetesis_ferraris::Limits::default(),
        &Cancellation::default(),
    )
    .unwrap()
}

struct Prepared {
    plan: ObjectivePlan,
    earlier: Score,
    later: Score,
    work: u64,
}

fn prepared(owner: &AdmittedFormula) -> Prepared {
    let cancellation = Cancellation::default();
    let mut plan = ObjectivePlan::new(
        owner.theory(),
        owner.atoms(),
        owner.objectives(),
        ObjectivePlanLimits::default(),
        &cancellation,
    )
    .unwrap();
    let earlier = named(owner, "b");
    let later = named(owner, "a");
    for incumbent in [&earlier, &later] {
        assert!(
            zetesis_sat::check(owner.theory(), incumbent, Limits::default(), &cancellation)
                .accepted()
        );
    }
    let earlier = score(&plan, &earlier);
    let later = score(&plan, &later);
    assert_eq!(earlier.costs(), &[(0, 4)]);
    assert_eq!(later.costs(), &[(0, 3)]);
    let exact = plan
        .bound(&later, ObjectiveBoundLimits::default(), &cancellation)
        .unwrap();
    plan.prepare_choice_bounds(
        owner.required_choices().unwrap(),
        ObjectivePlanLimits::default(),
        u128::MAX,
        &cancellation,
    )
    .unwrap();
    assert_eq!(plan.choice_bound_groups(), 1);
    let strengthened = plan
        .bound(&earlier, ObjectiveBoundLimits::default(), &cancellation)
        .unwrap();
    assert!(strengthened.choice_failure().is_none());

    // The exact cost-3 bound does not imply the old required-cost bound on
    // arbitrary interpretations: {e} pays 3, but leaves the required group
    // unpaid. Its lower cost is 3 + 3, above the old incumbent's cost 4.
    let outside_original = named(owner, "e");
    assert!(!holds(owner.theory(), &outside_original));
    assert!(holds(exact.theory(), &outside_original));
    assert!(!holds(strengthened.theory(), &outside_original));
    Prepared {
        plan,
        earlier,
        later,
        work: strengthened.statistics().work + exact.statistics().work,
    }
}

#[derive(Default)]
struct Observations {
    installed: usize,
    refusals: Vec<ObjectiveBoundErrorKind>,
}

impl ExecutionObserver for Observations {
    type Error = std::convert::Infallible;

    fn observe(&mut self, event: ExecutionObservation<'_>) -> Result<(), Self::Error> {
        match event {
            ExecutionObservation::ObjectiveBound { .. } => self.installed += 1,
            ExecutionObservation::ChoiceObjectiveBoundUnavailable(error) => {
                self.refusals.push(error.kind());
            }
            _ => panic!("both exact bounds must install; only the later grouping may stop"),
        }
        Ok(())
    }
}

fn retain_optimal(
    prepared: &Prepared,
    candidate: &Interpretation,
    answers: &mut BTreeSet<Vec<usize>>,
) {
    let cost = score(&prepared.plan, candidate);
    assert_ne!(cost.compare_costs(&prepared.earlier), Ordering::Greater);
    if cost.compare_costs(&prepared.later) == Ordering::Equal {
        assert!(answers.insert(candidate.atoms().collect()));
    }
}

#[test]
fn exact_fallback_after_strengthening_retains_optima() {
    let owner = formula(include_str!(
        "../../../tests/fixtures/choice-bound-replacement.lp"
    ));
    let prepared = prepared(&owner);
    let expected = BTreeSet::from([
        named(&owner, "a").atoms().collect::<Vec<_>>(),
        named(&owner, "c").atoms().collect::<Vec<_>>(),
    ]);
    for method in [SearchMethod::Regions, SearchMethod::Clauses] {
        let options = SolveConfig {
            max_objective_bound_work: prepared.work,
            models: 0,
            ..Default::default()
        };
        let cancellation = Cancellation::default();
        let mut models = StableModels::with_method(
            owner.theory(),
            method,
            Limits::default(),
            cancellation.clone(),
        )
        .unwrap();
        let mut bounds = Bounds::new(&options);
        let mut observations = Observations::default();
        bounds
            .improve(
                Some(&prepared.plan),
                &prepared.earlier,
                &mut models,
                &options,
                &mut Observer(&mut observations),
                &cancellation,
            )
            .unwrap();
        assert_eq!(observations.installed, 1);
        assert!(observations.refusals.is_empty());
        let first = models.next().unwrap().unwrap();
        let mut answers = BTreeSet::new();
        retain_optimal(&prepared, &first, &mut answers);

        // The remaining allowance admits exact construction but no optional
        // strengthening. Replacing the live bound must retain prior decisions
        // without treating them as knowledge about the replacement DAG.
        bounds
            .improve(
                Some(&prepared.plan),
                &prepared.later,
                &mut models,
                &options,
                &mut Observer(&mut observations),
                &cancellation,
            )
            .unwrap();
        assert!(bounds.enabled);
        assert_eq!(bounds.work, options.max_objective_bound_work);
        assert_eq!(observations.installed, 2);
        assert_eq!(
            observations.refusals,
            [ObjectiveBoundErrorKind::Limit(ObjectiveBoundResource::Work)]
        );
        for candidate in models.by_ref() {
            retain_optimal(&prepared, &candidate.unwrap(), &mut answers);
        }
        assert!(models.exhausted());
        assert!(models.theory().same_instance(owner.theory()));
        assert_eq!(models.statistics().candidate_restrictions, 2);
        assert_eq!(answers, expected);
        assert_eq!(
            models.statistics().candidate_queries == 0,
            method == SearchMethod::Regions
        );
    }
}
