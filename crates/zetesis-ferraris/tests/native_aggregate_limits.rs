//! Ownership, identity, occurrence and bounded-work contracts for native groups.

use std::time::Instant;

use proptest::prelude::*;
use zetesis_core::Value as Term;
use zetesis_cpu::{Cancellation, Stop};
use zetesis_ferraris::{
    AdmissionLimits, AggregateComparison as Comparison, Interpretation, Node, Theory,
    native_aggregate::{
        self as native, Bound, ErrorKind, Function, Group, Guard, Phase, Resource, Tuple,
    },
};

fn theory() -> Theory {
    Theory::new(
        1,
        vec![
            Node::False,
            Node::Atom(0),
            Node::Implies(1, 0),
            Node::Implies(2, 0),
        ],
        vec![],
        AdmissionLimits::default(),
    )
    .unwrap()
}
fn tuples() -> Vec<Tuple> {
    vec![
        Tuple {
            key: vec![Term::Number(3), Term::Symbol("b".into())],
            condition: 1,
        },
        Tuple {
            key: vec![Term::Number(-2), Term::Symbol("a".into())],
            condition: 2,
        },
        Tuple {
            key: vec![Term::Number(0), Term::Symbol("c".into())],
            condition: 3,
        },
    ]
}
fn guards() -> Vec<Guard> {
    vec![
        Guard {
            comparison: Comparison::Ge,
            bound: Bound::Integer(0),
        },
        Guard {
            comparison: Comparison::Lt,
            bound: Bound::Term(Term::Symbol("z".into())),
        },
    ]
}
fn group(theory: &Theory) -> Group {
    Group::new(
        theory,
        Function::Sum,
        tuples(),
        guards(),
        native::AdmissionLimits::default(),
        &Cancellation::default(),
    )
    .unwrap()
}

proptest! {
    #[test]
    fn admission_recognizes_whole_key_equivalence(
        keys in prop::collection::vec(prop::collection::vec(-3_i32..4, 0..4), 0..25),
    ) {
        // The bounded quadratic reference is independent of the admitted
        // group's index sort and includes empty, prefix and equal-first keys.
        let duplicate = keys.iter().enumerate().any(|(index, key)| {
            keys.iter().skip(index + 1).any(|other| key == other)
        });
        let tuples = keys.iter().map(|key| Tuple {
            key: key.iter().copied().map(Term::Number).collect(),
            condition: 1,
        }).collect();
        let result = Group::new(
            &theory(), Function::Count, tuples, vec![],
            native::AdmissionLimits::default(), &Cancellation::default(),
        );
        match result {
            Ok(group) => {
                prop_assert!(!duplicate);
                let retained: Vec<_> = group.tuples().iter().map(|tuple| {
                    tuple.key.iter().map(|value| match value {
                        Term::Number(value) => *value,
                        _ => unreachable!("integer fixture"),
                    }).collect::<Vec<_>>()
                }).collect();
                prop_assert_eq!(retained, keys);
            }
            Err(error) => {
                prop_assert!(duplicate);
                let ErrorKind::Duplicate { first, second } = error.kind() else {
                    prop_assert!(false, "unexpected refusal: {error}");
                    return Ok(());
                };
                prop_assert!(first < second);
                prop_assert_eq!(&keys[first], &keys[second]);
            }
        }
    }
}

#[test]
fn duplicate_complete_keys_require_caller_coalescing() {
    let theory = theory();
    for conditions in [[1, 1], [1, 2]] {
        let tuples = conditions
            .map(|condition| Tuple {
                key: vec![Term::Number(3)],
                condition,
            })
            .into();
        let error = Group::new(
            &theory,
            Function::Count,
            tuples,
            vec![],
            native::AdmissionLimits::default(),
            &Cancellation::default(),
        )
        .unwrap_err();
        assert_eq!(
            error.kind(),
            ErrorKind::Duplicate {
                first: 0,
                second: 1
            }
        );
    }
}

#[test]
fn duplicate_empty_keys_are_refused() {
    let error = Group::new(
        &theory(),
        Function::SumPlus,
        vec![
            Tuple {
                key: vec![],
                condition: 1,
            },
            Tuple {
                key: vec![],
                condition: 2,
            },
        ],
        vec![],
        native::AdmissionLimits::default(),
        &Cancellation::default(),
    )
    .unwrap_err();
    assert_eq!(
        error.kind(),
        ErrorKind::Duplicate {
            first: 0,
            second: 1
        }
    );
}

