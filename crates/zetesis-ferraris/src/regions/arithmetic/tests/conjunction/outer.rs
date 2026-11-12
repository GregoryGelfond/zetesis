//! The borrowed bottom frame preserves ordered occurrences and named owners.

use zetesis_algebra::{Cancellation, Meter, Stop};

use crate::{
    NodeView, Theory,
    regions::arithmetic::{
        budget::{Budget, BudgetError},
        conjunction::Conjunction,
        storage::Storage,
    },
    test_support::{constructed_theory, push},
};

fn shapes() -> Theory {
    constructed_theory(3, |nodes| {
        let empty = push(nodes, NodeView::And(&[]));
        let pair = push(nodes, NodeView::And(&[0, 1]));
        let mixed = push(nodes, NodeView::Or(&[0, 2]));
        let nested = push(nodes, NodeView::And(&[empty, pair, pair, mixed, 2]));
        let deep = push(nodes, NodeView::And(&[nested, pair]));
        vec![0, empty, pair, mixed, nested, deep]
    })
}

fn trace(theory: &Theory, node: usize, leaves: &mut Vec<usize>) {
    match theory.view().node(node).unwrap() {
        NodeView::And(children) => {
            for &child in children {
                trace(theory, child, leaves);
            }
        }
        _ => leaves.push(node),
    }
}

#[test]
fn the_outer_frame_preserves_every_completed_and_rejected_trace() {
    let theory = shapes();
    for &root in theory.roots() {
        let mut expected = Vec::new();
        trace(&theory, root, &mut expected);
        for reject_at in 0..=expected.len() {
            for borrowed in [false, true] {
                let cancellation = Cancellation::default();
                let mut work = Meter::local(u64::MAX);
                let mut budget = super::budget(&mut work, &cancellation);
                let mut storage = Storage::default();
                let mut frames = Conjunction::new(&mut storage, &mut budget).unwrap();
                let view = budget.node(&theory, root).unwrap();
                let mut seen = Vec::new();
                let visit = |node, _, _: &mut Storage, _: &mut Budget<'_, '_>| {
                    let accepted = seen.len() != reject_at;
                    seen.push(node);
                    Ok(accepted)
                };
                let result = if borrowed {
                    frames.walk_view(&theory, (root, view), &mut storage, &mut budget, visit)
                } else {
                    frames.walk(&theory, root, &mut storage, &mut budget, visit)
                };
                assert_eq!(result, Ok(reject_at == expected.len()));
                assert_eq!(seen, expected[..(reject_at + 1).min(expected.len())]);
                frames.release(&mut storage).unwrap();
                assert_eq!(storage.bytes(), 0);
            }
        }
    }
}

#[test]
fn successful_walks_reuse_the_owner_without_stale_occurrences() {
    let theory = shapes();
    let cancellation = Cancellation::default();
    let mut work = Meter::local(u64::MAX);
    let mut budget = super::budget(&mut work, &cancellation);
    let mut storage = Storage::default();
    let mut frames = Conjunction::new(&mut storage, &mut budget).unwrap();
    // A deep walk retains vector capacity; later leaf/empty walks still visit
    // exactly their own occurrences and must release each local frame.
    for &root in theory.roots().iter().rev().chain(theory.roots()) {
        let retained = storage.bytes();
        let mut seen = Vec::new();
        let result = frames.walk(&theory, root, &mut storage, &mut budget, |node, _, _, _| {
            seen.push(node);
            Ok(true)
        });
        let mut expected = Vec::new();
        trace(&theory, root, &mut expected);
        assert_eq!(result, Ok(true));
        assert_eq!(seen, expected);
        // Once the first (deepest) walk has allocated its nested capacity,
        // every other shape fits. The local frame cannot remain in Storage.
        if retained != size_of::<Vec<(usize, &[usize])>>() as u128 {
            assert_eq!(storage.bytes(), retained);
        }
    }
    frames.release(&mut storage).unwrap();
    assert_eq!(storage.bytes(), 0);
}

#[derive(Debug)]
struct Attempt {
    result: Result<bool, BudgetError>,
    seen: Vec<usize>,
    work: u64,
    peak: u128,
}

