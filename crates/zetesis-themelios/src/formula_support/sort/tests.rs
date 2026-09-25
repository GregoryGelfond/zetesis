use super::*;
use crate::formula_support::Counters;
use crate::{FormulaLimits, FormulaResource};
use themelios_base::{
    source::SourceId,
    span::{ByteOffset, Location, Span},
};

fn location() -> Location {
    Location {
        source: SourceId::new(99),
        span: Span::empty(ByteOffset::new(0)),
    }
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
