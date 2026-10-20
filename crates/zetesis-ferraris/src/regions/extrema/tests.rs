//! Ordinal cuts preserve witnesses without one leaf table per guard.

use crate::test_support::push_committed;
use crate::{
    AdmissionLimits, EvaluationLimits, EvaluationWorkspace, ExtremumCutRule, ExtremumRow,
    FormulaNodes, FormulaParts, FrozenSubject, Interpretation, Limits, Narrower, Narrowing,
    NarrowingScratch, Node, NodeView, OriginalSubject, Region, RegionLimits, Theory, models,
    models_reduct,
};
use zetesis_cpu::{Cancellation, Stop};

mod order;

fn owner(atoms: usize) -> FormulaNodes {
    FormulaNodes::new(FormulaParts::new((0..atoms).map(Node::atom).collect(), vec![]).unwrap())
}

fn cuts(
    owner: &mut FormulaNodes,
    conditions: &[usize],
    ranks: &[u64],
    values: u64,
    guards: &[ExtremumCutRule],
) -> Vec<usize> {
    let mut transaction = owner.transaction();
    let range = transaction
        .push_extremum_cut_family_range(
            ExtremumRow {
                conditions,
                ranks,
                rank_count: values,
            },
            guards,
            usize::MAX,
            usize::MAX,
        )
        .unwrap();
    transaction.commit();
    range.collect()
}

fn finish(owner: FormulaNodes, atoms: usize, roots: Vec<usize>) -> Theory {
    Theory::new(atoms, owner.into_parts(), roots, AdmissionLimits::default()).unwrap()
}

fn close(theory: &Theory, region: &mut Region, original: Option<usize>) -> Narrowing {
    let index = Narrower::new(theory);
    let cancellation = Cancellation::default();
    let mut known = index.knowledge();
    let mut scratch = NarrowingScratch::default();
    if let Some(mask) = original {
        let candidate = Interpretation::new(
            theory,
            (0..theory.atom_count()).filter(|&atom| mask & (1 << atom) != 0),
        )
        .unwrap();
        let mut workspace = EvaluationWorkspace::default();
        let evaluation = workspace.evaluate(&candidate, EvaluationLimits::default(), &cancellation);
        let truth = evaluation.result.unwrap();
        index
            .narrow_frozen_known(
                FrozenSubject::new(theory, truth.truth()),
                region,
                &mut known,
                &mut scratch,
                RegionLimits::default(),
                &cancellation,
            )
            .unwrap()
            .0
    } else {
        index
            .narrow_known(
                OriginalSubject::new(theory, None),
                region,
                &mut known,
                &mut scratch,
                RegionLimits::default(),
                &cancellation,
            )
            .unwrap()
            .0
    }
}

#[test]
fn repeated_conditions_preserve_extreme_witnesses() {
    for guard in [ExtremumCutRule::Prefix(2), ExtremumCutRule::Suffix(3)] {
        let mut nodes = owner(2);
        let cut = cuts(&mut nodes, &[0, 1, 0], &[1, 2, 3], 5, &[guard])[0];
        let theory = finish(nodes, 2, vec![cut]);
        let mut region = Region::all_open(2);
        assert_ne!(close(&theory, &mut region, None), Narrowing::Refuted);
        assert_eq!(region.decision(0), Some(true));
        assert_eq!(region.decision(1), None);
    }
}

fn compound() -> Theory {
    let mut nodes = owner(3);
    let either = push_committed(&mut nodes, NodeView::Or(&[0, 1]));
    let cut = cuts(
        &mut nodes,
        &[either, 0, 2],
        &[1, 3, 2],
        5,
        &[ExtremumCutRule::Prefix(4)],
    )[0];
    let no = push_committed(&mut nodes, NodeView::False);
    let not_b = push_committed(&mut nodes, NodeView::Implies(1, no));
    let not_c = push_committed(&mut nodes, NodeView::Implies(2, no));
    finish(nodes, 3, vec![cut, not_b, not_c])
}

#[test]
fn composite_witness_units_survive_native_cuts() {
    let theory = compound();
    for original in [None, Some(1)] {
        let mut region = Region::all_open(3);
        assert_ne!(close(&theory, &mut region, original), Narrowing::Refuted);
        assert_eq!(region.decision(0), Some(true));
    }
}

