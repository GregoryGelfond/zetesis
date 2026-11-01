use std::convert::Infallible;

use proptest::prelude::*;

use super::super::uniform::{BoundSide, Decline, UniformBound, uniform_bound};

const WORK_BOUND: u64 = 4 + 1 + 3 + 4 + 128 * (13 + 4) + 3 + 2 + 2;

fn reference(bound: i128, weight: u64, cardinality: u64, side: BoundSide) -> UniformBound {
    if bound <= 0 {
        return UniformBound::Declined(Decline::NonPositive);
    }
    if weight == 0 {
        return UniformBound::Declined(Decline::ZeroWeight);
    }
    // Independent wide-arithmetic oracle: quotient and remainder of b, rather
    // than restoring division of its predecessor. All additions are checked.
    let divisor = i128::from(weight);
    let quotient = bound.checked_div(divisor).unwrap();
    let remainder = bound.checked_rem(divisor).unwrap();
    let ceiling = quotient.checked_add(i128::from(remainder != 0)).unwrap();
    if ceiling > i128::from(cardinality) {
        return UniformBound::Declined(Decline::BeyondCardinality);
    }
    let value = u64::try_from(ceiling).unwrap();
    UniformBound::Valid {
        value: match side {
            BoundSide::Lower => value,
            BoundSide::Upper => value.checked_sub(1).unwrap(),
        },
    }
}

fn measured(bound: i128, weight: u64, cardinality: u64, side: BoundSide) -> (UniformBound, u64) {
    let mut work = 0;
    let result = uniform_bound(bound, weight, cardinality, side, &mut |amount| {
        work += amount;
        Ok::<_, Infallible>(())
    })
    .unwrap();
    (result, work)
}

#[test]
fn boundary_bounds_match_checked_wide_arithmetic() {
    let bounds = [
        i128::MIN,
        i128::MIN + 1,
        -1,
        0,
        1,
        2,
        3,
        i128::from(u64::MAX) - 1,
        i128::from(u64::MAX),
        i128::from(u64::MAX) + 1,
        i128::from(u64::MAX) + 2,
        i128::from(u64::MAX).checked_mul(2).unwrap(),
        i128::MAX - 1,
        i128::MAX,
    ];
    let words = [
        0,
        1,
        2,
        3,
        u64::from(u32::MAX),
        1 << 32,
        1 << 63,
        u64::MAX - 1,
        u64::MAX,
    ];
    for bound in bounds {
        for weight in words {
            for cardinality in words {
                for side in [BoundSide::Lower, BoundSide::Upper] {
                    let (actual, work) = measured(bound, weight, cardinality, side);
                    assert_eq!(actual, reference(bound, weight, cardinality, side));
                    assert!(work <= WORK_BOUND);
                }
            }
        }
    }
}

proptest! {
    #[test]
    fn arbitrary_bounds_match_checked_wide_arithmetic(
        bound in any::<i128>(),
        weight in any::<u64>(),
        cardinality in any::<u64>(),
    ) {
        for side in [BoundSide::Lower, BoundSide::Upper] {
            let (actual, work) = measured(bound, weight, cardinality, side);
            prop_assert_eq!(actual, reference(bound, weight, cardinality, side));
            prop_assert!(work <= WORK_BOUND);
        }
    }
}

#[test]
fn normalized_bounds_preserve_threshold_truth() {
    for cardinality in 1..=12 {
        for weight in 1..=12 {
            let largest = i128::from(weight)
                .checked_mul(i128::from(cardinality))
                .unwrap();
            for bound in 1..=largest {
                let (UniformBound::Valid { value: lower }, _) =
                    measured(bound, weight, cardinality, BoundSide::Lower)
                else {
                    panic!("nontrivial lower bound declined");
                };
                let (UniformBound::Valid { value: upper }, _) =
                    measured(bound, weight, cardinality, BoundSide::Upper)
                else {
                    panic!("nontrivial upper bound declined");
                };
                for count in 0..=cardinality {
                    let sum = i128::from(weight).checked_mul(i128::from(count)).unwrap();
                    assert_eq!(sum >= bound, count >= lower);
                    assert_eq!(sum < bound, count <= upper);
                }
            }
        }
    }
}

