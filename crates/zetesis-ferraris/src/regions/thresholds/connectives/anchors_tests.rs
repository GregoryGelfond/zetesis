//! Sparse anchor discovery does not authenticate or retain relation pairs.

use std::mem::size_of;

use super::{Chain, Connectives, Row, Work, anchor_keys};
use crate::Threshold;
use zetesis_cpu::Stop;

fn row(conditions: &[usize], weights: &[u64]) -> Row {
    Row::new(Threshold {
        conditions,
        weights,
        bound: 1,
    })
    .unwrap()
}

fn chain(leaves: &[usize]) -> Chain {
    Chain {
        disjunction: false,
        root: 0,
        operands: crate::regions::ChainOperands::Owned(leaves.to_vec()),
    }
}

#[test]
fn no_unit_rows_have_no_anchor_storage() {
    for rows in [vec![], vec![row(&[], &[])], vec![row(&[0, 1], &[2, 1])]] {
        let keys = anchor_keys(&rows).unwrap();
        assert!(keys.is_empty());
        assert_eq!(keys.capacity(), 0);
        // The prepare branch returns before calling chain_anchors.
        assert!(
            Connectives::prepare(
                &rows,
                &[chain(&[1, 0])],
                &[],
                &crate::regions::extrema::Extrema::default(),
                false
            )
            .unwrap()
            .is_none()
        );
    }
}

#[test]
fn native_receipt_includes_shared_anchor_storage() {
    use crate::{
        AdmissionLimits, ExtremumCut, ExtremumCutRule, ExtremumRow, FormulaNodes, FormulaParts,
        Narrower, Node, NodeView, Theory,
    };
    let mut nodes =
        FormulaNodes::new(FormulaParts::new(vec![Node::atom(0), Node::atom(1)], vec![]).unwrap());
    let mut transaction = nodes.transaction();
    transaction
        .push(
            NodeView::Threshold(Threshold {
                conditions: &[0, 1],
                weights: &[1, 1],
                bound: 1,
            }),
            10,
            10,
        )
        .unwrap();
    transaction
        .push(
            NodeView::ExtremumCut(ExtremumCut {
                row: ExtremumRow {
                    conditions: &[0, 1],
                    ranks: &[1, 2],
                    rank_count: 4,
                },
                rule: &ExtremumCutRule::Prefix(3),
            }),
            10,
            10,
        )
        .unwrap();
    transaction.commit();
    let theory = Theory::new(2, nodes.into_parts(), vec![], AdmissionLimits::default()).unwrap();
    let mut index = Narrower::new(&theory);
    let before = index.extremum_index_bytes();
    let anchors = index.thresholds.connectives.take().unwrap();
    assert!(
        anchors
            .by_chain
            .iter()
            .any(|&(chain, _)| index.chains[chain].ordinal())
    );
    let payload = anchors.rows.retained_bytes()
        + anchors.chains.retained_bytes()
        + anchors.anchors.capacity() as u128 * size_of::<usize>() as u128
        + anchors.by_chain.capacity() as u128 * size_of::<(usize, usize)>() as u128
        + anchors.anchored_cones.capacity() as u128 * size_of::<u64>() as u128;
    assert!(payload > 0);
    assert_eq!(before - index.extremum_index_bytes(), payload);
}

