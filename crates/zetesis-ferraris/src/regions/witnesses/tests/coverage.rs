//! Cover authentication consumes only reached edges and keeps compound cuts.

use super::super::{Scratch, Work, chain_position, expands};
use super::*;
use crate::test_support::Allowance;
use zetesis_cpu::Stop;

fn connective(nodes: &mut FormulaNodes, disjunction: bool, children: &[usize]) -> usize {
    push(
        nodes,
        if disjunction {
            NodeView::Or(children)
        } else {
            NodeView::And(children)
        },
    )
}

fn wide(disjunction: bool, width: usize) -> (Theory, usize, usize) {
    let mut nodes =
        FormulaNodes::new(FormulaParts::new((0..width).map(Node::atom).collect(), vec![]).unwrap());
    let first = connective(&mut nodes, disjunction, &[0, 1]);
    let children: Vec<_> = std::iter::once(first).chain(2..width).collect();
    let root = connective(&mut nodes, disjunction, &children);
    // A second consumer keeps the compound node as a shared chain boundary.
    push(&mut nodes, NodeView::Implies(first, 0));
    let theory = Theory::new(
        width,
        nodes.into_parts(),
        vec![],
        AdmissionLimits::default(),
    )
    .unwrap();
    (theory, root, first)
}

fn chain(index: &Narrower, root: usize) -> usize {
    let chain = chain_position(index.chain_of[root].unwrap());
    assert!(expands(index, chain));
    chain
}

#[test]
fn early_uncovered_paths_do_not_prepare_siblings() {
    for disjunction in [false, true] {
        for width in [16, 1024] {
            let (theory, root, first) = wide(disjunction, width);
            let index = Narrower::new(&theory);
            let chain = chain(&index, root);
            assert_eq!(index.chains[chain].leaves(&index.extrema).len(), width - 1);
            let mut scratch = warmed(&index);
            let mut work = Work::new(u64::MAX);
            let mut tested = Vec::new();
            let covered = scratch
                .covered_by(&index, chain, &mut work, |node, work| {
                    work.tick()?;
                    tested.push(node);
                    Ok(false)
                })
                .unwrap();
            assert!(!covered);
            assert_eq!(tested, [root, first, 0]);
            // Three nodes, two reached edges, three coverage callback reads.
            // Eager sibling preparation instead consumes width+1 edges.
            assert_eq!(work.spent, 8, "width={width}");
            assert!(!scratch.visited(1) && !scratch.visited(2));
            assert_eq!(scratch.frames.len(), 2);
            assert_eq!(scratch.stack.capacity(), 0);
            assert_eq!(scratch.leaves.capacity(), 0);
        }
    }
}

#[test]
fn a_late_uncovered_path_retains_full_visit_order() {
    for disjunction in [false, true] {
        let width = 32;
        let (theory, root, first) = wide(disjunction, width);
        let index = Narrower::new(&theory);
        let mut scratch = warmed(&index);
        let mut work = Work::new(u64::MAX);
        let mut tested = Vec::new();
        let covered = scratch
            .covered_by(&index, chain(&index, root), &mut work, |node, work| {
                work.tick()?;
                tested.push(node);
                Ok(node < width - 1)
            })
            .unwrap();
        assert!(!covered);
        let expected: Vec<_> = [root, first].into_iter().chain(0..width).collect();
        assert_eq!(tested, expected);
        // width+2 nodes and callbacks, width+1 edges, exactly the old full walk.
        assert_eq!(work.spent, u64::try_from(3 * width + 5).unwrap());
    }
}

#[test]
fn covered_compounds_prune_only_their_descendants() {
    for disjunction in [false, true] {
        let width = 16;
        let (theory, root, first) = wide(disjunction, width);
        let index = Narrower::new(&theory);
        let mut scratch = warmed(&index);
        let mut work = Work::new(u64::MAX);
        let mut tested = Vec::new();
        assert!(
            scratch
                .covered_by(&index, chain(&index, root), &mut work, |node, work| {
                    work.tick()?;
                    tested.push(node);
                    Ok(node == first || node < width)
                })
                .unwrap()
        );
        let expected: Vec<_> = [root, first].into_iter().chain(2..width).collect();
        assert_eq!(tested, expected);
        assert!(!scratch.visited(0) && !scratch.visited(1));
        assert_eq!(work.spent, u64::try_from(3 * width - 1).unwrap());
        assert!(scratch.frames.is_empty());
    }
}

#[test]
fn a_covered_root_reads_no_edges() {
    let (theory, root, first) = wide(true, 32);
    let index = Narrower::new(&theory);
    let mut scratch = warmed(&index);
    let mut work = Work::new(u64::MAX);
    let mut tested = Vec::new();
    assert!(
        scratch
            .covered_by(&index, chain(&index, root), &mut work, |node, work| {
                work.tick()?;
                tested.push(node);
                Ok(node == root)
            })
            .unwrap()
    );
    assert_eq!(tested, [root]);
    assert_eq!(work.spent, 2);
    assert!(!scratch.visited(first));
    assert_eq!(scratch.frames.capacity(), 0);
}

fn deep(disjunction: bool, depth: usize) -> (Theory, Vec<usize>) {
    let mut nodes =
        FormulaNodes::new(FormulaParts::new(vec![Node::atom(0), Node::atom(1)], vec![]).unwrap());
    let mut roots = vec![connective(&mut nodes, disjunction, &[0, 1])];
    for _ in 1..depth {
        roots.push(connective(
            &mut nodes,
            disjunction,
            &[*roots.last().unwrap(), 1],
        ));
    }
    for &root in &roots[..depth - 1] {
        push(&mut nodes, NodeView::Implies(root, 0));
    }
    (
        Theory::new(2, nodes.into_parts(), vec![], AdmissionLimits::default()).unwrap(),
        roots,
    )
}

