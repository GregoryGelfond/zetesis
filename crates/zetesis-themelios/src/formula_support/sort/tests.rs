use super::*;
use crate::ProgramSite;
use crate::formula_support::Counters;
use crate::{FormulaLimits, FormulaResource};
use themelios_base::{
    source::SourceId,
    span::{ByteOffset, Span},
};

fn location() -> ProgramSite {
    ProgramSite::source(themelios_base::span::Location {
        source: SourceId::new(99),
        span: Span::empty(ByteOffset::new(0)),
    })
}

fn compare<T: Ord>(
    left: &T,
    right: &T,
    work: &mut GroundingWork<'_>,
) -> Result<Ordering, FormulaFailure> {
    // A comparator can spend its own navigation permits in the same account.
    work.counters.work(work.limits, work.location)?;
    Ok(left.cmp(right))
}

#[test]
fn sorting_matches_integer_order() {
    for original in [
        vec![],
        vec![1],
        vec![1, 1],
        vec![3, 1, 2, 1],
        vec![4, 3, 2, 1],
    ] {
        let mut expected = original.clone();
        expected.sort_unstable();
        let mut actual = original;
        by(
            &mut actual,
            GroundingWork::new(
                &FormulaLimits::default(),
                &mut Counters::default(),
                location(),
            ),
            compare,
        )
        .unwrap();
        assert_eq!(actual, expected);
    }
}

#[test]
fn sorting_refusal_preserves_the_population() {
    let original = [3, 1, 2, 1, 5, 4];
    let mut expected = original;
    expected.sort_unstable();
    let mut counters = Counters::default();
    let mut complete = original;
    by(
        &mut complete,
        GroundingWork::new(&FormulaLimits::default(), &mut counters, location()),
        compare,
    )
    .unwrap();
    let needed = counters.accounting.work;
    for cutoff in 0..needed {
        let mut values = original;
        let limits = FormulaLimits {
            max_work: cutoff,
            ..Default::default()
        };
        let mut counters = Counters::default();
        assert!(matches!(by(&mut values,
            GroundingWork::new(&limits, &mut counters, location()), compare),
            Err(FormulaFailure::Limit { resource: FormulaResource::Work, observed, limit, location: found })
                if observed > limit && limit == u128::from(cutoff) && found == location()));
        values.sort_unstable();
        assert_eq!(values, expected);
    }
}

#[test]
fn comparison_requires_a_prior_permit() {
    let limits = FormulaLimits {
        max_work: 0,
        ..Default::default()
    };
    let mut called = false;
    let result = by(
        &mut [2, 1],
        GroundingWork::new(&limits, &mut Counters::default(), location()),
        |left, right, _| {
            called = true;
            Ok(left.cmp(right))
        },
    );
    assert!(matches!(
        result,
        Err(FormulaFailure::Limit {
            resource: FormulaResource::Work,
            ..
        })
    ));
    assert!(!called);
}

#[test]
fn sorting_respects_a_logarithmic_work_bound() {
    for count in 0_i32..=128 {
        let mut values: Vec<_> = (0..count).rev().collect();
        let mut counters = Counters::default();
        by(
            &mut values,
            GroundingWork::new(&FormulaLimits::default(), &mut counters, location()),
            |left, right, _| Ok(left.cmp(right)),
        )
        .unwrap();
        let count = u64::try_from(count).unwrap();
        let levels = u64::from(u64::BITS - count.saturating_sub(1).leading_zeros());
        // Fewer than 2n build/extract sifts, each bounded by the tree height,
        // with two comparisons and one swap per level, plus extraction swaps.
        assert!(counters.accounting.work <= 6 * count * levels + count);
        if count <= 1 {
            assert_eq!(counters.accounting.work, 0);
        }
    }
}

fn merged(original: &[i32], limits: &FormulaLimits) -> (Result<(), FormulaFailure>, Vec<i32>, u64) {
    let mut values = original.to_vec();
    let mut scratch = Vec::with_capacity(values.len());
    let mut runs = Vec::with_capacity(values.len() + 1);
    let mut counters = Counters::default();
    let result = merge_by(
        &mut values,
        &mut scratch,
        &mut runs,
        GroundingWork::new(limits, &mut counters, location()),
        compare,
    );
    (result, values, counters.accounting.work)
}

