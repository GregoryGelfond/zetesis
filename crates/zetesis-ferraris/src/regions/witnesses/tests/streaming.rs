//! Fold short-circuiting keeps the collecting view's order and frozen boundary.

use std::ops::ControlFlow;

use super::super::{Scratch, Work, chain_position, expands};
use super::*;
use crate::{EvaluationLimits, EvaluationWorkspace};
use zetesis_cpu::Stop;

fn early_witnesses(disjunction: bool, length: usize) -> (Theory, [usize; 3]) {
    let mut nodes = FormulaNodes::new(
        FormulaParts::new((0..=length).map(Node::atom).collect(), Vec::new()).unwrap(),
    );
    let group = |nodes: &mut FormulaNodes, children: &[usize]| {
        push(
            nodes,
            if disjunction {
                NodeView::Or(children)
            } else {
                NodeView::And(children)
            },
        )
    };
    let first = group(&mut nodes, &[0, 1]);
    // Atom 1 also occurs here: the complete view must retain it only once.
    let tail = group(&mut nodes, &(1..length).collect::<Vec<_>>());
    let root = group(&mut nodes, &[first, tail]);
    // The second consumers prevent either child being absorbed into the root.
    push(&mut nodes, NodeView::Implies(first, length));
    push(&mut nodes, NodeView::Implies(tail, length));
    let required = if disjunction {
        root
    } else {
        let falsum = push(&mut nodes, NodeView::False);
        push(&mut nodes, NodeView::Implies(root, falsum))
    };
    (
        Theory::new(
            length + 1,
            nodes.into_parts(),
            vec![required],
            AdmissionLimits::default(),
        )
        .unwrap(),
        [root, first, tail],
    )
}

#[test]
fn required_witness_checks_stop_after_two_open_leaves() {
    for disjunction in [false, true] {
        let (theory, [root, _, tail]) = early_witnesses(disjunction, 128);
        let index = Narrower::new(&theory);
        let chain = chain_position(index.chain_of[root].unwrap());
        assert!(expands(&index, chain));
        let mut scratch = NarrowingScratch::default();
        // Isolate repeated closure cost from the separately tested first-use
        // O(nodes) mark initialization. No witness leaves have been collected.
        scratch
            .witnesses
            .begin(index.parents.len(), &mut Work::new(u64::MAX))
            .unwrap();
        let mut region = Region::all_open(theory.atom_count());
        let (outcome, statistics) = index
            .narrow_known(
                OriginalSubject::new(&theory, None),
                &mut region,
                &mut index.knowledge(),
                &mut scratch,
                RegionLimits::default(),
                &Cancellation::default(),
            )
            .unwrap();
        assert_ne!(outcome, Narrowing::Refuted);
        assert!((0..theory.atom_count()).all(|atom| region.is_open(atom)));
        assert!(scratch.witnesses.visited(0) && scratch.witnesses.visited(1));
        assert!(!scratch.witnesses.visited(tail));
        assert_eq!(scratch.witnesses.leaves.capacity(), 0);
        // Includes the ordinary atom-preference scan. The old whole read also
        // expands the long tail, exceeding this bound and allocating its leaves.
        assert!(
            statistics.work < 2 * theory.atom_count() as u64,
            "{statistics:?}"
        );
    }
}

#[test]
fn streaming_and_collection_share_distinct_order_and_frozen_gates() {
    let (theory, [root, first, _]) = early_witnesses(true, 8);
    let index = Narrower::new(&theory);
    let chain = chain_position(index.chain_of[root].unwrap());
    let candidate = Interpretation::new(&theory, [2]).unwrap();
    let mut evaluation = EvaluationWorkspace::default();
    let cancellation = Cancellation::default();
    let evaluated = evaluation.evaluate(&candidate, EvaluationLimits::default(), &cancellation);
    let truth = evaluated.result.unwrap();
    assert!(!truth.truth()[first] && truth.truth()[root]);
    for frozen in [None, Some(truth.truth())] {
        let mut scratch = Scratch::default();
        let collected = scratch
            .read(&index, chain, frozen, &mut Work::new(u64::MAX))
            .unwrap()
            .to_vec();
        let mut streamed = Vec::new();
        let outcome = scratch
            .visit_expanded(
                &index,
                chain,
                frozen,
                &mut Work::new(u64::MAX),
                |leaf, work| {
                    work.tick()?;
                    streamed.push(leaf);
                    Ok(ControlFlow::Continue(()))
                },
            )
            .unwrap();
        assert!(outcome.is_continue());
        let expected = if frozen.is_some() {
            std::iter::once(first).chain(1..8).collect::<Vec<_>>()
        } else {
            (0..8).collect()
        };
        assert_eq!(streamed, expected);
        assert_eq!(collected, expected);
    }
}

