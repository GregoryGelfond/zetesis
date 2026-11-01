//! A refused scalar split keeps its unresolved partition and lineage.

mod copy;
mod inspection;

use std::cell::Cell;
use std::rc::Rc;

use super::*;

#[derive(Debug, PartialEq, Eq)]
enum Failure {
    Stopped(Stop),
    Narrowing(u32),
}

impl From<Stop> for Failure {
    fn from(stop: Stop) -> Self {
        Self::Stopped(stop)
    }
}

fn reserve<S>(pending: &mut Vec<Entry<S>>, additional: usize) -> Result<(), Failure> {
    pending
        .try_reserve(additional)
        .map_err(|_| Failure::from(Stop::Allocation))
}

fn refuse<S>(pending: &mut Vec<Entry<S>>, additional: usize) -> Result<(), Failure> {
    assert_eq!(
        additional, 2,
        "the existing scalar split reserves two slots"
    );
    // A nonzero entry size makes this a deterministic capacity overflow, not
    // an attempt to exhaust the host. Exercise the production error conversion.
    reserve(pending, usize::MAX)
}

fn unchanged() -> impl FnMut(&mut Entry<()>) -> Result<Narrowing, Failure> {
    |_| Ok(Narrowing::Fixed { changed: false })
}

fn represented(region: &Region) -> Vec<usize> {
    (0..1 << region.len())
        .filter(|mask| {
            region
                .decided()
                .all(|(atom, held)| (mask & (1 << atom) != 0) == held)
        })
        .collect()
}

fn leaf(visit: Option<Visit>) -> usize {
    let Some(Visit::Leaf(region, ())) = visit else {
        panic!("a leaf remains")
    };
    let values = represented(&region);
    assert_eq!(values.len(), 1);
    values[0]
}

#[test]
fn a_refused_root_split_keeps_the_whole_partition() {
    for held_first in [false, true] {
        let mut walk = Walk::new(Region::all_open(3), Counting::Never);
        walk.held_first = held_first;
        assert_eq!(
            walk.next_with_reservation(unchanged(), refuse),
            Err(Failure::Stopped(Stop::Allocation))
        );
        assert_eq!(walk.pending(), 1);
        assert!(
            walk.root,
            "an uncommitted split does not consume root policy"
        );
        assert_eq!(
            walk.statistics(),
            RegionStatistics {
                regions: 1,
                refuted: 0,
                leaves: 0,
                counted: 0
            }
        );
        // The receipt is the existing attempted visit receipt, not a claim that
        // children were published. Do not silently change decision accounting.
        assert_eq!(
            walk.statistics().splits_since(RegionStatistics::default()),
            1
        );
        let pending = walk.surrender();
        assert_eq!(pending.len(), 1);
        assert_eq!(represented(&pending[0].region), (0..8).collect::<Vec<_>>());
        assert_eq!(walk.next(unchanged()), Ok(None));
        assert_eq!(walk.statistics().regions, 1);
    }
}

#[derive(Debug)]
struct Lineage {
    visits: usize,
    clones: Rc<Cell<usize>>,
}

impl Clone for Lineage {
    fn clone(&self) -> Self {
        self.clones.set(self.clones.get() + 1);
        Self {
            visits: self.visits,
            clones: Rc::clone(&self.clones),
        }
    }
}

#[test]
fn a_refused_split_keeps_narrowing_and_does_not_clone_lineage() {
    let clones = Rc::new(Cell::new(0));
    let state = Lineage {
        visits: 0,
        clones: Rc::clone(&clones),
    };
    let mut walk = Walk::with_state(Region::all_open(4), Counting::Never, state);
    let result = walk.next_with_reservation(
        |entry| {
            assert!(entry.region.hold(0));
            assert!(entry.region.cut(3));
            entry.prefer(1);
            entry.state.visits += 1;
            Ok::<_, Failure>(Narrowing::Fixed { changed: true })
        },
        refuse,
    );
    assert!(matches!(result, Err(Failure::Stopped(Stop::Allocation))));
    assert_eq!(clones.get(), 0);
    let pending = walk.surrender();
    assert_eq!(pending.len(), 1);
    let region = &pending[0].region;
    let state = &pending[0].state;
    assert_eq!(
        region.decided().collect::<Vec<_>>(),
        vec![(0, true), (3, false)]
    );
    assert_eq!(
        pending[0].split_atom(),
        Some(1),
        "the preference has not been consumed"
    );
    assert_eq!(state.visits, 1);
    assert_eq!(represented(region), vec![1, 3, 5, 7]);
    assert_eq!(walk.statistics().regions, 1);
}