#[test]
fn equal_first_components_do_not_merge_distinct_keys() {
    let theory = theory();
    let group = Group::new(
        &theory,
        Function::Count,
        vec![
            Tuple {
                key: vec![Term::Number(3), Term::Number(2)],
                condition: 1,
            },
            Tuple {
                key: vec![Term::Number(3), Term::Number(1)],
                condition: 1,
            },
        ],
        vec![],
        native::AdmissionLimits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    assert_eq!(group.tuples()[0].key, [Term::Number(3), Term::Number(2)]);
    let value = group
        .reduce(
            &[true, true],
            None,
            native::ReductionLimits::default(),
            &Cancellation::default(),
        )
        .unwrap()
        .original()
        .value();
    let native::Value::Integer(value) = value else {
        panic!("expected count");
    };
    assert_eq!(value, 2);
}

#[test]
fn invalid_condition_ids_are_refused() {
    let error = Group::new(
        &theory(),
        Function::Count,
        vec![Tuple {
            key: vec![],
            condition: 4,
        }],
        vec![],
        native::AdmissionLimits::default(),
        &Cancellation::default(),
    )
    .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Condition { tuple: 0, node: 4 });
}

#[test]
fn admission_requires_complete_shape_and_sort_work() {
    let theory = theory();
    let full = group(&theory).statistics().work;
    for maximum in 0..=full {
        let limits = native::AdmissionLimits {
            max_work: maximum,
            ..native::AdmissionLimits::default()
        };
        match Group::new(
            &theory,
            Function::Sum,
            tuples(),
            guards(),
            limits,
            &Cancellation::default(),
        ) {
            Ok(_) => assert_eq!(maximum, full),
            Err(error) => {
                assert_eq!(error.kind(), ErrorKind::Limit(Resource::Work));
                assert!(error.statistics().work <= maximum);
            }
        }
    }
}

#[test]
fn admission_bounds_transient_index_storage() {
    let theory = theory();
    let full = group(&theory).statistics();
    assert!(full.peak_bytes > full.resident_bytes);
    for maximum in [full.peak_bytes - 1, full.peak_bytes] {
        let limits = native::AdmissionLimits {
            max_bytes: maximum,
            ..native::AdmissionLimits::default()
        };
        match Group::new(
            &theory,
            Function::Sum,
            tuples(),
            guards(),
            limits,
            &Cancellation::default(),
        ) {
            Ok(_) => assert_eq!(maximum, full.peak_bytes),
            Err(error) => assert_eq!(error.kind(), ErrorKind::Limit(Resource::Bytes)),
        }
    }
}

#[test]
fn admission_bounds_each_owned_population() {
    for (limits, resource) in [
        (
            native::AdmissionLimits {
                max_tuples: 2,
                ..native::AdmissionLimits::default()
            },
            Resource::Tuples,
        ),
        (
            native::AdmissionLimits {
                max_tuple_values: 5,
                ..native::AdmissionLimits::default()
            },
            Resource::TupleValues,
        ),
        (
            native::AdmissionLimits {
                max_value_nodes: 6,
                ..native::AdmissionLimits::default()
            },
            Resource::ValueNodes,
        ),
        (
            native::AdmissionLimits {
                max_guards: 1,
                ..native::AdmissionLimits::default()
            },
            Resource::Guards,
        ),
    ] {
        let error = Group::new(
            &theory(),
            Function::Sum,
            tuples(),
            guards(),
            limits,
            &Cancellation::default(),
        )
        .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Limit(resource));
    }
}

#[test]
fn mask_shape_failure_precedes_any_reduction() {
    let theory = theory();
    let group = group(&theory);
    for (original, frozen, phase) in [
        (&[true][..], None, Phase::Original),
        (&[true, false, true][..], Some(&[true][..]), Phase::Frozen),
    ] {
        let error = group
            .reduce(
                original,
                frozen,
                native::ReductionLimits::default(),
                &Cancellation::default(),
            )
            .unwrap_err();
        assert_eq!(
            error.kind(),
            ErrorKind::Mask {
                phase,
                expected: 3,
                actual: 1
            }
        );
        assert_eq!(error.statistics().work, 0);
    }
}

