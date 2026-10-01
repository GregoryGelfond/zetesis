use std::mem::size_of;

use super::{Knowledge, Known};

fn shared_occurrences() -> crate::Theory {
    use crate::Node::{Atom, Implies, Or};

    crate::Theory::new(
        4,
        vec![
            Atom(0),
            Atom(0),
            Atom(1),
            Atom(2),
            Or(0, 1),
            Or(4, 2),
            Implies(0, 0),
            Implies(1, 3),
            Implies(1, 3),
        ],
        vec![5, 6, 7, 8],
        crate::AdmissionLimits::default(),
    )
    .unwrap()
}

#[test]
fn compact_narrower_keeps_shared_and_aliased_occurrences() {
    let theory = shared_occurrences();
    let index = super::Narrower::try_new(&theory).unwrap();
    // Chain 5 absorbs node 4. Its distinct leaves 0 and 1 both name atom 0,
    // while implication 6 reads node 0 on both sides and contributes once.
    assert_eq!(
        index.parents.iter().collect::<Vec<_>>(),
        vec![
            &[5, 6][..],
            &[5, 7, 8][..],
            &[5][..],
            &[7, 8][..],
            &[][..],
            &[][..],
            &[][..],
            &[][..],
            &[][..],
        ]
    );
    assert_eq!(
        index.atom_nodes.iter().collect::<Vec<_>>(),
        vec![&[0, 1][..], &[2][..], &[3][..], &[][..]]
    );
    assert_eq!(
        index.atom_operands.iter().collect::<Vec<_>>(),
        vec![
            &[][..],
            &[][..],
            &[][..],
            &[][..],
            &[][..],
            &[0, 0, 1][..],
            &[0][..],
            &[0, 2][..],
            &[0, 2][..],
        ]
    );
    assert_eq!(index.knowledge().known.unknown, [5, 1, 2, 0]);
    assert_eq!(index.work(), 9);
}

#[test]
fn compact_support_keeps_original_producer_occurrences() {
    let theory = shared_occurrences();
    let extracted = super::producers(
        &theory,
        super::RegionLimits::default(),
        &zetesis_cpu::Cancellation::default(),
    )
    .unwrap();
    let producers = extracted.producers.unwrap();
    // The head-set extraction coalesces atom 0 in the first disjunction as
    // before. The two original rules 7 and 8 remain separate producers.
    assert_eq!(
        producers.by_head.iter().collect::<Vec<_>>(),
        vec![&[0, 1][..], &[0][..], &[2, 3][..], &[][..]]
    );
    assert_eq!(
        producers.by_body.iter().collect::<Vec<_>>(),
        vec![
            &[1][..],
            &[2, 3][..],
            &[][..],
            &[][..],
            &[][..],
            &[][..],
            &[][..],
            &[][..],
            &[][..],
        ]
    );
}

#[test]
fn compact_adjacency_uses_less_retained_storage() {
    use crate::Node;

    // Sparse immutable incidence is the representation's intended population:
    // a ring of ordinary implications gives each atom two parents, one atom
    // node and one supporting producer, with many empty node-indexed rows.
    let atoms = 4096;
    let mut nodes: Vec<_> = (0..atoms).map(Node::Atom).collect();
    nodes.extend((0..atoms).map(|atom| Node::Implies(atom, (atom + 1) % atoms)));
    let theory = crate::Theory::new(
        atoms,
        nodes,
        (atoms..2 * atoms).collect(),
        crate::AdmissionLimits::default(),
    )
    .unwrap();
    let index = super::Narrower::try_new(&theory).unwrap();
    let extracted = super::producers(
        &theory,
        super::RegionLimits::default(),
        &zetesis_cpu::Cancellation::default(),
    )
    .unwrap();
    let producers = extracted.producers.unwrap();
    let maps = [
        &index.parents,
        &index.atom_nodes,
        &index.atom_operands,
        &producers.by_head,
        &producers.by_body,
    ];
    let mut previous = 0_u128;
    let mut compact = 0_u128;
    let mut rows = 0;
    let mut entries = 0;
    for map in maps {
        // Recreate the previous representation's allocations with the same
        // rows, entry order and push growth policy. Measure actual capacities,
        // not only logical entry lengths or assumed allocator sizes.
        let mut nested = vec![Vec::new(); map.len()];
        for (row, values) in map.iter().enumerate() {
            for &value in values {
                nested[row].push(value);
            }
            assert_eq!(nested[row], values);
            entries += values.len();
        }
        rows += nested.len();
        previous += size_of::<Vec<Vec<usize>>>() as u128
            + nested.capacity() as u128 * size_of::<Vec<usize>>() as u128
            + nested
                .iter()
                .map(|row| row.capacity() as u128)
                .sum::<u128>()
                * size_of::<usize>() as u128;
        compact += map.retained_bytes();
    }
    assert!(compact < previous);
    // A receipt for the retained maps only: chain/rule/theory/Knowledge storage,
    // construction scratch, allocator bookkeeping and RSS are excluded.
    eprintln!(
        "adjacency rows={rows} entries={entries} nested_bytes={previous} \
         compact_bytes={compact} saved_bytes={}",
        previous - compact
    );
}

