use std::mem::size_of;

use zetesis_cpu::Cancellation;

use super::super::{
    budget::{Budget, BudgetError},
    storage::Storage,
};
use crate::regions::Work;

#[test]
fn simultaneous_growth_keeps_both_actual_capacities() {
    let cancellation = Cancellation::default();
    let mut budget = Budget::new(Work::new(u64::MAX), u64::MAX, usize::MAX, &cancellation);
    let mut storage = Storage::default();
    let mut values = storage.reserve::<u64>(1, &mut budget).unwrap();
    storage.push(&mut values, 7, 1, &mut budget).unwrap();
    let old = size_of::<Vec<u64>>() + values.capacity() * size_of::<u64>();
    storage.push(&mut values, 11, 1, &mut budget).unwrap();
    let new = size_of::<Vec<u64>>() + values.capacity() * size_of::<u64>();
    assert_eq!(values, [7, 11]);
    assert_eq!(storage.bytes(), new as u128);
    assert_eq!(budget.peak_bytes(), (old + new) as u128);
    storage.release(values).unwrap();
    assert_eq!(storage.bytes(), 0);
}

#[test]
fn embedded_payload_does_not_count_another_header() {
    let cancellation = Cancellation::default();
    let mut budget = Budget::new(Work::new(u64::MAX), u64::MAX, usize::MAX, &cancellation);
    let mut storage = Storage::default();
    let inner = storage.filled(3, 1_u64, 1, &mut budget).unwrap();
    let payload = inner.capacity() * size_of::<u64>();
    let mut outer = storage.reserve(1, &mut budget).unwrap();
    storage.push(&mut outer, inner, 3, &mut budget).unwrap();
    storage.embed::<u64>().unwrap();
    let enclosing = size_of::<Vec<Vec<u64>>>() + outer.capacity() * size_of::<Vec<u64>>();
    assert_eq!(storage.bytes(), (enclosing + payload) as u128);
    let inner = outer.pop().unwrap();
    storage.release_embedded(inner).unwrap();
    assert_eq!(storage.bytes(), enclosing as u128);
    storage.release(outer).unwrap();
    assert_eq!(storage.bytes(), 0);
}

#[test]
fn refused_growth_does_not_drain_the_old_buffer() {
    let cancellation = Cancellation::default();
    let mut budget = Budget::new(Work::new(u64::MAX), 8, usize::MAX, &cancellation);
    let mut storage = Storage::default();
    let mut values = storage.reserve::<u64>(1, &mut budget).unwrap();
    storage.push(&mut values, 7, 1, &mut budget).unwrap();
    // Three setup operations and three for the first push are spent. The next
    // capacity check fits, but its new header cannot be initialized.
    assert_eq!(
        storage.push(&mut values, 11, 1, &mut budget),
        Err(BudgetError::WorkDeclined)
    );
    assert_eq!(values, [7]);
    assert_eq!(budget.work(), 7);
    assert_eq!(
        storage.bytes(),
        (size_of::<Vec<u64>>() + size_of::<u64>()) as u128
    );
}

#[test]
fn fixed_scratch_is_absent_from_later_storage_requirements() {
    let cancellation = Cancellation::default();
    let mut budget = Budget::new(Work::new(u64::MAX), u64::MAX, 100, &cancellation);
    let mut storage = Storage::with_live_bytes(12);
    storage.admit_record(80, &mut budget).unwrap();
    storage.release_record(80).unwrap();
    storage.admit_record(88, &mut budget).unwrap();
    assert_eq!((storage.bytes(), budget.peak_bytes()), (100, 100));
    assert_eq!(
        storage.admit_record(1, &mut budget),
        Err(BudgetError::StorageDeclined {
            required: 101,
            limit: 100
        })
    );
    assert_eq!(storage.bytes(), 100);
    assert_eq!(budget.peak_bytes(), 101);
}