#[test]
fn every_guard_is_charged_even_after_a_false_guard() {
    let theory = theory();
    let group = group(&theory);
    let full = group
        .reduce(
            &[false, true, false],
            Some(&[false, false, false]),
            native::ReductionLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    assert!(!full.original().holds());
    // 3+1 original mask/contribution visits; 3 frozen visits. Each phase evaluates
    // both guards, including the ordered symbol comparison after the first guard.
    assert_eq!(full.statistics().work, 21);
    for maximum in 0..=full.statistics().work {
        let result = group.reduce(
            &[false, true, false],
            Some(&[false, false, false]),
            native::ReductionLimits { max_work: maximum },
            &Cancellation::default(),
        );
        match result {
            Ok(_) => assert_eq!(maximum, full.statistics().work),
            Err(error) => {
                assert_eq!(error.kind(), ErrorKind::Limit(Resource::Work));
                assert!(error.statistics().work <= maximum);
            }
        }
    }
}

#[test]
fn eligibility_uses_original_node_truth_before_freezing() {
    let theory = theory();
    let group = group(&theory);
    let candidate = Interpretation::new(&theory, [0]).unwrap();
    let tested = Interpretation::new(&theory, []).unwrap();
    let observations = group
        .eligibility(
            &candidate,
            Some(&tested),
            native::EligibilityLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    assert_eq!(observations.original(), [true, false, true]);
    assert_eq!(observations.frozen(), Some(&[false, false, true][..]));
    assert_eq!(observations.statistics().work, 14);
}

#[test]
fn foreign_interpretations_cannot_acquire_eligibility() {
    let theory = theory();
    let group = group(&theory);
    let actual = Interpretation::new(&theory, []).unwrap();
    let foreign = Theory::new(
        theory.atom_count(),
        theory.nodes().to_vec(),
        theory.roots().to_vec(),
        AdmissionLimits::default(),
    )
    .unwrap();
    let foreign = Interpretation::new(&foreign, []).unwrap();
    for (candidate, tested) in [(&foreign, None), (&actual, Some(&foreign))] {
        let error = group
            .eligibility(
                candidate,
                tested,
                native::EligibilityLimits::default(),
                &Cancellation::default(),
            )
            .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::WrongTheory);
        assert_eq!(error.statistics().work, 0);
    }
}

#[test]
fn acquisition_refuses_incomplete_prefix_work() {
    let theory = theory();
    let group = group(&theory);
    let candidate = Interpretation::new(&theory, [0]).unwrap();
    for maximum in 0..=14 {
        let limits = native::EligibilityLimits {
            max_work: maximum,
            ..native::EligibilityLimits::default()
        };
        match group.eligibility(
            &candidate,
            Some(&candidate),
            limits,
            &Cancellation::default(),
        ) {
            Ok(_) => assert_eq!(maximum, 14),
            Err(error) => {
                assert_eq!(error.kind(), ErrorKind::Limit(Resource::Work));
                assert_eq!(error.statistics().work, maximum);
            }
        }
    }
}

#[test]
fn acquisition_bounds_simultaneous_node_and_mask_storage() {
    let theory = theory();
    let group = group(&theory);
    let candidate = Interpretation::new(&theory, []).unwrap();
    let full = group
        .eligibility(
            &candidate,
            Some(&candidate),
            native::EligibilityLimits::default(),
            &Cancellation::default(),
        )
        .unwrap()
        .statistics();
    assert_eq!(full.resident_bytes, 6);
    assert_eq!(full.peak_bytes, 14);
    let limits = native::EligibilityLimits {
        max_bytes: 13,
        ..native::EligibilityLimits::default()
    };
    let error = group
        .eligibility(
            &candidate,
            Some(&candidate),
            limits,
            &Cancellation::default(),
        )
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Limit(Resource::Bytes));
    assert_eq!(error.statistics().work, 0);
}

#[test]
fn cancelled_empty_operations_never_commit_success() {
    let theory = theory();
    let group = Group::new(
        &theory,
        Function::Count,
        vec![],
        vec![],
        native::AdmissionLimits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    let candidate = Interpretation::new(&theory, []).unwrap();
    let cancellation = Cancellation::default();
    cancellation.cancel();
    for error in [
        Group::new(
            &theory,
            Function::Count,
            vec![],
            vec![],
            native::AdmissionLimits::default(),
            &cancellation,
        )
        .unwrap_err(),
        group
            .reduce(&[], None, native::ReductionLimits::default(), &cancellation)
            .unwrap_err(),
        group
            .eligibility(
                &candidate,
                None,
                native::EligibilityLimits::default(),
                &cancellation,
            )
            .unwrap_err(),
    ] {
        assert_eq!(error.kind(), ErrorKind::Stopped(Stop::Cancelled));
        assert_eq!(error.statistics().work, 0);
    }
}

#[test]
fn an_expired_deadline_prevents_eligibility_acquisition() {
    let theory = theory();
    let group = group(&theory);
    let candidate = Interpretation::new(&theory, []).unwrap();
    let error = group
        .eligibility(
            &candidate,
            None,
            native::EligibilityLimits::default(),
            &Cancellation::with_deadline(Instant::now()).unwrap(),
        )
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Stopped(Stop::Deadline));
}
