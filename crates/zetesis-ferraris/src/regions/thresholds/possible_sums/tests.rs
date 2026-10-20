//! Independent subset enumeration checks the relaxation, not its shift kernel.

use super::*;
use crate::regions::Width;
use crate::test_support::{SinglePermits, close, constructed_theory, push, world};
use crate::{Limits, Narrowing, NarrowingScratch, NodeView, Region, models};
use zetesis_cpu::Cancellation;

fn interval(atoms: usize, terms: &[(usize, u64)], bounds: [i128; 2]) -> crate::Theory {
    constructed_theory(atoms, |nodes| {
        let (conditions, weights): (Vec<_>, Vec<_>) = terms.iter().copied().unzip();
        let guards = nodes
            .push_threshold_family(&conditions, &weights, &bounds, usize::MAX, usize::MAX)
            .unwrap();
        let falsum = push(nodes, NodeView::False);
        let upper = push(nodes, NodeView::Implies(guards[1], falsum));
        vec![guards[0], upper]
    })
}

fn row(weights: &[i128]) -> Row {
    let mut terms: Vec<_> = weights
        .iter()
        .copied()
        .enumerate()
        .filter(|&(_, weight)| weight > 0)
        .collect();
    terms.sort_unstable_by_key(|&(condition, weight)| (std::cmp::Reverse(weight), condition));
    Row {
        terms,
        guards: vec![],
        total: weights.iter().sum(),
    }
}

fn state(row: &Row, sure: u64, never: u64, bounds: [i128; 2]) -> State {
    State {
        known: row
            .terms
            .iter()
            .filter(|&&(condition, _)| sure & (1 << condition) != 0)
            .map(|&(_, weight)| weight)
            .sum(),
        possible: row
            .terms
            .iter()
            .filter(|&&(condition, _)| never & (1 << condition) == 0)
            .map(|&(_, weight)| weight)
            .sum(),
        lower: bounds[0],
        upper: Some(bounds[1]),
        sure_guard: 0,
        never_guard: 0,
        true_term: 0,
        false_term: 0,
    }
}

#[test]
fn an_unattainable_value_with_unit_gcd_refutes() {
    let terms = [(0, 4), (1, 6), (2, 7), (3, 9)];
    let theory = interval(4, &terms, [12, 13]);
    let index = Narrower::new(&theory);
    let mut scratch = NarrowingScratch::default();
    assert_eq!(
        close(
            &theory,
            &index,
            &mut Region::all_open(4),
            &mut index.knowledge(),
            &mut scratch,
        )
        .0,
        Narrowing::Refuted
    );
    assert!(scratch.possible_sums.capacity() > 0, "the packed route ran");
    assert!((0..16).all(|mask| {
        terms
            .iter()
            .filter(|&&(atom, _)| mask & (1 << atom) != 0)
            .map(|&(_, weight)| weight)
            .sum::<u64>()
            != 12
    }));
}