#[test]
fn retained_bytes_counts_the_seen_mask() {
    // 130 atoms give a three-word seen mask (ceil(130/64) = 3).
    let knowledge = Knowledge {
        known: Known::empty(5, 2, vec![0; 130]),
    };
    let k = &knowledge.known;
    let expected = size_of::<Knowledge>() as u128
        + (k.sure.len() + k.never.len() + k.atom_sure.len() + k.atom_never.len()) as u128
            * size_of::<u64>() as u128
        + (k.sure_operands.capacity()
            + k.never_operands.capacity()
            + k.unknown.capacity()
            + k.learned.capacity()
            + k.heads.capacity()) as u128
            * size_of::<usize>() as u128
        + k.nodes.capacity() as u128 * size_of::<(usize, bool)>() as u128
        + k.seen.len() as u128 * size_of::<u64>() as u128;
    assert_eq!(knowledge.retained_bytes(), expected);
    assert!(k.seen.len() >= 3, "the seen mask is a real allocation here");
}

#[test]
fn empty_worklists_retain_their_allocated_bytes() {
    let mut knowledge = Knowledge {
        known: Known::empty(5, 2, vec![0; 3]),
    };
    let original = knowledge.retained_bytes();
    knowledge.known.learned.reserve_exact(7);
    knowledge.known.nodes.reserve_exact(11);
    knowledge.known.heads.reserve_exact(13);
    let worklists = knowledge.known.learned.capacity() as u128 * size_of::<usize>() as u128
        + knowledge.known.nodes.capacity() as u128 * size_of::<(usize, bool)>() as u128
        + knowledge.known.heads.capacity() as u128 * size_of::<usize>() as u128;
    assert_eq!(knowledge.retained_bytes(), original + worklists);
    knowledge.known.learned.push(1);
    knowledge.known.nodes.push((1, true));
    knowledge.known.heads.push(1);
    knowledge.known.learned.clear();
    knowledge.known.nodes.clear();
    knowledge.known.heads.clear();
    assert_eq!(knowledge.retained_bytes(), original + worklists);
}

fn propagated_decisions_are_seen(frozen: bool) {
    use crate::{
        AdmissionLimits, EvaluationLimits, EvaluationWorkspace, Interpretation, Narrower,
        Narrowing, Node, Region, RegionLimits, Theory,
    };
    use zetesis_cpu::Cancellation;

    let mut nodes: Vec<_> = (0..130).map(Node::Atom).collect();
    nodes.extend([Node::Or(63, 64), Node::Implies(64, 129)]);
    let theory = Theory::new(130, nodes, vec![130, 131], AdmissionLimits::default()).unwrap();
    let candidate_atoms = [0, 63, 64, 129];
    let candidate = Interpretation::new(&theory, candidate_atoms).unwrap();
    let cancellation = Cancellation::default();
    let mut workspace = EvaluationWorkspace::default();
    let evaluation = workspace
        .evaluate(&candidate, EvaluationLimits::default(), &cancellation)
        .result
        .unwrap();
    assert!(evaluation.is_model());
    let narrower = Narrower::new(&theory);
    let mut knowledge = narrower.knowledge();
    let mut region = Region::all_open(130);
    if frozen {
        for atom in (0..130).filter(|atom| !candidate_atoms.contains(atom)) {
            assert!(region.cut(atom));
        }
    }
    assert!(region.hold(0));
    assert!(region.cut(128));
    let close = |region: &mut Region, knowledge: &mut Knowledge| {
        if frozen {
            narrower.narrow_frozen_known(
                &theory,
                evaluation.truth(),
                region,
                knowledge,
                RegionLimits::default(),
                &cancellation,
            )
        } else {
            narrower.narrow_known(
                &theory,
                None,
                region,
                knowledge,
                RegionLimits::default(),
                &cancellation,
            )
        }
        .unwrap()
        .0
    };
    assert_eq!(
        close(&mut region, &mut knowledge),
        Narrowing::Fixed { changed: false }
    );
    let (mut child, _) = region.split(63);
    assert_eq!(
        close(&mut child, &mut knowledge),
        Narrowing::Fixed { changed: true }
    );
    // The final snapshot must include newly propagated 64 and 129, not just
    // the decisions supplied before this closure. Repeat propagation counts
    // alone cannot detect a stale snapshot because learning is idempotent.
    let expected = if frozen {
        [u64::MAX, u64::MAX, 3]
    } else {
        [1 | (1 << 63), 1, 3]
    };
    assert_eq!(*knowledge.known.seen, expected);
}

#[test]
fn original_completed_knowledge_records_propagated_decisions() {
    propagated_decisions_are_seen(false);
}

#[test]
fn frozen_completed_knowledge_records_propagated_decisions() {
    propagated_decisions_are_seen(true);
}
