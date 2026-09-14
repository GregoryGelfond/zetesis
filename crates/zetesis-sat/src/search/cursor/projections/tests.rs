use super::{NodeId, Projections, suffix_size};
use crate::search::cursor::tests::budget;
use crate::{AdmissionError, Assignment, Control, Incomplete};

fn blocked(index: &mut Projections) -> Vec<usize> {
    let control = Control::default();
    (0..1_usize << index.width)
        .filter(|bits| {
            let assignment = Assignment(
                (0..index.width)
                    .map(|variable| bits & (1 << variable) != 0)
                    .collect(),
            );
            !index.permits(&assignment, &mut budget(&control)).unwrap()
        })
        .collect()
}

fn first_key() -> Projections {
    let mut index = Projections::new(4, crate::ProjectionLimits::default()).unwrap();
    index
        .insert(4, |_| false, &mut budget(&Control::default()))
        .unwrap();
    index
}

#[test]
fn retained_trie_nodes_occupy_eight_bytes() {
    let index = first_key();
    assert_eq!(size_of_val(&index.nodes[0]), 8);
}

#[test]
fn node_id_endpoints_preserve_zero_based_positions() {
    let maximum = usize::try_from(u32::MAX).unwrap();
    for index in [0, 1, maximum - 1] {
        let node = NodeId::new(index).unwrap();
        assert_eq!(node.index(), Ok(index));
        assert_eq!(u64::from(node.0.get()), u64::try_from(index).unwrap() + 1);
    }
    assert_eq!(
        NodeId::new(maximum),
        Err(Incomplete::Admission(AdmissionError::Overflow))
    );
}

#[test]
fn the_last_representable_suffix_is_admitted() {
    let maximum = usize::try_from(u32::MAX).unwrap();
    // A terminal node completes the arena at the inclusive node-count bound.
    assert_eq!(suffix_size(4, 4, maximum - 1), Ok(1));
}

#[test]
fn host_length_overflow_precedes_key_reads() {
    let control = Control::default();
    let mut charged = budget(&control);
    let mut index = Projections::new(usize::MAX, crate::ProjectionLimits::default()).unwrap();
    assert_eq!(
        index.insert(usize::MAX, |_| panic!("no suffix read"), &mut charged),
        Err(Incomplete::CounterOverflow)
    );
    assert_eq!(charged.statistics.work, 1);
    assert_eq!(index.nodes.capacity(), 0);
    assert_eq!(index.entries, 0);
}

#[test]
#[cfg(target_pointer_width = "64")]
fn compact_index_overflow_precedes_allocation() {
    let control = Control::default();
    let mut charged = budget(&control);
    let width = usize::try_from(u32::MAX).unwrap();
    let mut index = Projections::new(width, crate::ProjectionLimits::default()).unwrap();
    // Host arithmetic fits, but width + the root exceeds compact node IDs.
    assert_eq!(
        index.insert(width, |_| panic!("no suffix read"), &mut charged),
        Err(Incomplete::Admission(AdmissionError::Overflow))
    );
    assert_eq!(charged.statistics.work, 1);
    assert_eq!(index.nodes.capacity(), 0);
    assert_eq!(index.entries, 0);
}

#[test]
fn cancellation_precedes_suffix_shape_refusal() {
    let control = Control::default();
    control.cancel();
    let mut charged = budget(&control);
    let mut index = Projections::new(usize::MAX, crate::ProjectionLimits::default()).unwrap();
    assert_eq!(
        index.insert(usize::MAX, |_| panic!("no key read"), &mut charged),
        Err(Incomplete::Cancelled)
    );
    assert_eq!(charged.statistics.work, 0);
    assert_eq!(index.nodes.capacity(), 0);
}

#[test]
fn every_interrupted_suffix_preserves_the_old_key_set() {
    let control = Control::default();
    // Diverge at the root and at the last bit of the existing all-false key.
    for key in [1, 8] {
        // One insertion tick, four bit visits and one terminal-node tick.
        for ceiling in 0..=6 {
            let mut index = first_key();
            let before = index.nodes.clone();
            let mut charged = budget(&control);
            charged.limits.max_work = ceiling;
            let result = index.insert(4, |bit| key & (1 << bit) != 0, &mut charged);
            assert_eq!(charged.statistics.work, ceiling);
            if ceiling < 6 {
                assert_eq!(result, Err(Incomplete::WorkLimit));
                assert_eq!(index.nodes, before);
                assert_eq!(blocked(&mut index), [0]);
            } else {
                assert_eq!(result, Ok(()));
                assert_eq!(blocked(&mut index), [0, key]);
            }
        }
    }
}

#[test]
fn cancellation_discards_a_partially_built_suffix() {
    let control = Control::default();
    let mut index = first_key();
    let before = index.nodes.clone();
    let mut charged = budget(&control);
    assert_eq!(
        index.insert(
            4,
            |bit| {
                if bit == 2 {
                    control.cancel();
                }
                true
            },
            &mut charged,
        ),
        Err(Incomplete::Cancelled)
    );
    // Insertion, the root branch, then two appended bits were charged.
    assert_eq!(charged.statistics.work, 4);
    assert_eq!(index.nodes, before);
    assert_eq!(blocked(&mut index), [0]);
}

#[test]
fn duplicate_keys_do_not_grow_the_arena() {
    let control = Control::default();
    let mut index = first_key();
    let before = index.nodes.clone();
    let mut charged = budget(&control);
    index.insert(4, |_| false, &mut charged).unwrap();
    assert_eq!(charged.statistics.work, 5);
    assert_eq!(index.nodes, before);
    assert_eq!(blocked(&mut index), [0]);
}
