//! Candidate-only signed normalization against independently enumerated keys.

use std::cmp::Ordering;
use std::collections::BTreeSet;

use zetesis_core::{Atom, AtomPattern, Model, Predicate, Term, Value};
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{AggregateErrorKind, AggregateLimits, Interpretation, Node, Theory, models};
use zetesis_objective::{ObjectiveProgram, ObjectiveTemplate, Score, evaluate};
use zetesis_sat::StableModels;
use zetesis_themelios::objective_bound::{
    ObjectiveBound, ObjectiveBoundErrorKind as Kind, ObjectiveBoundLimits, ObjectiveBoundResource,
    ObjectivePlan, ObjectivePlanLimits,
};

// Priority, signed weight, full one-scalar tuple, condition atom. Repeated first
// three fields are a single key with OR eligibility, regardless of source order.
type Entry = (i32, i32, i32, usize);

struct Fixture {
    atoms: Vec<Atom>,
    original: Theory,
    objectives: ObjectiveProgram,
    entries: Vec<Entry>,
    plan: ObjectivePlan,
}

impl Fixture {
    fn new(atom_count: usize, entries: Vec<Entry>) -> Self {
        let atoms: Vec<_> = (0..atom_count)
            .map(|index| {
                Atom::new(Predicate::new(format!("p{index}"), 0).unwrap(), vec![]).unwrap()
            })
            .collect();
        let mut nodes = vec![Node::False];
        let mut roots = Vec::new();
        for index in 0..atom_count {
            let atom = nodes.len();
            nodes.push(Node::Atom(index));
            nodes.push(Node::Implies(atom, 0));
            roots.push(nodes.len());
            nodes.push(Node::Or(atom, atom + 1));
        }
        let original = Theory::new(
            atom_count,
            nodes,
            roots,
            zetesis_ferraris::AdmissionLimits::default(),
        )
        .unwrap();
        let objectives = ObjectiveProgram::new(
            entries
                .iter()
                .map(|&(priority, weight, tuple, atom)| {
                    ObjectiveTemplate::new(
                        Term::Constant(Value::Number(weight)),
                        priority,
                        vec![Term::Constant(Value::Number(tuple))],
                        vec![AtomPattern::new(atoms[atom].predicate().clone(), vec![]).unwrap()],
                        vec![],
                    )
                })
                .collect(),
            zetesis_objective::AdmissionLimits::default(),
        )
        .unwrap();
        let catalog = zetesis_core::AtomCatalog::new(atoms.clone()).unwrap();
        let plan = ObjectivePlan::new(
            &original,
            catalog.atoms(),
            &objectives,
            ObjectivePlanLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
        Self {
            atoms,
            original,
            objectives,
            entries,
            plan,
        }
    }

    fn score(&self, mask: usize) -> Score {
        let model = Model::new(
            self.atoms
                .iter()
                .enumerate()
                .filter(|(index, _)| mask & (1 << index) != 0)
                .map(|(_, atom)| atom.clone()),
        )
        .unwrap();
        let score = evaluate(
            &self.objectives,
            &model,
            zetesis_objective::Limits::default(),
            &Cancellation::default(),
        )
        .unwrap()
        .score()
        .clone();
        let active: BTreeSet<_> = self
            .entries
            .iter()
            .filter(|(_, _, _, atom)| mask & (1 << atom) != 0)
            .map(|&(priority, weight, tuple, _)| (priority, weight, tuple))
            .collect();
        let priorities: BTreeSet<_> = self.entries.iter().map(|entry| entry.0).collect();
        let expected: Vec<_> = priorities
            .into_iter()
            .rev()
            .map(|priority| {
                (
                    priority,
                    active
                        .iter()
                        .filter(|entry| entry.0 == priority)
                        .map(|entry| i64::from(entry.1))
                        .sum::<i64>(),
                )
            })
            .collect();
        assert_eq!(score.costs(), expected);
        score
    }

    fn compare_all(&self, limits: ObjectiveBoundLimits) {
        let scores: Vec<_> = (0..1 << self.atoms.len())
            .map(|mask| self.score(mask))
            .collect();
        for incumbent in &scores {
            let bound = self
                .plan
                .bound(incumbent, limits, &Cancellation::default())
                .unwrap();
            assert!(bound.original().same_instance(&self.original));
            for (mask, score) in scores.iter().enumerate() {
                assert_eq!(
                    permits(&bound, mask),
                    score.compare_costs(incumbent) != Ordering::Greater,
                    "mask={mask}, incumbent={:?}",
                    incumbent.costs()
                );
            }
        }
    }
}

fn permits(bound: &ObjectiveBound, mask: usize) -> bool {
    let candidate = Interpretation::new(
        bound.theory(),
        (0..bound.theory().atom_count()).filter(|index| mask & (1 << index) != 0),
    )
    .unwrap();
    models(
        bound.theory(),
        &candidate,
        zetesis_ferraris::Limits::default(),
        &Cancellation::default(),
    )
    .unwrap()
}

fn no_subsets() -> ObjectiveBoundLimits {
    ObjectiveBoundLimits {
        aggregate: AggregateLimits {
            max_subsets: 0,
            ..AggregateLimits::default()
        },
        ..ObjectiveBoundLimits::default()
    }
}

#[test]
fn every_interpretation_retains_global_key_coalescing_priorities_and_ties() {
    let fixture = Fixture::new(
        4,
        vec![
            (7, -2, 0, 0),
            (7, -2, 0, 1), // OR alternatives, charged once.
            (7, -2, 1, 0), // Different tuple, same weight/condition: charged again.
            (7, 2, 0, 0),  // Opposite signed key remains distinct.
            (7, -1, 2, 2),
            (0, -1, 0, 3),
            (0, 1, 0, 0),
            (3, 0, 0, 1),
        ],
    );
    fixture.compare_all(no_subsets());
    // Raw -2 alternatives must not create two offsets; all transformed entries
    // must not be coalesced merely because their new magnitude is equal.
    assert_eq!(fixture.score(1).costs(), [(7, -2), (3, 0), (0, 1)]);
    assert_eq!(fixture.score(2).costs(), [(7, -2), (3, 0), (0, 0)]);
}

#[test]
fn tiny_large_magnitude_families_keep_the_bounded_signed_representation() {
    for weights in [[i32::MIN, 1], [-i32::MAX, i32::MAX], [-100_000, 99_999]] {
        let fixture = Fixture::new(2, vec![(0, weights[0], 0, 0), (0, weights[1], 1, 1)]);
        let limits = ObjectiveBoundLimits {
            aggregate: AggregateLimits {
                max_states: 2,
                max_subsets: 8,
                ..AggregateLimits::default()
            },
            max_work: 1_000,
        };
        fixture.compare_all(limits);
        let error = fixture
            .plan
            .bound(&fixture.score(0), no_subsets(), &Cancellation::default())
            .unwrap_err();
        assert!(
            matches!(error.kind(), Kind::Aggregate(error) if error.kind() == AggregateErrorKind::SubsetLimit)
        );
        fixture.compare_all(limits); // An optional refusal cannot poison the plan.
    }
}

fn many_negative() -> Fixture {
    Fixture::new(
        20,
        (0..20)
            .map(|index| (0, -1, i32::try_from(index).unwrap(), index))
            .collect(),
    )
}

#[test]
fn a_linear_threshold_family_avoids_exponential_subset_admission() {
    let fixture = many_negative();
    let incumbent = fixture.score((1 << 20) - 2); // Cost -19: at least 19 selected.
    let limits = ObjectiveBoundLimits {
        aggregate: AggregateLimits {
            max_states: 6,
            max_nodes: 256,
            max_subsets: 0,
            ..AggregateLimits::default()
        },
        max_work: 1_000,
    };
    let bound = fixture
        .plan
        .bound(&incumbent, limits, &Cancellation::default())
        .unwrap();
    assert!(bound.statistics().work <= limits.max_work);
    assert!(bound.theory().nodes().len() <= limits.aggregate.max_nodes);
    for mask in [
        0,
        1,
        (1 << 18) - 1,
        (1 << 19) - 1,
        (1 << 20) - 2,
        (1 << 20) - 1,
    ] {
        assert_eq!(permits(&bound, mask), mask.count_ones() >= 19);
    }
}

#[test]
fn a_subset_count_beyond_machine_width_does_not_disable_small_thresholds() {
    let fixture = Fixture::new(1, (0..130).map(|tuple| (0, -1, tuple, 0)).collect());
    let bound = fixture
        .plan
        .bound(&fixture.score(1), no_subsets(), &Cancellation::default())
        .unwrap();
    assert!(!permits(&bound, 0));
    assert!(permits(&bound, 1));
    assert!(bound.theory().nodes().len() < 1_000);
}

#[test]
fn outside_range_ceilings_need_no_threshold_states_and_keep_exact_truth() {
    let fixture = Fixture::new(2, vec![(0, -2, 0, 0), (0, 1, 1, 1)]);
    for (ceiling, expected) in [(-3, false), (2, true)] {
        let constant = ObjectiveProgram::new(
            vec![ObjectiveTemplate::new(
                Term::Constant(Value::Number(ceiling)),
                0,
                vec![],
                vec![],
                vec![],
            )],
            zetesis_objective::AdmissionLimits::default(),
        )
        .unwrap();
        let model = Model::new([]).unwrap();
        let incumbent = evaluate(
            &constant,
            &model,
            zetesis_objective::Limits::default(),
            &Cancellation::default(),
        )
        .unwrap();
        let limits = ObjectiveBoundLimits {
            aggregate: AggregateLimits {
                max_states: 0,
                ..no_subsets().aggregate
            },
            ..no_subsets()
        };
        let bound = fixture
            .plan
            .bound(incumbent.score(), limits, &Cancellation::default())
            .unwrap();
        for mask in 0..4 {
            assert_eq!(permits(&bound, mask), expected);
        }
    }
}

#[test]
fn normalization_limits_are_inclusive_and_failed_attempts_leave_exact_retry() {
    let fixture = many_negative();
    let incumbent = fixture.score((1 << 20) - 2);
    let complete = fixture
        .plan
        .bound(&incumbent, no_subsets(), &Cancellation::default())
        .unwrap();
    let nodes = complete.theory().nodes().to_vec();
    let roots = complete.theory().roots().to_vec();
    let exact = ObjectiveBoundLimits {
        max_work: complete.statistics().work,
        aggregate: AggregateLimits {
            max_elements: 20,
            max_nodes: nodes.len(),
            max_states: 6,
            max_subsets: 0,
            ..AggregateLimits::default()
        },
    };
    let retry = fixture
        .plan
        .bound(&incumbent, exact, &Cancellation::default())
        .unwrap();
    assert_eq!(retry.statistics(), complete.statistics());
    assert_eq!(retry.theory().nodes(), nodes);
    assert_eq!(retry.theory().roots(), roots);
    for limits in [
        ObjectiveBoundLimits {
            max_work: exact.max_work - 1,
            ..exact
        },
        ObjectiveBoundLimits {
            max_work: 0,
            ..exact
        },
        ObjectiveBoundLimits {
            aggregate: AggregateLimits {
                max_elements: 19,
                ..exact.aggregate
            },
            ..exact
        },
        ObjectiveBoundLimits {
            aggregate: AggregateLimits {
                max_nodes: nodes.len() - 1,
                ..exact.aggregate
            },
            ..exact
        },
        ObjectiveBoundLimits {
            aggregate: AggregateLimits {
                max_states: 5,
                ..exact.aggregate
            },
            ..exact
        },
    ] {
        let error = fixture
            .plan
            .bound(&incumbent, limits, &Cancellation::default())
            .unwrap_err();
        assert!(error.statistics().work <= limits.max_work);
        assert!(matches!(
            error.kind(),
            Kind::Aggregate(_)
                | Kind::Limit(ObjectiveBoundResource::Work | ObjectiveBoundResource::Nodes)
        ));
    }
    let cancellation = Cancellation::default();
    cancellation.cancel();
    assert!(matches!(
        fixture
            .plan
            .bound(&incumbent, exact, &cancellation)
            .unwrap_err()
            .kind(),
        Kind::Control(_)
    ));
    let retry = fixture
        .plan
        .bound(&incumbent, exact, &Cancellation::default())
        .unwrap();
    assert_eq!(retry.theory().nodes(), nodes);
    assert_eq!(retry.theory().roots(), roots);
}

#[test]
fn nonstrict_normalized_bound_preserves_all_original_stable_optimal_ties() {
    let fixture = Fixture::new(3, vec![(0, -2, 0, 0), (0, -2, 0, 1), (0, 1, 1, 2)]);
    let original_nodes = fixture.original.nodes().to_vec();
    let incumbent = fixture.score(1);
    let bound = fixture
        .plan
        .bound(&incumbent, no_subsets(), &Cancellation::default())
        .unwrap();
    let mut search = StableModels::new(
        &fixture.original,
        zetesis_sat::Limits::default(),
        Cancellation::default(),
    )
    .unwrap();
    search.restrict_candidates(bound.theory()).unwrap();
    let actual: BTreeSet<_> = search
        .by_ref()
        .map(|model| model.unwrap().atoms().collect::<Vec<_>>())
        .collect();
    assert!(search.exhausted());
    assert_eq!(actual, BTreeSet::from([vec![0], vec![1], vec![0, 1]]));
    assert_eq!(fixture.original.nodes(), original_nodes);
    assert!(bound.original().same_instance(&fixture.original));
}
