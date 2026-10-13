mod resources;

use super::*;
use crate::formula_assignment::sums::sums;
use crate::formula_support::testing::Fixture;
use crate::test_support::given_site;
use crate::{ExpansionFailure, FormulaLimits};
use std::collections::BTreeSet;
use themelios_program::term::EvalError;

fn expected(weights: &[i32]) -> Vec<i32> {
    let mut sums = BTreeSet::from([0_i32]);
    for &weight in weights {
        let shifted: Vec<_> = sums
            .iter()
            .map(|value| value.checked_add(weight).unwrap())
            .collect();
        sums.extend(shifted);
    }
    sums.into_iter().collect()
}

fn result_and_work(weights: &[i32], merge_only: bool) -> (Vec<i32>, u64) {
    Fixture::default().with(given_site(), |_, computation, counters| {
        let limits = FormulaLimits::default();
        let before = counters.accounting.work;
        let result = if merge_only {
            let mut context = Context::new(&*computation, &limits, counters, given_site());
            Runs::new(0, &mut context)
                .unwrap()
                .finish(weights.iter().copied(), &mut context)
                .unwrap()
        } else {
            sums(
                0,
                weights.iter().copied(),
                computation,
                &limits,
                counters,
                given_site(),
            )
            .unwrap()
        };
        (result.slice().to_vec(), counters.accounting.work - before)
    })
}

#[test]
fn dense_occurrences_reuse_words_and_keep_canonical_output() {
    let weights = [1; 256];
    let (actual, packed_work) = result_and_work(&weights, false);
    let (reference, merged_work) = result_and_work(&weights, true);
    assert_eq!(actual, (0..=256).collect::<Vec<_>>());
    assert_eq!(actual, reference);
    // Rebuilding/enumerating the carrier after every occurrence would still
    // perform quadratic work. A retained word state must remove that work.
    assert!(
        packed_work * 8 < merged_work,
        "{packed_work} versus {merged_work}"
    );
}

#[test]
fn zero_contributions_retain_packed_reachability() {
    let (reference, work) = result_and_work(&[1; 24], false);
    let weights: Vec<_> = std::iter::repeat_n(1, 24)
        .chain(std::iter::repeat_n(0, 70))
        .collect();
    let (actual, with_zeros) = result_and_work(&weights, false);
    assert_eq!(actual, reference);
    assert_eq!(
        with_zeros - work,
        70,
        "one dispatch per zero, no word update"
    );
}

#[test]
fn repeated_weights_are_separate_optional_occurrences() {
    let weights = [3; 96];
    let (actual, _) = result_and_work(&weights, false);
    assert_eq!(actual, (0..=96).map(|count| count * 3).collect::<Vec<_>>());
}

#[test]
fn shifts_across_63_64_and_65_match_independent_subset_sets() {
    for suffix in [vec![63], vec![64], vec![65], vec![63, 64, 65]] {
        let mut weights = vec![2; 96];
        weights.extend(suffix);
        assert_eq!(result_and_work(&weights, false).0, expected(&weights));
    }
}

#[test]
fn sparse_and_signed_controls_keep_merge_work() {
    for weights in [
        vec![1_000_000, 3_000_000, 2],
        vec![-2, 3, 5],
        vec![0; 20],
        vec![],
    ] {
        let actual = result_and_work(&weights, false);
        assert_eq!(actual.0, expected(&weights));
        assert_eq!(actual, result_and_work(&weights, true));
    }
}

#[test]
fn a_signed_tail_returns_to_checked_sorted_merging() {
    let mut weights = vec![1; 24];
    weights.extend([-7, 13, 0, -5]);
    assert_eq!(result_and_work(&weights, false).0, expected(&weights));
}

#[test]
fn a_wide_tail_declines_before_allocating_its_span() {
    let mut weights = vec![1; 24];
    weights.push(i32::MAX - 24);
    Fixture::default().with(given_site(), |_, computation, counters| {
        let limits = FormulaLimits {
            max_support_bytes: 64 * 1024,
            ..Default::default()
        };
        let actual = sums(
            0,
            weights.iter().copied(),
            computation,
            &limits,
            counters,
            given_site(),
        )
        .unwrap();
        assert_eq!(actual.slice(), expected(&weights));
    });
}

#[test]
fn overflow_remains_a_typed_failure_before_and_after_packing() {
    let mut positive = vec![1; 32];
    positive.push(i32::MAX);
    let mut negative = vec![1; 32];
    negative.extend([i32::MIN, -1]);
    for weights in [vec![i32::MAX, 1], vec![i32::MIN, -1], positive, negative] {
        Fixture::default().with(given_site(), |_, computation, counters| {
            let before = counters.workspace_bytes();
            assert!(matches!(
                sums(
                    0,
                    weights,
                    computation,
                    &FormulaLimits::default(),
                    counters,
                    given_site()
                ),
                Err(FormulaFailure::Expansion(ExpansionFailure::Evaluation {
                    error: EvalError::Overflow,
                    ..
                }))
            ));
            assert_eq!(counters.workspace_bytes(), before);
        });
    }
}

#[test]
fn packed_distinct_limit_reports_the_first_excess_value() {
    Fixture::default().with(given_site(), |_, computation, counters| {
        let limits = FormulaLimits {
            max_assignment_values: 65,
            ..Default::default()
        };
        let fitting = sums(0, [1; 64], computation, &limits, counters, given_site()).unwrap();
        assert_eq!(fitting.slice(), (0..=64).collect::<Vec<_>>());
        drop(fitting);
        let limits = FormulaLimits {
            max_assignment_values: 40,
            ..limits
        };
        let weights = std::iter::repeat_n(1, 32).chain([64]);
        assert!(matches!(
            sums(0, weights, computation, &limits, counters, given_site()),
            Err(FormulaFailure::Limit {
                resource: FormulaResource::AssignmentValues,
                observed: 41,
                limit: 40,
                ..
            })
        ));
    });
}
