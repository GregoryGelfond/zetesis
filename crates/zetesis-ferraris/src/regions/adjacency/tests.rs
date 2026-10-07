use std::mem::size_of;

use proptest::prelude::*;

use super::Adjacency;
use zetesis_cpu::Stop;

#[test]
fn rows_preserve_order_and_duplicate_occurrences() {
    let edges = [(3, 9), (1, 4), (3, 2), (1, 4), (3, 9), (0, 7)];
    let adjacency = Adjacency::build(5, edges.into_iter()).unwrap();
    assert_eq!(
        adjacency.iter().collect::<Vec<_>>(),
        vec![&[7][..], &[4, 4][..], &[][..], &[9, 2, 9][..], &[][..]]
    );
}

#[test]
fn no_rows_has_one_boundary_and_no_entries() {
    let adjacency = Adjacency::build(0, std::iter::empty()).unwrap();
    assert_eq!(adjacency.offsets, [0]);
    assert!(adjacency.entries.is_empty());
    assert_eq!(adjacency.len(), 0);
    assert_eq!(adjacency.iter().count(), 0);
}

#[test]
fn unrepresentable_row_storage_refuses_before_allocation() {
    assert!(matches!(
        Adjacency::build(usize::MAX, std::iter::empty()),
        Err(Stop::Allocation)
    ));
    assert!(matches!(
        Adjacency::build(usize::MAX / size_of::<usize>(), std::iter::empty()),
        Err(Stop::Allocation)
    ));
}

#[test]
fn sparse_rows_remove_per_row_vector_headers() {
    let rows = 1024;
    let edges = [(0, 3), (512, 4), (1023, 5)];
    let adjacency = Adjacency::build(rows, edges.into_iter()).unwrap();
    let compact = size_of::<Adjacency>()
        + (adjacency.offsets.capacity() + adjacency.entries.capacity()) * size_of::<usize>();
    let old_headers = size_of::<Vec<Vec<usize>>>() + rows * size_of::<Vec<usize>>();
    assert!(compact < old_headers);
}

proptest! {
    #[test]
    fn compact_rows_equal_the_ordered_edge_subsequences(
        edges in prop::collection::vec((0_usize..16, any::<usize>()), 0..128),
    ) {
        let adjacency = Adjacency::build(16, edges.iter().copied()).unwrap();
        for row in 0..16 {
            let expected: Vec<_> = edges.iter()
                .filter_map(|&(source, value)| (source == row).then_some(value))
                .collect();
            prop_assert_eq!(&adjacency[row], expected.as_slice());
        }
    }
}

#[test]
fn failed_edge_reads_keep_their_original_stop() {
    for stop in [Stop::InvalidProgram, Stop::Allocation] {
        let edges = [Ok((0, 7)), Err(stop)].into_iter();
        assert_eq!(Adjacency::try_build(1, edges).unwrap_err(), stop);
    }
}
