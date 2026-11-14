//! When a table that only narrows a probe is worth preparing.

use zetesis_core::table;

use super::super::TableCost;

fn cost(entries: u128, mask_words: u128, preparation_bytes: u128) -> TableCost {
    TableCost {
        entries,
        mask_words,
        preparation_bytes,
    }
}

#[test]
fn a_table_is_small_up_to_the_mask_bound() {
    // 1440 distinct keys over 1440 rows: 1440 masks of 45 words.
    assert!(cost(1_440, 64_800, 0).small());
    assert!(cost(1, 65_536, 0).small());
    assert!(!cost(1, 65_537, 0).small());
}

#[test]
fn rows_read_repay_a_table_at_eight_words_a_row() {
    assert!(!cost(1, 801, 0).repaid(100));
    assert!(cost(1, 800, 0).repaid(100));
    assert!(!cost(1, 1, 0).repaid(0));
    assert!(cost(1, u128::from(u64::MAX) * 8, 0).repaid(u64::MAX));
}

#[test]
fn a_table_fits_within_the_entries_and_the_bytes_left() {
    let allowance = |max_entries, max_bytes| table::Limits {
        max_entries,
        max_bytes,
        max_work: 0,
    };
    // Ten entries, and a preparation charged 400 bytes beside a relation of
    // 50; the masks' own words are not what is weighed.
    let table = cost(10, 1_000_000, 400);
    assert!(table.fits(50, &allowance(10, 450)));
    assert!(!table.fits(50, &allowance(9, 450)));
    assert!(!table.fits(50, &allowance(10, 449)));
    assert!(!table.fits(51, &allowance(10, 450)));
}
