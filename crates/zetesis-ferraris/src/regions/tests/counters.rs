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

fn copied_counts<A: Count, B: Count>(counts: &Counters<A>) -> Counters<B> {
    let mut copied = Counters::zeros(counts.len());
    for (index, value) in values(counts).into_iter().enumerate() {
        copied.add(index, value);
    }
    copied
}

fn copied_known<A: Count, B: Count>(known: &Known<A>) -> Known<B> {
    Known {
        masks: known.masks.clone(),
        sure_operands: copied_counts(&known.sure_operands),
        never_operands: copied_counts(&known.never_operands),
        unknown: copied_counts(&known.unknown),
        seeded: known.seeded,
    }
}

fn copied_width<C: Count>(knowledge: &Knowledge) -> Known<C> {
    match &knowledge.width {
        Width::Compact16(known) => copied_known(known),
        Width::Compact32(known) => copied_known(known),
        Width::Native(known) => copied_known(known),
    }
}

/// The same closure state at the native width.
pub(super) fn native(knowledge: &Knowledge) -> Knowledge {
    Knowledge {
        width: Width::Native(copied_width(knowledge)),
    }
}

/// The same bounded test state at the intermediate width.
pub(super) fn compact32(knowledge: &Knowledge) -> Knowledge {
    Knowledge {
        width: Width::Compact32(copied_width(knowledge)),
    }
}

/// The three counter arrays of any width, as native values.
fn counts(knowledge: &Knowledge) -> [Vec<usize>; 3] {
    fn arrays<C: Count>(known: &Known<C>) -> [Vec<usize>; 3] {
        [
            values(&known.sure_operands),
            values(&known.never_operands),
            values(&known.unknown),
        ]
    }
    match &knowledge.width {
        Width::Compact16(k) => arrays(k),
        Width::Compact32(k) => arrays(k),
        Width::Native(k) => arrays(k),
    }
}

fn same_known<A: Count, B: Count>(left: &Known<A>, right: &Known<B>) {
    assert_eq!(left.masks.slices(), right.masks.slices());
    assert_eq!(values(&left.sure_operands), values(&right.sure_operands));
    assert_eq!(values(&left.never_operands), values(&right.never_operands));
    assert_eq!(values(&left.unknown), values(&right.unknown));
    assert_eq!(left.seeded, right.seeded);
}

