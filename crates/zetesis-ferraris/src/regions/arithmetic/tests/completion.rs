mod packed;
mod views;

use zetesis_algebra::{Cancellation, Stop};

use crate::{
    AdmissionError, EvaluationLimits, EvaluationWorkspace, Node, NodeView, Producers, RegionLimits,
    Theory,
    regions::{
        Work,
        arithmetic::{
            budget::{Budget, BudgetError},
            completion::{Completion, completion},
        },
    },
    test_support::{Allowance, constructed_theory, push, theory, world},
};

fn inventory(owner: &Theory) -> Producers {
    crate::producers(owner, RegionLimits::default(), &Cancellation::default())
        .unwrap()
        .producers
        .unwrap()
}

fn budget(cancellation: &Cancellation) -> Budget<'_, 'static> {
    Budget::new(Work::new(u64::MAX), u64::MAX, usize::MAX, cancellation)
}

fn nested_xor() -> Theory {
    constructed_theory(5, |nodes| {
        let falsum = push(nodes, NodeView::False);
        let not_first = push(nodes, NodeView::Implies(0, falsum));
        let not_second = push(nodes, NodeView::Implies(1, falsum));
        let repeated = push(nodes, NodeView::And(&[0, 0]));
        let first = push(nodes, NodeView::And(&[repeated, not_second]));
        let second = push(nodes, NodeView::And(&[not_first, 1]));
        vec![
            push(nodes, NodeView::Implies(first, 4)),
            push(nodes, NodeView::Implies(second, 4)),
        ]
    })
}

fn forbidden(record: &Completion, pattern: usize) -> bool {
    record.forbidden[pattern / 64] & (1 << (pattern % 64)) != 0
}

fn assert_matches_original_bodies(owner: &Theory, producers: &Producers, record: &Completion) {
    let cancellation = Cancellation::default();
    let mut evaluator = EvaluationWorkspace::default();
    for pattern in 0..1 << record.support.len {
        let interpretation = record.support.atoms[..record.support.len]
            .iter()
            .enumerate()
            .fold(0, |mask, (position, atom)| {
                if pattern & (1 << position) == 0 {
                    mask
                } else {
                    mask | (1 << atom)
                }
            });
        let candidate = world(owner, interpretation);
        let evaluated = evaluator.evaluate(&candidate, EvaluationLimits::default(), &cancellation);
        let truth = evaluated.result.unwrap();
        let any_body = producers.by_head[record.head].iter().any(|&rule| {
            producers.rules[rule]
                .body
                .is_none_or(|body| truth.node_truth(body).unwrap())
        });
        let head_present = interpretation & (1 << record.head) != 0;
        assert_eq!(
            forbidden(record, pattern),
            head_present != any_body,
            "pattern {pattern}"
        );
    }
    for pattern in 1 << record.support.len..256 {
        assert!(!forbidden(record, pattern), "padding {pattern}");
    }
}

#[test]
fn nested_completion_matches_original_formula_truth() {
    let owner = nested_xor();
    let producers = inventory(&owner);
    let cancellation = Cancellation::default();
    let record = completion(&owner, &producers, 4, 0, &mut budget(&cancellation))
        .unwrap()
        .unwrap();
    assert_eq!(record.support.len, 3);
    assert_eq!(record.support.atoms, [0, 1, 4, 0, 0, 0, 0, 0]);
    assert_matches_original_bodies(&owner, &producers, &record);
}

#[test]
fn contradictory_literals_do_not_support_the_head() {
    let owner = constructed_theory(2, |nodes| {
        let falsum = push(nodes, NodeView::False);
        let negation = push(nodes, NodeView::Implies(0, falsum));
        let contradiction = push(nodes, NodeView::And(&[0, negation]));
        vec![push(nodes, NodeView::Implies(contradiction, 1))]
    });
    let producers = inventory(&owner);
    let cancellation = Cancellation::default();
    let record = completion(&owner, &producers, 1, 0, &mut budget(&cancellation))
        .unwrap()
        .unwrap();
    assert_eq!(record.forbidden, [0b1100, 0, 0, 0]);
    assert_matches_original_bodies(&owner, &producers, &record);
}

