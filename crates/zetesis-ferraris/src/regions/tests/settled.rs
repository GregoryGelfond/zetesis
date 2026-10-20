//! A processed absorbing witness retires only its chain's bookkeeping.

use zetesis_cpu::{Cancellation, Stop};

use crate::test_support::SinglePermits;
use crate::{
    AdmissionLimits, FormulaParts, FrozenSubject, Knowledge, Narrower, Narrowing, NarrowingScratch,
    Node, OperandSpan, OriginalSubject, Region, RegionLimits, Theory,
};

use super::super::{Closure, Known, Step, Subject, Width, Work, bit, counters::Count};
use super::{compact_mut, counters};

fn theory(disjunction: bool, other_parent: bool) -> Theory {
    let mut nodes: Vec<_> = (0..4).map(Node::atom).collect();
    let operands = OperandSpan {
        start: 0,
        length: 4,
    };
    nodes.push(if disjunction {
        Node::or_span(operands)
    } else {
        Node::and_span(operands)
    });
    if other_parent {
        nodes.push(Node::implies(0, 3));
    }
    Theory::new(
        4,
        FormulaParts::new(nodes, (0..4).collect()).unwrap(),
        vec![],
        AdmissionLimits::default(),
    )
    .unwrap()
}

/// Inspect each counter width without converting the tested state.
fn evidence(knowledge: &Knowledge) -> (bool, usize, Option<bool>, Vec<usize>) {
    fn inspect<C: Count>(known: &Known<C>) -> (bool, usize, Option<bool>, Vec<usize>) {
        let masks = known.masks.slices();
        let root = if bit(masks[0], 4) {
            Some(true)
        } else if bit(masks[1], 4) {
            Some(false)
        } else {
            None
        };
        (
            bit(masks[5], 0),
            known.neutral_operands.get(0),
            root,
            (0..4).map(|atom| known.unknown.get(atom)).collect(),
        )
    }
    match &knowledge.width {
        Width::Compact16(known) => inspect(known),
        Width::Compact32(known) => inspect(known),
        Width::Native(known) => inspect(known),
    }
}

#[test]
fn witness_retirement_preserves_every_operand_order() {
    for disjunction in [false, true] {
        let theory = theory(disjunction, false);
        let index = Narrower::new(&theory);
        assert_eq!(index.chains.len(), 1);
        assert_eq!(index.chains[0].ordinary_leaves(), [0, 1, 2, 3]);
        // The all-true original candidate makes every node active in its
        // frozen reduct, for either connective. Both subjects execute the
        // retiring parent path rather than taking a masked-parent exit.
        let truth = [true; 5];
        for frozen in [false, true] {
            for width in 0..3 {
                for values in 0..16 {
                    for mut order in 0..4_usize.pow(4) {
                        let order: [usize; 4] = std::array::from_fn(|_| {
                            let next = order % 4;
                            order /= 4;
                            next
                        });
                        if (0..4).any(|position| order[..position].contains(&order[position])) {
                            continue;
                        }
                        let fresh = index.knowledge();
                        let mut knowledge = match width {
                            0 => fresh,
                            1 => counters::compact32(&fresh),
                            _ => counters::native(&fresh),
                        };
                        let mut region = Region::all_open(4);
                        let mut scratch = NarrowingScratch::default();
                        let mut witnessed = false;
                        let mut neutral = 0;
                        for (position, atom) in order.into_iter().enumerate() {
                            let value = values & (1 << atom) != 0;
                            assert!(if value {
                                region.hold(atom)
                            } else {
                                region.cut(atom)
                            });
                            if !witnessed {
                                if value == disjunction {
                                    witnessed = true;
                                } else {
                                    neutral += 1;
                                }
                            }
                            let attempt = if frozen {
                                index.narrow_frozen_known(
                                    FrozenSubject::new(&theory, &truth),
                                    &mut region,
                                    &mut knowledge,
                                    &mut scratch,
                                    RegionLimits::default(),
                                    &Cancellation::default(),
                                )
                            } else {
                                index.narrow_known(
                                    OriginalSubject::new(&theory, None),
                                    &mut region,
                                    &mut knowledge,
                                    &mut scratch,
                                    RegionLimits::default(),
                                    &Cancellation::default(),
                                )
                            };
                            assert_eq!(attempt.unwrap().0, Narrowing::Fixed { changed: false });
                            let root = if witnessed {
                                Some(disjunction)
                            } else if position == 3 {
                                Some(!disjunction)
                            } else {
                                None
                            };
                            // Root events still remove every parent occurrence
                            // from split ranking, including open child atoms.
                            let ranks = vec![usize::from(root.is_none()); 4];
                            assert_eq!(evidence(&knowledge), (witnessed, neutral, root, ranks));
                            assert!(scratch.nodes.is_empty());
                            assert!(scratch.learned.is_empty());
                        }
                    }
                }
            }
        }
    }
}

fn assign(knowledge: &mut Knowledge, node: usize, value: bool) {
    let known = compact_mut(knowledge);
    let mask = if value { known.sure } else { known.never };
    mask[0] |= 1 << node;
    if node < 4 {
        let atoms = if value {
            known.atom_sure
        } else {
            known.atom_never
        };
        atoms[0] |= 1 << node;
    }
}

