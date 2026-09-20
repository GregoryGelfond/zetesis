use std::mem::size_of;

use super::{Knowledge, Known};

#[test]
fn empty_worklists_retain_their_allocated_bytes() {
    let mut knowledge = Knowledge {
        known: Known::empty(5, 2, vec![0; 3]),
    };
    let original = knowledge.retained_bytes();
    knowledge.known.learned.reserve_exact(7);
    knowledge.known.nodes.reserve_exact(11);
    knowledge.known.heads.reserve_exact(13);
    let worklists = knowledge.known.learned.capacity() as u128 * size_of::<usize>() as u128
        + knowledge.known.nodes.capacity() as u128 * size_of::<(usize, bool)>() as u128
        + knowledge.known.heads.capacity() as u128 * size_of::<usize>() as u128;
    assert_eq!(knowledge.retained_bytes(), original + worklists);
    knowledge.known.learned.push(1);
    knowledge.known.nodes.push((1, true));
    knowledge.known.heads.push(1);
    knowledge.known.learned.clear();
    knowledge.known.nodes.clear();
    knowledge.known.heads.clear();
    assert_eq!(knowledge.retained_bytes(), original + worklists);
}
