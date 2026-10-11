//! Class identity is authenticated once, including fingerprint collisions.

use super::EqualWitnesses;
use crate::regions::{Work, chain_position};
use crate::{
    AdmissionLimits, EvaluationLimits, EvaluationWorkspace, ExtremumCutRule, ExtremumRow,
    FormulaNodes, FormulaParts, FrozenSubject, Interpretation, Limits, Narrower, Narrowing,
    NarrowingScratch, Node, NodeView, Region, RegionLimits, Theory, models_reduct,
};
use zetesis_cpu::{Cancellation, Stop};

fn owner(atoms: usize) -> FormulaNodes {
    FormulaNodes::new(FormulaParts::new((0..atoms).map(Node::atom).collect(), vec![]).unwrap())
}

fn push(owner: &mut FormulaNodes, value: NodeView<'_>) -> usize {
    let mut transaction = owner.transaction();
    let node = transaction.push(value, usize::MAX, usize::MAX).unwrap();
    transaction.commit();
    node
}

fn cuts(owner: &mut FormulaNodes, conditions: &[usize], count: usize) -> Vec<usize> {
    let ranks = vec![1; conditions.len()];
    let rules = vec![ExtremumCutRule::Prefix(2); count];
    let mut transaction = owner.transaction();
    let nodes = transaction
        .push_extremum_cut_family_range(
            ExtremumRow {
                conditions,
                ranks: &ranks,
                rank_count: 3,
            },
            &rules,
            usize::MAX,
            usize::MAX,
        )
        .unwrap();
    transaction.commit();
    nodes.collect()
}

fn finish(nodes: FormulaNodes, atoms: usize, roots: Vec<usize>) -> Theory {
    Theory::new(atoms, nodes.into_parts(), roots, AdmissionLimits::default()).unwrap()
}

fn chain(index: &Narrower, node: usize) -> usize {
    chain_position(index.chain_of[node].unwrap())
}

fn members(index: &Narrower, node: usize) -> Vec<usize> {
    let mut result = index
        .equal_extrema
        .members(chain(index, node), &mut Work::new(u64::MAX))
        .unwrap()
        .to_vec();
    result.sort_unstable();
    result
}

#[test]
fn a_collision_bucket_retains_separate_exact_classes() {
    let mut nodes = owner(8);
    // Equal cardinality, minimum, sum and xor, but different leaf sets.
    let first = cuts(&mut nodes, &[0, 3, 4, 7], 1)[0];
    let second = cuts(&mut nodes, &[0, 1, 6, 7], 1)[0];
    let first_boolean = push(&mut nodes, NodeView::Or(&[7, 4, 3, 0, 3]));
    let second_boolean = push(&mut nodes, NodeView::Or(&[7, 6, 1, 0, 1]));
    let theory = finish(nodes, 8, vec![]);
    let index = Narrower::new(&theory);
    assert_eq!(index.equal_extrema.classes.len(), 2);
    assert_eq!(index.equal_extrema.members.len(), 4);
    assert_eq!(index.equal_extrema.by_chain.len(), 4);
    for (native, boolean) in [(first, first_boolean), (second, second_boolean)] {
        let mut expected = vec![chain(&index, native), chain(&index, boolean)];
        expected.sort_unstable();
        assert_eq!(members(&index, native), expected);
        assert_eq!(members(&index, boolean), expected);
    }
}

#[test]
fn eligible_collision_buckets_preserve_boolean_subgroups() {
    let mut nodes = owner(8);
    cuts(&mut nodes, &[0, 3, 4, 7], 1);
    let first = push(&mut nodes, NodeView::Or(&[0, 1, 6, 7]));
    let second = push(&mut nodes, NodeView::Or(&[7, 6, 1, 0]));
    let theory = finish(nodes, 8, vec![]);
    let index = Narrower::new(&theory);
    // The old runtime candidate bucket admitted this exact Boolean equality,
    // even though its colliding native member has a different leaf set.
    assert_eq!(index.equal_extrema.classes.len(), 1);
    let mut expected = vec![chain(&index, first), chain(&index, second)];
    expected.sort_unstable();
    assert_eq!(members(&index, first), expected);
    assert_eq!(members(&index, second), expected);
}

#[test]
fn unmatched_collision_members_do_not_form_a_class() {
    let mut nodes = owner(8);
    let native = cuts(&mut nodes, &[0, 3, 4, 7], 1)[0];
    let boolean = push(&mut nodes, NodeView::Or(&[0, 1, 6, 7]));
    let theory = finish(nodes, 8, vec![]);
    let index = Narrower::new(&theory);
    assert!(members(&index, native).is_empty());
    assert!(members(&index, boolean).is_empty());
    assert_eq!(index.equal_extrema.retained_bytes(), 0);
}

fn repeated_class(atoms: usize) -> Theory {
    let mut nodes = owner(atoms);
    let conditions: Vec<_> = (0..atoms).collect();
    cuts(&mut nodes, &conditions, 32);
    push(&mut nodes, NodeView::Or(&conditions));
    finish(nodes, atoms, vec![])
}

