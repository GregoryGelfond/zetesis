use std::mem::size_of;

use super::{CandidateKnowledge, Frontier, PendingRegion};
use zetesis_ferraris::{AdmissionLimits, Narrower, Node, Region, Theory};

fn index() -> Narrower {
    let theory = Theory::new(
        2,
        vec![Node::Atom(0), Node::Atom(1)],
        vec![],
        AdmissionLimits::default(),
    )
    .unwrap();
    Narrower::new(&theory)
}

/// A region after its first narrowing, knowing only the original theory.
fn entry() -> PendingRegion {
    let mut knowledge = CandidateKnowledge::default();
    knowledge.permanent(0, &index()).unwrap();
    (Region::all_open(2), knowledge)
}

#[test]
fn frontier_counts_unused_entry_slots() {
    let entry = entry();
    let entry_bytes = entry.0.retained_bytes()
        + size_of::<CandidateKnowledge>() as u128
        + entry.1.allocated_bytes();
    let mut frontier = Frontier::new(entry).unwrap();
    frontier.try_reserve(7).unwrap();
    let observed = frontier.statistics;
    assert_eq!(observed.regions, 1);
    assert!(observed.capacity >= 8);
    assert_eq!(
        observed.retained_bytes,
        size_of::<Vec<PendingRegion>>() as u128
            + entry_bytes
            + (observed.capacity - 1) as u128 * size_of::<PendingRegion>() as u128
    );
}

#[test]
fn removing_regions_preserves_capacity_and_peak() {
    let mut frontier = Frontier::new(entry()).unwrap();
    frontier.try_reserve(1).unwrap();
    frontier.push(entry());
    let populated = frontier.statistics;
    assert_eq!(populated.peak_regions, 2);
    let _active = frontier.pop().unwrap();
    let _other = frontier.pop().unwrap();
    let empty = frontier.statistics;
    assert_eq!(empty.regions, 0);
    assert_eq!(empty.capacity, populated.capacity);
    assert_eq!(
        empty.retained_bytes,
        size_of::<Vec<PendingRegion>>() as u128
            + empty.capacity as u128 * size_of::<PendingRegion>() as u128
    );
    assert_eq!(empty.peak_retained_bytes, populated.retained_bytes);
    assert_eq!(empty.peak_regions, 2);
    assert_eq!(empty.peak_capacity, populated.capacity);
}

#[test]
fn returning_active_knowledge_counts_its_new_capacity() {
    let mut frontier = Frontier::new(entry()).unwrap();
    let initial = frontier.statistics;
    let mut active = frontier.pop().unwrap();
    assert!(active.0.hold(1));
    // Restrictions added while a region was pending acquire fresh knowledge
    // only when it is narrowed, outside the frontier's ownership.
    active.1.permanent(1, &index()).unwrap();
    frontier.push(active);
    assert!(frontier.statistics.retained_bytes > initial.retained_bytes);
    assert_eq!(frontier.statistics.regions, 1);
    assert_eq!(
        frontier.statistics.peak_retained_bytes,
        frontier.statistics.retained_bytes
    );
}
