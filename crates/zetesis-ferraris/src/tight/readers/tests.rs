//! Packed incidence identity and its optional representation boundary.

use super::{Reader, ReaderKind};

#[test]
fn row_occurrences_keep_the_eight_byte_reader() {
    assert_eq!(size_of::<Reader>(), 8);
    let last = usize::try_from(Reader::ROW_MASK).unwrap();
    for row in [0, 1, last] {
        for occurrence in [0, 1, last] {
            let reader = Reader::row(row, occurrence).unwrap();
            assert_eq!(reader.kind(), ReaderKind::Row);
            assert_eq!(reader.row_occurrence(), (row, occurrence));
        }
    }
    assert!(Reader::row(last + 1, 0).is_err());
    assert!(Reader::row(0, last + 1).is_err());
}

#[test]
fn numeric_representation_limits_only_the_optional_index() {
    let length = usize::try_from(Reader::ROW_MASK).unwrap() + 1;
    assert!(Reader::rows_fit(length, length));
    assert!(!Reader::rows_fit(length + 1, 0));
    assert!(!Reader::rows_fit(0, length + 1));
    for kind in [ReaderKind::Node, ReaderKind::Producer, ReaderKind::Root] {
        let reader = Reader::new(kind, length + 1).unwrap();
        assert_eq!(reader.kind(), kind);
        assert_eq!(reader.index(), length + 1);
    }
}

#[test]
fn row_dedup_preserves_bits_across_word_boundaries() {
    let values = vec![0_u64; 3];
    let pointer = values.as_ptr();
    let capacity = values.capacity();
    let mut seen = super::Words::new(values);
    for row in [0, 63, 64, 127, 128, 129] {
        assert!(super::first(&mut seen, row).unwrap());
        assert!(!super::first(&mut seen, row).unwrap());
    }
    assert_eq!(&*seen, [1 | (1 << 63), 1 | (1 << 63), 3]);
    assert_eq!(seen.as_ptr(), pointer);
    assert_eq!(seen.capacity(), capacity);
}

#[test]
fn invalid_row_dedup_retains_the_completed_prefix() {
    let mut seen = super::Words::new(vec![0_u64; 2]);
    assert!(super::first(&mut seen, 63).unwrap());
    for row in [128, usize::MAX] {
        assert_eq!(
            super::first(&mut seen, row),
            Err(super::TightError::Stopped(super::Stop::InvalidProgram))
        );
        assert_eq!(&*seen, [1 << 63, 0]);
    }
    assert!(super::first(&mut seen, 64).unwrap());
    assert!(!super::first(&mut seen, 63).unwrap());
    assert_eq!(&*seen, [1 << 63, 1]);
    let mut empty = super::Words::new(Vec::<u64>::new());
    assert_eq!(
        super::first(&mut empty, 0),
        Err(super::TightError::Stopped(super::Stop::InvalidProgram))
    );
    assert_eq!(empty.capacity(), 0);
}