pub(super) fn same_knowledge(left: &Knowledge, right: &Knowledge) {
    fn compare<C: Count>(left: &Known<C>, right: &Knowledge) {
        match &right.width {
            Width::Compact16(r) => same_known(left, r),
            Width::Compact32(r) => same_known(left, r),
            Width::Native(r) => same_known(left, r),
        }
    }
    match &left.width {
        Width::Compact16(l) => compare(l, right),
        Width::Compact32(l) => compare(l, right),
        Width::Native(l) => compare(l, right),
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

fn compare_closures(theory: &Theory, frozen: Option<&[bool]>, max_work: u64) {
    let narrower = Narrower::new(theory);
    let fresh = narrower.knowledge();
    assert_eq!(
        matches!(fresh.width, Width::Compact16(_)),
        size_of::<u16>() < size_of::<usize>()
    );
    let cancellation = Cancellation::default();
    let extracted =
        super::super::producers(theory, RegionLimits::default(), &cancellation).unwrap();
    for code in 0..3usize.pow(4) {
        let mut compact_region = region(code);
        let mut native_region = compact_region.clone();
        let mut compact = fresh.clone();
        let mut intermediate = compact32(&fresh);
        let mut intermediate_region = compact_region.clone();
        let mut native = native(&fresh);
        let close = |region: &mut Region, knowledge: &mut Knowledge| {
            if let Some(truth) = frozen {
                narrower.narrow_frozen_known(
                    crate::FrozenSubject::new(theory, truth),
                    region,
                    knowledge,
                    &mut crate::NarrowingScratch::default(),
                    RegionLimits { max_work },
                    &cancellation,
                )
            } else {
                narrower.narrow_known(
                    crate::OriginalSubject::new(theory, extracted.producers.as_ref()),
                    region,
                    knowledge,
                    &mut crate::NarrowingScratch::default(),
                    RegionLimits { max_work },
                    &cancellation,
                )
            }
        };
        let expected = close(&mut native_region, &mut native);
        assert_eq!(
            close(&mut compact_region, &mut compact),
            expected,
            "region {code}"
        );
        assert_eq!(
            close(&mut intermediate_region, &mut intermediate),
            expected,
            "region {code}"
        );
        assert_eq!(compact_region, native_region, "region {code}");
        assert_eq!(intermediate_region, native_region, "region {code}");
        same_knowledge(&compact, &native);
        same_knowledge(&intermediate, &native);
    }
}

#[test]
fn counter_width_preserves_original_closure() {
    for max_work in [0, 1, 5, u64::MAX] {
        compare_closures(&shared_occurrences(), None, max_work);
    }
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
        for max_work in [0, 1, 5, u64::MAX] {
            compare_closures(&theory, Some(evaluation.truth()), max_work);
        }
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
            crate::OriginalSubject::new(&theory, None),
            &mut parent_region,
            &mut parent,
            &mut crate::NarrowingScratch::default(),
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
            crate::OriginalSubject::new(&theory, None),
            &mut child_region,
            &mut child,
            &mut crate::NarrowingScratch::default(),
            RegionLimits::default(),
            &cancellation,
        )
        .unwrap();
    assert_ne!(counts(&child)[..2], counts(&before)[..2]);
    same_knowledge(&parent, &before);
}

fn counter_bytes(knowledge: &Knowledge) -> u128 {
    fn bytes<C: Count>(known: &Known<C>) -> u128 {
        [&known.sure_operands, &known.never_operands, &known.unknown]
            .into_iter()
            .map(Counters::allocated_bytes)
            .sum()
    }
    match &knowledge.width {
        Width::Compact16(k) => bytes(k),
        Width::Compact32(k) => bytes(k),
        Width::Native(k) => bytes(k),
    }
}

#[test]
fn compact_counters_reduce_clone_payload() {
    let atoms = 4096;
    let mut nodes: Vec<_> = (0..atoms).map(Node::atom).collect();
    // Disjoint four-operand chains exercise both per-chain arrays as well as
    // atom occurrence counts. No solving or timing claim relies on this graph.
    for first in (0..atoms).step_by(4) {
        nodes.push(Node::or_pair([first, first + 1]));
        nodes.push(Node::or_pair([nodes.len() - 1, first + 2]));
        nodes.push(Node::or_pair([nodes.len() - 1, first + 3]));
    }
    let theory = Theory::new(
        atoms,
        crate::FormulaParts::new(nodes, vec![]).unwrap(),
        vec![],
        AdmissionLimits::default(),
    )
    .unwrap();
    let narrower = Narrower::new(&theory);
    let compact = narrower.knowledge();
    let native = native(&compact);
    let values = atoms + 2 * narrower.chains.len();
    let compact_bytes = counter_bytes(&compact);
    let native_bytes = counter_bytes(&native);
    let width = size_of::<u16>().min(size_of::<usize>()) as u128;
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

#[test]
fn cell_bound_stays_small_when_total_incidence_is_large() {
    let theory = super::implication_chain(usize::from(u16::MAX) / 2 + 2);
    let narrower = Narrower::new(&theory);
    let total: usize = narrower.parents.iter().map(<[usize]>::len).sum();
    assert!(total > usize::from(u16::MAX));
    assert_eq!(narrower.counter_bound, 2);
    let knowledge = narrower.knowledge();
    assert!(matches!(knowledge.width, Width::Compact16(_)));
    let expected = counts(&native(&knowledge));
    assert_eq!(counts(&knowledge), expected);
}

fn one_chain(atoms: usize) -> Theory {
    let mut nodes: Vec<_> = (0..atoms).map(Node::atom).collect();
    nodes.push(Node::and_span(crate::OperandSpan {
        start: 0,
        length: atoms,
    }));
    Theory::new(
        atoms,
        crate::FormulaParts::new(nodes, (0..atoms).collect()).unwrap(),
        vec![atoms],
        AdmissionLimits::default(),
    )
    .unwrap()
}

fn assert_bound_width(knowledge: &Knowledge, bound: usize) {
    if u16::try_from(bound).is_ok() {
        assert!(matches!(knowledge.width, Width::Compact16(_)));
    } else if size_of::<u32>() < size_of::<usize>() {
        assert!(matches!(knowledge.width, Width::Compact32(_)));
    } else {
        assert!(matches!(knowledge.width, Width::Native(_)));
    }
}

#[test]
fn chain_counts_cross_the_smallest_width_without_truncation() {
    for atoms in [usize::from(u16::MAX), usize::from(u16::MAX) + 1] {
        let theory = one_chain(atoms);
        let narrower = Narrower::new(&theory);
        let mut knowledge = narrower.knowledge();
        assert_eq!(narrower.counter_bound, atoms);
        assert_bound_width(&knowledge, atoms);
        let mut region = Region::all_open(atoms);
        narrower
            .narrow_known(
                crate::OriginalSubject::new(&theory, None),
                &mut region,
                &mut knowledge,
                &mut crate::NarrowingScratch::default(),
                RegionLimits::default(),
                &Cancellation::default(),
            )
            .unwrap();
        assert_eq!(counts(&knowledge)[0], [atoms]);
        assert!((0..atoms).all(|atom| region.is_held(atom)));
    }
}

#[test]
fn atom_counts_include_every_node_occurrence() {
    for occurrences in [usize::from(u16::MAX), usize::from(u16::MAX) + 1] {
        let mut nodes = vec![Node::atom(0); occurrences];
        // Each distinct atom node occurs twice in the same implication. The
        // existing incidence map coalesces that repeated node, but must not
        // coalesce distinct nodes merely because they carry the same atom.
        nodes.extend((0..occurrences).map(|node| Node::implies(node, node)));
        let theory = Theory::new(
            1,
            crate::FormulaParts::new(nodes, vec![]).unwrap(),
            vec![],
            AdmissionLimits::default(),
        )
        .unwrap();
        let narrower = Narrower::new(&theory);
        let mut knowledge = narrower.knowledge();
        assert!(narrower.chains.is_empty());
        assert_eq!(narrower.counter_bound, occurrences);
        assert_bound_width(&knowledge, occurrences);
        assert_eq!(counts(&knowledge)[2], [occurrences]);
        let mut region = Region::all_open(1);
        assert!(region.hold(0));
        narrower
            .narrow_known(
                crate::OriginalSubject::new(&theory, None),
                &mut region,
                &mut knowledge,
                &mut crate::NarrowingScratch::default(),
                RegionLimits::default(),
                &Cancellation::default(),
            )
            .unwrap();
        assert_eq!(counts(&knowledge)[2], [0]);
    }
}
