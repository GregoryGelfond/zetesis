//! Counter width preserves complete closure, split preference and work receipts.

use std::mem::size_of;

use crate::{
    AdmissionLimits, EvaluationLimits, EvaluationWorkspace, Interpretation, Narrower, Node, Region,
    RegionLimits, Theory,
};
use zetesis_cpu::Cancellation;

use super::{Counters, Knowledge, Known, shared_occurrences};

fn values(counts: &Counters) -> Vec<usize> {
    (0..counts.len()).map(|index| counts.get(index)).collect()
}

pub(super) fn native(knowledge: &Knowledge) -> Knowledge {
    let mut result = knowledge.clone();
    for counts in [
        &mut result.known.sure_operands,
        &mut result.known.never_operands,
        &mut result.known.unknown,
    ] {
        *counts = Counters::Native(values(counts).into_boxed_slice());
    }
    result
}

fn same_known(left: &Known, right: &Known) {
    assert_eq!(left.sure, right.sure);
    assert_eq!(left.never, right.never);
    assert_eq!(left.atom_sure, right.atom_sure);
    assert_eq!(left.atom_never, right.atom_never);
    assert_eq!(values(&left.sure_operands), values(&right.sure_operands));
    assert_eq!(values(&left.never_operands), values(&right.never_operands));
    assert_eq!(values(&left.unknown), values(&right.unknown));
    assert_eq!(left.learned, right.learned);
    assert_eq!(left.nodes, right.nodes);
    assert_eq!(left.heads, right.heads);
    assert_eq!(left.seen, right.seen);
    assert_eq!(left.seeded, right.seeded);
}

fn region(mut code: usize) -> Region {
    let mut region = Region::all_open(4);
    for atom in 0..4 {
        match code % 3 {
            1 => assert!(region.cut(atom)),
            2 => assert!(region.hold(atom)),
            _ => {}
        }
        code /= 3;
    }
    region
}

fn compare_closures(theory: &Theory, frozen: Option<&[bool]>) {
    let narrower = Narrower::new(theory);
    let fresh = narrower.knowledge();
    assert_eq!(
        matches!(fresh.known.unknown, Counters::Compact(_)),
        size_of::<u32>() < size_of::<usize>()
    );
    let cancellation = Cancellation::default();
    let extracted =
        super::super::producers(theory, RegionLimits::default(), &cancellation).unwrap();
    for code in 0..3usize.pow(4) {
        let mut compact_region = region(code);
        let mut native_region = compact_region.clone();
        let mut compact = fresh.clone();
        let mut native = native(&fresh);
        let close = |region: &mut Region, knowledge: &mut Knowledge| {
            if let Some(truth) = frozen {
                narrower.narrow_frozen_known(
                    theory,
                    truth,
                    region,
                    knowledge,
                    RegionLimits::default(),
                    &cancellation,
                )
            } else {
                narrower.narrow_known(
                    theory,
                    extracted.producers.as_ref(),
                    region,
                    knowledge,
                    RegionLimits::default(),
                    &cancellation,
                )
            }
        };
        assert_eq!(
            close(&mut compact_region, &mut compact),
            close(&mut native_region, &mut native),
            "region {code}"
        );
        assert_eq!(compact_region, native_region, "region {code}");
        same_known(&compact.known, &native.known);
    }
}

#[test]
fn counter_width_preserves_original_closure() {
    compare_closures(&shared_occurrences(), None);
}

#[test]
fn counter_width_preserves_frozen_closure() {
    let theory = shared_occurrences();
    let mut workspace = EvaluationWorkspace::default();
    let cancellation = Cancellation::default();
    for mask in 0..16 {
        let candidate =
            Interpretation::new(&theory, (0..4).filter(|atom| mask & (1 << atom) != 0)).unwrap();
        let attempt = workspace.evaluate(&candidate, EvaluationLimits::default(), &cancellation);
        let evaluation = attempt.result.unwrap();
        compare_closures(&theory, Some(evaluation.truth()));
    }
}

#[test]
fn child_propagation_keeps_parent_knowledge_unchanged() {
    let theory = shared_occurrences();
    let narrower = Narrower::new(&theory);
    let mut parent = narrower.knowledge();
    let mut parent_region = Region::all_open(4);
    let cancellation = Cancellation::default();
    narrower
        .narrow_known(
            &theory,
            None,
            &mut parent_region,
            &mut parent,
            RegionLimits::default(),
            &cancellation,
        )
        .unwrap();
    let before = native(&parent);
    let mut child = parent.clone();
    let atom = parent_region.split_atom().unwrap();
    let (mut child_region, _) = parent_region.split(atom);
    narrower
        .narrow_known(
            &theory,
            None,
            &mut child_region,
            &mut child,
            RegionLimits::default(),
            &cancellation,
        )
        .unwrap();
    assert_ne!(
        (
            values(&child.known.sure_operands),
            values(&child.known.never_operands)
        ),
        (
            values(&before.known.sure_operands),
            values(&before.known.never_operands)
        )
    );
    same_known(&parent.known, &before.known);
}

fn counter_bytes(knowledge: &Knowledge) -> u128 {
    [
        &knowledge.known.sure_operands,
        &knowledge.known.never_operands,
        &knowledge.known.unknown,
    ]
    .into_iter()
    .map(Counters::allocated_bytes)
    .sum()
}

#[test]
fn compact_counters_reduce_clone_payload() {
    let atoms = 4096;
    let mut nodes: Vec<_> = (0..atoms).map(Node::Atom).collect();
    // Disjoint four-operand chains exercise both per-chain arrays as well as
    // atom occurrence counts. No solving or timing claim relies on this graph.
    for first in (0..atoms).step_by(4) {
        nodes.push(Node::Or(first, first + 1));
        nodes.push(Node::Or(nodes.len() - 1, first + 2));
        nodes.push(Node::Or(nodes.len() - 1, first + 3));
    }
    let theory = Theory::new(atoms, nodes, vec![], AdmissionLimits::default()).unwrap();
    let narrower = Narrower::new(&theory);
    let compact = narrower.knowledge();
    let native = native(&compact);
    let values = atoms + 2 * narrower.chains.len();
    let compact_bytes = counter_bytes(&compact);
    let native_bytes = counter_bytes(&native);
    let width = size_of::<u32>().min(size_of::<usize>()) as u128;
    assert_eq!(compact_bytes, values as u128 * width);
    assert_eq!(native_bytes, values as u128 * size_of::<usize>() as u128);
    assert_eq!(
        native.retained_bytes() - compact.retained_bytes(),
        native_bytes - compact_bytes
    );
    assert_eq!(counter_bytes(&compact.clone()), compact_bytes);
    println!(
        "knowledge atoms={atoms} chains={} count_values={values} native_counter_bytes={native_bytes} \
         selected_counter_bytes={compact_bytes} saved_counter_bytes={} native_knowledge_bytes={} \
         selected_knowledge_bytes={}",
        narrower.chains.len(),
        native_bytes - compact_bytes,
        native.retained_bytes(),
        compact.retained_bytes()
    );
}