#[test]
fn compact_postings_record_each_owner_once() {
    let count = 128;
    let rows: Vec<_> = (0..count).map(|_| row(&[0, 1], &[1, 1])).collect();
    let chains: Vec<_> = (0..count).map(|_| chain(&[1, 0])).collect();
    let anchors = Connectives::prepare(
        &rows,
        &chains,
        &[],
        &crate::regions::extrema::Extrema::default(),
        false,
    )
    .unwrap()
    .unwrap();
    assert_eq!(anchors.anchors, [0]);
    assert_eq!(anchors.rows.len(), 1);
    assert_eq!(anchors.chains.len(), 1);
    assert_eq!(anchors.rows[0], (0..count).collect::<Vec<_>>());
    assert_eq!(anchors.chains[0], (0..count).collect::<Vec<_>>());
    assert_eq!(
        anchors.by_chain,
        (0..count).map(|chain| (chain, 0)).collect::<Vec<_>>()
    );
    // Every pair really qualifies, yet only owner postings and a reverse
    // projection are retained: no count * count authenticated relation table.
    assert_eq!(
        anchors.rows.iter().map(<[usize]>::len).sum::<usize>(),
        count
    );
    assert_eq!(
        anchors.chains.iter().map(<[usize]>::len).sum::<usize>(),
        count
    );
    let payload = anchors.rows.retained_bytes()
        + anchors.chains.retained_bytes()
        + anchors.anchors.capacity() as u128 * size_of::<usize>() as u128
        + anchors.by_chain.capacity() as u128 * size_of::<(usize, usize)>() as u128;
    assert!(payload < (count * count * size_of::<usize>()) as u128);
}

#[test]
fn a_shared_parent_with_a_different_minimum_is_excluded() {
    let rows = [row(&[1, 2], &[1, 1])];
    let chains = [chain(&[0, 2, 1]), chain(&[2, 1])];
    let anchors = Connectives::prepare(
        &rows,
        &chains,
        &[],
        &crate::regions::extrema::Extrema::default(),
        false,
    )
    .unwrap()
    .unwrap();
    let mut work = Work::new(u64::MAX);
    // Both chains contain condition 1, but only the second may be an exact
    // prefix or extension. Exclusion occurs before exact_prefix is called.
    assert_eq!(anchors.chains(1, &mut work).unwrap(), [1]);
    assert!(anchors.rows(0, &mut work).unwrap().is_empty());
    assert_eq!(anchors.rows(1, &mut work).unwrap(), [0]);
    assert_eq!(chains[0].ordinary_leaves(), [0, 2, 1]);
    assert_eq!(chains[1].ordinary_leaves(), [2, 1]);
}

#[test]
fn repeated_anchors_preserve_both_owner_orders() {
    let rows = [
        row(&[40, 42], &[1, 1]),
        row(&[3, 5], &[1, 1]),
        row(&[40, 44], &[1, 1]),
        row(&[3, 4], &[2, 1]),
    ];
    let chains = [
        chain(&[5, 3]),
        chain(&[1, 3]),
        chain(&[44, 40]),
        chain(&[42, 40]),
        chain(&[9]),
    ];
    let anchors = Connectives::prepare(
        &rows,
        &chains,
        &[],
        &crate::regions::extrema::Extrema::default(),
        false,
    )
    .unwrap()
    .unwrap();
    let mut work = Work::new(u64::MAX);
    assert_eq!(anchors.anchors, [3, 40]);
    assert_eq!(anchors.rows(0, &mut work).unwrap(), [1]);
    assert_eq!(anchors.rows(2, &mut work).unwrap(), [0, 2]);
    assert_eq!(anchors.rows(3, &mut work).unwrap(), [0, 2]);
    assert_eq!(anchors.chains(3, &mut work).unwrap(), [0]);
    assert_eq!(anchors.chains(40, &mut work).unwrap(), [2, 3]);
    assert!(anchors.rows(1, &mut work).unwrap().is_empty());
    assert!(anchors.rows(4, &mut work).unwrap().is_empty());
    assert!(anchors.chains(9, &mut work).unwrap().is_empty());
}

#[test]
fn rebuilding_does_not_reuse_another_owners_chain_anchor() {
    let first = Connectives::prepare(
        &[row(&[3, 5], &[1, 1])],
        &[chain(&[5, 3])],
        &[],
        &crate::regions::extrema::Extrema::default(),
        false,
    )
    .unwrap()
    .unwrap();
    let second = Connectives::prepare(
        &[row(&[40, 42], &[1, 1])],
        &[chain(&[42, 40])],
        &[],
        &crate::regions::extrema::Extrema::default(),
        false,
    )
    .unwrap()
    .unwrap();
    let mut work = Work::new(u64::MAX);
    assert_eq!(first.chains(3, &mut work).unwrap(), [0]);
    assert!(first.chains(40, &mut work).unwrap().is_empty());
    assert_eq!(second.chains(40, &mut work).unwrap(), [0]);
    assert!(second.chains(3, &mut work).unwrap().is_empty());
    assert_eq!(first.rows(0, &mut work).unwrap(), [0]);
    assert_eq!(second.rows(0, &mut work).unwrap(), [0]);
}

