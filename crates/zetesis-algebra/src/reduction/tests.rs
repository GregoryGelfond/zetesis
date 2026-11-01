use super::{All, Any, CheckedSum, Minimum, Reduction, Verdict};

fn fold<R: Reduction>(reduction: &R, values: impl IntoIterator<Item = R::Value>) -> R::Value {
    values.into_iter().fold(reduction.unit(), |left, right| {
        reduction.combine(left, right)
    })
}

#[test]
fn boolean_folds_match_their_quantifiers() {
    for length in 0..=8 {
        for pattern in 0..1_u16 << length {
            let mut values = (0..length).map(|bit| pattern & (1 << bit) != 0);
            assert_eq!(
                fold(&All, values.clone()),
                values.clone().all(|value| value)
            );
            assert_eq!(fold(&Any, values.clone()), values.any(|value| value));
        }
    }
}

fn boolean_laws(reduction: &impl Reduction<Value = bool>) {
    for left in [false, true] {
        assert_eq!(reduction.combine(reduction.unit(), left), left);
        assert_eq!(reduction.combine(left, reduction.unit()), left);
        assert_eq!(reduction.combine(left, left), left);
        for right in [false, true] {
            assert_eq!(
                reduction.combine(left, right),
                reduction.combine(right, left)
            );
            for last in [false, true] {
                assert_eq!(
                    reduction.combine(reduction.combine(left, right), last),
                    reduction.combine(left, reduction.combine(right, last))
                );
            }
            if reduction.is_absorbing(&left) {
                assert_eq!(reduction.combine(left, right), left);
            }
        }
    }
}

#[test]
fn boolean_instances_obey_the_value_laws() {
    boolean_laws(&All);
    boolean_laws(&Any);
    assert!(All.is_absorbing(&false));
    assert!(!All.is_absorbing(&true));
    assert!(Any.is_absorbing(&true));
    assert!(!Any.is_absorbing(&false));
}

#[test]
fn a_product_preserves_both_coordinate_folds() {
    let values = [(false, false), (false, true), (true, false), (true, true)];
    for first in values {
        for second in values {
            for third in values {
                let sequence = [first, second, third];
                let expected = (
                    sequence.iter().all(|value| value.0),
                    sequence.iter().any(|value| value.1),
                );
                assert_eq!(fold(&(All, Any), sequence), expected);
                assert_eq!(fold(&(All, Any), [third, first, second]), expected);
            }
        }
    }
}

#[test]
fn one_absorbed_coordinate_does_not_absorb_a_query() {
    let reduction = (Any, Any);
    for value in [(false, false), (false, true), (true, false), (true, true)] {
        let continuations_preserve = [(false, false), (false, true), (true, false), (true, true)]
            .into_iter()
            .all(|next| reduction.combine(value, next) == value);
        assert_eq!(reduction.is_absorbing(&value), continuations_preserve);
    }
    assert_eq!(
        reduction.combine((true, false), (false, true)),
        (true, true)
    );
}

#[test]
fn a_nonempty_unit_is_distinct_from_empty_coverage() {
    assert!(matches!(
        Verdict::<bool, (), ()>::completed(None),
        Verdict::Empty
    ));
    assert!(matches!(
        Verdict::<bool, (), ()>::completed(Some(All.unit())),
        Verdict::Complete(true)
    ));
    assert!(matches!(
        Verdict::<bool, (), ()>::completed(Some(Any.unit())),
        Verdict::Complete(false)
    ));
}

#[test]
fn completion_moves_a_value_without_requiring_a_clone() {
    struct Value(String);

    let value = Value(String::from("a completed value"));
    let allocation = value.0.as_ptr();
    let verdict = Verdict::<Value, (), ()>::completed(Some(value));
    let Verdict::Complete(value) = verdict else {
        panic!("nonempty completion lost its value");
    };
    assert_eq!(value.0, "a completed value");
    assert_eq!(value.0.as_ptr(), allocation);
}

