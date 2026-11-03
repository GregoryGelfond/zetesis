//! Reservation preserves the pending owner until a split or return commits.

use std::cell::Cell;

use super::*;
use crate::region::{Entry, Region};

type RegionEntry = Entry<Packet>;
type RegionFrontier = Frontier<RegionEntry, Refused>;

fn root() -> RegionEntry {
    let mut entry = Entry::new(Region::all_open(3), packet(11));
    assert!(entry.region.hold(2));
    entry.prefer(0);
    entry
}

/// Full finite interpretation multiset; no deduplication can hide an overlap.
fn represented<'a>(entries: impl IntoIterator<Item = &'a RegionEntry>) -> Vec<usize> {
    let mut values = Vec::new();
    for entry in entries {
        for value in 0..(1_usize << entry.region.len()) {
            if (0..entry.region.len()).all(|atom| match entry.region.decision(atom) {
                Some(held) => (value & (1 << atom) != 0) == held,
                None => true,
            }) {
                values.push(value);
            }
        }
    }
    values.sort_unstable();
    values
}

#[test]
fn refused_publication_returns_the_exact_narrowed_parent() {
    let frontier = RegionFrontier::measured(1, root()).unwrap();
    let parent = frontier.take_local(0).unwrap();
    let region = parent.region.clone();
    let state_pointer = parent.state.bytes.as_ptr();
    let expected = represented([&parent]);
    let receipt = frontier.statistics();
    let invoked = Cell::new(false);
    let result = frontier.publish_split_with::<2, ()>(
        0,
        parent,
        |_, additional| {
            assert_eq!(additional, 2);
            assert!(!invoked.get());
            Err(Refused::Injected)
        },
        |_| {
            invoked.set(true);
            panic!("a refused reservation must not consume the parent")
        },
    );
    let (error, returned) = result.unwrap_err();
    assert_eq!(error, Refused::Injected);
    assert!(!invoked.get());
    assert_eq!(returned.region, region);
    assert_eq!(
        returned.split_atom(),
        Some(0),
        "preference stays with the parent"
    );
    assert_eq!(returned.state.id, 11);
    assert_eq!(returned.state.bytes.as_ptr(), state_pointer);
    assert_eq!(represented([&returned]), expected);
    assert_eq!(frontier.statistics(), receipt);
    assert_eq!(frontier.termination.unresolved(), 1);
    assert!(frontier.queue(0).is_empty());
    assert!(!frontier.termination.has_ended());
}

#[test]
fn admitted_children_partition_the_parent_in_existing_queue_order() {
    let frontier = RegionFrontier::measured(2, root()).unwrap();
    let parent = frontier.take_local(0).unwrap();
    let expected = represented([&parent]);
    let state_pointer = parent.state.bytes.as_ptr();
    let prepared = packet(12);
    let admitted = Cell::new(false);
    frontier
        .publish_split_with(
            0,
            parent,
            |queue, count| {
                assert_eq!(count, 2);
                reserve_entries::<_, Refused>(queue, count)?;
                admitted.set(true);
                Ok(())
            },
            |parent| {
                assert!(admitted.get(), "construct children only after admission");
                let (cut, held) = parent.split_with(0, |state| (prepared, state));
                ([cut, held], ())
            },
        )
        .unwrap();
    let held = frontier.take_local(0).unwrap();
    let cut = frontier.steal(1).unwrap();
    assert!(held.region.is_held(0));
    assert!(cut.region.is_cut(0));
    assert_eq!(held.state.bytes.as_ptr(), state_pointer);
    assert_eq!(held.state.id, 11);
    assert_eq!(cut.state.id, 12);
    assert_eq!(represented([&cut, &held]), expected);
    assert_eq!(frontier.termination.unresolved(), 2);
    assert_eq!(frontier.statistics().unwrap().regions, 0);
    assert!(!frontier.termination.has_ended());
}

#[test]
fn one_queued_child_and_the_held_hand_keep_the_partition() {
    let frontier = RegionFrontier::measured(1, root()).unwrap();
    let parent = frontier.take_local(0).unwrap();
    let expected = represented([&parent]);
    let prepared = packet(12);
    let held = frontier
        .publish_split_with(0, parent, reserve_entries::<_, Refused>, |parent| {
            let (cut, held) = parent.split_with(0, |state| (prepared, state));
            ([cut], held)
        })
        .unwrap();
    assert!(held.region.is_held(0));
    assert_eq!(
        frontier.termination.unresolved(),
        2,
        "one child stays in hand"
    );
    assert_eq!(frontier.statistics().unwrap().regions, 1);
    let cut = frontier.take_local(0).unwrap();
    assert!(cut.region.is_cut(0));
    assert_eq!(represented([&cut, &held]), expected);
}

#[test]
fn a_stolen_entry_survives_a_refused_zero_capacity_return() {
    let frontier = RegionFrontier::measured(2, root()).unwrap();
    let parent = frontier.steal(1).unwrap();
    let expected = represented([&parent]);
    let state_pointer = parent.state.bytes.as_ptr();
    let receipt = frontier.statistics();
    assert_eq!(frontier.queue(1).capacity(), 0);
    let result = frontier.requeue(1, parent, |queue, additional| {
        assert_eq!(additional, 1);
        reserve_entries::<_, Refused>(queue, usize::MAX)
    });
    let (error, returned) = result.unwrap_err();
    assert_eq!(error, Refused::Stopped(Stop::Allocation));
    assert_eq!(returned.state.bytes.as_ptr(), state_pointer);
    assert_eq!(returned.split_atom(), Some(0));
    assert_eq!(represented([&returned]), expected);
    assert_eq!(frontier.statistics(), receipt);
    assert_eq!(frontier.termination.unresolved(), 1);
    assert!(frontier.queue(0).is_empty());
    assert!(frontier.queue(1).is_empty());
    assert!(!frontier.termination.has_ended());
}

#[test]
fn a_refused_shared_home_return_keeps_queued_and_in_hand_coverage() {
    let frontier = RegionFrontier::measured(2, root()).unwrap();
    let parent = frontier.take_local(0).unwrap();
    let expected = represented([&parent]);
    let prepared = packet(12);
    // Shared-order producers publish to deque zero; that queue is no longer
    // empty when this producer tries to return its held hand there.
    let held = frontier
        .publish_split_with(0, parent, reserve_entries::<_, Refused>, |parent| {
            let (cut, held) = parent.split_with(0, |state| (prepared, state));
            ([cut], held)
        })
        .unwrap();
    let state_pointer = held.state.bytes.as_ptr();
    let receipt = frontier.statistics();
    let result = frontier.requeue(0, held, |queue, additional| {
        assert_eq!(additional, 1);
        assert_eq!(queue.len(), 1);
        Err(Refused::Injected)
    });
    let (error, returned) = result.unwrap_err();
    assert_eq!(error, Refused::Injected);
    assert_eq!(returned.state.bytes.as_ptr(), state_pointer);
    assert!(returned.region.is_held(0));
    assert_eq!(frontier.statistics(), receipt);
    assert_eq!(frontier.termination.unresolved(), 2);
    let queued = frontier.take_local(0).unwrap();
    assert!(queued.region.is_cut(0));
    assert_eq!(represented([&queued, &returned]), expected);
    assert!(!frontier.termination.has_ended());
}