#[test]
fn complete_deep_covers_keep_distinct_callback_order() {
    for disjunction in [false, true] {
        let depth = 128;
        let (theory, roots) = deep(disjunction, depth);
        let index = Narrower::new(&theory);
        for &root in &roots {
            let chain = chain_position(index.chain_of[root].unwrap());
            assert_eq!(index.chains[chain].leaves(&index.extrema).len(), 2);
        }
        let mut scratch = warmed(&index);
        let mut work = Work::new(u64::MAX);
        let mut tested = Vec::new();
        assert!(
            scratch
                .covered_by(
                    &index,
                    chain(&index, *roots.last().unwrap()),
                    &mut work,
                    |node, work| {
                        work.tick()?;
                        tested.push(node);
                        Ok(node < 2)
                    }
                )
                .unwrap()
        );
        let expected: Vec<_> = roots.iter().rev().copied().chain([0, 1]).collect();
        assert_eq!(tested, expected, "shared atom one is tested once");
        // depth chains plus depth+1 atom encounters, 2*depth edges and
        // depth+2 distinct coverage callbacks. Dedup does not erase edge work.
        assert_eq!(work.spent, u64::try_from(5 * depth + 3).unwrap());
        assert!(scratch.frames.is_empty());
        assert!(scratch.frames.capacity() >= depth);
        assert_eq!(scratch.stack.capacity(), 0);
        assert_eq!(scratch.leaves.capacity(), 0);
        let bytes = scratch.seen.capacity() * size_of::<u32>()
            + scratch.frames.capacity() * size_of::<super::super::Frame>();
        assert_eq!(scratch.retained_bytes(), bytes as u128);
    }
}

#[test]
fn cone_collection_keeps_compounds_and_full_receipt() {
    let depth = 8;
    let (theory, roots) = deep(true, depth);
    let index = Narrower::new(&theory);
    let chain = chain(&index, *roots.last().unwrap());
    let expected: Vec<_> = roots.iter().rev().copied().chain([0, 1]).collect();
    let mut scratch = warmed(&index);
    let mut work = Work::new(u64::MAX);
    scratch.cone(&index, chain, &mut work).unwrap();
    assert_eq!(scratch.prepared(), expected);
    assert_eq!(work.spent, u64::try_from(4 * depth + 1).unwrap());
    assert!(scratch.frames.is_empty());
    let capacity = scratch.leaves.capacity();
    for limit in 0..work.spent {
        let mut limited = Work::new(limit);
        assert_eq!(
            scratch.cone(&index, chain, &mut limited),
            Err(Stop::WorkLimit)
        );
        assert_eq!(limited.spent, limit);
        assert_eq!(scratch.leaves.capacity(), capacity);
        scratch
            .cone(&index, chain, &mut Work::new(u64::MAX))
            .unwrap();
        assert_eq!(scratch.prepared(), expected);
        assert!(scratch.frames.is_empty());
    }
}

#[test]
fn every_cover_work_prefix_can_retry_with_another_cut() {
    let width = 8;
    let (theory, root, first) = wide(true, width);
    let index = Narrower::new(&theory);
    let chain = chain(&index, root);
    let expected: Vec<_> = [root, first].into_iter().chain(0..width).collect();
    let complete = u64::try_from(3 * width + 5).unwrap();
    for fresh in [false, true] {
        let maximum = complete + if fresh { index.parents.len() as u64 } else { 0 };
        for limit in 0..maximum {
            let mut scratch = if fresh {
                Scratch::default()
            } else {
                warmed(&index)
            };
            let mut work = Work::new(limit);
            assert_eq!(
                scratch.covered_by(&index, chain, &mut work, |node, work| {
                    work.tick()?;
                    Ok(node < width)
                }),
                Err(Stop::WorkLimit),
            );
            assert_eq!(work.spent, limit);
            let mut tested = Vec::new();
            let mut retry = Work::new(u64::MAX);
            assert!(
                !scratch
                    .covered_by(&index, chain, &mut retry, |node, work| {
                        work.tick()?;
                        tested.push(node);
                        Ok(node < width - 1)
                    })
                    .unwrap()
            );
            assert_eq!(tested, expected);
            assert_eq!(scratch.leaves.capacity(), 0);
        }
    }
}
#[test]
fn cancelled_cover_prefixes_do_not_establish_coverage() {
    let width = 8;
    let (theory, root, _) = wide(false, width);
    let index = Narrower::new(&theory);
    let chain = chain(&index, root);
    for allowed in 0..u64::try_from(3 * width + 5).unwrap() {
        let mut scratch = warmed(&index);
        let mut quota = Allowance {
            remaining: allowed,
            failure: Stop::Cancelled,
        };
        let mut work = Work::reserved(&mut quota);
        let result = scratch.covered_by(&index, chain, &mut work, |node, work| {
            work.tick()?;
            Ok(node < width)
        });
        assert_eq!(result, Err(Stop::Cancelled));
        assert_eq!(work.spent, allowed);
        work.refund();
        assert_eq!(quota.remaining, 0);
        assert!(
            scratch
                .covered_by(&index, chain, &mut Work::new(u64::MAX), |node, work| {
                    work.tick()?;
                    Ok(node < width)
                })
                .unwrap()
        );
        assert!(scratch.frames.is_empty());
    }
}