#[test]
fn empty_and_fact_definitions_have_opposite_tables() {
    let owner = theory(2, vec![Node::atom(0), Node::atom(1)], vec![1]);
    let producers = inventory(&owner);
    let cancellation = Cancellation::default();
    for (head, bits) in [(0, 0b10), (1, 0b01)] {
        let record = completion(&owner, &producers, head, 0, &mut budget(&cancellation))
            .unwrap()
            .unwrap();
        assert_eq!(record.support.len, 1);
        assert_eq!(record.support.atoms[0], head);
        assert_eq!(record.forbidden, [bits, 0, 0, 0]);
        assert_matches_original_bodies(&owner, &producers, &record);
    }
}

#[test]
fn every_producer_must_be_normal_and_sole_headed() {
    for choice in [false, true] {
        let owner = constructed_theory(2, |nodes| {
            let other = if choice {
                let falsum = push(nodes, NodeView::False);
                push(nodes, NodeView::Implies(0, falsum))
            } else {
                1
            };
            let nonnormal = push(nodes, NodeView::Or(&[0, other]));
            vec![0, nonnormal]
        });
        let producers = inventory(&owner);
        let cancellation = Cancellation::default();
        assert!(
            completion(&owner, &producers, 0, 0, &mut budget(&cancellation))
                .unwrap()
                .is_none()
        );
    }
}

#[test]
fn foreign_equal_inventory_does_not_authenticate() {
    let owner = nested_xor();
    let foreign = nested_xor();
    let producers = inventory(&foreign);
    let cancellation = Cancellation::default();
    assert!(
        completion(&owner, &producers, 4, 0, &mut budget(&cancellation))
            .unwrap()
            .is_none()
    );
    assert!(
        completion(
            &foreign.clone(),
            &producers,
            4,
            0,
            &mut budget(&cancellation)
        )
        .unwrap()
        .is_some()
    );
}

#[test]
fn a_ninth_distinct_atom_declines_the_whole_definition() {
    let owner = constructed_theory(9, |nodes| {
        let body = push(nodes, NodeView::And(&[0, 1, 2, 3, 4, 5, 6, 7]));
        vec![push(nodes, NodeView::Implies(body, 8))]
    });
    let producers = inventory(&owner);
    let cancellation = Cancellation::default();
    assert!(
        completion(&owner, &producers, 8, 0, &mut budget(&cancellation))
            .unwrap()
            .is_none()
    );
}

#[test]
fn unsupported_nested_bodies_publish_no_completion() {
    let owner = constructed_theory(3, |nodes| {
        let disjunction = push(nodes, NodeView::Or(&[0, 1]));
        let nested = push(nodes, NodeView::And(&[0, disjunction]));
        vec![push(nodes, NodeView::Implies(nested, 2))]
    });
    let producers = inventory(&owner);
    let cancellation = Cancellation::default();
    assert!(
        completion(&owner, &producers, 2, 0, &mut budget(&cancellation))
            .unwrap()
            .is_none()
    );
}

fn repeated_body(depth: usize) -> Theory {
    let mut nodes = vec![Node::atom(0), Node::atom(1)];
    let mut body = 0;
    for _ in 0..depth {
        nodes.push(Node::and_pair([body, 0]));
        body = nodes.len() - 1;
    }
    nodes.push(Node::implies(body, 1));
    let root = nodes.len() - 1;
    theory(2, nodes, vec![root])
}

#[test]
fn deep_bodies_use_borrowed_frames_without_recursion() {
    let owner = repeated_body(4096);
    let producers = inventory(&owner);
    let cancellation = Cancellation::default();
    let record = completion(&owner, &producers, 1, 0, &mut budget(&cancellation))
        .unwrap()
        .unwrap();
    assert_eq!(record.forbidden, [0b0110, 0, 0, 0]);
    assert_matches_original_bodies(&owner, &producers, &record);
}

#[test]
fn repeated_occurrences_remain_in_the_work_receipt() {
    let cancellation = Cancellation::default();
    let mut previous = 0;
    for depth in [1, 8, 32, 128] {
        let owner = repeated_body(depth);
        let producers = inventory(&owner);
        let mut meter = budget(&cancellation);
        let record = completion(&owner, &producers, 1, 0, &mut meter)
            .unwrap()
            .unwrap();
        assert_eq!(record.forbidden, [0b0110, 0, 0, 0]);
        assert!(meter.work() > previous);
        previous = meter.work();
    }
}