#[test]
fn ordered_cuts_propagate_without_choosing_a_witness() {
    for suffix in [false, true] {
        let mut nodes = owner(4);
        let guards = if suffix {
            [ExtremumCutRule::Suffix(2), ExtremumCutRule::Suffix(1)]
        } else {
            [ExtremumCutRule::Prefix(3), ExtremumCutRule::Prefix(4)]
        };
        let roots = cuts(&mut nodes, &[0, 1, 2], &[1, 2, 3], 5, &guards);
        let observed = push_committed(&mut nodes, NodeView::Implies(roots[1], 3));
        let theory = finish(nodes, 4, vec![roots[0], observed]);
        let mut region = Region::all_open(4);
        assert_ne!(close(&theory, &mut region, None), Narrowing::Refuted);
        assert_eq!(region.decision(3), Some(true));
        assert!((0..3).all(|atom| region.decision(atom).is_none()));
    }
}

#[test]
fn equal_witness_cuts_transfer_between_different_rows() {
    let mut nodes = owner(4);
    let first = cuts(
        &mut nodes,
        &[0, 1, 2],
        &[1, 2, 3],
        5,
        &[ExtremumCutRule::Prefix(3)],
    )[0];
    let second = cuts(
        &mut nodes,
        &[2, 1, 0],
        &[0, 3, 4],
        6,
        &[ExtremumCutRule::Suffix(3)],
    )[0];
    let observed = push_committed(&mut nodes, NodeView::Implies(second, 3));
    let theory = finish(nodes, 4, vec![first, observed]);
    for original in [None, Some(9)] {
        let mut region = Region::all_open(4);
        assert_ne!(close(&theory, &mut region, original), Narrowing::Refuted);
        assert_eq!(region.decision(3), Some(true));
    }
}

#[test]
fn equal_compound_witnesses_transfer_to_boolean_chains() {
    let mut nodes = owner(3);
    let either = push_committed(&mut nodes, NodeView::Or(&[0, 1]));
    let cut = cuts(
        &mut nodes,
        &[either, 0],
        &[1, 2],
        4,
        &[ExtremumCutRule::Prefix(3)],
    )[0];
    let observed = push_committed(&mut nodes, NodeView::Implies(either, 2));
    let theory = finish(nodes, 3, vec![cut, observed]);
    let mut region = Region::all_open(3);
    assert_ne!(close(&theory, &mut region, None), Narrowing::Refuted);
    assert_eq!(region.decision(2), Some(true));
    assert_eq!(region.decision(0), None);
    assert_eq!(region.decision(1), None);
}

#[test]
fn unit_rows_receive_native_cut_witnesses() {
    let mut nodes = owner(4);
    let cut = cuts(
        &mut nodes,
        &[0, 1, 2],
        &[1, 2, 3],
        5,
        &[ExtremumCutRule::Prefix(4)],
    )[0];
    let mut transaction = nodes.transaction();
    let thresholds = transaction
        .push_threshold_family_range(&[0, 1, 2], &[1, 1, 1], &[1, 2], usize::MAX, usize::MAX)
        .unwrap();
    transaction.commit();
    let observed = push_committed(&mut nodes, NodeView::Implies(thresholds.start, 3));
    let theory = finish(nodes, 4, vec![cut, observed]);
    let mut region = Region::all_open(4);
    assert_ne!(close(&theory, &mut region, None), Narrowing::Refuted);
    assert_eq!(region.decision(3), Some(true));
}

#[test]
fn endpoint_cuts_respect_empty_extrema() {
    let mut nodes = owner(1);
    let roots = cuts(
        &mut nodes,
        &[],
        &[],
        5,
        &[
            ExtremumCutRule::Prefix(0),
            ExtremumCutRule::Prefix(4),
            ExtremumCutRule::Prefix(5),
            ExtremumCutRule::Suffix(0),
            ExtremumCutRule::Suffix(1),
            ExtremumCutRule::Suffix(5),
        ],
    );
    let false_node = push_committed(&mut nodes, NodeView::False);
    let mut required = vec![roots[2], roots[3]];
    for position in [0, 1, 4, 5] {
        required.push(push_committed(
            &mut nodes,
            NodeView::Implies(roots[position], false_node),
        ));
    }
    let theory = finish(nodes, 1, required);
    assert_ne!(
        close(&theory, &mut Region::all_open(1), None),
        Narrowing::Refuted
    );
}

