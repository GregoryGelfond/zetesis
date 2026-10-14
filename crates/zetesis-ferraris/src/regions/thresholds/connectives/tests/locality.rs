//! Only anchors connected to shared same-kind geometry need ancestor discovery.

use super::*;

fn push(nodes: &mut FormulaNodes, node: NodeView<'_>) -> usize {
    let mut transaction = nodes.transaction();
    let result = transaction.push(node, usize::MAX, usize::MAX).unwrap();
    transaction.commit();
    result
}

/// The bridge fixture's five atoms stay separate from a shared component.
/// Extra untouched atoms make accidental whole-theory walker initialization
/// distinguishable from the handful of local parent probes.
fn disconnected(positive: bool) -> Theory {
    let original = witness(true, positive);
    let mut nodes = FormulaNodes::new(original.parts().try_clone().unwrap());
    let mut others = Vec::new();
    for atom in 5..69 {
        others.push(push(&mut nodes, NodeView::Atom(atom)));
    }
    let inner = push(&mut nodes, NodeView::Or(&others[..2]));
    push(&mut nodes, NodeView::Or(&[inner, others[2]]));
    push(&mut nodes, NodeView::Implies(inner, others[3]));
    Theory::new(
        69,
        nodes.into_parts(),
        original.roots().to_vec(),
        AdmissionLimits::default(),
    )
    .unwrap()
}

#[test]
fn unrelated_shared_components_keep_flat_discovery() {
    for positive in [false, true] {
        let theory = disconnected(positive);
        let index = Narrower::new(&theory);
        assert!(!index.witness_chains.is_empty());
        // Only the disconnected component has shared roles. Clearing those
        // roles supplies the existing flat-route control for the same theory.
        let mut flat = index.clone();
        flat.witness_chains.clear();
        let mut expected = Region::all_open(69);
        let reference = close(
            &theory,
            &flat,
            &mut expected,
            &mut flat.knowledge(),
            &mut NarrowingScratch::default(),
        );
        let mut actual = Region::all_open(69);
        let mut scratch = NarrowingScratch::default();
        let result = close(
            &theory,
            &index,
            &mut actual,
            &mut index.knowledge(),
            &mut scratch,
        );
        assert_eq!(result.0, reference.0);
        assert_eq!(actual, expected);
        assert_eq!(actual.decision(4), Some(positive));
        assert!((0..4).chain(5..69).all(|atom| actual.is_open(atom)));
        // One anchor and its direct flat parents are sufficient. In particular
        // no candidate/cone materialization is performed for this bridge.
        assert!(scratch.witnesses.candidates().is_empty());
        assert!(
            result.1.work <= reference.1.work + 8,
            "positive={positive}: actual={:?}, flat={:?}",
            result.1,
            reference.1,
        );
    }
}

/// The native row names an intermediate cut [compound, 2, 3]. Its least
/// condition is 2, which is neither the outer chain's opaque nor expanded
/// minimum. Following the prefix's same-kind parents is therefore necessary.
fn connected(disjunction: bool) -> (Theory, usize) {
    let mut nodes =
        FormulaNodes::new(FormulaParts::new((0..5).map(Node::atom).collect(), Vec::new()).unwrap());
    let connective = |nodes: &mut FormulaNodes, children: &[usize]| {
        push(
            nodes,
            if disjunction {
                NodeView::Or(children)
            } else {
                NodeView::And(children)
            },
        )
    };
    let compound = connective(&mut nodes, &[0, 1]);
    let prefix = connective(&mut nodes, &[compound, 2]);
    let full = connective(&mut nodes, &[prefix, 3]);
    push(&mut nodes, NodeView::Implies(prefix, compound));
    let guards = nodes
        .push_threshold_family(
            &[compound, 2, 3],
            &[1, 1, 1],
            &[1, 3],
            usize::MAX,
            usize::MAX,
        )
        .unwrap();
    let falsum = push(&mut nodes, NodeView::False);
    let required = if disjunction {
        guards[0]
    } else {
        push(&mut nodes, NodeView::Implies(guards[1], falsum))
    };
    let operands = [full, 4];
    let observer = push(
        &mut nodes,
        if disjunction {
            NodeView::Implies(full, 4)
        } else {
            NodeView::Or(&operands)
        },
    );
    (
        Theory::new(
            5,
            nodes.into_parts(),
            vec![required, observer],
            AdmissionLimits::default(),
        )
        .unwrap(),
        compound,
    )
}

#[test]
fn mixed_cut_anchors_keep_dynamic_discovery() {
    for disjunction in [false, true] {
        let (theory, _) = connected(disjunction);
        let index = Narrower::new(&theory);
        let mut work = crate::regions::Work::new(u64::MAX);
        assert!(super::super::needs_ancestor_discovery(&index, 2, &mut work).unwrap());
        let mut region = Region::all_open(5);
        let result = close(
            &theory,
            &index,
            &mut region,
            &mut index.knowledge(),
            &mut NarrowingScratch::default(),
        );
        assert_ne!(result.0, Narrowing::Refuted);
        assert_eq!(region.decision(4), Some(true));
        assert!((0..4).all(|atom| region.is_open(atom)));
    }
}

