//! Shared associative views retain model soundness and the scalar witnesses.

mod coverage;
mod marking;
mod notifications;
mod parents;
mod streaming;

use crate::{
    AdmissionLimits, AggregateComparison, AggregateExtremum, AggregateFamilyLimits,
    AggregateLimits, FormulaNodes, FormulaParts, Interpretation, Limits, Narrower, Narrowing,
    NarrowingScratch, Node, NodeView, OriginalSubject, Region, RegionLimits, Theory,
    ValueExtremumElement, ValueExtremumGuard, append_value_extremum_family_refs,
    append_value_extremum_refs, models,
};
use zetesis_core::{Value, catalog::TermRef};
use zetesis_cpu::Cancellation;

#[derive(Clone, Copy, Debug)]
enum Preparation {
    Scalar,
    Shared,
}

fn push(nodes: &mut FormulaNodes, node: NodeView<'_>) -> usize {
    let limits = AdmissionLimits::default();
    let mut transaction = nodes.transaction();
    let index = transaction
        .push(node, limits.max_nodes, limits.max_operands)
        .unwrap();
    transaction.commit();
    index
}

fn theory(preparation: Preparation, repeated_condition: bool) -> Theory {
    let cancellation = Cancellation::default();
    let mut nodes =
        FormulaNodes::new(FormulaParts::new((0..4).map(Node::atom).collect(), Vec::new()).unwrap());
    // This is exactly the complete equality guard roster of an extrema
    // assignment: every possible first value, followed by the empty endpoint.
    let values = [Value::Number(1), Value::Number(2), Value::Number(3)];
    let endpoint = Value::Supremum;
    let conditions = if repeated_condition {
        [0, 1, 0]
    } else {
        [0, 1, 2]
    };
    let elements: Vec<_> = values
        .iter()
        .zip(conditions)
        .map(|(value, condition)| ValueExtremumElement {
            value: TermRef::from(value),
            condition,
        })
        .collect();
    let guards: Vec<_> = values
        .iter()
        .chain(std::iter::once(&endpoint))
        .map(|bound| ValueExtremumGuard {
            comparison: AggregateComparison::Eq,
            bound: TermRef::from(bound),
        })
        .collect();
    let guards: Vec<_> = match preparation {
        Preparation::Scalar => guards
            .iter()
            .map(|guard| {
                append_value_extremum_refs(
                    &mut nodes,
                    elements.iter().cloned(),
                    AggregateExtremum::Min,
                    guard.comparison,
                    guard.bound,
                    AggregateLimits::default(),
                    &cancellation,
                )
                .unwrap()
                .root()
            })
            .collect(),
        Preparation::Shared => append_value_extremum_family_refs(
            &mut nodes,
            &elements,
            AggregateExtremum::Min,
            &guards,
            AggregateFamilyLimits::default(),
            &cancellation,
        )
        .result
        .unwrap()
        .roots()
        .to_vec(),
    };
    let limits = AdmissionLimits::default();
    // Keep both numerical guards native. A scalar >=1 construction may instead
    // emit an OR and would not test the native/Boolean bridge at issue here.
    let row_conditions: &[usize] = if repeated_condition {
        &[0, 1]
    } else {
        &[0, 1, 2]
    };
    let row = nodes
        .push_threshold_family(
            row_conditions,
            &vec![1; row_conditions.len()],
            &[1, 2],
            limits.max_nodes,
            limits.max_operands,
        )
        .unwrap();
    let falsum = push(&mut nodes, NodeView::False);
    let nonempty = push(&mut nodes, NodeView::Implies(guards[3], falsum));
    // The observer makes the inferred truth of the >=1 native guard visible as
    // an atom decision; it does not itself require that guard or any input atom.
    let observed = push(&mut nodes, NodeView::Implies(row[0], 3));
    let mut roots = vec![nonempty, observed];
    if repeated_condition {
        roots.push(push(&mut nodes, NodeView::Implies(1, falsum)));
    }
    Theory::new(4, nodes.into_parts(), roots, limits).unwrap()
}