#[test]
fn native_closure_preserves_arbitrary_frozen_models() {
    let theory = compound();
    let cancellation = Cancellation::default();
    for code in 0..27 {
        let mut before = Region::all_open(3);
        let mut choices = code;
        for atom in 0..3 {
            match choices % 3 {
                1 => assert!(before.hold(atom)),
                2 => assert!(before.cut(atom)),
                _ => {}
            }
            choices /= 3;
        }
        for outer in 0..=8 {
            let mut after = before.clone();
            let outcome = close(&theory, &mut after, (outer < 8).then_some(outer));
            let candidate =
                Interpretation::new(&theory, (0..3).filter(|&atom| outer & (1 << atom) != 0))
                    .unwrap();
            for inner in 0..8 {
                if (0..3).any(|atom| {
                    before
                        .decision(atom)
                        .is_some_and(|value| value != (inner & (1 << atom) != 0))
                }) {
                    continue;
                }
                let world =
                    Interpretation::new(&theory, (0..3).filter(|&atom| inner & (1 << atom) != 0))
                        .unwrap();
                let satisfies = if outer == 8 {
                    models(&theory, &world, Limits::default(), &cancellation)
                } else {
                    models_reduct(
                        &theory,
                        &candidate,
                        &world,
                        Limits::default(),
                        &cancellation,
                    )
                }
                .unwrap();
                if satisfies {
                    assert_ne!(
                        outcome,
                        Narrowing::Refuted,
                        "outer={outer},inner={inner},code={code}"
                    );
                    for atom in 0..3 {
                        if let Some(value) = after.decision(atom) {
                            assert_eq!(
                                value,
                                inner & (1 << atom) != 0,
                                "outer={outer},inner={inner},code={code}"
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn ordinal_indexes_do_not_retain_guard_leaf_products() {
    let count = 64usize;
    let mut nodes = owner(count);
    let conditions: Vec<_> = (0..count).collect();
    let ranks: Vec<_> = (1..=count as u64).collect();
    let guards: Vec<_> = (2..=count as u64 + 1)
        .map(ExtremumCutRule::Prefix)
        .collect();
    let cuts = cuts(&mut nodes, &conditions, &ranks, count as u64 + 2, &guards);
    let theory = finish(nodes, count, vec![]);
    let index = Narrower::new(&theory);
    assert_eq!(
        index
            .extrema
            .columns
            .iter()
            .map(|column| column.conditions.len())
            .sum::<usize>(),
        2 * count
    );
    assert!(index.extrema.incidences.len() <= 2 * count);
    assert_eq!(index.extrema.cuts.len(), count);
    for cut in cuts {
        assert!(index.atom_operands[cut].is_empty());
        assert!(index.parents.iter().all(|parents| !parents.contains(&cut)));
        assert!(index.chains[super::super::chain_position(index.chain_of[cut].unwrap())].ordinal());
    }
    assert!(index.extremum_index_bytes() < (count * count * size_of::<usize>()) as u128);
}

#[test]
fn failed_native_closure_reuses_only_clean_scratch() {
    let theory = compound();
    let index = Narrower::new(&theory);
    let cancellation = Cancellation::default();
    let mut scratch = NarrowingScratch::default();
    let mut region = Region::all_open(3);
    let result = index.narrow_known(
        OriginalSubject::new(&theory, None),
        &mut region,
        &mut index.knowledge(),
        &mut scratch,
        RegionLimits { max_work: 0 },
        &cancellation,
    );
    assert_eq!(result, Err(Stop::WorkLimit));
    let mut region = Region::all_open(3);
    index
        .narrow_known(
            OriginalSubject::new(&theory, None),
            &mut region,
            &mut index.knowledge(),
            &mut scratch,
            RegionLimits::default(),
            &cancellation,
        )
        .unwrap();
    assert_eq!(region.decision(0), Some(true));
}

#[test]
fn fingerprint_collisions_do_not_establish_equality() {
    let mut nodes = owner(9);
    // Equal cardinality, minimum, sum and xor; the witness sets differ.
    let first = cuts(
        &mut nodes,
        &[0, 3, 4, 7],
        &[1, 1, 1, 1],
        3,
        &[ExtremumCutRule::Prefix(2)],
    )[0];
    let second = cuts(
        &mut nodes,
        &[0, 1, 6, 7],
        &[1, 1, 1, 1],
        3,
        &[ExtremumCutRule::Prefix(2)],
    )[0];
    let observed = push_committed(&mut nodes, NodeView::Implies(second, 8));
    let theory = finish(nodes, 9, vec![first, observed]);
    let mut region = Region::all_open(9);
    assert!(region.cut(8));
    assert_ne!(close(&theory, &mut region, None), Narrowing::Refuted);
    assert_eq!(region.decision(3), None);
    assert_eq!(region.decision(4), None);
    let model = Interpretation::new(&theory, [3]).unwrap();
    assert!(models(&theory, &model, Limits::default(), &Cancellation::default()).unwrap());
}