#[test]
fn a_later_refused_split_surrenders_every_unvisited_leaf_once() {
    for held_first in [false, true] {
        let mut walk = Walk::new(Region::all_open(3), Counting::Never);
        walk.held_first = held_first;
        let mut emitted = vec![leaf(walk.next(unchanged()).unwrap())];
        emitted.push(leaf(walk.next(unchanged()).unwrap()));
        let before = walk.statistics();
        assert_eq!(
            walk.next_with_reservation(unchanged(), refuse),
            Err(Failure::Stopped(Stop::Allocation))
        );
        assert_eq!(walk.statistics().regions, before.regions + 1);
        assert_eq!(walk.statistics().leaves, 2);
        assert_eq!(walk.statistics().splits_since(before), 1);
        assert_eq!(walk.pending(), 2);
        let pending = walk.surrender();
        // The failed in-hand parent is last, above the older root sibling.
        assert_eq!(pending[1].region.decision(2), Some(held_first));
        assert_eq!(pending[1].region.decision(1), Some(!held_first));
        assert!(pending[1].region.is_open(0));
        for entry in pending {
            emitted.extend(represented(&entry.region));
        }
        emitted.sort_unstable();
        assert_eq!(emitted, (0..8).collect::<Vec<_>>());
    }
}

#[test]
fn retrying_a_refused_root_split_retains_the_root_counting_rule() {
    let mut walk = Walk::new(Region::all_open(2), Counting::Unchanged);
    assert_eq!(
        walk.next_with_reservation(unchanged(), refuse),
        Err(Failure::Stopped(Stop::Allocation))
    );
    let Some(Visit::Counted(first, ())) = walk.next(unchanged()).unwrap() else {
        panic!("the first child is counted")
    };
    let Some(Visit::Counted(second, ())) = walk.next(unchanged()).unwrap() else {
        panic!("the second child is counted")
    };
    assert!(first.is_cut(1) && first.is_open(0));
    assert!(second.is_held(1) && second.is_open(0));
    assert_eq!(walk.next(unchanged()), Ok(None));
    assert_eq!(
        walk.statistics().regions,
        4,
        "the refused attempt remains charged"
    );
    assert_eq!(walk.statistics().counted, 2);
}

#[test]
fn a_refused_pre_narrowed_root_keeps_its_narrowing_flag() {
    let mut walk =
        Walk::with_narrowed_root(Entry::new(Region::all_open(2), ()), Counting::Unchanged);
    assert_eq!(
        walk.next_with_reservation(
            |_| -> Result<Narrowing, Failure> { panic!("root was already narrowed") },
            refuse,
        ),
        Err(Failure::Stopped(Stop::Allocation))
    );
    let mut narrowed = Vec::new();
    while let Some(visit) = walk
        .next(|entry| {
            let region = &mut entry.region;
            narrowed.push(region.decided().collect::<Vec<_>>());
            Ok::<_, Failure>(Narrowing::Fixed { changed: false })
        })
        .unwrap()
    {
        assert!(matches!(visit, Visit::Counted(..)));
    }
    assert_eq!(narrowed, vec![vec![(1, false)], vec![(1, true)]]);
    assert_eq!(walk.statistics().regions, 4);
    assert_eq!(walk.statistics().counted, 2);
}

#[test]
fn a_zero_atom_root_needs_no_split_reservation() {
    let mut walk = Walk::new(Region::all_open(0), Counting::Unchanged);
    let visit = walk
        .next_with_reservation(unchanged(), |_, _| -> Result<(), Failure> {
            panic!("a decided root needs no split capacity")
        })
        .unwrap();
    assert_eq!(leaf(visit), 0);
    assert_eq!(walk.pending(), 0);
    assert_eq!(walk.statistics().regions, 1);
    assert_eq!(walk.statistics().leaves, 1);
}

#[test]
fn a_narrowing_failure_keeps_its_typed_payload_and_partial_state() {
    let mut walk = Walk::with_state(Region::all_open(3), Counting::Never, 0);
    let result = walk.next_with_reservation(
        |entry| {
            assert!(entry.region.hold(1));
            entry.state += 1;
            Err(Failure::Narrowing(23))
        },
        |_, _| -> Result<(), Failure> { panic!("stopped narrowing cannot reserve a split") },
    );
    assert_eq!(result, Err(Failure::Narrowing(23)));
    assert_eq!(walk.statistics(), RegionStatistics::default());
    let pending = walk.surrender();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].state, 1);
    assert_eq!(represented(&pending[0].region), vec![2, 3, 6, 7]);
}