#[test]
fn checked_sums_match_exact_natural_sums_through_the_boundary() {
    let values = [0, 1, u64::MAX / 2, u64::MAX / 2 + 1, u64::MAX - 1, u64::MAX];
    for left in values {
        for right in values {
            let exact = u128::from(left) + u128::from(right);
            assert_eq!(
                CheckedSum.combine(Some(left), Some(right)),
                u64::try_from(exact).ok()
            );
        }
    }
    assert_eq!(fold(&CheckedSum, [Some(2), Some(0), Some(3)]), Some(5));
    assert_eq!(fold(&CheckedSum, [Some(u64::MAX), Some(1)]), None);
}

#[test]
fn checked_sums_preserve_the_value_laws_without_early_success() {
    let values = [None, Some(0), Some(1), Some(u64::MAX - 1), Some(u64::MAX)];
    for left in values {
        assert_eq!(CheckedSum.combine(CheckedSum.unit(), left), left);
        assert_eq!(CheckedSum.combine(left, CheckedSum.unit()), left);
        assert!(!CheckedSum.is_absorbing(&left));
        for right in values {
            assert_eq!(
                CheckedSum.combine(left, right),
                CheckedSum.combine(right, left)
            );
            for last in values {
                assert_eq!(
                    CheckedSum.combine(CheckedSum.combine(left, right), last),
                    CheckedSum.combine(left, CheckedSum.combine(right, last))
                );
            }
        }
    }
}

#[test]
fn minimum_has_an_empty_identity_and_obeys_the_value_laws() {
    let minimum = Minimum::<i64>::new();
    let values = [None, Some(i64::MIN), Some(-1), Some(0), Some(i64::MAX)];
    for first in values {
        assert_eq!(minimum.combine(minimum.unit(), first), first);
        assert_eq!(minimum.combine(first, minimum.unit()), first);
        assert_eq!(minimum.combine(first, first), first);
        assert!(!minimum.is_absorbing(&first));
        for second in values {
            assert_eq!(
                minimum.combine(first, second),
                minimum.combine(second, first)
            );
            for third in values {
                assert_eq!(
                    minimum.combine(minimum.combine(first, second), third),
                    minimum.combine(first, minimum.combine(second, third))
                );
            }
        }
    }
}

#[test]
fn minimum_step_uses_one_comparison_and_preserves_equal_representatives() {
    use std::cell::Cell;
    use std::cmp::Ordering;

    // Test-only observation of call count; production orders must be pure.
    #[derive(Clone, Copy, Debug)]
    struct Observed<'a> {
        cost: i32,
        representative: u8,
        calls: &'a Cell<usize>,
    }
    impl PartialEq for Observed<'_> {
        fn eq(&self, other: &Self) -> bool {
            self.cost == other.cost
        }
    }
    impl Eq for Observed<'_> {}
    impl PartialOrd for Observed<'_> {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(self.cmp(other))
        }
    }
    impl Ord for Observed<'_> {
        fn cmp(&self, other: &Self) -> Ordering {
            self.calls.set(self.calls.get() + 1);
            self.cost.cmp(&other.cost)
        }
    }
    let calls = Cell::new(0);
    let old = Observed {
        cost: 3,
        representative: 1,
        calls: &calls,
    };
    let minimum = Minimum::new();
    let first = minimum.step(None, old);
    assert_eq!(calls.get(), 0);
    assert_eq!(first.incoming, Ordering::Less);
    for (cost, expected, representative) in [
        (2, Ordering::Less, 2),
        (3, Ordering::Equal, 1),
        (4, Ordering::Greater, 1),
    ] {
        calls.set(0);
        let incoming = Observed {
            cost,
            representative: 2,
            calls: &calls,
        };
        let result = minimum.step(Some(old), incoming);
        assert_eq!(calls.get(), 1);
        assert_eq!(result.incoming, expected);
        assert_eq!(result.value.representative, representative);
        calls.set(0);
        assert_eq!(
            minimum.combine(Some(old), Some(incoming)),
            Some(result.value)
        );
        assert_eq!(calls.get(), 1);
    }
}