#[test]
fn repeated_literals_are_charged_on_both_traversals() {
    let cancellation = Cancellation::default();
    let mut receipts = Vec::new();
    for count in [3, 9] {
        let owner = constructed_theory(2, |nodes| {
            let body = push(nodes, NodeView::And(&vec![0; count]));
            vec![push(nodes, NodeView::Implies(body, 1))]
        });
        let producers = inventory(&owner);
        let body = producers.rules[producers.by_head[1][0]].body.unwrap();
        assert!(
            matches!(owner.view().node(body).unwrap(), NodeView::And(children) if children.len() == count)
        );
        let mut meter = budget(&cancellation);
        let record = completion(&owner, &producers, 1, 0, &mut meter)
            .unwrap()
            .unwrap();
        assert_eq!(record.forbidden, [0b0110, 0, 0, 0]);
        receipts.push(meter.work());
    }
    // Every additional repeated atom costs 13 on support collection and 15 on
    // mask construction, plus two eight-operation child-frame advances.
    // Both sources are wide conjunctions, so their node-view costs agree.
    assert_eq!(receipts[1] - receipts[0], 6 * (13 + 15 + 2 * 8));
}

#[test]
fn preparation_limits_expose_no_partial_table() {
    let owner = nested_xor();
    let producers = inventory(&owner);
    let cancellation = Cancellation::default();
    let mut complete = budget(&cancellation);
    let expected = completion(&owner, &producers, 4, 17, &mut complete)
        .unwrap()
        .unwrap();
    for maximum in 0..complete.work() {
        let mut limited = Budget::new(Work::new(u64::MAX), maximum, usize::MAX, &cancellation);
        assert_eq!(
            completion(&owner, &producers, 4, 17, &mut limited),
            Err(BudgetError::WorkDeclined)
        );
        assert!(limited.work() <= maximum);
    }
    for maximum in 0..usize::try_from(complete.peak_bytes()).unwrap() {
        let mut limited = Budget::new(Work::new(u64::MAX), u64::MAX, maximum, &cancellation);
        assert!(
            matches!(completion(&owner, &producers, 4, 17, &mut limited),
            Err(BudgetError::StorageDeclined { required, limit }) if required > limit as u128)
        );
    }
    assert_eq!(
        completion(&owner, &producers, 4, 17, &mut budget(&cancellation)),
        Ok(Some(expected))
    );
}

#[test]
fn global_refusals_preserve_their_typed_prefix() {
    let owner = nested_xor();
    let producers = inventory(&owner);
    let cancellation = Cancellation::default();
    let mut complete = budget(&cancellation);
    completion(&owner, &producers, 4, 0, &mut complete)
        .unwrap()
        .unwrap();
    for failure in [Stop::Cancelled, Stop::WorkLimit, Stop::Allocation] {
        for remaining in 0..complete.work() {
            let mut allowance = Allowance { remaining, failure };
            let mut limited = Budget::new(
                Work::reserved(&mut allowance),
                u64::MAX,
                usize::MAX,
                &cancellation,
            );
            assert_eq!(
                completion(&owner, &producers, 4, 0, &mut limited),
                Err(BudgetError::Stopped(failure))
            );
            assert!(limited.work() <= remaining);
            limited.finish();
        }
    }
}

#[test]
fn cancellation_precedes_source_and_storage_checks() {
    let owner = nested_xor();
    let producers = inventory(&owner);
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let mut meter = Budget::new(Work::new(0), 0, 0, &cancellation);
    assert_eq!(
        completion(&owner, &producers, usize::MAX, u128::MAX, &mut meter),
        Err(BudgetError::Stopped(Stop::Cancelled))
    );
    assert_eq!(meter.work(), 0);
}

#[test]
fn unrepresentable_storage_remains_an_allocation_stop() {
    let owner = nested_xor();
    let producers = inventory(&owner);
    let cancellation = Cancellation::default();
    assert_eq!(
        completion(&owner, &producers, 4, u128::MAX, &mut budget(&cancellation)),
        Err(BudgetError::Stopped(Stop::Allocation))
    );
}

#[test]
fn an_absent_head_is_a_typed_source_failure() {
    let owner = nested_xor();
    let producers = inventory(&owner);
    let cancellation = Cancellation::default();
    assert_eq!(
        completion(&owner, &producers, 5, 0, &mut budget(&cancellation)),
        Err(BudgetError::Source(AdmissionError::Atom))
    );
}