#[test]
fn compound_anchors_keep_their_self_candidate() {
    for disjunction in [false, true] {
        let (theory, compound) = connected(disjunction);
        let index = Narrower::new(&theory);
        let mut work = crate::regions::Work::new(u64::MAX);
        assert!(super::super::needs_ancestor_discovery(&index, compound, &mut work).unwrap());
        assert_eq!(work.spent, 1);
    }
}

#[test]
fn flat_theories_need_no_geometry_probes() {
    let index = Narrower::new(&witness(true, true));
    assert!(index.witness_chains.is_empty());
    let mut work = crate::regions::Work::new(0);
    assert!(!super::super::needs_ancestor_discovery(&index, 0, &mut work).unwrap());
    assert_eq!(work.spent, 0);
}

#[test]
fn applicability_probes_preserve_typed_refusals() {
    let index = Narrower::new(&disconnected(true));
    let query = |work: &mut crate::regions::Work<'_>| {
        super::super::needs_ancestor_discovery(&index, 0, work)
    };
    let mut complete = crate::regions::Work::new(u64::MAX);
    assert!(!query(&mut complete).unwrap());
    assert!(complete.spent > 1);
    for remaining in 0..complete.spent {
        let mut bounded = crate::regions::Work::new(remaining);
        assert_eq!(query(&mut bounded), Err(Stop::WorkLimit));
        assert_eq!(bounded.spent, remaining);
        for failure in [Stop::WorkLimit, Stop::Cancelled, Stop::Deadline] {
            let mut quota = Allowance { remaining, failure };
            let mut interrupted = crate::regions::Work::reserved(&mut quota);
            assert_eq!(query(&mut interrupted), Err(failure));
            assert_eq!(interrupted.spent, remaining);
        }
        // The query retains no state that could survive a partial probe.
        let mut exact = crate::regions::Work::new(complete.spent);
        assert!(!query(&mut exact).unwrap());
        assert_eq!(exact.spent, complete.spent);
    }
}

#[test]
fn ordinal_parents_keep_their_discovery_order() {
    use crate::{ExtremumCut, ExtremumCutRule, ExtremumRow};

    let original = disconnected(true);
    let mut nodes = FormulaNodes::new(original.parts().try_clone().unwrap());
    for boundary in [3, 2] {
        push(
            &mut nodes,
            NodeView::ExtremumCut(ExtremumCut {
                row: ExtremumRow {
                    conditions: &[0, 1],
                    ranks: &[1, 2],
                    rank_count: 4,
                },
                rule: &ExtremumCutRule::Prefix(boundary),
            }),
        );
    }
    let theory = Theory::new(
        original.atom_count(),
        nodes.into_parts(),
        original.roots().to_vec(),
        AdmissionLimits::default(),
    )
    .unwrap();
    let index = Narrower::new(&theory);
    assert!(index.logical_parents(0).any(|parent| {
        index.chain_of[parent].is_some_and(|link| {
            let chain = crate::regions::chain_position(link);
            index.chains[chain].ordinal() && !crate::regions::witnesses::participates(&index, chain)
        })
    }));
    // These two guards deliberately have the reverse declaration/size order.
    // Locality must retain the established ordinal-parent discovery route.
    assert!(
        super::super::needs_ancestor_discovery(
            &index,
            0,
            &mut crate::regions::Work::new(u64::MAX),
        )
        .unwrap()
    );
}

#[test]
fn native_requirement_authentication_collects_no_cone_result() {
    use crate::regions::{Closure, Step, Width, Work, witnesses};

    for disjunction in [false, true] {
        let (theory, _) = connected(disjunction);
        let index = Narrower::new(&theory);
        assert_eq!(index.thresholds.rows.len(), 1);
        let mut without = index.clone();
        without.thresholds.connectives = None;
        let mut region = Region::all_open(5);
        let mut knowledge = without.knowledge();
        close(
            &theory,
            &without,
            &mut region,
            &mut knowledge,
            &mut NarrowingScratch::default(),
        );
        assert!((0..5).all(|atom| region.is_open(atom)));
        let Width::Compact16(known) = &mut knowledge.width else {
            panic!("the small witness uses compact counters");
        };
        let mut scratch = NarrowingScratch::default();
        scratch
            .prepare_thresholds(index.thresholds.rows.len())
            .unwrap();
        let result = Closure {
            known: known.borrow(),
            lists: &mut scratch,
        }
        .threshold_requirements(&index, 0, &mut Work::new(u64::MAX))
        .unwrap();
        assert!(result == Step::Changed);
        assert!(
            scratch
                .witnesses
                .candidates()
                .iter()
                .any(|&chain| witnesses::expands(&index, chain))
        );
        // This operation consumes membership marks only. Replacing its adapter
        // with the collecting cone preserves truth but fails this route check.
        assert!(scratch.witnesses.prepared().is_empty());
    }
}
