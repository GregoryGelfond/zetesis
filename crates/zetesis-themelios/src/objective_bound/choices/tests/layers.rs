//! Value/formula correspondence and private runtime-bound qualification.
//! The fixture reuses the existing source implication and disjoint-key checker;
//! the runtime module checks narrowing and the existing optional transaction.
use super::*;
use std::cmp::Ordering;
use zetesis_algebra::reduction::{Minimum, Reduction};

mod boundaries;
mod fixture;

fn value(weights: &[i32], enabled: &[bool], active: bool) -> i64 {
    assert_eq!(weights.len(), enabled.len());
    assert!(!weights.is_empty());
    assert!(weights.iter().all(|weight| *weight >= 0));
    if !active {
        return 0;
    }
    let mut levels = weights.to_vec();
    levels.sort_unstable();
    levels.dedup();
    let mut sum = i64::from(levels[0]);
    for adjacent in levels.windows(2) {
        let none_below = weights
            .iter()
            .zip(enabled)
            .all(|(weight, selected)| !selected || *weight >= adjacent[1]);
        if none_below {
            sum += i64::from(adjacent[1] - adjacent[0]);
        }
    }
    sum
}

/// Exercise the actual counted emitter on a sorted fixture key order. The
/// authenticated full-plan cases below separately traverse production ordering.
fn compile(
    nodes: &mut FormulaNodes,
    activation: usize,
    keys: &[AggregateElement],
) -> Vec<AggregateElement> {
    assert!(!keys.is_empty());
    assert!(keys.windows(2).all(|pair| pair[0].weight <= pair[1].weight));
    let group = Group {
        priority: 0,
        activation,
        weight: keys[0].weight,
        keys: (0..keys.len()).collect(),
    };
    let cancellation = Cancellation::default();
    let mut work = Work::new(ObjectivePlanLimits::default(), &cancellation);
    let mut terms = work.reserve(keys.len()).unwrap();
    super::super::layers::append_group(
        keys, &group, activation, nodes, &mut terms, &mut None, &mut work,
    )
    .unwrap();
    assert!(work.statistics().work > 0);
    terms
}

#[test]
fn layers_refine_the_shared_minimum_including_zero_and_equal_weights() {
    let minimum = Minimum::<i32>::new();
    for encoded in 0..64 {
        let weights = [encoded % 4, (encoded / 4) % 4, (encoded / 16) % 4];
        for mask in 0..8 {
            let enabled = [mask & 1 != 0, mask & 2 != 0, mask & 4 != 0];
            let expected = weights
                .iter()
                .zip(enabled)
                .filter(|(_, on)| *on)
                .fold(minimum.unit(), |old, (weight, _)| {
                    minimum.combine(old, Some(*weight))
                });
            assert_eq!(value(&weights, &enabled, false), 0);
            if let Some(expected) = expected {
                assert_eq!(value(&weights, &enabled, true), i64::from(expected));
                let exact: i64 = weights
                    .iter()
                    .zip(enabled)
                    .filter(|(_, on)| *on)
                    .map(|(weight, _)| i64::from(*weight))
                    .sum();
                assert!(value(&weights, &enabled, true) <= exact);
            }
        }
    }
    // No eligible key under an active mandatory group is not a zero-cost case.
    assert_eq!(value(&[0, 5], &[false, false], true), 5);
    assert_eq!(value(&[0, 5], &[true, false], true), 0);
}

#[test]
fn layer_formula_matches_every_key_selection_and_activation() {
    for weights in [[0, 0, 0], [0, 5, 5], [1, 5, 6]] {
        let mut nodes = fixture::atoms();
        let keys: Vec<_> = weights
            .into_iter()
            .enumerate()
            .map(|(condition, weight)| AggregateElement { weight, condition })
            .collect();
        let terms = compile(&mut nodes, 6, &keys);
        let parts = nodes.into_parts();
        let conditions: Vec<_> = terms
            .iter()
            .map(|term| {
                Theory::new(
                    8,
                    parts.try_clone().unwrap(),
                    vec![term.condition],
                    AdmissionLimits::default(),
                )
                .unwrap()
            })
            .collect();
        for mask in 0..128 {
            let enabled = [mask & 1 != 0, mask & 2 != 0, mask & 4 != 0];
            let actual: i64 = terms
                .iter()
                .zip(&conditions)
                .filter(|(_, condition)| fixture::holds(condition, mask))
                .map(|(term, _)| i64::from(term.weight))
                .sum();
            assert_eq!(actual, value(&weights, &enabled, mask & 64 != 0));
        }
    }
}

#[test]
fn certified_disjoint_layer_bounds_preserve_lexicographic_models_and_all_ties() {
    for upper in [5, 6] {
        for active in [false, true] {
            let (mut plan, required) = fixture::plan(upper, active);
            let groups = fixture::checked_groups(&plan, &required);
            assert_eq!(groups.len(), 2);
            assert_eq!(groups[0].keys, [0, 1, 2]);
            assert_eq!(groups[1].keys, [3, 4, 5]);
            let layers = fixture::layer_plan(&plan, &groups);
            plan.prepare_choice_bounds(
                &required,
                ObjectivePlanLimits::default(),
                u128::MAX,
                &Cancellation::default(),
            )
            .unwrap();
            let masks: Vec<_> = (0..256)
                .filter(|mask| fixture::holds(plan.original(), *mask))
                .collect();
            for ceiling in [9, 10, 11, 12] {
                let incumbent = Score::from_costs(vec![(1, ceiling), (0, 0)]).unwrap();
                for strict in [false, true] {
                    let original = fixture::bound(&plan, &incumbent, strict);
                    let additional = fixture::bound(&layers, &incumbent, strict);
                    for &mask in &masks {
                        let exact = fixture::score(&plan, mask);
                        let expected = match exact.compare_costs(&incumbent) {
                            Ordering::Less => true,
                            Ordering::Equal => !strict,
                            Ordering::Greater => false,
                        };
                        let old = fixture::holds(original.theory(), mask);
                        let extra = fixture::holds(additional.theory(), mask);
                        assert_eq!(old, expected);
                        assert_eq!(old && extra, expected);
                    }
                }
            }
            if upper == 5 && active {
                let optimal: Vec<_> = masks
                    .iter()
                    .copied()
                    .filter(|&mask| fixture::score(&plan, mask).costs() == [(1, 10), (0, -3)])
                    .collect();
                assert_eq!(optimal.len(), 4, "all four equal-cost key choices survive");
            }
        }
    }
}

#[test]
fn independent_lower_scores_must_be_conjoined_not_added() {
    let weights = [1, 5, 6];
    let one = [false, true, false];
    let several = [false, true, true];
    let residual = |enabled: &[bool]| {
        1 + weights
            .iter()
            .zip(enabled)
            .filter(|(_, on)| **on)
            .map(|(weight, _)| i64::from(*weight - 1))
            .sum::<i64>()
    };
    assert_eq!(value(&weights, &one, true), 5);
    assert_eq!(residual(&one), 5);
    assert!(value(&weights, &one, true) + residual(&one) > 5);
    assert_eq!(value(&weights, &several, true), 5);
    assert_eq!(residual(&several), 10);
    assert!(residual(&several) <= 11);
}

mod runtime;