fn close(theory: &Theory) -> (Narrowing, Region) {
    let narrower = Narrower::new(theory);
    let mut region = Region::all_open(4);
    let (outcome, _) = narrower
        .narrow_known(
            OriginalSubject::new(theory, None),
            &mut region,
            &mut narrower.knowledge(),
            &mut NarrowingScratch::default(),
            RegionLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    (outcome, region)
}

fn check_models(theory: &Theory, repeated_condition: bool, closure: &(Narrowing, Region)) {
    let cancellation = Cancellation::default();
    for mask in 0_u32..16 {
        let candidate =
            Interpretation::new(theory, (0..4).filter(|&atom| mask & (1 << atom) != 0)).unwrap();
        let actual = models(theory, &candidate, Limits::default(), &cancellation).unwrap();
        let expected = if repeated_condition {
            mask & 1 != 0 && mask & 2 == 0 && mask & 8 != 0
        } else {
            mask & 7 != 0 && mask & 8 != 0
        };
        assert_eq!(actual, expected, "mask={mask}");
        if actual {
            assert_ne!(closure.0, Narrowing::Refuted);
            for atom in 0..4 {
                if let Some(value) = closure.1.decision(atom) {
                    assert_eq!(value, mask & (1 << atom) != 0, "atom={atom}, mask={mask}");
                }
            }
        }
    }
}

#[test]
fn shared_equality_family_preserves_the_native_nonempty_bound() {
    let scalar = theory(Preparation::Scalar, false);
    let shared = theory(Preparation::Shared, false);
    let scalar_closure = close(&scalar);
    let shared_closure = close(&shared);
    check_models(&scalar, false, &scalar_closure);
    check_models(&shared, false, &shared_closure);
    assert_eq!(scalar_closure.1.decision(3), Some(true));
    assert_eq!(shared_closure.1.decision(3), Some(true));
    assert!((0..3).all(|atom| scalar_closure.1.is_open(atom)));
    assert!((0..3).all(|atom| shared_closure.1.is_open(atom)));
}

#[test]
fn shared_equality_family_preserves_repeated_condition_units() {
    let scalar = theory(Preparation::Scalar, true);
    let shared = theory(Preparation::Shared, true);
    let scalar_closure = close(&scalar);
    let shared_closure = close(&shared);
    check_models(&scalar, true, &scalar_closure);
    check_models(&shared, true, &shared_closure);
    assert_eq!(scalar_closure.1.decision(0), Some(true));
    // The extra occurrence is a different tuple with the same condition. The
    // min/max measure is unchanged; Boolean unit consequences should be too.
    assert_eq!(shared_closure.1.decision(0), Some(true));
}

fn compound_witness() -> Theory {
    // c=(a or b) has two parents, so neither the c nor outer witness chain
    // can absorb it. Repeated a must still count once in the outer union.
    let nodes = FormulaParts::new(
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::atom(2),
            Node::or_pair([0, 1]),
            Node::or_pair([3, 0]),
            Node::implies(3, 2),
        ],
        Vec::new(),
    )
    .unwrap();
    Theory::new(3, nodes, vec![4, 5], AdmissionLimits::default()).unwrap()
}

#[test]
fn an_undecided_shared_parent_forwards_a_child_wakeup() {
    let theory = compound_witness();
    let narrower = Narrower::new(&theory);
    let mut known = narrower.knowledge();
    let mut scratch = NarrowingScratch::default();
    let mut region = Region::all_open(3);
    let close =
        |region: &mut Region, known: &mut crate::Knowledge, scratch: &mut NarrowingScratch| {
            narrower
                .narrow_known(
                    OriginalSubject::new(&theory, None),
                    region,
                    known,
                    scratch,
                    RegionLimits::default(),
                    &Cancellation::default(),
                )
                .unwrap()
                .0
        };
    assert_ne!(
        close(&mut region, &mut known, &mut scratch),
        Narrowing::Refuted
    );
    assert!(region.is_open(0) && region.is_open(1));
    assert!(region.cut(1));
    let mut fresh = region.clone();
    assert_ne!(
        close(&mut region, &mut known, &mut scratch),
        Narrowing::Refuted
    );
    assert_ne!(
        close(
            &mut fresh,
            &mut narrower.knowledge(),
            &mut NarrowingScratch::default()
        ),
        Narrowing::Refuted
    );
    assert_eq!(region, fresh);
    assert_eq!(region.decision(0), Some(true));
    assert_eq!(region.decision(2), Some(true));
    assert!(scratch.witnesses.dirty.is_empty());
    assert!(scratch.witnesses.pending.iter().all(|&word| word == 0));
}