#[test]
fn every_small_completed_mask_matches_independent_subsets() {
    for a in 0..=4 {
        for b in 0..=4 {
            for c in 0..=4 {
                let data = row(&[a, b, c]);
                let mut words = Vec::new();
                for code in 0..27 {
                    let mut remaining = code;
                    let (mut sure, mut never) = (0, 0);
                    for atom in 0..3 {
                        match remaining % 3 {
                            1 => sure |= 1 << atom,
                            2 => never |= 1 << atom,
                            _ => {}
                        }
                        remaining /= 3;
                    }
                    for lower in 0..=data.total + 1 {
                        for width in 1..=3 {
                            let bounds = [lower, lower + width];
                            let snapshot = state(&data, sure, never, bounds);
                            let Some(plan) = Plan::for_row(&data, snapshot) else {
                                continue;
                            };
                            let possible = (0..8).any(|mask| {
                                mask & sure == sure && mask & never == 0 && {
                                    let sum: i128 = [a, b, c]
                                        .iter()
                                        .enumerate()
                                        .filter(|&(atom, _)| mask & (1 << atom) != 0)
                                        .map(|(_, weight)| *weight)
                                        .sum();
                                    bounds[0] <= sum && sum < bounds[1]
                                }
                            });
                            // The mathematical operation is checked independently
                            // of the deliberately smaller production allowance.
                            let mut work = Work::new(u64::MAX);
                            let actual = attempt(
                                &data,
                                &[sure],
                                &[never],
                                &mut words,
                                plan,
                                &mut Allowance {
                                    remaining: usize::MAX,
                                    work: &mut work,
                                },
                            )
                            .unwrap();
                            assert_eq!(
                                actual == Step::Contradiction,
                                !possible,
                                "weights={:?}, code={code}, bounds={bounds:?}",
                                [a, b, c]
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn boundary_offsets_and_nonzero_bases_keep_exact_answers() {
    for weights in [[63, 64, 65, 129], [1, 64, 65, 129], [7, 9, 127, 129]] {
        let data = row(&weights);
        for (sure, never) in [(0, 0), (1, 0), (0, 1), (1, 2)] {
            for target in [63, 64, 65, 127, 128, 129, 130, 191, 192, 193] {
                let snapshot = state(&data, sure, never, [target, target + 1]);
                let Some(plan) = Plan::for_row(&data, snapshot) else {
                    continue;
                };
                let possible = (0..16).any(|mask| {
                    mask & sure == sure
                        && mask & never == 0
                        && weights
                            .iter()
                            .enumerate()
                            .filter(|&(atom, _)| mask & (1 << atom) != 0)
                            .map(|(_, weight)| *weight)
                            .sum::<i128>()
                            == target
                });
                let result = attempt(
                    &data,
                    &[sure],
                    &[never],
                    &mut Vec::new(),
                    plan,
                    &mut Allowance {
                        remaining: usize::MAX,
                        work: &mut Work::new(u64::MAX),
                    },
                )
                .unwrap();
                assert_eq!(result == Step::Contradiction, !possible);
            }
        }
    }
}

#[test]
fn optional_declines_do_not_become_refutations() {
    let data = row(&[4, 6, 7, 9]);
    let plan = Plan::for_row(&data, state(&data, 0, 0, [12, 13])).unwrap();
    let mut complete_work = Work::new(u64::MAX);
    assert!(matches!(
        attempt(
            &data,
            &[0],
            &[0],
            &mut Vec::new(),
            plan,
            &mut Allowance {
                remaining: plan.visits,
                work: &mut complete_work,
            },
        ),
        Ok(Step::Contradiction)
    ));
    assert!(complete_work.spent <= plan.visits as u64);
    for limit in 0..complete_work.spent {
        let mut work = Work::new(u64::MAX);
        let mut words = Vec::new();
        assert!(matches!(
            attempt(
                &data,
                &[0],
                &[0],
                &mut words,
                plan,
                &mut Allowance {
                    remaining: usize::try_from(limit).unwrap(),
                    work: &mut work,
                },
            ),
            Err(Failure::Declined)
        ));
        assert_eq!(work.spent, limit);
        assert!(matches!(
            attempt(
                &data,
                &[0],
                &[0],
                &mut words,
                plan,
                &mut Allowance {
                    remaining: plan.visits,
                    work: &mut Work::new(u64::MAX),
                },
            ),
            Ok(Step::Contradiction)
        ));
    }
}

#[test]
fn global_refusals_keep_their_kind_and_retry_from_zero() {
    let data = row(&[4, 6, 7, 9]);
    let plan = Plan::for_row(&data, state(&data, 0, 0, [12, 13])).unwrap();
    let mut complete_work = Work::new(u64::MAX);
    attempt(
        &data,
        &[0],
        &[0],
        &mut Vec::new(),
        plan,
        &mut Allowance {
            remaining: plan.visits,
            work: &mut complete_work,
        },
    )
    .unwrap();
    for failure in [Stop::WorkLimit, Stop::Cancelled, Stop::Deadline] {
        for limit in 0..complete_work.spent {
            let mut quota = SinglePermits::new(limit, failure);
            let mut work = Work::reserved(&mut quota);
            let mut words = Vec::new();
            assert!(matches!(
                attempt(
                    &data,
                    &[0],
                    &[0],
                    &mut words,
                    plan,
                    &mut Allowance {
                        remaining: plan.visits,
                        work: &mut work,
                    },
                ),
                Err(Failure::Stopped(actual)) if actual == failure
            ));
            assert_eq!(work.spent, limit);
            assert!(matches!(
                attempt(
                    &data,
                    &[0],
                    &[0],
                    &mut words,
                    plan,
                    &mut Allowance {
                        remaining: plan.visits,
                        work: &mut Work::new(u64::MAX),
                    },
                ),
                Ok(Step::Contradiction)
            ));
        }
    }
}

#[test]
fn representation_ineligibility_has_no_work_or_storage() {
    for (weights, bounds) in [
        (vec![1; 4], [2, 3]),
        (vec![4, 6, 7, 9], [0, 13]),
        (vec![4, 6, 7, 9], [12, 27]),
        (
            vec![1, i128::MAX / 4, i128::MAX / 3],
            [i128::MAX / 8, i128::MAX / 7],
        ),
    ] {
        let data = row(&weights);
        let state = state(&data, 0, 0, bounds);
        assert!(Plan::for_row(&data, state).is_none());
    }
    let theory = interval(4, &[(0, 1), (1, 1), (2, 1), (3, 1)], [2, 3]);
    let index = Narrower::new(&theory);
    let mut knowledge = index.knowledge();
    let Width::Compact16(known) = &mut knowledge.width else {
        unreachable!()
    };
    known.thresholds[0].lower = 2;
    known.thresholds[0].upper = Some(3);
    let mut scratch = NarrowingScratch::default();
    let mut work = Work::new(0);
    assert!(matches!(
        Closure {
            known: known.borrow(),
            lists: &mut scratch,
        }
        .threshold_possible_sums(&index, 0, &mut work),
        Ok(Step::Unchanged)
    ));
    assert_eq!(work.spent, 0);
    assert_eq!(scratch.possible_sums.capacity(), 0);
}

#[test]
fn allocation_refusal_keeps_typed_failure_and_allows_retry() {
    let impossible = Plan {
        lower: 1,
        maximum: usize::MAX,
        words: usize::MAX,
        visits: usize::MAX,
    };
    let mut words = vec![u64::MAX];
    assert!(matches!(
        prepare(
            &mut words,
            impossible,
            &mut Allowance {
                remaining: usize::MAX,
                work: &mut Work::new(u64::MAX),
            }
        ),
        Err(Failure::Stopped(Stop::Allocation))
    ));
    let data = row(&[4, 6, 7, 9]);
    let plan = Plan::for_row(&data, state(&data, 0, 0, [12, 13])).unwrap();
    assert!(matches!(
        attempt(
            &data,
            &[0],
            &[0],
            &mut words,
            plan,
            &mut Allowance {
                remaining: plan.visits,
                work: &mut Work::new(u64::MAX),
            }
        ),
        Ok(Step::Contradiction)
    ));
}

#[test]
fn scratch_is_accounted_and_contains_no_sibling_authority() {
    let mut scratch = NarrowingScratch::default();
    for target in [12, 13, 12] {
        let theory = interval(4, &[(0, 4), (1, 6), (2, 7), (3, 9)], [target, target + 1]);
        let index = Narrower::new(&theory);
        let result = close(
            &theory,
            &index,
            &mut Region::all_open(4),
            &mut index.knowledge(),
            &mut scratch,
        );
        assert_eq!(result.0 == Narrowing::Refuted, target == 12);
        assert!(scratch.possible_sums.capacity() > 0);
        let before = scratch.retained_bytes();
        let capacity = scratch.possible_sums.capacity();
        scratch.prepare(4);
        assert!(scratch.possible_sums.is_empty());
        assert_eq!(scratch.retained_bytes(), before);
        scratch.possible_sums = Vec::new();
        assert_eq!(
            scratch.retained_bytes() + capacity as u128 * size_of::<u64>() as u128,
            before
        );
    }
}

#[test]
fn repeated_and_correlated_conditions_preserve_every_model() {
    let theory = constructed_theory(3, |nodes| {
        let both = push(nodes, NodeView::And(&[0, 1]));
        let either = push(nodes, NodeView::Or(&[0, 1]));
        let guards = nodes
            .push_threshold_family(
                &[both, both, either, 2],
                &[2, 2, 6, 7],
                &[10, 14],
                usize::MAX,
                usize::MAX,
            )
            .unwrap();
        let falsum = push(nodes, NodeView::False);
        let upper = push(nodes, NodeView::Implies(guards[1], falsum));
        vec![guards[0], upper]
    });
    let index = Narrower::new(&theory);
    assert_eq!(index.thresholds.rows[0].terms.len(), 3);
    let expected: Vec<_> = (0..8)
        .filter(|&mask| {
            models(
                &theory,
                &world(&theory, mask),
                Limits::default(),
                &Cancellation::default(),
            )
            .unwrap()
        })
        .collect();
    assert!(!expected.is_empty());
    for code in 0..27 {
        let mut remainder = code;
        let mut region = Region::all_open(3);
        for atom in 0..3 {
            match remainder % 3 {
                1 => assert!(region.hold(atom)),
                2 => assert!(region.cut(atom)),
                _ => {}
            }
            remainder /= 3;
        }
        let compatible: Vec<_> = expected
            .iter()
            .copied()
            .filter(|&mask| {
                (0..3).all(|atom| {
                    region
                        .decision(atom)
                        .is_none_or(|value| value == (mask & (1 << atom) != 0))
                })
            })
            .collect();
        let outcome = close(
            &theory,
            &index,
            &mut region,
            &mut index.knowledge(),
            &mut NarrowingScratch::default(),
        )
        .0;
        for mask in compatible {
            assert_ne!(outcome, Narrowing::Refuted);
            assert!((0..3).all(|atom| {
                region
                    .decision(atom)
                    .is_none_or(|value| value == (mask & (1 << atom) != 0))
            }));
        }
    }
}

#[test]
fn an_independent_sum_does_not_establish_a_model() {
    let theory = constructed_theory(3, |nodes| {
        let falsum = push(nodes, NodeView::False);
        let negative = push(nodes, NodeView::Implies(0, falsum));
        let guards = nodes
            .push_threshold_family(
                &[0, negative, 1, 2],
                &[4, 6, 7, 9],
                &[10, 11],
                usize::MAX,
                usize::MAX,
            )
            .unwrap();
        let upper = push(nodes, NodeView::Implies(guards[1], falsum));
        vec![guards[0], upper]
    });
    let index = Narrower::new(&theory);
    let data = &index.thresholds.rows[0];
    // The independent sum 4+6 exists, but its two conditions are complements.
    // The relaxation may decline refutation; it cannot certify that sum as a
    // compatible Boolean assignment or as an answer set.
    let plan = Plan::for_row(data, state(data, 0, 0, [10, 11])).unwrap();
    assert!(matches!(
        attempt(
            data,
            &[0],
            &[0],
            &mut Vec::new(),
            plan,
            &mut Allowance {
                remaining: plan.visits,
                work: &mut Work::new(u64::MAX),
            }
        ),
        Ok(Step::Unchanged)
    ));
    assert!((0..8).all(|mask| {
        !models(
            &theory,
            &world(&theory, mask),
            Limits::default(),
            &Cancellation::default(),
        )
        .unwrap()
    }));
}
