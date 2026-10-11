//! Ancestor discovery prunes only branches discharged by ordinary propagation.

use super::super::{Scratch, Work, chain_position};
use super::*;

/// Both associative branches share atom zero. Each inner connective has a
/// second non-associative consumer, so its parent links remain explicit.
fn branches() -> (Theory, [[usize; 3]; 2]) {
    let mut nodes =
        FormulaNodes::new(FormulaParts::new((0..5).map(Node::atom).collect(), Vec::new()).unwrap());
    let mut roots = [[0; 3]; 2];
    for (kind, disjunction) in [false, true].into_iter().enumerate() {
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
        let second = group(&mut nodes, &[first, 2]);
        let third = group(&mut nodes, &[second, 3]);
        push(&mut nodes, NodeView::Implies(first, 4));
        push(&mut nodes, NodeView::Implies(second, 4));
        roots[kind] = [first, second, third];
    }
    (
        Theory::new(5, nodes.into_parts(), vec![], AdmissionLimits::default()).unwrap(),
        roots,
    )
}

fn chain_ids(index: &Narrower, roots: &[usize]) -> Vec<usize> {
    roots
        .iter()
        .map(|&root| chain_position(index.chain_of[root].unwrap()))
        .collect()
}

#[test]
fn irrelevant_polarity_skips_its_ancestors_and_keeps_candidate_order() {
    let (theory, roots) = branches();
    let index = Narrower::new(&theory);
    let mut full = Scratch::default();
    full.parent_chains(&index, 0, &mut Work::new(u64::MAX), |_| true)
        .unwrap();
    for (kind, disjunction) in [false, true].into_iter().enumerate() {
        let mut selected = Scratch::default();
        selected
            .parent_chains(&index, 0, &mut Work::new(u64::MAX), |chain| {
                chain.disjunction == disjunction
            })
            .unwrap();
        let expected: Vec<_> = full
            .candidates()
            .iter()
            .copied()
            .filter(|&chain| index.chains[chain].disjunction == disjunction)
            .collect();
        assert_eq!(selected.candidates(), expected);
        assert_eq!(selected.candidates(), chain_ids(&index, &roots[kind]));
        assert!(selected.visited(roots[1 - kind][0]));
        assert!(!selected.visited(roots[1 - kind][1]));
        assert!(!selected.visited(roots[1 - kind][2]));
    }
}

#[test]
fn absorbing_truth_prunes_a_branch_but_opposite_truth_stays_actionable() {
    let (theory, roots) = branches();
    let index = Narrower::new(&theory);
    for (kind, disjunction) in [false, true].into_iter().enumerate() {
        for value in [false, true] {
            let mut selected = Scratch::default();
            selected
                .parent_chains(&index, 0, &mut Work::new(u64::MAX), |chain| {
                    let known = (chain.root == roots[kind][0]).then_some(value);
                    chain.disjunction == disjunction && known != Some(chain.disjunction)
                })
                .unwrap();
            if value == disjunction {
                assert!(selected.candidates().is_empty());
                assert!(!selected.visited(roots[kind][1]));
                assert!(!selected.visited(roots[kind][2]));
            } else {
                assert_eq!(selected.candidates(), chain_ids(&index, &roots[kind]));
            }
        }
    }
}

#[test]
fn pruning_does_not_read_or_retain_the_discarded_ancestor_tail() {
    let (theory, roots) = branches();
    let index = Narrower::new(&theory);
    let mut full = Scratch::default();
    let mut complete = Work::new(u64::MAX);
    full.parent_chains(&index, 0, &mut complete, |_| true)
        .unwrap();
    let mut selected = Scratch::default();
    let mut filtered = Work::new(u64::MAX);
    selected
        .parent_chains(&index, 0, &mut filtered, |chain| {
            chain.disjunction && chain.root != roots[1][0]
        })
        .unwrap();
    assert!(selected.candidates().is_empty());
    assert!(filtered.spent < complete.spent);
    assert_eq!(selected.candidates.capacity(), 0);
    assert!(!selected.visited(roots[0][1]) && !selected.visited(roots[1][1]));
}

#[test]
fn every_refused_discovery_prefix_can_retry_without_stale_candidates() {
    let (theory, roots) = branches();
    let index = Narrower::new(&theory);
    let expected = chain_ids(&index, &roots[1]);
    let mut complete = Work::new(u64::MAX);
    Scratch::default()
        .parent_chains(&index, 0, &mut complete, |chain| chain.disjunction)
        .unwrap();
    for limit in 0..complete.spent {
        let mut scratch = Scratch::default();
        let mut work = Work::new(limit);
        assert_eq!(
            scratch.parent_chains(&index, 0, &mut work, |chain| chain.disjunction),
            Err(zetesis_cpu::Stop::WorkLimit),
        );
        assert_eq!(work.spent, limit);
        scratch
            .parent_chains(&index, 0, &mut Work::new(u64::MAX), |chain| {
                chain.disjunction
            })
            .unwrap();
        assert_eq!(scratch.candidates(), expected);
    }
}
