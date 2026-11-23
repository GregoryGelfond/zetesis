use super::*;

#[test]
fn a_rejected_capacity_proposal_does_not_raise_the_peak() {
    let header = Storage::new(usize::MAX).unwrap().current_bytes();
    let storage = Storage::new(usize::try_from(header).unwrap() + 16).unwrap();
    let mut values = Vec::<u32>::new();
    assert!(matches!(
        storage.reserve(&mut values, 8, true),
        Err(Refusal::Storage { .. })
    ));
    assert_eq!(values.capacity(), 0);
    assert_eq!(storage.current_bytes(), header);
    assert_eq!(storage.peak_bytes(), header);
}

#[test]
fn rounded_table_growth_is_refused_before_allocation() {
    let header = Storage::new(usize::MAX).unwrap().current_bytes();
    let storage = Storage::new(usize::try_from(header).unwrap() + 20).unwrap();
    let mut table = HashTable::<u32>::new();
    let result = storage.table(&mut table, 4, |&word| u64::from(word));
    assert!(matches!(result, Err(Refusal::Storage { required, .. }) if required == header + 35));
    assert_eq!(table.capacity(), 0);
    assert_eq!(storage.current_bytes(), header);
    assert_eq!(storage.peak_bytes(), header);
}

#[test]
fn prospective_table_capacity_matches_the_pinned_growth_rule() {
    let storage = Storage::new(usize::MAX).unwrap();
    for needed in 1..4096 {
        let mut table = HashTable::<u32>::new();
        storage
            .table(&mut table, needed, |&word| u64::from(word))
            .unwrap();
        assert_eq!(table.capacity(), table_capacity(needed).unwrap());
        storage.release(super::super::table_bytes(&table));
    }
}

#[test]
fn refused_observed_capacity_is_retained_in_the_receipt() {
    let mut state = State {
        current: 12,
        peak: 12,
        limit: 24,
    };
    assert!(matches!(
        observe(&mut state, 8, 16),
        Err(Refusal::Storage {
            required: 28,
            limit: 24
        })
    ));
    assert_eq!(state.current, 20);
    assert_eq!(state.peak, 28);
}
