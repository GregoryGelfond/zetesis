use std::mem::size_of;

use super::{Knowledge, Known};

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
