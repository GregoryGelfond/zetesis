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