fn attempt(
    theory: &Theory,
    global: u64,
    optional: u64,
    bytes: usize,
    cancel_after_leaf: bool,
) -> Attempt {
    let cancellation = Cancellation::default();
    let mut work = Meter::local(global);
    let output = {
        let mut budget = Budget::new(&mut work, optional, bytes, &cancellation);
        let mut storage = Storage::default();
        let mut seen = Vec::new();
        let result = match Conjunction::new(&mut storage, &mut budget) {
            Err(error) => Err(error),
            Ok(mut frames) => {
                let result = frames.walk(
                    theory,
                    theory.roots()[0],
                    &mut storage,
                    &mut budget,
                    |node, _, _, _| {
                        seen.push(node);
                        if cancel_after_leaf {
                            cancellation.cancel();
                        }
                        Ok(true)
                    },
                );
                frames.release(&mut storage).unwrap();
                result
            }
        };
        assert_eq!(storage.bytes(), 0);
        Attempt {
            result,
            seen,
            work: budget.work(),
            peak: budget.peak_bytes(),
        }
    };
    assert_eq!(work.charged(), output.work);
    output
}

fn flat() -> Theory {
    constructed_theory(2, |nodes| vec![push(nodes, NodeView::And(&[0, 1]))])
}

#[test]
fn a_flat_walk_accounts_its_frame_without_an_allocation() {
    let theory = flat();
    let bytes = size_of::<Vec<(usize, &[usize])>>() + size_of::<(usize, &[usize])>();
    let complete = attempt(&theory, u64::MAX, u64::MAX, bytes, false);
    assert_eq!(complete.result, Ok(true));
    assert_eq!(complete.seen, [0, 1]);
    assert_eq!(complete.work, 45);
    assert_eq!(complete.peak, bytes as u128);
    let refused = attempt(&theory, u64::MAX, u64::MAX, bytes - 1, false);
    assert_eq!(
        refused.result,
        Err(BudgetError::StorageDeclined {
            required: bytes as u128,
            limit: bytes - 1,
        })
    );
    // Entry and depth were read; the refused local frame was not initialized.
    assert_eq!(refused.work, 11);
    assert!(refused.seen.is_empty());
}

#[test]
fn nested_growth_counts_the_still_live_outer_frame() {
    let theory = constructed_theory(2, |nodes| {
        let nested = push(nodes, NodeView::And(&[0, 1]));
        vec![push(nodes, NodeView::And(&[nested, 0]))]
    });
    let header = size_of::<Vec<(usize, &[usize])>>();
    let frame = size_of::<(usize, &[usize])>();
    // Existing header + local frame + the new vector header and first frame.
    let required = 2 * header + 2 * frame;
    let refused = attempt(&theory, u64::MAX, u64::MAX, required - 1, false);
    assert_eq!(
        refused.result,
        Err(BudgetError::StorageDeclined {
            required: required as u128,
            limit: required - 1,
        })
    );
    assert_eq!(refused.work, 34);
    assert_eq!(refused.peak, required as u128);
    assert!(refused.seen.is_empty());
}

#[test]
fn every_work_boundary_retains_only_the_executed_prefix() {
    let theory = constructed_theory(3, |nodes| {
        let pair = push(nodes, NodeView::And(&[0, 1]));
        let nested = push(nodes, NodeView::And(&[pair, pair, 2]));
        vec![push(nodes, NodeView::And(&[nested, pair]))]
    });
    let complete = attempt(&theory, u64::MAX, u64::MAX, usize::MAX, false);
    assert_eq!(complete.result, Ok(true));
    for limit in 0..=complete.work + 24 {
        for (global, optional, expected) in [
            (u64::MAX, limit, BudgetError::WorkDeclined),
            (limit, u64::MAX, BudgetError::Stopped(Stop::WorkLimit)),
        ] {
            let result = attempt(&theory, global, optional, usize::MAX, false);
            assert!(result.work <= limit);
            assert_eq!(result.seen, complete.seen[..result.seen.len()]);
            match result.result {
                Ok(true) => {
                    assert_eq!(result.work, complete.work);
                    assert_eq!(result.seen, complete.seen);
                }
                other => assert_eq!(other, Err(expected)),
            }
        }
    }
}

#[test]
fn typed_refusals_release_the_outer_frame_and_keep_their_precedence() {
    let theory = flat();
    // The second atom's bounded24-unit view cannot be admitted after35work.
    for (global, optional, expected) in [
        (u64::MAX, 50, BudgetError::WorkDeclined),
        (50, u64::MAX, BudgetError::Stopped(Stop::WorkLimit)),
        (50, 50, BudgetError::Stopped(Stop::WorkLimit)),
    ] {
        let refused = attempt(&theory, global, optional, usize::MAX, false);
        assert_eq!(refused.result, Err(expected));
        assert_eq!(refused.work, 35);
        assert_eq!(refused.seen, [0]);
        let cancelled = attempt(&theory, global, optional, usize::MAX, true);
        assert_eq!(cancelled.result, Err(BudgetError::Stopped(Stop::Cancelled)));
        assert_eq!(cancelled.work, 35);
        assert_eq!(cancelled.seen, [0]);
    }
}
