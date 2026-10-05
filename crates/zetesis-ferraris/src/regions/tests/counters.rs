//! Counter width preserves complete closure, split preference and work receipts.

use std::mem::size_of;

use crate::{
    AdmissionLimits, EvaluationLimits, EvaluationWorkspace, Interpretation, Narrower, Node, Region,
    RegionLimits, Theory,
};
use zetesis_cpu::Cancellation;

use super::super::Width;
use super::super::counters::Count;
use super::{Counters, Knowledge, Known, shared_occurrences};

fn values<C: Count>(counts: &Counters<C>) -> Vec<usize> {
    (0..counts.len()).map(|index| counts.get(index)).collect()
}

fn widened(counts: &Counters<u32>) -> Counters<usize> {
    let mut wide = Counters::zeros(counts.len());
    for (index, value) in values(counts).into_iter().enumerate() {
        wide.add(index, value);
    }
    wide
}

/// The same closure state at the native width.
pub(super) fn native(knowledge: &Knowledge) -> Knowledge {
    let Width::Compact(known) = &knowledge.width else {
        panic!("the test theories' knowledge starts compact")
    };
    Knowledge {
        width: Width::Native(Known {
            sure: known.sure.clone(),
            never: known.never.clone(),
            atom_sure: known.atom_sure.clone(),
            atom_never: known.atom_never.clone(),
            sure_operands: widened(&known.sure_operands),
            never_operands: widened(&known.never_operands),
            unknown: widened(&known.unknown),
            learned: known.learned.clone(),
            nodes: known.nodes.clone(),
            heads: known.heads.clone(),
            seen: known.seen.clone(),
            seeded: known.seeded,
        }),
    }
}

/// The three counter arrays of either width, as native values.
fn counts(knowledge: &Knowledge) -> [Vec<usize>; 3] {
    match &knowledge.width {
        Width::Compact(k) => [
            values(&k.sure_operands),
            values(&k.never_operands),
            values(&k.unknown),
        ],
        Width::Native(k) => [
            values(&k.sure_operands),
            values(&k.never_operands),
            values(&k.unknown),
        ],
    }
}

fn same_known<A: Count, B: Count>(left: &Known<A>, right: &Known<B>) {
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

fn same_knowledge(left: &Knowledge, right: &Knowledge) {
    match (&left.width, &right.width) {
        (Width::Compact(l), Width::Native(r)) => same_known(l, r),
        (Width::Compact(l), Width::Compact(r)) => same_known(l, r),
        (Width::Native(l), Width::Native(r)) => same_known(l, r),
        (Width::Native(l), Width::Compact(r)) => same_known(l, r),
    }
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
        matches!(fresh.width, Width::Compact(_)),
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
        same_knowledge(&compact, &native);
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
    assert_ne!(counts(&child)[..2], counts(&before)[..2]);
    same_knowledge(&parent, &before);
}

fn counter_bytes(knowledge: &Knowledge) -> u128 {
    match &knowledge.width {
        Width::Compact(k) => [&k.sure_operands, &k.never_operands, &k.unknown]
            .into_iter()
            .map(Counters::allocated_bytes)
            .sum(),
        Width::Native(k) => [&k.sure_operands, &k.never_operands, &k.unknown]
            .into_iter()
            .map(Counters::allocated_bytes)
            .sum(),
    }
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
