use std::mem::size_of;

use proptest::prelude::*;

use super::Counters;

#[test]
fn compact_width_includes_its_maximum_count() {
    let maximum = usize::try_from(u32::MAX).unwrap();
    let mut counts = Counters::zeros(1, maximum);
    assert_eq!(
        matches!(counts, Counters::Compact(_)),
        size_of::<u32>() < size_of::<usize>()
    );
    counts.add(0, maximum);
    counts.decrement(0);
    counts.add(0, 1);
    assert_eq!(counts.get(0), maximum);
}

#[test]
fn native_width_preserves_its_maximum_count() {
    // On narrower hosts the native representation remains the fallback even
    // though no representable count exceeds u32; no width saves payload there.
    let mut counts = Counters::zeros(1, usize::MAX);
    assert!(matches!(counts, Counters::Native(_)));
    counts.add(0, usize::MAX);
    counts.decrement(0);
    assert_eq!(counts.get(0), usize::MAX - 1);
}

#[test]
fn empty_counters_have_no_allocated_payload() {
    for bound in [0, usize::MAX] {
        let counts = Counters::zeros(0, bound);
        assert_eq!(counts.len(), 0);
        assert_eq!(counts.allocated_bytes(), 0);
    }
}

#[test]
fn counter_clones_own_independent_values() {
    for bound in [8, usize::MAX] {
        let mut original = Counters::zeros(2, bound);
        original.add(0, 3);
        let mut child = original.clone();
        child.decrement(0);
        child.add(1, 1);
        assert_eq!((original.get(0), original.get(1)), (3, 0));
        assert_eq!((child.get(0), child.get(1)), (2, 1));
    }
}

proptest! {
    #[test]
    fn stored_counts_match_native_arithmetic(
        operations in prop::collection::vec((0usize..4, any::<bool>()), 0..128),
    ) {
        let mut counts = Counters::zeros(4, operations.len());
        let mut reference = [0usize; 4];
        for (index, increase) in operations {
            if increase {
                counts.add(index, 1);
                reference[index] += 1;
            } else if reference[index] != 0 {
                counts.decrement(index);
                reference[index] -= 1;
            }
            for (index, expected) in reference.into_iter().enumerate() {
                prop_assert_eq!(counts.get(index), expected);
            }
        }
    }
}
