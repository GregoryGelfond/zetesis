//! Refutation needs only the prefix that establishes a contradiction.

use zetesis_cpu::{Cancellation, Stop};

use crate::{
    AdmissionLimits, FormulaParts, Narrower, NarrowingQuota, Node, OperandSpan, OriginalSubject,
    Region, RegionLimits, Theory,
};

use super::super::{Closure, NarrowingScratch, Step, Subject, Work, bit, producers};
use super::compact_mut;

/// Single permits expose the exact read at which a typed refusal is observed.
struct Quota {
    remaining: u64,
    failure: Stop,
}

impl NarrowingQuota for Quota {
    fn reserve(&mut self, _: u64) -> Result<u64, Stop> {
        if self.remaining == 0 {
            return Err(self.failure);
        }
        self.remaining -= 1;
        Ok(1)
    }

    fn refund(&mut self, unspent: u64) {
        self.remaining += unspent;
    }
}

fn chain(disjunction: bool, atoms: usize) -> Theory {
    let operands = OperandSpan {
        start: 0,
        length: atoms,
    };
    let mut nodes: Vec<_> = (0..atoms).map(Node::atom).collect();
    nodes.push(if disjunction {
        Node::or_span(operands)
    } else {
        Node::and_span(operands)
    });
    Theory::new(
        atoms,
        FormulaParts::new(nodes, (0..atoms).collect()).unwrap(),
        vec![atoms],
        AdmissionLimits::default(),
    )
    .unwrap()
}

#[test]
fn chain_refutation_visits_only_its_prefix() {
    for disjunction in [false, true] {
        let theory = chain(disjunction, 9);
        let index = Narrower::new(&theory);
        for conflict in [0, 4, 8] {
            let needed = u64::try_from(conflict + 1).unwrap();
            for failure in [Stop::WorkLimit, Stop::Cancelled, Stop::Deadline] {
                for limit in [needed - 1, needed] {
                    let mut knowledge = index.knowledge();
                    let known = compact_mut(&mut knowledge);
                    // These bits are known before their events update the
                    // counters. Teaching the root conflicts at this operand.
                    if disjunction {
                        known.never[0] |= 1 << 9;
                        known.sure[0] |= 1 << conflict;
                    } else {
                        known.sure[0] |= 1 << 9;
                        known.never[0] |= 1 << conflict;
                    }
                    let mut scratch = NarrowingScratch::default();
                    scratch.prepare(9);
                    let mut quota = Quota {
                        remaining: limit,
                        failure,
                    };
                    let mut work = Work::reserved(&mut quota);
                    let result = Closure {
                        known: compact_mut(&mut knowledge),
                        lists: &mut scratch,
                    }
                    .teach_chain(&index, 0, &mut work);
                    work.settle();
                    assert_eq!(work.spent, limit);
                    if limit < needed {
                        assert!(result == Err(failure));
                    } else {
                        assert!(result == Ok(Step::Contradiction));
                    }
                    // No event from the unread tail may survive the shortcut.
                    assert_eq!(
                        scratch.nodes,
                        (0..conflict)
                            .map(|operand| (operand, !disjunction))
                            .collect::<Vec<_>>()
                    );
                }
            }
        }
    }
}

#[test]
fn parent_refutation_visits_only_its_prefix() {
    let mut nodes: Vec<_> = (0..9).map(Node::atom).collect();
    nodes.extend((1..9).map(|atom| Node::implies(0, atom)));
    let theory = Theory::new(
        9,
        FormulaParts::new(nodes, vec![]).unwrap(),
        (9..17).collect(),
        AdmissionLimits::default(),
    )
    .unwrap();
    let index = Narrower::new(&theory);
    let subject = Subject::from(OriginalSubject::new(&theory, None));
    for failure in [Stop::WorkLimit, Stop::Cancelled, Stop::Deadline] {
        for limit in 0..=2 {
            let mut knowledge = index.knowledge();
            let known = compact_mut(&mut knowledge);
            known.sure[0] |= 1;
            known.never[0] |= 1 << 1;
            for parent in 9..17 {
                known.sure[0] |= 1 << parent;
            }
            let mut scratch = NarrowingScratch::default();
            scratch.prepare(9);
            let mut quota = Quota {
                remaining: limit,
                failure,
            };
            let mut work = Work::reserved(&mut quota);
            let result = Closure {
                known: compact_mut(&mut knowledge),
                lists: &mut scratch,
            }
            .revisit(subject, &index, 0, true, &mut work);
            work.settle();
            assert_eq!(work.spent, limit);
            if limit < 2 {
                assert!(result == Err(failure));
            } else {
                assert!(result == Ok(Step::Contradiction));
            }
            // The first parent conflicts; none of its siblings can force its
            // consequent after that refutation, even with a fresh work permit.
            let known = compact_mut(&mut knowledge);
            assert!((2..9).all(|atom| !bit(known.sure, atom)));
            assert!(scratch.nodes.is_empty());
        }
    }
}

