use std::mem::size_of;

use proptest::prelude::*;

use super::{Count, Counters, compact_fits};

#[test]
fn compact_width_includes_its_maximum_count() {
    let maximum = usize::try_from(u32::MAX).unwrap();
    assert_eq!(compact_fits(maximum), size_of::<u32>() < size_of::<usize>());
    let mut counts = Counters::<u32>::zeros(1);
    counts.add(0, maximum);
    counts.decrement(0);
    counts.add(0, 1);
    assert_eq!(counts.get(0), maximum);
}

#[test]
fn native_width_preserves_its_maximum_count() {
    // On narrower hosts the native representation remains the fallback even
    // though no representable count exceeds u32; no width saves payload there.
    assert!(!compact_fits(usize::MAX));
    let mut counts = Counters::<usize>::zeros(1);
    counts.add(0, usize::MAX);
    counts.decrement(0);
    assert_eq!(counts.get(0), usize::MAX - 1);
}

#[test]
fn empty_counters_have_no_allocated_payload() {
    let compact = Counters::<u32>::zeros(0);
    let native = Counters::<usize>::zeros(0);
    assert_eq!((compact.len(), compact.allocated_bytes()), (0, 0));
    assert_eq!((native.len(), native.allocated_bytes()), (0, 0));
}

fn clones_are_independent<C: Count>() {
    let mut original = Counters::<C>::zeros(2);
    original.add(0, 3);
    let mut child = original.clone();
    child.decrement(0);
    child.add(1, 1);
    assert_eq!((original.get(0), original.get(1)), (3, 0));
    assert_eq!((child.get(0), child.get(1)), (2, 1));
}

#[test]
fn counter_clones_own_independent_values() {
    clones_are_independent::<u32>();
    clones_are_independent::<usize>();
}

fn reuse<C: Count>() {
    let mut source = Counters::<C>::zeros(2);
    source.add(0, 3);
    let mut destination = Counters::<C>::zeros(2);
    destination.add(1, 7);
    let before = destination.0.as_ptr();
    destination.clone_from(&source);
    assert_eq!(destination.0.as_ptr(), before);
    assert_ne!(destination.0.as_ptr(), source.0.as_ptr());
    assert_eq!((destination.get(0), destination.get(1)), (3, 0));
    destination.decrement(0);
    source.add(1, 1);
    assert_eq!((source.get(0), source.get(1)), (3, 1));
    assert_eq!((destination.get(0), destination.get(1)), (2, 0));
}

#[test]
fn equal_length_copies_reuse_independent_counter_storage() {
    reuse::<u32>();
    reuse::<usize>();
}

fn follow<C: Count>(operations: &[(usize, bool)]) -> Result<(), TestCaseError> {
    let mut counts = Counters::<C>::zeros(4);
    let mut reference = [0usize; 4];
    for &(index, increase) in operations {
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
    Ok(())
}

proptest! {
    #[test]
    fn stored_counts_match_native_arithmetic(
        operations in prop::collection::vec((0usize..4, any::<bool>()), 0..128),
    ) {
        follow::<u32>(&operations)?;
        follow::<usize>(&operations)?;
    }
}
