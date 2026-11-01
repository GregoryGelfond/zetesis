use super::*;

const FIXED: ResidualReason = ResidualReason::FixedPoint;
const ROUND: ResidualReason = ResidualReason::RoundLimit;

#[test]
fn no_residuals_need_no_storage() {
    let mut records = DecodeJournal::default();
    records.release_unless_fits(0);
    assert_eq!(records.rows(), Some([].as_slice()));
    assert_eq!(records.bytes(), 0);
}

#[test]
fn a_refused_journal_stays_unavailable_until_retry() {
    let mut records = DecodeJournal::default();
    let requested = u64::try_from(payload(5)).unwrap();
    records.record(2, 5, FIXED, 7, requested - 1);
    assert!(records.rows().is_none());
    records.record(4, 5, ROUND, 9, u64::MAX);
    assert!(records.rows().is_none());
    assert_eq!(records.bytes(), 0);
    let mut retry = DecodeJournal::default();
    retry.record(4, 5, ROUND, 9, u64::MAX);
    assert_eq!(retry.rows().unwrap()[0].slot, 4);
}

#[test]
fn sparse_submissions_retain_their_original_slots() {
    let mut records = DecodeJournal::default();
    records.record(1, 9, ROUND, 70, u64::MAX);
    let limit = records.bytes();
    assert!(u128::from(limit) >= payload(9));
    let base = records.rows().unwrap().as_ptr();
    records.record(7, 9, FIXED, 0, limit);
    records.record(8, 9, ResidualReason::WorkLimit, 64, limit);
    assert_eq!(records.rows().unwrap().as_ptr(), base);
    assert_eq!(records.bytes(), limit);
    assert_eq!(
        records.rows().unwrap(),
        [
            DecodedSlot {
                slot: 1,
                reason: ROUND,
                open_atoms: 70
            },
            DecodedSlot {
                slot: 7,
                reason: FIXED,
                open_atoms: 0
            },
            DecodedSlot {
                slot: 8,
                reason: ResidualReason::WorkLimit,
                open_atoms: 64
            },
        ]
    );
}

#[test]
fn a_later_row_cannot_grow_the_original_reservation() {
    let mut records = DecodeJournal::default();
    records.record(0, 1, ROUND, 9, u64::MAX);
    let actual = records.slots.as_ref().unwrap().capacity();
    records.record(actual, actual + 1, ROUND, 9, u64::MAX);
    assert!(records.rows().is_none());
    assert_eq!(records.bytes(), 0);
}

#[test]
fn transfer_keeps_only_an_admitted_journal() {
    let mut records = DecodeJournal::default();
    records.record(1, 2, FIXED, 5, u64::MAX);
    let actual = records.bytes();
    records.release_unless_fits(actual);
    assert_eq!(records.rows().unwrap()[0].open_atoms, 5);
    records.release_unless_fits(actual - 1);
    assert!(records.rows().is_none());
    assert_eq!(records.bytes(), 0);
    records.record(0, 2, ROUND, 0, u64::MAX);
    assert!(records.rows().is_none());
}

#[test]
fn disordered_rows_cannot_be_paired_with_queries() {
    for invalid in [1, 2, 8] {
        let mut records = DecodeJournal::default();
        records.record(2, 8, ROUND, 9, u64::MAX);
        records.record(invalid, 8, FIXED, 4, u64::MAX);
        assert!(records.rows().is_none());
    }
}