#[test]
fn downward_refutation_omits_support_wakeups() {
    let theory = Theory::new(
        3,
        FormulaParts::new(
            vec![
                Node::atom(0),
                Node::atom(1),
                Node::atom(2),
                Node::or_pair([0, 1]),
                Node::implies(3, 2),
            ],
            vec![],
        )
        .unwrap(),
        vec![4],
        AdmissionLimits::default(),
    )
    .unwrap();
    let index = Narrower::new(&theory);
    let producers = producers(&theory, RegionLimits::default(), &Cancellation::default())
        .unwrap()
        .producers
        .unwrap();
    let mut knowledge = index.knowledge();
    let known = compact_mut(&mut knowledge);
    known.sure[0] |= 1;
    known.never[0] |= 1 << 3;
    let mut scratch = NarrowingScratch::default();
    scratch.prepare(3);
    let mut work = Work::new(u64::MAX);
    let result = Closure {
        known: compact_mut(&mut knowledge),
        lists: &mut scratch,
    }
    .revisit(
        Subject::from(OriginalSubject::new(&theory, Some(&producers))),
        &index,
        3,
        false,
        &mut work,
    );
    assert!(result == Ok(Step::Contradiction));
    assert_eq!(work.spent, 2);
    assert!(scratch.heads.is_empty());
    assert!(scratch.pending.iter().all(|word| *word == 0));
}

#[test]
fn early_refutation_leaves_no_scratch_authority() {
    let theory = chain(false, 9);
    let index = Narrower::new(&theory);
    let mut knowledge = index.knowledge();
    let known = compact_mut(&mut knowledge);
    known.sure[0] |= 1 << 9;
    known.never[0] |= 1 << 4;
    let mut scratch = NarrowingScratch::default();
    scratch.prepare(9);
    let result = Closure {
        known: compact_mut(&mut knowledge),
        lists: &mut scratch,
    }
    .teach_chain(&index, 0, &mut Work::new(u64::MAX));
    assert!(result == Ok(Step::Contradiction));
    assert_eq!(scratch.nodes.len(), 4);
    let close = |scratch: &mut NarrowingScratch| {
        let mut region = Region::all_open(9);
        let result = index
            .narrow_known(
                OriginalSubject::new(&theory, None),
                &mut region,
                &mut index.knowledge(),
                scratch,
                RegionLimits::default(),
                &Cancellation::default(),
            )
            .unwrap();
        (result, region)
    };
    let reused = close(&mut scratch);
    assert_eq!(reused, close(&mut NarrowingScratch::default()));
    assert_eq!(reused.1.open().count(), 0);
}

#[test]
fn atom_refutation_stops_before_later_occurrences() {
    let theory = Theory::new(
        1,
        FormulaParts::new(vec![Node::atom(0); 3], vec![]).unwrap(),
        vec![2],
        AdmissionLimits::default(),
    )
    .unwrap();
    let index = Narrower::new(&theory);
    let producers = producers(&theory, RegionLimits::default(), &Cancellation::default())
        .unwrap()
        .producers
        .unwrap();
    let mut knowledge = index.knowledge();
    compact_mut(&mut knowledge).never[0] |= 1 << 1;
    let mut scratch = NarrowingScratch::default();
    scratch.prepare(1);
    let result = Closure {
        known: compact_mut(&mut knowledge),
        lists: &mut scratch,
    }
    .atom(&index, Some(&producers), None, 0, true);
    assert!(result == Step::Contradiction);
    assert_eq!(scratch.nodes, [(0, true)]);
    assert!(scratch.heads.is_empty());
    assert!(!bit(compact_mut(&mut knowledge).sure, 2));
}

#[test]
fn root_refutation_stops_seeding() {
    let theory = Theory::new(
        1,
        FormulaParts::new(vec![Node::falsum(), Node::atom(0)], vec![]).unwrap(),
        vec![0, 1],
        AdmissionLimits::default(),
    )
    .unwrap();
    let index = Narrower::new(&theory);
    let producers = producers(&theory, RegionLimits::default(), &Cancellation::default())
        .unwrap()
        .producers
        .unwrap();
    let mut knowledge = index.knowledge();
    let mut scratch = NarrowingScratch::default();
    let (result, receipt) = index
        .narrow_known(
            OriginalSubject::new(&theory, Some(&producers)),
            &mut Region::all_open(1),
            &mut knowledge,
            &mut scratch,
            RegionLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    assert_eq!(result, crate::Narrowing::Refuted);
    assert_eq!(receipt.work, 0);
    assert_eq!(scratch.nodes, [(0, false)]);
    assert!(scratch.heads.is_empty());
    assert!(!bit(compact_mut(&mut knowledge).sure, 1));
}

#[test]
fn decision_refutation_restores_unadvanced_seen_storage() {
    let theory = Theory::new(
        2,
        FormulaParts::new(vec![Node::atom(0), Node::atom(1)], vec![]).unwrap(),
        vec![0],
        AdmissionLimits::default(),
    )
    .unwrap();
    let index = Narrower::new(&theory);
    let mut knowledge = index.knowledge();
    let mut region = Region::all_open(2);
    assert!(region.cut(0));
    assert!(region.hold(1));
    let (result, _) = index
        .narrow_known(
            OriginalSubject::new(&theory, None),
            &mut region,
            &mut knowledge,
            &mut NarrowingScratch::default(),
            RegionLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    assert_eq!(result, crate::Narrowing::Refuted);
    let known = compact_mut(&mut knowledge);
    assert_eq!(known.seen, &[0]);
    assert!(!bit(known.atom_sure, 1));
}