#[test]
fn trivial_shapes_keep_their_decline_reason() {
    for side in [BoundSide::Lower, BoundSide::Upper] {
        for (bound, weight, cardinality, reason) in [
            (i128::MIN, 0, 0, Decline::NonPositive),
            (-1, 1, 1, Decline::NonPositive),
            (0, 1, 1, Decline::NonPositive),
            (1, 0, 1, Decline::ZeroWeight),
            (1, 1, 0, Decline::BeyondCardinality),
            (10, 3, 3, Decline::BeyondCardinality),
            (i128::MAX, 1, u64::MAX, Decline::BeyondCardinality),
        ] {
            assert_eq!(
                measured(bound, weight, cardinality, side).0,
                UniformBound::Declined(reason)
            );
        }
    }
}

#[test]
fn receipts_count_only_executed_subtractions() {
    for side in [BoundSide::Lower, BoundSide::Upper] {
        assert_eq!(measured(i128::MIN, 1, 1, side).1, 3);
        assert_eq!(measured(0, 1, 1, side).1, 5);
        assert_eq!(measured(1, 0, 1, side).1, 6);
        for (bound, weight, cardinality) in [
            (1, 1, 1),
            (9, 3, 3),
            (10, 3, 4),
            (i128::from(u64::MAX) + 1, 1, u64::MAX),
            (i128::MAX, 1, u64::MAX),
            (i128::MAX, u64::MAX, u64::MAX),
            (i128::MAX, u64::MAX, 0),
        ] {
            let (result, work) = measured(bound, weight, cardinality, side);
            // Each taken subtraction emits exactly one quotient bit. Native
            // division and popcount reconstruct that count independently.
            let quotient = bound
                .checked_sub(1)
                .unwrap()
                .checked_div(i128::from(weight))
                .unwrap();
            let subtractions = u64::from(quotient.count_ones());
            let ceiling = quotient.checked_add(1).unwrap();
            let comparisons = if ceiling > i128::from(u64::MAX) { 1 } else { 2 };
            let publication = match result {
                UniformBound::Declined(_) => 1,
                UniformBound::Valid { .. } => 2,
            };
            assert_eq!(
                work,
                4 + 1 + 3 + 4 + 128 * 13 + 4 * subtractions + 3 + comparisons + publication
            );
            assert!(work <= WORK_BOUND);
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
struct Refusal {
    call: usize,
    admitted: u64,
    requested: u64,
}

#[test]
fn every_callback_refusal_preserves_its_typed_prefix() {
    for side in [BoundSide::Lower, BoundSide::Upper] {
        for (bound, weight, cardinality) in [
            (i128::MIN, 0, 0),
            (0, 1, 1),
            (1, 0, 1),
            (1, 1, 0),
            (1, 1, 1),
            (9, 3, 3),
            (10, 3, 4),
            (i128::from(u64::MAX) + 1, 1, u64::MAX),
            (i128::MAX, 1, u64::MAX),
            (i128::MAX, u64::MAX, u64::MAX),
        ] {
            let mut chunks = Vec::new();
            let complete = uniform_bound(bound, weight, cardinality, side, &mut |amount| {
                chunks.push(amount);
                Ok::<_, Infallible>(())
            })
            .unwrap();
            assert_eq!(complete, reference(bound, weight, cardinality, side));
            for refused_call in 0..chunks.len() {
                let mut calls = 0;
                let mut admitted = 0;
                let stopped = uniform_bound(bound, weight, cardinality, side, &mut |amount| {
                    assert_eq!(amount, chunks[calls]);
                    let call = calls;
                    calls += 1;
                    if call == refused_call {
                        return Err(Refusal {
                            call,
                            admitted,
                            requested: amount,
                        });
                    }
                    admitted += amount;
                    Ok(())
                });
                assert_eq!(calls, refused_call + 1);
                assert_eq!(admitted, chunks[..refused_call].iter().sum());
                assert_eq!(
                    stopped,
                    Err(Refusal {
                        call: refused_call,
                        admitted,
                        requested: chunks[refused_call],
                    })
                );
            }
        }
    }
}
