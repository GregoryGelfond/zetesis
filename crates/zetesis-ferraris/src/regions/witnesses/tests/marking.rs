//! Membership-only cone scans retain traversal receipts without result storage.

use super::super::{Scratch, Work, chain_position, expands};
use super::*;
use crate::test_support::Allowance;
use zetesis_cpu::Stop;

fn dag(disjunction: bool, depth: usize) -> (Theory, Vec<usize>) {
    let mut nodes =
        FormulaNodes::new(FormulaParts::new(vec![Node::atom(0), Node::atom(1)], vec![]).unwrap());
    let mut roots = Vec::new();
    for _ in 0..depth {
        let children = [roots.last().copied().unwrap_or(0), 1];
        roots.push(push(
            &mut nodes,
            if disjunction {
                NodeView::Or(&children)
            } else {
                NodeView::And(&children)
            },
        ));
    }
    // Keep each inner connective as a shared boundary. The atom 1 recurs at
    // every level; deduplication must not erase the repeated node/edge charges.
    for &root in &roots[..depth - 1] {
        push(&mut nodes, NodeView::Implies(root, 0));
    }
    (
        Theory::new(2, nodes.into_parts(), vec![], AdmissionLimits::default()).unwrap(),
        roots,
    )
}

fn assert_marks(scratch: &Scratch, index: &Narrower, expected: &[usize]) {
    for node in 0..index.parents.len() {
        assert_eq!(
            scratch.visited(node),
            expected.contains(&node),
            "node={node}"
        );
    }
    assert!(scratch.frames.is_empty());
}

#[test]
fn marking_keeps_the_complete_receipt_without_collecting_nodes() {
    for disjunction in [false, true] {
        for depth in [2, 128] {
            let (theory, roots) = dag(disjunction, depth);
            let index = Narrower::new(&theory);
            let chain = chain_position(index.chain_of[*roots.last().unwrap()].unwrap());
            assert!(expands(&index, chain));
            let expected: Vec<_> = roots.iter().rev().copied().chain([0, 1]).collect();
            let mut collected = warmed(&index);
            let mut collected_work = Work::new(u64::MAX);
            collected.cone(&index, chain, &mut collected_work).unwrap();
            assert_eq!(collected.prepared(), expected);
            assert_eq!(collected_work.spent, u64::try_from(4 * depth + 1).unwrap());
            for fresh in [false, true] {
                let mut marked = if fresh {
                    Scratch::default()
                } else {
                    warmed(&index)
                };
                let mut work = Work::new(u64::MAX);
                marked.mark_cone(&index, chain, &mut work).unwrap();
                assert_marks(&marked, &index, &expected);
                let initialization = if fresh { index.parents.len() as u64 } else { 0 };
                assert_eq!(work.spent, collected_work.spent + initialization);
                assert_eq!(marked.leaves.capacity(), 0);
                assert!(marked.prepared().is_empty());
                let bytes = marked.seen.capacity() * size_of::<u32>()
                    + marked.frames.capacity() * size_of::<super::super::Frame>();
                assert_eq!(marked.retained_bytes(), bytes as u128);
            }
        }
    }
}

#[test]
fn every_refused_mark_prefix_matches_collection_and_retries() {
    let depth = 8;
    let (theory, roots) = dag(true, depth);
    let index = Narrower::new(&theory);
    let chain = chain_position(index.chain_of[*roots.last().unwrap()].unwrap());
    let expected: Vec<_> = roots.iter().rev().copied().chain([0, 1]).collect();
    let maximum = u64::try_from(index.parents.len() + 4 * depth + 1).unwrap();
    for limit in 0..maximum {
        let mut marked = Scratch::default();
        let mut collected = Scratch::default();
        let mut mark_work = Work::new(limit);
        let mut collect_work = Work::new(limit);
        assert_eq!(
            marked.mark_cone(&index, chain, &mut mark_work),
            Err(Stop::WorkLimit)
        );
        assert_eq!(
            collected.cone(&index, chain, &mut collect_work),
            Err(Stop::WorkLimit)
        );
        assert_eq!(mark_work.spent, limit);
        assert_eq!(mark_work.spent, collect_work.spent);
        assert_eq!(marked.generation, collected.generation);
        assert_eq!(marked.seen, collected.seen);
        let cursors = |scratch: &Scratch| {
            scratch
                .frames
                .iter()
                .map(|frame| (frame.chain, frame.next_child))
                .collect::<Vec<_>>()
        };
        assert_eq!(cursors(&marked), cursors(&collected));
        marked
            .mark_cone(&index, chain, &mut Work::new(u64::MAX))
            .unwrap();
        assert_marks(&marked, &index, &expected);
        assert_eq!(marked.leaves.capacity(), 0);
    }
}
#[test]
fn cancelled_marking_prefixes_retry_with_fresh_membership() {
    let depth = 8;
    let (theory, roots) = dag(false, depth);
    let index = Narrower::new(&theory);
    let chain = chain_position(index.chain_of[*roots.last().unwrap()].unwrap());
    let expected: Vec<_> = roots.iter().rev().copied().chain([0, 1]).collect();
    for allowed in 0..u64::try_from(4 * depth + 1).unwrap() {
        let mut scratch = warmed(&index);
        let mut quota = Allowance {
            remaining: allowed,
            failure: Stop::Cancelled,
        };
        let mut work = Work::reserved(&mut quota);
        assert_eq!(
            scratch.mark_cone(&index, chain, &mut work),
            Err(Stop::Cancelled)
        );
        assert_eq!(work.spent, allowed);
        work.refund();
        assert_eq!(quota.remaining, 0);
        scratch
            .mark_cone(&index, chain, &mut Work::new(u64::MAX))
            .unwrap();
        assert_marks(&scratch, &index, &expected);
        assert_eq!(scratch.leaves.capacity(), 0);
    }
}

#[test]
fn marking_discards_prior_membership_and_collected_results() {
    let (theory, roots) = dag(true, 8);
    let index = Narrower::new(&theory);
    let outer = chain_position(index.chain_of[*roots.last().unwrap()].unwrap());
    let inner = chain_position(index.chain_of[roots[0]].unwrap());
    for rollover in [false, true] {
        let mut scratch = warmed(&index);
        scratch
            .cone(&index, outer, &mut Work::new(u64::MAX))
            .unwrap();
        let capacity = scratch.leaves.capacity();
        assert!(capacity > 0);
        if rollover {
            scratch.generation = u32::MAX;
        }
        let mut work = Work::new(u64::MAX);
        scratch.mark_cone(&index, inner, &mut work).unwrap();
        assert_marks(&scratch, &index, &[roots[0], 0, 1]);
        assert!(scratch.prepared().is_empty());
        assert_eq!(scratch.leaves.capacity(), capacity);
        let reset = if rollover {
            index.parents.len() as u64
        } else {
            0
        };
        assert_eq!(work.spent, 5 + reset);
    }
}
