//! The shared frontier protocols and the exact receipts of a measured frontier.

use std::mem::{align_of, size_of};
use std::sync::Mutex;

use super::super::regions::RegionFrontierStatistics;

use zetesis_ferraris::{AdmissionLimits, Narrower, Node, Theory};

use super::*;

fn index() -> Narrower {
    let theory = Theory::new(
        2,
        zetesis_ferraris::FormulaParts::new(vec![Node::atom(0), Node::atom(1)], vec![]).unwrap(),
        vec![],
        AdmissionLimits::default(),
    )
    .unwrap();
    Narrower::new(&theory)
}

/// A region after its first narrowing, knowing only the original theory.
fn entry() -> Entry {
    let mut knowledge = CandidateKnowledge::default();
    knowledge.permanent(0, &index()).unwrap();
    Entry(Region::all_open(2), knowledge)
}

/// An entry's header and owned payload, computed from its parts rather than
/// from the generic payload method.
fn entry_bytes(Entry(region, knowledge): &Entry) -> u128 {
    region.retained_bytes() + size_of::<CandidateKnowledge>() as u128 + knowledge.allocated_bytes()
}

/// The deque vector's and deques' headers of a frontier of `workers`.
fn headers(workers: usize) -> u128 {
    (size_of::<Vec<Mutex<VecDeque<Entry>>>>() + workers * size_of::<Mutex<VecDeque<Entry>>>())
        as u128
}

fn receipts(frontier: &RegionFrontier) -> RegionFrontierStatistics {
    frontier.statistics().unwrap()
}

#[test]
fn receipts_count_unused_entry_capacity() {
    let root = entry();
    let bytes = entry_bytes(&root);
    let frontier = RegionFrontier::measured(1, root).unwrap();
    let _hand = frontier.take_local(0).unwrap();
    frontier
        .publish_split(0, [entry()], |queue, additional| {
            reserve_regions(queue, additional + 7)
        })
        .unwrap();
    let observed = receipts(&frontier);
    assert_eq!(observed.regions, 1);
    assert!(observed.capacity >= 8);
    assert_eq!(
        observed.retained_bytes,
        headers(1) + bytes + (observed.capacity - 1) as u128 * size_of::<Entry>() as u128
    );
}

#[test]
fn removing_entries_keeps_capacity_and_peaks() {
    let frontier = RegionFrontier::measured(1, entry()).unwrap();
    let _hand = frontier.take_local(0).unwrap();
    frontier
        .publish_split(0, [entry(), entry()], reserve_regions)
        .unwrap();
    let populated = receipts(&frontier);
    assert_eq!(populated.peak_regions, 2);
    let _first = frontier.take_local(0).unwrap();
    let _second = frontier.take_local(0).unwrap();
    let empty = receipts(&frontier);
    assert_eq!(empty.regions, 0);
    assert_eq!(empty.capacity, populated.capacity);
    assert_eq!(
        empty.retained_bytes,
        headers(1) + empty.capacity as u128 * size_of::<Entry>() as u128
    );
    assert_eq!(empty.peak_regions, 2);
    assert_eq!(empty.peak_capacity, populated.capacity);
    assert_eq!(empty.peak_retained_bytes, populated.retained_bytes);
}

#[test]
fn a_returned_region_counts_its_new_knowledge_capacity() {
    let frontier = RegionFrontier::measured(1, entry()).unwrap();
    let initial = receipts(&frontier);
    let mut hand = frontier.take_local(0).unwrap();
    assert!(hand.0.hold(1));
    // Restrictions added while a region was pending acquire fresh knowledge
    // only when it is narrowed, outside the frontier's ownership.
    hand.1.permanent(1, &index()).unwrap();
    frontier.requeue(0, hand, reserve_regions).unwrap();
    let returned = receipts(&frontier);
    assert!(returned.retained_bytes > initial.retained_bytes);
    assert_eq!(returned.regions, 1);
    assert_eq!(returned.peak_retained_bytes, returned.retained_bytes);
}

#[test]
fn receipts_follow_entries_across_steals_and_deques() {
    let frontier = RegionFrontier::measured(2, entry()).unwrap();
    let stolen = frontier.steal(1).unwrap();
    frontier.requeue(1, stolen, reserve_regions).unwrap();
    let moved = receipts(&frontier);
    assert_eq!(moved.regions, 1);
    let capacity = frontier.queue(0).capacity() + frontier.queue(1).capacity();
    assert_eq!(moved.capacity, capacity);
    assert_eq!(
        moved.retained_bytes,
        headers(2) + entry_bytes(&entry()) + (capacity - 1) as u128 * size_of::<Entry>() as u128
    );
}

#[test]
fn the_peak_is_the_maximum_of_the_counter_history() {
    let frontier = RegionFrontier::measured(2, entry()).unwrap();
    let mut history = vec![1usize];
    let mut hand = frontier.take_local(0).unwrap();
    history.push(0);
    for step in 0..6 {
        if step % 3 == 2 {
            frontier.requeue(step % 2, hand, reserve_regions).unwrap();
            hand = frontier.take_local(step % 2).unwrap();
            history.extend([*history.last().unwrap() + 1, *history.last().unwrap()]);
        } else {
            frontier
                .publish_split(step % 2, [entry(), entry()], reserve_regions)
                .unwrap();
            history.push(history.last().unwrap() + 2);
            let _taken = frontier.take_local(step % 2).unwrap();
            history.push(history.last().unwrap() - 1);
        }
    }
    let observed = receipts(&frontier);
    assert_eq!(observed.regions, *history.last().unwrap());
    assert_eq!(observed.peak_regions, *history.iter().max().unwrap());
}

#[test]
fn the_candidate_entry_preserves_tuple_layout_and_payload() {
    assert_eq!(
        size_of::<Entry>(),
        size_of::<(Region, CandidateKnowledge)>()
    );
    assert_eq!(
        align_of::<Entry>(),
        align_of::<(Region, CandidateKnowledge)>()
    );
    let entry = entry();
    assert_eq!(
        entry.allocated_bytes(),
        entry_bytes(&entry) - size_of::<Entry>() as u128
    );
}