#[test]
fn sparse_lookup_charges_every_attempted_comparison() {
    let rows = [row(&[1, 2], &[1, 1]), row(&[4, 5], &[1, 1])];
    let chains = [chain(&[2, 1]), chain(&[0, 3]), chain(&[5, 4])];
    let anchors = Connectives::prepare(
        &rows,
        &chains,
        &[],
        &crate::regions::extrema::Extrema::default(),
        false,
    )
    .unwrap()
    .unwrap();
    for reverse in [false, true] {
        for key in 0..7 {
            let query = |work: &mut Work<'_>| {
                if reverse {
                    anchors.rows(key, work)
                } else {
                    anchors.chains(key, work)
                }
            };
            let mut full = Work::new(u64::MAX);
            query(&mut full).unwrap();
            assert!(full.spent > 0);
            for limit in 0..full.spent {
                let mut limited = Work::new(limit);
                assert_eq!(query(&mut limited), Err(Stop::WorkLimit));
                assert_eq!(limited.spent, limit);
            }
            let mut exact = Work::new(full.spent);
            assert_eq!(query(&mut exact).unwrap(), query(&mut full).unwrap());
        }
    }
}

/// A chain's summary bit is set exactly when a row anchor lies in its same-kind
/// cone: through a shared chain of the same connective, but never through a
/// chain of the other connective, whose node is an opaque leaf of the cone.
/// The chain roots of [`cone_theory`], by node.
struct Cones {
    shared: usize,
    reaching: usize,
    also_reaching: usize,
    conjunction: usize,
    crossing: usize,
    hidden: usize,
    unanchored: usize,
}

/// One unit row over atoms 2 and 3, whose only anchor is node 2, beside
/// same-kind chains that do and do not reach it.
fn cone_theory() -> (crate::Theory, Cones) {
    use crate::{AdmissionLimits, FormulaNodes, FormulaParts, Node, NodeView, Theory};
    let mut nodes =
        FormulaNodes::new(FormulaParts::new((0..6).map(Node::atom).collect(), vec![]).unwrap());
    nodes
        .push_threshold_family(&[2, 3], &[1, 1], &[1], usize::MAX, usize::MAX)
        .unwrap();
    let mut append = nodes.transaction();
    // Each shared disjunction has two parents, so it roots its own chain.
    let shared = append
        .push(NodeView::Or(&[2, 4]), usize::MAX, usize::MAX)
        .unwrap();
    let reaching = append
        .push(NodeView::Or(&[0, shared]), usize::MAX, usize::MAX)
        .unwrap();
    let also_reaching = append
        .push(NodeView::Or(&[1, shared]), usize::MAX, usize::MAX)
        .unwrap();
    let conjunction = append
        .push(NodeView::And(&[2, 5]), usize::MAX, usize::MAX)
        .unwrap();
    let crossing = append
        .push(NodeView::Or(&[0, conjunction]), usize::MAX, usize::MAX)
        .unwrap();
    let hidden = append
        .push(NodeView::Or(&[4, 5]), usize::MAX, usize::MAX)
        .unwrap();
    let unanchored = append
        .push(NodeView::Or(&[0, hidden]), usize::MAX, usize::MAX)
        .unwrap();
    let also_unanchored = append
        .push(NodeView::Or(&[1, hidden]), usize::MAX, usize::MAX)
        .unwrap();
    append.commit();
    let theory = Theory::new(
        6,
        nodes.into_parts(),
        vec![
            reaching,
            also_reaching,
            crossing,
            unanchored,
            also_unanchored,
        ],
        AdmissionLimits::default(),
    )
    .unwrap();
    let cones = Cones {
        shared,
        reaching,
        also_reaching,
        conjunction,
        crossing,
        hidden,
        unanchored,
    };
    (theory, cones)
}