#[test]
fn associative_units_preserve_every_original_and_frozen_model() {
    use crate::{EvaluationLimits, EvaluationWorkspace, FrozenSubject, models_reduct};
    let theory = compound_witness();
    let narrower = Narrower::new(&theory);
    let cancellation = Cancellation::default();
    let mut scratch = NarrowingScratch::default();
    for code in 0..27 {
        let mut region = Region::all_open(3);
        let mut choices = code;
        for atom in 0..3 {
            match choices % 3 {
                1 => assert!(region.hold(atom)),
                2 => assert!(region.cut(atom)),
                _ => {}
            }
            choices /= 3;
        }
        let before = region.clone();
        for outer in 0..=8 {
            let mut after = before.clone();
            let candidate =
                Interpretation::new(&theory, (0..3).filter(|&atom| outer & (1 << atom) != 0))
                    .unwrap();
            let mut workspace = EvaluationWorkspace::default();
            let evaluation =
                workspace.evaluate(&candidate, EvaluationLimits::default(), &cancellation);
            let truth = evaluation.result.unwrap();
            let outcome = if outer == 8 {
                narrower.narrow_known(
                    OriginalSubject::new(&theory, None),
                    &mut after,
                    &mut narrower.knowledge(),
                    &mut scratch,
                    RegionLimits::default(),
                    &cancellation,
                )
            } else {
                narrower.narrow_frozen_known(
                    FrozenSubject::new(&theory, truth.truth()),
                    &mut after,
                    &mut narrower.knowledge(),
                    &mut scratch,
                    RegionLimits::default(),
                    &cancellation,
                )
            }
            .unwrap()
            .0;
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
                        "outer={outer}, inner={inner}, code={code}"
                    );
                    for atom in 0..3 {
                        if let Some(value) = after.decision(atom) {
                            assert_eq!(value, inner & (1 << atom) != 0);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn associative_scratch_recovers_after_refusal_and_generation_wrap() {
    use super::{Scratch, Work};
    let theory = compound_witness();
    let narrower = Narrower::new(&theory);
    let chain = super::chain_position(narrower.chain_of[4].unwrap());
    let mut scratch = Scratch::default();
    assert_eq!(
        scratch.read(&narrower, chain, None, &mut Work::new(0)),
        Err(zetesis_cpu::Stop::WorkLimit)
    );
    assert_eq!(
        scratch
            .read(&narrower, chain, None, &mut Work::new(u64::MAX))
            .unwrap(),
        &[0, 1]
    );
    scratch.generation = u32::MAX;
    assert_eq!(
        scratch
            .read(&narrower, chain, None, &mut Work::new(u64::MAX))
            .unwrap(),
        &[0, 1]
    );
    scratch.prepare_dirty(narrower.chains.len()).unwrap();
    scratch.queue_dirty(chain);
    scratch.queue_dirty(chain);
    assert_eq!(scratch.dirty.len(), 1);
    scratch.clear();
    assert!(scratch.dirty.is_empty());
    assert!(scratch.pending.iter().all(|&word| word == 0));
    assert_eq!(
        Scratch::default().prepare_dirty(usize::MAX),
        Err(zetesis_cpu::Stop::Allocation)
    );
}

#[test]
fn shared_conjunctions_retain_the_dual_unit_consequence() {
    let parts = FormulaParts::new(
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::atom(2),
            Node::and_pair([0, 1]),
            Node::and_pair([3, 0]),
            Node::implies(2, 3),
            Node::falsum(),
            Node::implies(4, 6),
        ],
        Vec::new(),
    )
    .unwrap();
    let theory = Theory::new(3, parts, vec![5, 7], AdmissionLimits::default()).unwrap();
    let index = Narrower::new(&theory);
    let mut region = Region::all_open(3);
    assert!(region.hold(1));
    let (outcome, _) = index
        .narrow_known(
            OriginalSubject::new(&theory, None),
            &mut region,
            &mut index.knowledge(),
            &mut NarrowingScratch::default(),
            RegionLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    assert_ne!(outcome, Narrowing::Refuted);
    assert_eq!(region.decision(0), Some(false));
    assert_eq!(region.decision(2), Some(false));
}

#[test]
fn opaque_and_expanded_native_bridges_both_survive() {
    let mut nodes =
        FormulaNodes::new(FormulaParts::new((0..5).map(Node::atom).collect(), Vec::new()).unwrap());
    let inner = push(&mut nodes, NodeView::Or(&[0, 1]));
    let outer = push(&mut nodes, NodeView::Or(&[inner, 2]));
    let opaque = nodes
        .push_threshold_family(&[inner, 2], &[1, 1], &[1, 2], usize::MAX, usize::MAX)
        .unwrap();
    let expanded = nodes
        .push_threshold_family(&[0, 1, 2], &[1, 1, 1], &[1, 2], usize::MAX, usize::MAX)
        .unwrap();
    let left = push(&mut nodes, NodeView::Implies(opaque[0], 3));
    let right = push(&mut nodes, NodeView::Implies(expanded[0], 4));
    let theory = Theory::new(
        5,
        nodes.into_parts(),
        vec![outer, left, right],
        AdmissionLimits::default(),
    )
    .unwrap();
    let index = Narrower::new(&theory);
    let mut region = Region::all_open(5);
    let (outcome, _) = index
        .narrow_known(
            OriginalSubject::new(&theory, None),
            &mut region,
            &mut index.knowledge(),
            &mut NarrowingScratch::default(),
            RegionLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    assert_ne!(outcome, Narrowing::Refuted);
    assert_eq!(region.decision(3), Some(true));
    assert_eq!(region.decision(4), Some(true));
    assert!((0..3).all(|atom| region.is_open(atom)));
}

#[derive(Clone, Copy)]
enum Transfer {
    BooleanToNative,
    NativeToBoolean,
}

/// C=(a op x), P=(C op b), Q=(P op d). P is shared, while the
/// native row names [C,b,d]. Its anchor b is neither the opaque minimum d
/// of Q nor its fully unfolded minimum a. A native condition can therefore
/// be an intermediate cut through a same-kind cone.
fn mixed_cut(disjunction: bool, transfer: Transfer) -> Theory {
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
    // This unasserted consumer keeps P shared without restricting its models.
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
    let row = guards[usize::from(!disjunction)];
    let (source, target) = match transfer {
        Transfer::BooleanToNative => (full, row),
        Transfer::NativeToBoolean => (row, full),
    };
    let required = if disjunction {
        source
    } else {
        push(&mut nodes, NodeView::Implies(source, falsum))
    };
    // Expose the transported requirement through observer atom 4.
    let operands = [target, 4];
    let observer = push(
        &mut nodes,
        if disjunction {
            NodeView::Implies(target, 4)
        } else {
            NodeView::Or(&operands)
        },
    );
    Theory::new(
        5,
        nodes.into_parts(),
        vec![required, observer],
        AdmissionLimits::default(),
    )
    .unwrap()
}

#[test]
fn compound_cuts_transport_boolean_witnesses() {
    for disjunction in [false, true] {
        for transfer in [Transfer::BooleanToNative, Transfer::NativeToBoolean] {
            let theory = mixed_cut(disjunction, transfer);
            let index = Narrower::new(&theory);
            let mut region = Region::all_open(5);
            let (outcome, _) = index
                .narrow_known(
                    OriginalSubject::new(&theory, None),
                    &mut region,
                    &mut index.knowledge(),
                    &mut NarrowingScratch::default(),
                    RegionLimits::default(),
                    &Cancellation::default(),
                )
                .unwrap();
            assert_ne!(outcome, Narrowing::Refuted);
            assert_eq!(region.decision(4), Some(true));
            assert!((0..4).all(|atom| region.is_open(atom)));
        }
    }
}

#[test]
fn compound_cut_inference_preserves_original_models() {
    let cancellation = Cancellation::default();
    for disjunction in [false, true] {
        for transfer in [Transfer::BooleanToNative, Transfer::NativeToBoolean] {
            let theory = mixed_cut(disjunction, transfer);
            let index = Narrower::new(&theory);
            let worlds: Vec<_> = (0_u32..32)
                .map(|mask| {
                    let world = Interpretation::new(
                        &theory,
                        (0..5).filter(|&atom| mask & (1 << atom) != 0),
                    )
                    .unwrap();
                    (
                        mask,
                        models(&theory, &world, Limits::default(), &cancellation).unwrap(),
                    )
                })
                .collect();
            let mut scratch = NarrowingScratch::default();
            for code in 0..243 {
                let mut before = Region::all_open(5);
                let mut choices = code;
                for atom in 0..5 {
                    match choices % 3 {
                        1 => assert!(before.hold(atom)),
                        2 => assert!(before.cut(atom)),
                        _ => {}
                    }
                    choices /= 3;
                }
                let mut after = before.clone();
                let (outcome, _) = index
                    .narrow_known(
                        OriginalSubject::new(&theory, None),
                        &mut after,
                        &mut index.knowledge(),
                        &mut scratch,
                        RegionLimits::default(),
                        &cancellation,
                    )
                    .unwrap();
                for &(mask, satisfies) in &worlds {
                    if !satisfies
                        || (0..5).any(|atom| {
                            before
                                .decision(atom)
                                .is_some_and(|value| value != (mask & (1 << atom) != 0))
                        })
                    {
                        continue;
                    }
                    assert_ne!(outcome, Narrowing::Refuted, "region={code}, mask={mask}");
                    for atom in 0..5 {
                        if let Some(value) = after.decision(atom) {
                            assert_eq!(value, mask & (1 << atom) != 0);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn compound_cut_inference_leaves_arbitrary_frozen_worlds_sound() {
    use crate::{EvaluationLimits, EvaluationWorkspace, FrozenSubject, models_reduct};
    let cancellation = Cancellation::default();
    for disjunction in [false, true] {
        for transfer in [Transfer::BooleanToNative, Transfer::NativeToBoolean] {
            let theory = mixed_cut(disjunction, transfer);
            let index = Narrower::new(&theory);
            let mut scratch = NarrowingScratch::default();
            for outer in 0_u32..32 {
                let candidate =
                    Interpretation::new(&theory, (0..5).filter(|&atom| outer & (1 << atom) != 0))
                        .unwrap();
                let mut workspace = EvaluationWorkspace::default();
                let evaluation =
                    workspace.evaluate(&candidate, EvaluationLimits::default(), &cancellation);
                let truth = evaluation.result.unwrap();
                let mut after = Region::all_open(5);
                let (outcome, _) = index
                    .narrow_frozen_known(
                        FrozenSubject::new(&theory, truth.truth()),
                        &mut after,
                        &mut index.knowledge(),
                        &mut scratch,
                        RegionLimits::default(),
                        &cancellation,
                    )
                    .unwrap();
                for inner in 0_u32..32 {
                    let world = Interpretation::new(
                        &theory,
                        (0..5).filter(|&atom| inner & (1 << atom) != 0),
                    )
                    .unwrap();
                    if models_reduct(
                        &theory,
                        &candidate,
                        &world,
                        Limits::default(),
                        &cancellation,
                    )
                    .unwrap()
                    {
                        assert_ne!(outcome, Narrowing::Refuted, "outer={outer}, inner={inner}");
                        for atom in 0..5 {
                            if let Some(value) = after.decision(atom) {
                                assert_eq!(value, inner & (1 << atom) != 0);
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn a_native_upper_bound_refutes_its_smaller_boolean_conjunction() {
    let mut nodes =
        FormulaNodes::new(FormulaParts::new((0..5).map(Node::atom).collect(), Vec::new()).unwrap());
    let prefix = push(&mut nodes, NodeView::And(&[0, 1]));
    let full = push(&mut nodes, NodeView::And(&[prefix, 2]));
    push(&mut nodes, NodeView::Implies(prefix, 3));
    let guards = nodes
        .push_threshold_family(
            &[0, 1, 2, 3],
            &[1, 1, 1, 1],
            &[1, 3],
            usize::MAX,
            usize::MAX,
        )
        .unwrap();
    let falsum = push(&mut nodes, NodeView::False);
    let requirement = push(&mut nodes, NodeView::Implies(guards[1], falsum));
    let observer = push(&mut nodes, NodeView::Or(&[full, 4]));
    let theory = Theory::new(
        5,
        nodes.into_parts(),
        vec![requirement, observer],
        AdmissionLimits::default(),
    )
    .unwrap();
    let index = Narrower::new(&theory);
    let mut region = Region::all_open(5);
    let (outcome, _) = index
        .narrow_known(
            OriginalSubject::new(&theory, None),
            &mut region,
            &mut index.knowledge(),
            &mut NarrowingScratch::default(),
            RegionLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    assert_ne!(outcome, Narrowing::Refuted);
    assert_eq!(region.decision(4), Some(true));
    assert!((0..4).all(|atom| region.is_open(atom)));
}