#[test]
fn merge_sorting_matches_integer_order_and_heapsort() {
    for original in [
        vec![],
        vec![1],
        vec![1, 1],
        vec![3, 1, 2, 1],
        vec![4, 3, 2, 1],
        (0..1000).map(|i| (i * 7919) % 1009).collect(),
    ] {
        let mut expected = original.clone();
        expected.sort_unstable();
        let (result, actual, merge_work) = merged(&original, &FormulaLimits::default());
        result.unwrap();
        assert_eq!(actual, expected);
        let mut heap = original.clone();
        let mut counters = Counters::default();
        by(
            &mut heap,
            GroundingWork::new(&FormulaLimits::default(), &mut counters, location()),
            compare,
        )
        .unwrap();
        assert_eq!(heap, expected);
        if original.len() >= 1000 {
            // Fewer comparisons and no swaps: well under heapsort's receipt.
            assert!(merge_work * 2 < counters.accounting.work, "{merge_work}");
        }
    }
}

#[test]
fn merge_sorting_refusal_preserves_the_population() {
    let original = [3, 1, 2, 1, 5, 4, 9, 0];
    let mut expected = original.to_vec();
    expected.sort_unstable();
    let (_, _, needed) = merged(&original, &FormulaLimits::default());
    for cutoff in 0..needed {
        let limits = FormulaLimits {
            max_work: cutoff,
            ..Default::default()
        };
        let (result, mut values, _) = merged(&original, &limits);
        assert!(matches!(
            result,
            Err(FormulaFailure::Limit {
                resource: FormulaResource::Work,
                ..
            })
        ));
        values.sort_unstable();
        assert_eq!(values, expected);
    }
}

#[test]
fn presorted_merge_sorting_costs_one_comparison_per_value() {
    for count in [0_i32, 1, 2, 100, 1000] {
        let original: Vec<_> = (0..count).collect();
        let (result, actual, work) = merged(&original, &FormulaLimits::default());
        result.unwrap();
        assert_eq!(actual, original);
        // One permit and one comparator permit per adjacent pair.
        assert_eq!(work, 2 * u64::try_from(count.max(1) - 1).unwrap());
    }
}

#[test]
fn ordered_heapsort_input_costs_one_comparison_per_value() {
    for count in [0_i32, 1, 2, 100] {
        let mut values: Vec<_> = (0..count).collect();
        let mut counters = Counters::default();
        by(
            &mut values,
            GroundingWork::new(&FormulaLimits::default(), &mut counters, location()),
            compare,
        )
        .unwrap();
        assert_eq!(values, (0..count).collect::<Vec<_>>());
        assert_eq!(
            counters.accounting.work,
            2 * u64::try_from(count.max(1) - 1).unwrap()
        );
    }
}

#[test]
fn galloping_merges_disjoint_runs_in_logarithmic_comparisons() {
    // Two long runs that do not interleave, in either order, and runs that
    // interleave in blocks of every length.
    let mut cases: Vec<Vec<i32>> = vec![
        (500..1000).chain(0..500).collect(),
        (0..500).chain(250..750).collect(),
    ];
    for block in 1..20 {
        let mut values: Vec<i32> = (0..400).filter(|v| (v / block) % 2 == 0).collect();
        values.extend((0..400).filter(|v| (v / block) % 2 == 1));
        cases.push(values);
    }
    for original in cases {
        let mut expected = original.clone();
        expected.sort_unstable();
        let (result, actual, _) = merged(&original, &FormulaLimits::default());
        result.unwrap();
        assert_eq!(actual, expected);
    }
    let original: Vec<i32> = (500..1000).chain(0..500).collect();
    let (_, _, work) = merged(&original, &FormulaLimits::default());
    // 999 run-finding comparisons, then about 2 log2 500 merge probes.
    assert!(work < 2 * 999 + 80, "{work}");
}

#[test]
fn galloping_merges_ties_left_first() {
    // Equal keys from the left run precede those from the right run.
    let original: Vec<(i32, i32)> = (0..20)
        .map(|v| (v, 0))
        .chain((0..20).map(|v| (v, 1)))
        .collect();
    let mut values = original.clone();
    let mut scratch = Vec::with_capacity(values.len());
    let mut runs = Vec::with_capacity(values.len() + 1);
    let mut counters = Counters::default();
    merge_by(
        &mut values,
        &mut scratch,
        &mut runs,
        GroundingWork::new(&FormulaLimits::default(), &mut counters, location()),
        |left: &(i32, i32), right: &(i32, i32), work| {
            work.counters.work(work.limits, work.location)?;
            Ok(left.0.cmp(&right.0))
        },
    )
    .unwrap();
    let mut expected = original;
    expected.sort_by_key(|value| value.0);
    assert_eq!(values, expected);
}
