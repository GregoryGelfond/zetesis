//! The region traversal is the coverage tree of `Search.lean` walked by a
//! stack: a region is narrowed, refuted, decided outright, split on its
//! highest open atom with the cut branch first, or counted when its
//! narrowing decided nothing beyond the split. Every candidate of the root
//! lies in exactly one leaf or counted region visited, and the leaves come
//! in the counter's order.

use zetesis_cpu::Stop;
use zetesis_cpu::regions::{Counting, Narrowing, Region, Traversal, Visit};

/// Every visit of a traversal, with a narrowing that never decides anything.
fn visits(mut traversal: Traversal) -> Vec<Visit> {
    let mut visits = Vec::new();
    while let Some(visit) = traversal
        .next(|_, ()| Ok::<_, Stop>(Narrowing::Fixed { changed: false }))
        .unwrap()
    {
        visits.push(visit);
    }
    visits
}

fn held(region: &Region) -> Vec<usize> {
    region.held().collect()
}

#[test]
fn a_region_holds_cuts_or_leaves_each_atom_open() {
    let mut region = Region::all_open(3);
    assert!(region.is_open(0) && region.is_open(1) && region.is_open(2));
    assert!(region.hold(1));
    assert!(region.cut(2));
    assert!(region.is_held(1) && region.is_cut(2) && region.is_open(0));
    assert!(region.hold(1), "holding a held atom is idle");
    assert!(!region.cut(1), "cutting a held atom is refused");
    assert!(!region.hold(3), "an atom outside the region is refused");
    assert_eq!(region.decision(1), Some(true));
    assert_eq!(region.decision(0), None);
    assert_eq!(held(&region), vec![1]);
    assert_eq!(region.open().collect::<Vec<_>>(), vec![0]);
    assert_eq!(region.highest_open(), Some(0));
}

#[test]
fn a_region_logs_its_decisions_in_order() {
    let mut region = Region::all_open(3);
    assert!(region.hold(2));
    assert!(region.cut(0));
    assert!(region.hold(2), "an idle hold is not a decision");
    assert_eq!(region.decisions(), [2, 0]);
    let (cut, held) = region.split(1);
    assert_eq!(cut.decisions(), [2, 0, 1]);
    assert_eq!(held.decisions(), [2, 0, 1]);
}

#[test]
fn regions_are_equal_by_their_decisions_whatever_their_order() {
    let mut region = Region::all_open(3);
    assert!(region.hold(2));
    assert!(region.cut(0));
    let mut other = Region::all_open(3);
    assert!(other.cut(0));
    assert!(other.hold(2));
    assert_eq!(other, region);
    assert_ne!(other.decisions(), region.decisions());
}

#[test]
fn a_split_partitions_a_region_on_one_atom_cut_first() {
    let region = Region::all_open(2);
    let (cut, held) = region.split(1);
    assert!(cut.is_cut(1) && cut.is_open(0));
    assert!(held.is_held(1) && held.is_open(0));
}

#[test]
fn never_counting_visits_every_leaf_in_counter_order() {
    let traversal = Traversal::new(Region::all_open(2), Counting::Never);
    let leaves: Vec<Vec<usize>> = visits(traversal)
        .iter()
        .map(|visit| match visit {
            Visit::Leaf(region, ()) => held(region),
            Visit::Counted(..) => panic!("never counted"),
        })
        .collect();
    // Empty, {0}, {1}, {0,1}: atom 0 is the low bit of the counter.
    assert_eq!(leaves, vec![vec![], vec![0], vec![1], vec![0, 1]]);
}

#[test]
fn the_root_is_split_even_when_its_narrowing_changes_nothing() {
    let traversal = Traversal::new(Region::all_open(2), Counting::Unchanged);
    let visits = visits(traversal);
    // The root splits on atom 1; each child, unchanged by narrowing, is counted.
    assert_eq!(visits.len(), 2);
    assert!(matches!(&visits[0], Visit::Counted(region, ()) if region.is_cut(1)));
    assert!(matches!(&visits[1], Visit::Counted(region, ()) if region.is_held(1)));
}

#[test]
fn a_narrowing_that_decides_an_atom_keeps_splitting_below_it() {
    // Narrowing cuts atom 0 whenever atom 2 is held: the held branch then
    // has one open atom left and is split, not counted.
    let mut traversal = Traversal::new(Region::all_open(3), Counting::Unchanged);
    let mut seen = Vec::new();
    while let Some(visit) = traversal
        .next(|region, ()| {
            if region.is_held(2) && region.is_open(0) {
                assert!(region.cut(0));
                Ok::<_, Stop>(Narrowing::Fixed { changed: true })
            } else {
                Ok(Narrowing::Fixed { changed: false })
            }
        })
        .unwrap()
    {
        seen.push(visit);
    }
    assert_eq!(seen.len(), 3);
    assert!(matches!(&seen[0], Visit::Counted(region, ()) if region.is_cut(2)));
    assert!(matches!(&seen[1], Visit::Leaf(region, ()) if held(region) == vec![2]));
    assert!(matches!(&seen[2], Visit::Leaf(region, ()) if held(region) == vec![1, 2]));
}