#[test]
fn class_storage_depends_on_members_not_expanded_leaves() {
    let mut receipts = Vec::new();
    for atoms in [16, 128] {
        let theory = repeated_class(atoms);
        let index = Narrower::new(&theory);
        let classes = &index.equal_extrema;
        assert_eq!(classes.members.len(), 33);
        assert_eq!(classes.classes.len(), 1);
        assert_eq!(classes.by_chain.len(), 33);
        let expected = classes.members.capacity() * size_of::<usize>()
            + classes.classes.capacity() * size_of::<std::ops::Range<usize>>()
            + classes.by_chain.capacity() * size_of::<(usize, usize)>();
        assert_eq!(classes.retained_bytes(), expected as u128);
        receipts.push((classes.retained_bytes(), classes.preparation_work()));
    }
    assert_eq!(receipts[0].0, receipts[1].0);
    assert!(receipts[0].1 < receipts[1].1);
}

#[test]
fn authentication_refuses_every_incomplete_work_prefix() {
    let mut nodes = owner(8);
    cuts(&mut nodes, &[0, 3, 4, 7], 1);
    cuts(&mut nodes, &[0, 1, 6, 7], 1);
    push(&mut nodes, NodeView::Or(&[7, 4, 3, 0, 3]));
    push(&mut nodes, NodeView::Or(&[7, 6, 1, 0, 1]));
    let theory = finish(nodes, 8, vec![]);
    let index = Narrower::new(&theory);
    let expected = &index.equal_extrema;
    let receipt = expected.preparation_work();
    // This checks the private metered operation, not a new constructor quota.
    for budget in 0..receipt {
        let mut work = Work::new(budget);
        assert!(matches!(
            EqualWitnesses::prepare_with_work(&index, &mut work),
            Err(Stop::WorkLimit)
        ));
        assert_eq!(work.spent, budget);
    }
    let mut work = Work::new(receipt);
    let actual = EqualWitnesses::prepare_with_work(&index, &mut work).unwrap();
    assert_eq!(actual.members, expected.members);
    assert_eq!(actual.classes, expected.classes);
    assert_eq!(actual.by_chain, expected.by_chain);
    assert_eq!(work.spent, receipt);
}

#[test]
fn exact_classes_preserve_arbitrary_frozen_models() {
    let mut nodes = owner(3);
    let either = push(&mut nodes, NodeView::Or(&[0, 1]));
    let cut = cuts(&mut nodes, &[either, 0], 1)[0];
    let observed = push(&mut nodes, NodeView::Implies(either, 2));
    let theory = finish(nodes, 3, vec![cut, observed]);
    let index = Narrower::new(&theory);
    assert_eq!(index.equal_extrema.classes.len(), 1);
    assert_eq!(members(&index, cut), members(&index, either));
    let cancellation = Cancellation::default();
    for outer in 0..8 {
        let candidate =
            Interpretation::new(&theory, (0..3).filter(|&atom| outer & (1 << atom) != 0)).unwrap();
        let mut workspace = EvaluationWorkspace::default();
        let evaluation = workspace.evaluate(&candidate, EvaluationLimits::default(), &cancellation);
        let truth = evaluation.result.unwrap();
        for inner in 0..8 {
            let world =
                Interpretation::new(&theory, (0..3).filter(|&atom| inner & (1 << atom) != 0))
                    .unwrap();
            if !models_reduct(
                &theory,
                &candidate,
                &world,
                Limits::default(),
                &cancellation,
            )
            .unwrap()
            {
                continue;
            }
            let mut region = Region::all_open(3);
            if inner & 2 == 0 {
                assert!(region.cut(1));
            } else {
                assert!(region.hold(1));
            }
            let (outcome, _) = index
                .narrow_frozen_known(
                    FrozenSubject::new(&theory, truth.truth()),
                    &mut region,
                    &mut index.knowledge(),
                    &mut NarrowingScratch::default(),
                    RegionLimits::default(),
                    &cancellation,
                )
                .unwrap();
            assert_ne!(outcome, Narrowing::Refuted, "M={outer}, J={inner}");
            for atom in 0..3 {
                if let Some(value) = region.decision(atom) {
                    assert_eq!(value, inner & (1 << atom) != 0, "M={outer}, J={inner}");
                }
            }
        }
    }
}

#[test]
fn collision_partition_preserves_target_chain_order() {
    let mut nodes = owner(8);
    let first = [0, 3, 4, 7];
    let second = [0, 1, 6, 7];
    // The bucket's class order is A, B, B, A, B. Pulling the second A
    // forward swaps the first B behind the second B before B is partitioned.
    let roots = [
        cuts(&mut nodes, &first, 1)[0],
        cuts(&mut nodes, &second, 1)[0],
        cuts(&mut nodes, &second, 1)[0],
        cuts(&mut nodes, &first, 1)[0],
        cuts(&mut nodes, &second, 1)[0],
    ];
    let theory = finish(nodes, 8, vec![]);
    let index = Narrower::new(&theory);
    assert_eq!(index.equal_extrema.classes.len(), 2);
    let expected = [roots[1], roots[2], roots[4]].map(|node| chain(&index, node));
    assert!(expected.windows(2).all(|pair| pair[0] < pair[1]));
    // Inspect the actual runtime slice; the convenience helper sorts a copy.
    let actual = index
        .equal_extrema
        .members(chain(&index, roots[4]), &mut Work::new(u64::MAX))
        .unwrap();
    assert_eq!(actual, expected);
}