#[test]
fn cone_summary_follows_only_same_kind_chains_to_anchors() {
    let (theory, cones) = cone_theory();
    let index = crate::Narrower::new(&theory);
    let chain = |node: usize| super::chain_position(index.chain_of[node].expect("chain root"));
    let summary = index
        .thresholds
        .connectives
        .as_ref()
        .expect("a unit row and shared same-kind chains");
    assert!(summary.anchored_cone(chain(cones.shared)));
    assert!(summary.anchored_cone(chain(cones.reaching)));
    assert!(summary.anchored_cone(chain(cones.also_reaching)));
    assert!(summary.anchored_cone(chain(cones.conjunction)));
    assert!(!summary.anchored_cone(chain(cones.crossing)));
    assert!(!summary.anchored_cone(chain(cones.hidden)));
    assert!(!summary.anchored_cone(chain(cones.unanchored)));
}

#[test]
fn an_anchor_at_a_chain_root_marks_that_chain() {
    use crate::{AdmissionLimits, FormulaNodes, FormulaParts, Narrower, Node, NodeView, Theory};
    let mut nodes =
        FormulaNodes::new(FormulaParts::new((0..6).map(Node::atom).collect(), vec![]).unwrap());
    let mut append = nodes.transaction();
    let anchored = append
        .push(NodeView::Or(&[0, 1]), usize::MAX, usize::MAX)
        .unwrap();
    let other = append
        .push(NodeView::Or(&[4, 5]), usize::MAX, usize::MAX)
        .unwrap();
    // A second parent shares the anchored disjunction, which then roots its own
    // chain inside the outer one.
    let outer = append
        .push(NodeView::Or(&[anchored, 2]), usize::MAX, usize::MAX)
        .unwrap();
    append.commit();
    // The unit row's least condition, its anchor, is the compound `anchored`;
    // no leaf of that chain is an anchor, so only its root can mark it.
    let row = nodes
        .push_threshold_family(&[anchored, other], &[1, 1], &[1], usize::MAX, usize::MAX)
        .unwrap()[0];
    let theory = Theory::new(
        6,
        nodes.into_parts(),
        vec![row, outer],
        AdmissionLimits::default(),
    )
    .unwrap();
    let index = Narrower::new(&theory);
    let chain = |node: usize| super::chain_position(index.chain_of[node].expect("chain root"));
    let summary = index
        .thresholds
        .connectives
        .as_ref()
        .expect("a unit row and a shared same-kind chain");
    assert!(summary.anchored_cone(chain(anchored)));
    assert!(summary.anchored_cone(chain(outer)));
    assert!(!summary.anchored_cone(chain(other)));
}

#[test]
fn an_unanchored_chain_skips_connective_discovery() {
    use crate::regions::{Closure, Step, Width, witnesses};
    let (theory, cones) = cone_theory();
    let index = crate::Narrower::new(&theory);
    let chain = |node: usize| super::chain_position(index.chain_of[node].expect("chain root"));
    let mut knowledge = index.knowledge();
    let Width::Compact16(known) = &mut knowledge.width else {
        panic!("the small theory uses compact counters");
    };
    let mut discover = |node: usize| {
        let chain = chain(node);
        // Both chains take the expanded walk when discovery runs at all.
        assert!(witnesses::expands(&index, chain));
        let mut scratch = crate::NarrowingScratch::default();
        scratch
            .prepare_thresholds(index.thresholds.rows.len())
            .unwrap();
        let mut work = Work::new(u64::MAX);
        let step = Closure {
            known: known.borrow(),
            lists: &mut scratch,
        }
        .threshold_connective(&index, chain, true, &mut work)
        .unwrap();
        (step, work.spent)
    };
    // The contrast: an anchored chain's discovery walks its cone and probes
    // anchors, charging work, though it finds nothing to transport here.
    assert!(discover(cones.reaching).1 > 0);
    // The unanchored chain returns before its walk, charging nothing.
    assert!(discover(cones.unanchored) == (Step::Unchanged, 0));
}