fn stop_after_two(scratch: &mut Scratch, index: &Narrower, chain: usize, work: &mut Work<'_>) {
    let mut count = 0;
    assert!(
        scratch
            .visit_expanded(index, chain, None, work, |_, work| {
                work.tick()?;
                count += 1;
                Ok(if count == 2 {
                    ControlFlow::Break(())
                } else {
                    ControlFlow::Continue(())
                })
            })
            .unwrap()
            .is_break()
    );
    assert_eq!(count, 2);
}

#[test]
fn fresh_streaming_charges_its_mark_initialization() {
    let (theory, [root, _, tail]) = early_witnesses(true, 32);
    let index = Narrower::new(&theory);
    let chain = chain_position(index.chain_of[root].unwrap());
    let mut scratch = Scratch::default();
    let mut cold = Work::new(u64::MAX);
    stop_after_two(&mut scratch, &index, chain, &mut cold);
    let mut warm = Work::new(u64::MAX);
    stop_after_two(&mut scratch, &index, chain, &mut warm);
    assert_eq!(cold.spent - warm.spent, index.parents.len() as u64);
    assert!(!scratch.visited(tail));
    assert_eq!(scratch.leaves.capacity(), 0);
    assert_eq!(
        scratch
            .read(&index, chain, None, &mut Work::new(u64::MAX))
            .unwrap(),
        &(0..32).collect::<Vec<_>>()
    );
}

#[test]
fn refused_collection_restores_its_reusable_leaf_allocation() {
    let (theory, [root, _, _]) = early_witnesses(true, 16);
    let index = Narrower::new(&theory);
    let chain = chain_position(index.chain_of[root].unwrap());
    let mut scratch = Scratch::default();
    let expected = (0..16).collect::<Vec<_>>();
    assert_eq!(
        scratch
            .read(&index, chain, None, &mut Work::new(u64::MAX))
            .unwrap(),
        expected
    );
    let capacity = scratch.leaves.capacity();
    let mut complete = Work::new(u64::MAX);
    scratch.read(&index, chain, None, &mut complete).unwrap();
    for limit in 0..complete.spent {
        let mut work = Work::new(limit);
        assert!(matches!(
            scratch.read(&index, chain, None, &mut work),
            Err(Stop::WorkLimit)
        ));
        assert_eq!(work.spent, limit);
        assert_eq!(scratch.leaves.capacity(), capacity);
        assert_eq!(
            scratch
                .read(&index, chain, None, &mut Work::new(u64::MAX))
                .unwrap(),
            expected
        );
    }
}

#[test]
fn a_stopped_visitor_cannot_leak_traversal_marks_into_collection() {
    let (theory, [root, _, tail]) = early_witnesses(true, 16);
    let index = Narrower::new(&theory);
    let chain = chain_position(index.chain_of[root].unwrap());
    let mut scratch = Scratch::default();
    let result =
        scratch.visit_expanded(&index, chain, None, &mut Work::new(u64::MAX), |leaf, _| {
            assert_eq!(leaf, 0);
            Err(Stop::Cancelled)
        });
    assert_eq!(result, Err(Stop::Cancelled));
    assert!(!scratch.visited(tail));
    assert_eq!(
        scratch
            .read(&index, chain, None, &mut Work::new(u64::MAX))
            .unwrap(),
        &(0..16).collect::<Vec<_>>()
    );
}
