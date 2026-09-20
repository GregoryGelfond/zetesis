use std::mem::size_of;

use super::{Frontier, PendingRegion};
use zetesis_ferraris::{AdmissionLimits, Knowledge, Narrower, Node, Region, Theory};

fn entry() -> PendingRegion {
    let theory = Theory::new(
        2,
        vec![Node::Atom(0), Node::Atom(1)],
        vec![],
        AdmissionLimits::default(),
    )
    .unwrap();
    let mut knowledge = Vec::with_capacity(4);
    knowledge.push(Narrower::new(&theory).knowledge());
    (Region::all_open(2), knowledge)
}

#[test]
fn frontier_counts_unused_entry_and_knowledge_slots() {
    let entry = entry();
    let entry_bytes = entry.0.retained_bytes()
        + size_of::<Vec<zetesis_ferraris::Knowledge>>() as u128
        + entry.1.iter().map(Knowledge::retained_bytes).sum::<u128>()
        + (entry.1.capacity() - entry.1.len()) as u128
            * size_of::<zetesis_ferraris::Knowledge>() as u128;
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
    active.1.push(active.1[0].clone());
    frontier.push(active);
    assert!(frontier.statistics.retained_bytes > initial.retained_bytes);
    assert_eq!(frontier.statistics.regions, 1);
    assert_eq!(
        frontier.statistics.peak_retained_bytes,
        frontier.statistics.retained_bytes
    );
}
