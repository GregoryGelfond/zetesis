//! Coverage-tree receipts of the same ordinary Walk used by the regional query.

use super::*;
use zetesis_algebra::{
    Stop,
    region::Region,
    walk::{Counting, Walk},
};

const FIXED: Narrowing = Narrowing::Fixed { changed: false };

fn exhaust(
    atoms: usize,
    mut narrow: impl FnMut(&mut Region) -> Narrowing,
    workspace: Option<&mut DepthWorkspace>,
) -> QueryDepth {
    let mut counts = QueryDepth::default();
    let mut tracking = Recording::new(&mut counts, workspace);
    let mut walk = Walk::new(Region::all_open(atoms), Counting::Never);
    loop {
        let visit = walk.next(|entry| {
            tracking.enter();
            let result = narrow(&mut entry.region);
            tracking.narrowed(result);
            Ok::<_, Stop>(result)
        });
        tracking.returned(walk.statistics(), visit.is_err());
        if visit.unwrap().is_none() {
            return counts;
        }
    }
}

#[test]
fn distinct_query_trees_add_their_levels() {
    let mut worker = DepthWorkspace::new(3, u128::MAX).unwrap();
    // The cut branch fixes its second atom immediately, and one held branch
    // refutes. These known uneven levels exercise successive leaf returns.
    let uneven = exhaust(
        3,
        |region| {
            if region.is_cut(2) && region.is_open(1) {
                region.hold(1);
            }
            if region.is_held(2) && region.is_held(1) {
                Narrowing::Refuted
            } else {
                FIXED
            }
        },
        Some(&mut worker),
    );
    assert_eq!(worker.levels(), Some([1, 2, 4, 2].as_slice()));
    assert_eq!(uneven.deepest, Some(3));
    assert_eq!(uneven.published_splits, 4);
    let other = exhaust(2, |_| FIXED, Some(&mut worker));
    assert_eq!(other.deepest, Some(2));
    assert_eq!(worker.levels(), Some([2, 4, 8, 2].as_slice()));
}

#[test]
fn a_refused_copy_does_not_publish_its_split() {
    let mut query = QueryDepth::default();
    let mut worker = DepthWorkspace::new(1, u128::MAX).unwrap();
    let mut tracing = Recording::new(&mut query, Some(&mut worker));
    let mut walk = Walk::new(Region::all_open(1), Counting::Never);
    let visit = walk.next_with_copy(
        &mut tracing,
        |trace, _| {
            trace.enter();
            trace.narrowed(FIXED);
            Ok(FIXED)
        },
        |_, ()| Err(Stop::Allocation),
    );
    tracing.returned(walk.statistics(), visit.is_err());
    assert_eq!(visit, Err(Stop::Allocation));
    assert_eq!(
        query,
        QueryDepth {
            attempted_splits: 1,
            published_splits: 0,
            deepest: Some(0)
        }
    );
    assert_eq!(worker.levels(), Some([1].as_slice()));
}

#[test]
fn an_unvisited_child_retains_its_parent_publication() {
    let mut query = QueryDepth::default();
    let mut worker = DepthWorkspace::new(1, u128::MAX).unwrap();
    let mut tracing = Recording::new(&mut query, Some(&mut worker));
    let mut walk = Walk::new(Region::all_open(1), Counting::Never);
    let mut calls = 0;
    let visit = walk.next(|_| {
        tracing.enter();
        calls += 1;
        if calls == 2 {
            return Err(Stop::WorkLimit);
        }
        tracing.narrowed(FIXED);
        Ok(FIXED)
    });
    tracing.returned(walk.statistics(), visit.is_err());
    assert_eq!(visit, Err(Stop::WorkLimit));
    assert_eq!(
        query,
        QueryDepth {
            attempted_splits: 1,
            published_splits: 1,
            deepest: Some(0)
        }
    );
    assert_eq!(worker.levels(), Some([1].as_slice()));
}

#[test]
fn recording_decline_preserves_exact_split_counts() {
    let bytes = 4 * (size_of::<usize>() + size_of::<u64>()) as u128;
    assert!(DepthWorkspace::new(3, bytes - 1).is_none());
    assert!(DepthWorkspace::new(usize::MAX, u128::MAX).is_none());
    let req = DepthWorkspace::new(3, bytes).unwrap();
    assert_eq!(req.payload_bytes(), bytes);
    let expected = QueryDepth {
        attempted_splits: 7,
        published_splits: 7,
        deepest: None,
    };
    assert_eq!(exhaust(3, |_| FIXED, None), expected);
    let mut small = DepthWorkspace::new(0, u128::MAX).unwrap();
    assert_eq!(exhaust(3, |_| FIXED, Some(&mut small)), expected);
    assert!(small.levels().is_none());
}

#[test]
fn one_full_bin_preserves_independent_query_depth() {
    let mut worker = DepthWorkspace::new(1, u128::MAX).unwrap();
    worker.levels.push(u64::MAX);
    let query = exhaust(1, |_| FIXED, Some(&mut worker));
    assert_eq!(query.deepest, Some(1));
    assert_eq!(query.published_splits, 1);
    assert!(worker.levels().is_none());
}

#[test]
fn declined_level_storage_retains_the_pending_peak() {
    // The first actual capacity is larger than the requested root slot.
    // Its own reservation fits; its overlap with a single bin does not.
    let worker = DepthWorkspace {
        pending: Vec::with_capacity(2),
        levels: Vec::new(),
        incomplete: false,
    };
    let pending = worker.payload_bytes();
    let mut peak = 0;
    assert!(worker.reserve_levels(1, pending, &mut peak).is_none());
    assert_eq!(peak, pending);
}