#[test]
fn a_refuted_region_is_skipped_with_its_whole_subtree() {
    let mut traversal = Traversal::new(Region::all_open(3), Counting::Never);
    let mut leaves = Vec::new();
    while let Some(visit) = traversal
        .next(|region, ()| {
            if region.is_held(2) {
                Ok::<_, Stop>(Narrowing::Refuted)
            } else {
                Ok(Narrowing::Fixed { changed: false })
            }
        })
        .unwrap()
    {
        if let Visit::Leaf(region, ()) = visit {
            leaves.push(held(&region));
        }
    }
    assert_eq!(leaves, vec![vec![], vec![0], vec![1], vec![0, 1]]);
    let statistics = traversal.statistics();
    assert_eq!(statistics.refuted, 1);
    assert_eq!(statistics.decided, 4);
    assert_eq!(statistics.counted, 0);
    // The root, its two children, and the cut child's subtree of six.
    assert_eq!(statistics.regions, 1 + 2 + 6);
}

#[test]
fn a_decided_root_is_one_leaf() {
    let mut root = Region::all_open(2);
    assert!(root.hold(0) && root.cut(1));
    let visits = visits(Traversal::new(root, Counting::Unchanged));
    assert_eq!(visits.len(), 1);
    assert!(matches!(&visits[0], Visit::Leaf(region, ()) if held(region) == vec![0]));
}

#[test]
fn a_stopped_narrowing_stops_the_traversal_and_keeps_the_region() {
    let mut traversal = Traversal::new(Region::all_open(2), Counting::Never);
    assert_eq!(
        traversal.next(|_, ()| Err(Stop::WorkLimit)).unwrap_err(),
        Stop::WorkLimit
    );
    // The region whose narrowing stopped is visited again on the next call.
    let visits = visits(traversal);
    assert_eq!(visits.len(), 4);
}

#[test]
fn a_narrowing_may_choose_the_split_atom() {
    // Preferring atom 0 at every region visits the leaves with atom 0 as
    // the high bit of the order: empty, {1}, {0}, {0,1}.
    let mut traversal = Traversal::new(Region::all_open(2), Counting::Never);
    let mut leaves = Vec::new();
    while let Some(visit) = traversal
        .next(|region, ()| {
            region.prefer(0);
            Ok::<_, Stop>(Narrowing::Fixed { changed: false })
        })
        .unwrap()
    {
        if let Visit::Leaf(region, ()) = visit {
            leaves.push(held(&region));
        }
    }
    assert_eq!(leaves, vec![vec![], vec![1], vec![0], vec![0, 1]]);
    let mut region = Region::all_open(2);
    region.prefer(1);
    assert!(region.hold(1));
    assert_eq!(region.split_atom(), Some(0), "a decided preference lapses");
}

#[test]
fn each_leaf_carries_the_state_of_its_own_lineage() {
    // A split clones the state into both children. The state counts the
    // regions on its own lineage: each narrowing adds one, so a leaf at
    // depth two has seen three regions, and siblings do not see each
    // other's count.
    let mut traversal = Traversal::with_state(Region::all_open(2), Counting::Never, 0usize);
    let mut depths = Vec::new();
    while let Some(visit) = traversal
        .next(|_, seen: &mut usize| {
            *seen += 1;
            Ok::<_, Stop>(Narrowing::Fixed { changed: false })
        })
        .unwrap()
    {
        if let Visit::Leaf(_, seen) = visit {
            depths.push(seen);
        }
    }
    assert_eq!(depths, vec![3, 3, 3, 3]);
}

#[test]
fn a_traversal_from_a_narrowed_root_does_not_narrow_it_again() {
    // The caller narrowed the root before the traversal began: the root is
    // split without a narrowing, and every region below it is narrowed
    // once, the counting never counting the root as a flat interval.
    let mut traversal = Traversal::with_narrowed_root(Region::all_open(2), Counting::Unchanged, ());
    let mut narrowed = Vec::new();
    while let Some(visit) = traversal
        .next(|region, ()| {
            narrowed.push(region.decisions().to_vec());
            Ok::<_, Stop>(Narrowing::Fixed { changed: false })
        })
        .unwrap()
    {
        assert!(matches!(visit, Visit::Counted(..)));
    }
    // The root's two children, each narrowed once and counted.
    assert_eq!(narrowed, vec![vec![1], vec![1]]);
    assert_eq!(traversal.statistics().regions, 3);
    assert_eq!(traversal.statistics().counted, 2);
}