#[test]
fn a_pending_witness_does_not_retire_the_counter() {
    for disjunction in [false, true] {
        let theory = theory(disjunction, false);
        let index = Narrower::new(&theory);
        let mut knowledge = index.knowledge();
        assign(&mut knowledge, 0, !disjunction);
        assign(&mut knowledge, 1, disjunction);
        let mut scratch = NarrowingScratch::default();
        scratch.prepare(4);
        scratch.nodes.push((1, disjunction));
        let mut work = Work::new(2);
        let result = Closure {
            known: compact_mut(&mut knowledge),
            lists: &mut scratch,
        }
        .revisit(
            Subject::from(OriginalSubject::new(&theory, None)),
            &index,
            0,
            !disjunction,
            &mut work,
        );
        assert!(result == Ok(Step::Unchanged));
        assert_eq!(evidence(&knowledge), (false, 1, None, vec![1; 4]));
        assert_eq!(scratch.nodes, [(1, disjunction)]);
        assert_eq!(work.spent, 2);
    }
}

#[test]
fn retired_parents_keep_their_read_permit() {
    for disjunction in [false, true] {
        let theory = theory(disjunction, false);
        let index = Narrower::new(&theory);
        for value in [false, true] {
            for stop in [Stop::WorkLimit, Stop::Cancelled, Stop::Deadline] {
                for limit in 0..=2 {
                    let mut knowledge = index.knowledge();
                    assign(&mut knowledge, 1, disjunction);
                    assign(&mut knowledge, 4, disjunction);
                    assign(&mut knowledge, 0, value);
                    compact_mut(&mut knowledge).witnessed[0] |= 1;
                    let before = evidence(&knowledge);
                    let mut scratch = NarrowingScratch::default();
                    scratch.prepare(4);
                    let mut quota = SinglePermits::new(limit, stop);
                    let mut work = Work::reserved(&mut quota);
                    let result = Closure {
                        known: compact_mut(&mut knowledge),
                        lists: &mut scratch,
                    }
                    .revisit(
                        Subject::from(OriginalSubject::new(&theory, None)),
                        &index,
                        0,
                        value,
                        &mut work,
                    );
                    work.refund();
                    assert_eq!(work.spent, limit);
                    if limit < 2 {
                        assert!(result == Err(stop));
                    } else {
                        assert!(result == Ok(Step::Unchanged));
                    }
                    assert_eq!(evidence(&knowledge), before);
                    assert!(scratch.nodes.is_empty());
                    assert!(scratch.learned.is_empty());
                }
            }
        }
    }
}

#[test]
fn a_first_witness_still_refutes_an_opposite_root() {
    for disjunction in [false, true] {
        let theory = theory(disjunction, false);
        let index = Narrower::new(&theory);
        let mut knowledge = index.knowledge();
        assign(&mut knowledge, 0, disjunction);
        assign(&mut knowledge, 4, !disjunction);
        let mut scratch = NarrowingScratch::default();
        scratch.prepare(4);
        let mut work = Work::new(2);
        let result = Closure {
            known: compact_mut(&mut knowledge),
            lists: &mut scratch,
        }
        .revisit(
            Subject::from(OriginalSubject::new(&theory, None)),
            &index,
            0,
            disjunction,
            &mut work,
        );
        assert!(result == Ok(Step::Contradiction));
        assert_eq!(work.spent, 2);
    }
}

#[test]
fn a_retired_chain_does_not_hide_another_parent() {
    for disjunction in [false, true] {
        let theory = theory(disjunction, true);
        let index = Narrower::new(&theory);
        assert_eq!(index.logical_parents(0).collect::<Vec<_>>(), [4, 5]);
        let mut knowledge = index.knowledge();
        for (node, value) in [
            (0, true),
            (1, disjunction),
            (3, false),
            (4, disjunction),
            (5, true),
        ] {
            assign(&mut knowledge, node, value);
        }
        compact_mut(&mut knowledge).witnessed[0] |= 1;
        let mut scratch = NarrowingScratch::default();
        scratch.prepare(4);
        let mut work = Work::new(3);
        let result = Closure {
            known: compact_mut(&mut knowledge),
            lists: &mut scratch,
        }
        .revisit(
            Subject::from(OriginalSubject::new(&theory, None)),
            &index,
            0,
            true,
            &mut work,
        );
        assert!(result == Ok(Step::Contradiction));
        assert_eq!(work.spent, 3);
        assert_eq!(evidence(&knowledge).1, 0);
    }
}

#[test]
fn a_frozen_falsum_parent_never_collects_a_witness() {
    for disjunction in [false, true] {
        let theory = theory(disjunction, false);
        let index = Narrower::new(&theory);
        // Actual truth masks: OR(false, false, false, false), or
        // AND(true, false, false, false). The parent is frozen falsum.
        let truth = [!disjunction, false, false, false, false];
        let mut knowledge = index.knowledge();
        assign(&mut knowledge, 0, !disjunction);
        assign(&mut knowledge, 4, false);
        let mut scratch = NarrowingScratch::default();
        scratch.prepare(4);
        let mut work = Work::new(2);
        let result = Closure {
            known: compact_mut(&mut knowledge),
            lists: &mut scratch,
        }
        .revisit(
            Subject::from(FrozenSubject::new(&theory, &truth)),
            &index,
            0,
            !disjunction,
            &mut work,
        );
        assert!(result == Ok(Step::Unchanged));
        assert_eq!(evidence(&knowledge), (false, 0, Some(false), vec![1; 4]));
        assert_eq!(work.spent, 2);
    }
}
