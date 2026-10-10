use super::*;
use themelios_base::source::SourceId;
use themelios_base::span::{ByteOffset, Location, Span};

fn location() -> ProgramSite {
    ProgramSite::source(Location {
        source: SourceId::new(74),
        span: Span::empty(ByteOffset::new(12)),
    })
}

fn completed(posting: &[usize], selected: u64) -> Receipt<'_> {
    let mut receipt = Receipt::default();
    receipt.start(&Probe::Indexed(delta::Rows::Posting(posting)), true);
    for position in 0..posting.len() {
        receipt.record(position, selected & (1 << position) != 0);
    }
    receipt.finish();
    receipt
}

fn replay(receipt: &Receipt<'_>, posting: &[usize]) -> Vec<usize> {
    let probe = Probe::Indexed(delta::Rows::Posting(posting));
    let mut counters = Counters::default();
    let mut position = 0;
    let mut result = Vec::new();
    while let Some(row) = receipt
        .next(
            &probe,
            &mut position,
            &FormulaLimits::default(),
            &mut counters,
            location(),
        )
        .unwrap()
    {
        result.push(row);
    }
    result
}

#[test]
fn selection_preserves_posting_order() {
    let posting = [2, 7, 12, 21, 33];
    let mut receipt = completed(&posting, 0b1_0101);
    receipt.start(&Probe::Indexed(delta::Rows::Posting(&posting)), true);
    assert!(receipt.replay);
    assert_eq!(replay(&receipt, &posting), [2, 12, 33]);
}

#[test]
fn full_word_preserves_last_position() {
    let posting = std::array::from_fn::<_, 64, _>(|index| 2 * index);
    let mut receipt = completed(&posting, 1_u64 << 63);
    receipt.start(&Probe::Indexed(delta::Rows::Posting(&posting)), true);
    assert!(receipt.replay);
    assert_eq!(replay(&receipt, &posting), [126]);
}

#[test]
fn empty_selection_finishes_without_row_visits() {
    let posting = [3, 8];
    let mut receipt = completed(&posting, 0);
    receipt.start(&Probe::Indexed(delta::Rows::Posting(&posting)), true);
    assert!(receipt.replay);
    assert!(replay(&receipt, &posting).is_empty());
}

#[test]
fn equal_contents_do_not_establish_posting_identity() {
    let posting = [3, 8];
    let another = posting;
    let mut receipt = completed(&posting, 0);
    receipt.start(&Probe::Indexed(delta::Rows::Posting(&another)), true);
    assert!(!receipt.replay);
    assert_eq!(replay(&receipt, &another), another);
}

#[test]
fn a_selection_gap_prevents_completion() {
    let posting = [3, 8, 11];
    let probe = Probe::Indexed(delta::Rows::Posting(&posting));
    let mut receipt = Receipt::default();
    receipt.start(&probe, true);
    receipt.record(0, false);
    // The filter failed on position 1. A resumed cursor may inspect position 2,
    // but that cannot supply the missing decision for position 1.
    receipt.record(2, false);
    receipt.finish();
    receipt.start(&probe, true);
    assert!(!receipt.replay);
    assert_eq!(replay(&receipt, &posting), posting);
}

#[test]
fn unfinished_selection_is_not_reused() {
    let posting = [3, 8];
    let probe = Probe::Indexed(delta::Rows::Posting(&posting));
    let mut receipt = Receipt::default();
    receipt.start(&probe, true);
    receipt.record(0, false);
    receipt.start(&probe, true);
    assert!(!receipt.replay);
    assert_eq!(replay(&receipt, &posting), posting);
}

#[test]
fn open_mode_change_discards_selection() {
    let posting = [3, 8];
    let mut receipt = completed(&posting, 0);
    receipt.start(&Probe::Indexed(delta::Rows::Posting(&posting)), false);
    assert!(!receipt.replay);
    assert_eq!(replay(&receipt, &posting), posting);
}

#[test]
fn unsupported_probes_discard_selection() {
    let posting = [3, 8];
    let too_wide = [0; 65];
    let mut receipt = completed(&posting, 0);
    for probe in [
        Probe::Indexed(delta::Rows::Interval(0..2)),
        Probe::Indexed(delta::Rows::Posting(&posting[..1])),
        Probe::Indexed(delta::Rows::Posting(&too_wide)),
    ] {
        receipt.start(&probe, true);
        assert!(!receipt.replay);
        assert!(receipt.posting.is_none());
    }
}

#[test]
fn stopped_replay_keeps_cursor_position() {
    let posting = [3, 8];
    let probe = Probe::Indexed(delta::Rows::Posting(&posting));
    let mut receipt = completed(&posting, 0b10);
    receipt.start(&probe, true);
    let mut position = 0;
    let mut counters = Counters::default();
    let stopped = FormulaLimits {
        max_work: 0,
        ..FormulaLimits::default()
    };
    assert!(
        receipt
            .next(&probe, &mut position, &stopped, &mut counters, location())
            .is_err()
    );
    assert_eq!(position, 0);
    assert_eq!(
        receipt
            .next(
                &probe,
                &mut position,
                &FormulaLimits::default(),
                &mut counters,
                location()
            )
            .unwrap(),
        Some(8)
    );
}
