//! Processed operands remove only redundant scans, not pending events.

use zetesis_cpu::{Cancellation, Stop};

use crate::{
    AdmissionLimits, EvaluationLimits, EvaluationWorkspace, FormulaParts, FrozenSubject,
    Interpretation, Knowledge, Narrower, Narrowing, NarrowingQuota, NarrowingScratch, Node,
    OriginalSubject, Region, RegionLimits, Theory,
};

use super::super::{Closure, Step, Subject, Width, Work, bit, counters::Count};
use super::{
    compact_mut,
    counters::{native, same_knowledge},
};

mod evidence;

fn chain(disjunction: bool) -> Theory {
    let join = if disjunction {
        Node::or_pair
    } else {
        Node::and_pair
    };
    Theory::new(
        3,
        FormulaParts::new(
            vec![
                Node::atom(0),
                Node::atom(1),
                Node::atom(1),
                Node::atom(2),
                join([0, 1]),
                join([4, 1]),
                join([5, 2]),
                join([6, 3]),
            ],
            vec![],
        )
        .unwrap(),
        vec![],
        AdmissionLimits::default(),
    )
    .unwrap()
}

/// The previous unit loop, retained only at this small dispatch boundary.
fn scan<C: Count>(
    closure: &mut Closure<'_, C>,
    index: &Narrower,
    work: &mut Work<'_>,
) -> Result<Step, Stop> {
    let chain = &index.chains[0];
    for &operand in &chain.operands {
        work.tick()?;
        let opposite: &[u64] = if chain.disjunction {
            closure.known.never
        } else {
            closure.known.sure
        };
        if !bit(opposite, operand) {
            return Ok(if chain.disjunction {
                closure.sure(operand)
            } else {
                closure.never(operand)
            });
        }
    }
    Ok(Step::Unchanged)
}

/// The complete downward scan before processed totals omitted it.
fn scan_operands<C: Count>(
    closure: &mut Closure<'_, C>,
    index: &Narrower,
    work: &mut Work<'_>,
) -> Result<Step, Stop> {
    let chain = &index.chains[0];
    let mut step = Step::Unchanged;
    for &operand in &chain.operands {
        work.tick()?;
        step = step.join(if chain.disjunction {
            closure.never(operand)
        } else {
            closure.sure(operand)
        });
    }
    Ok(step)
}

#[derive(Clone, Copy)]
enum Teaching {
    Unit,
    Operands,
}

struct Attempt {
    result: Result<Step, Stop>,
    work: u64,
    knowledge: Knowledge,
    scratch: NarrowingScratch,
}

/// 0 is open; 1/2 fail pending/counted; 3/4 hold pending/counted.
fn state(index: &Narrower, values: [usize; 4]) -> (Knowledge, NarrowingScratch) {
    let mut knowledge = index.knowledge();
    let known = compact_mut(&mut knowledge);
    let mut scratch = NarrowingScratch::default();
    scratch.prepare(3);
    let root = index.chains[0].root;
    if index.chains[0].disjunction {
        known.sure[0] |= 1 << root;
    } else {
        known.never[0] |= 1 << root;
    }
    scratch.nodes.push((root, index.chains[0].disjunction));
    for (operand, value) in values.into_iter().enumerate() {
        match value {
            1 | 2 => known.never[0] |= 1 << operand,
            3 | 4 => known.sure[0] |= 1 << operand,
            _ => {}
        }
        match value {
            1 | 3 => scratch.nodes.push((operand, value == 3)),
            2 | 4 if (value == 4) == index.chains[0].disjunction => {
                known.witnessed[0] |= 1;
            }
            2 | 4 => known.neutral_operands.add(0, 1),
            _ => {}
        }
    }
    (knowledge, scratch)
}

fn attempt(
    index: &Narrower,
    values: [usize; 4],
    wide: bool,
    scanning: bool,
    limit: u64,
    teaching: Teaching,
) -> Attempt {
    fn call<C: Count>(
        closure: &mut Closure<'_, C>,
        index: &Narrower,
        scanning: bool,
        work: &mut Work<'_>,
        teaching: Teaching,
    ) -> Result<Step, Stop> {
        match (teaching, scanning) {
            (Teaching::Unit, true) => scan(closure, index, work),
            (Teaching::Unit, false) => closure.unit(index, 0, work),
            (Teaching::Operands, true) => scan_operands(closure, index, work),
            (Teaching::Operands, false) => closure.teach_chain(index, 0, work),
        }
    }
    let (mut knowledge, mut scratch) = state(index, values);
    if matches!(teaching, Teaching::Operands) {
        let root = index.chains[0].root;
        let known = compact_mut(&mut knowledge);
        known.sure[0] ^= 1 << root;
        known.never[0] ^= 1 << root;
        scratch.nodes[0].1 = !index.chains[0].disjunction;
    }
    if wide {
        knowledge = native(&knowledge);
    }
    let mut work = Work::new(limit);
    let result = match &mut knowledge.width {
        Width::Compact16(known) => call(
            &mut Closure {
                known: known.borrow(),
                lists: &mut scratch,
            },
            index,
            scanning,
            &mut work,
            teaching,
        ),
        Width::Compact32(known) => call(
            &mut Closure {
                known: known.borrow(),
                lists: &mut scratch,
            },
            index,
            scanning,
            &mut work,
            teaching,
        ),
        Width::Native(known) => call(
            &mut Closure {
                known: known.borrow(),
                lists: &mut scratch,
            },
            index,
            scanning,
            &mut work,
            teaching,
        ),
    };
    Attempt {
        result,
        work: work.spent,
        knowledge,
        scratch,
    }
}

fn unit(index: &Narrower, values: [usize; 4], wide: bool, scanning: bool, limit: u64) -> Attempt {
    attempt(index, values, wide, scanning, limit, Teaching::Unit)
}

fn same(left: &Attempt, right: &Attempt) {
    assert!(left.result == right.result);
    same_knowledge(&left.knowledge, &right.knowledge);
    assert_eq!(left.scratch.nodes, right.scratch.nodes);
    assert_eq!(left.scratch.learned, right.scratch.learned);
    assert_eq!(left.scratch.heads, right.scratch.heads);
    assert_eq!(left.scratch.pending, right.scratch.pending);
}

#[test]
fn processed_totals_preserve_downward_teaching() {
    for disjunction in [false, true] {
        let index = Narrower::new(&chain(disjunction));
        // The fixture repeats node 1 in the syntax and gives atom 1 two
        // separate nodes. Counters follow the four distinct chain leaves.
        assert_eq!(index.chains[0].operands, [0, 1, 2, 3]);
        let counted = if disjunction { 2 } else { 4 };
        for mut code in 0..5usize.pow(4) {
            let values = std::array::from_fn(|_| {
                let value = code % 5;
                code /= 5;
                value
            });
            for wide in [false, true] {
                let previous = attempt(&index, values, wide, true, u64::MAX, Teaching::Operands);
                let current = attempt(&index, values, wide, false, u64::MAX, Teaching::Operands);
                assert_eq!(previous.work, 4);
                let conflict = values.iter().position(|&value| {
                    if disjunction {
                        matches!(value, 3 | 4)
                    } else {
                        matches!(value, 1 | 2)
                    }
                });
                if let Some(position) = conflict {
                    // The full scan and its shortened prefix both refute.
                    // Their partial knowledge is discarded, not compared as
                    // completed closure; only the prefix needs work permits.
                    assert!(previous.result == Ok(Step::Contradiction));
                    assert!(current.result == previous.result);
                    assert_eq!(current.work, u64::try_from(position + 1).unwrap());
                } else {
                    same(&current, &previous);
                    let complete = values.iter().all(|&value| value == counted);
                    assert_eq!(current.work, if complete { 0 } else { 4 });
                    if complete {
                        assert!(matches!(current.result, Ok(Step::Unchanged)));
                    }
                }
            }
        }
    }
}

#[test]
fn unprocessed_operands_keep_the_charged_scan() {
    for disjunction in [false, true] {
        let index = Narrower::new(&chain(disjunction));
        let counted = if disjunction { 2 } else { 4 };
        for wide in [false, true] {
            for last in [0, counted - 1] {
                // The last operand is open or already known with its event
                // pending. Both retain the full scan and its refusal points.
                let values = [counted, counted, counted, last];
                for limit in 0..=4 {
                    let previous = attempt(&index, values, wide, true, limit, Teaching::Operands);
                    let current = attempt(&index, values, wide, false, limit, Teaching::Operands);
                    same(&current, &previous);
                    assert_eq!(current.work, limit);
                    assert_eq!(current.work, previous.work);
                    if limit < 4 {
                        assert!(current.result == Err(Stop::WorkLimit));
                    }
                }
            }
        }
    }
}

#[test]
fn counted_and_pending_states_match_the_unit_scan() {
    for disjunction in [false, true] {
        let index = Narrower::new(&chain(disjunction));
        // Repeated node 1 is counted once; distinct nodes 1 and 2 naming the
        // same atom remain distinct operands with independently pending events.
        assert_eq!(index.chains[0].operands, [0, 1, 2, 3]);
        for mut code in 0..5usize.pow(4) {
            let values = std::array::from_fn(|_| {
                let value = code % 5;
                code /= 5;
                value
            });
            let opposite = if disjunction { 2 } else { 4 };
            if values.iter().filter(|&&value| value == opposite).count() != 3 {
                continue;
            }
            let witnessed = values.contains(&if disjunction { 4 } else { 2 });
            for wide in [false, true] {
                let previous = unit(&index, values, wide, true, u64::MAX);
                let current = unit(&index, values, wide, false, u64::MAX);
                same(&current, &previous);
                assert_eq!(current.work, if witnessed { 0 } else { previous.work });
                assert!(previous.work > 0);
            }
        }
    }
}

#[test]
fn only_a_processed_witness_removes_the_charged_scan() {
    for disjunction in [false, true] {
        let index = Narrower::new(&chain(disjunction));
        let opposite = if disjunction { 2 } else { 4 };
        let counted = if disjunction { 4 } else { 2 };
        for wide in [false, true] {
            let values = [opposite, opposite, opposite, counted];
            let previous = unit(&index, values, wide, true, 4);
            let current = unit(&index, values, wide, false, 0);
            same(&current, &previous);
            assert!(matches!(current.result, Ok(Step::Unchanged)));
            assert_eq!((current.work, previous.work), (0, 4));
            // An already set but uncounted witness still follows the ordinary
            // charged path, as does the sole genuinely open operand.
            for last in [0, counted - 1] {
                let values = [opposite, opposite, opposite, last];
                for limit in 0..=4 {
                    let previous = unit(&index, values, wide, true, limit);
                    let current = unit(&index, values, wide, false, limit);
                    same(&current, &previous);
                    assert_eq!(current.work, previous.work);
                    assert_eq!(current.work, limit);
                    if limit < 4 {
                        assert!(current.result == Err(Stop::WorkLimit));
                    }
                }
            }
        }
    }
}

#[test]
fn the_last_opposite_event_still_refutes_the_chain() {
    for disjunction in [false, true] {
        let index = Narrower::new(&chain(disjunction));
        let opposite = if disjunction { 2 } else { 4 };
        let (mut knowledge, mut scratch) =
            state(&index, [opposite, opposite, opposite, opposite - 1]);
        let mut closure = Closure {
            known: compact_mut(&mut knowledge),
            lists: &mut scratch,
        };
        let mut work = Work::new(4);
        // All bits are opposite, but the last event has not reached the chain.
        assert!(matches!(
            closure.unit(&index, 0, &mut work),
            Ok(Step::Unchanged)
        ));
        assert_eq!(work.spent, 4);
        assert!(matches!(
            closure.operand_changed(&index, 0, !disjunction, &mut work),
            Ok(Step::Contradiction)
        ));
        assert_eq!(work.spent, 4);
        assert!(bit(
            if disjunction {
                closure.known.sure
            } else {
                closure.known.never
            },
            7
        ));
        assert!(!bit(
            if disjunction {
                closure.known.never
            } else {
                closure.known.sure
            },
            7
        ));
    }
}

#[test]
fn an_absorbing_operand_still_refutes_an_opposite_root() {
    for disjunction in [false, true] {
        let index = Narrower::new(&chain(disjunction));
        let value = if disjunction { 3 } else { 1 };
        let (mut knowledge, mut scratch) = state(&index, [0, 0, 0, value]);
        let known = compact_mut(&mut knowledge);
        // A fresh witness's event contradicts the parent before any unit scan.
        known.sure[0] ^= 1 << 7;
        known.never[0] ^= 1 << 7;
        let mut work = Work::new(0);
        let result = Closure {
            known,
            lists: &mut scratch,
        }
        .operand_changed(&index, 0, disjunction, &mut work);
        assert!(matches!(result, Ok(Step::Contradiction)));
        assert_eq!(work.spent, 0);
    }
}

struct Quota {
    remaining: u64,
    calls: u64,
    stop: Stop,
}

impl NarrowingQuota for Quota {
    fn reserve(&mut self, _wanted: u64) -> Result<u64, Stop> {
        self.calls += 1;
        if self.remaining == 0 {
            return Err(self.stop);
        }
        self.remaining -= 1;
        Ok(1)
    }

    fn refund(&mut self, unspent: u64) {
        self.remaining += unspent;
    }
}

/// h <- a and b, or h <- a or b; the body is a live chain with a parent.
fn chain_body(disjunction: bool) -> Theory {
    let join = if disjunction {
        Node::or_pair
    } else {
        Node::and_pair
    };
    Theory::new(
        3,
        FormulaParts::new(
            vec![
                Node::atom(0),
                Node::atom(1),
                join([0, 1]),
                Node::atom(2),
                Node::implies(2, 3),
            ],
            vec![],
        )
        .unwrap(),
        vec![4],
        AdmissionLimits::default(),
    )
    .unwrap()
}

/// The asserted implication has run; both body leaves have been processed,
/// learning the body whose event is about to run. No atom has been published
/// to a region: these tests exercise the event before that boundary.
fn processed_body(index: &Narrower) -> (Knowledge, NarrowingScratch) {
    assert_eq!(index.chains[0].root, 2);
    assert_eq!(index.chains[0].operands, [0, 1]);
    let mut knowledge = index.knowledge();
    let known = compact_mut(&mut knowledge);
    if index.chains[0].disjunction {
        known.never[0] = 0b111;
        known.atom_never[0] = 0b11;
        known.neutral_operands.add(0, 2);
    } else {
        known.sure[0] = 0b111;
        known.atom_sure[0] = 0b11;
        known.neutral_operands.add(0, 2);
    }
    known.sure[0] |= 1 << 4;
    known.unknown.decrement(2);
    *known.seeded = true;
    let mut scratch = NarrowingScratch::default();
    scratch.prepare(3);
    (knowledge, scratch)
}

#[test]
fn processed_roots_keep_the_node_and_parent_permits() {
    for disjunction in [false, true] {
        let theory = chain_body(disjunction);
        let index = Narrower::new(&theory);
        let producers =
            super::super::producers(&theory, RegionLimits::default(), &Cancellation::default())
                .unwrap()
                .producers
                .expect("the implication is a producer");
        let subject = Subject::from(OriginalSubject::new(&theory, Some(&producers)));
        for stop in [Stop::WorkLimit, Stop::Cancelled, Stop::Deadline] {
            for limit in 0..=2 {
                let (mut knowledge, mut scratch) = processed_body(&index);
                let mut expected = knowledge.clone();
                let mut quota = Quota {
                    remaining: limit,
                    calls: 0,
                    stop,
                };
                let mut work = Work::reserved(&mut quota);
                let result = Closure {
                    known: compact_mut(&mut knowledge),
                    lists: &mut scratch,
                }
                .revisit(subject, &index, 2, !disjunction, &mut work);
                assert_eq!(work.spent, limit);
                work.settle();
                let known = compact_mut(&mut expected);
                if limit > 0 {
                    known.unknown.decrement(0);
                    known.unknown.decrement(1);
                }
                if limit == 2 {
                    if disjunction {
                        assert!(matches!(result, Ok(Step::Unchanged)));
                    } else {
                        known.sure[0] |= 1 << 3;
                        assert!(matches!(result, Ok(Step::Changed)));
                    }
                } else {
                    assert!(result == Err(stop));
                }
                same_knowledge(&knowledge, &expected);
                let taught_head = !disjunction && limit == 2;
                assert_eq!(
                    scratch.nodes,
                    if taught_head { vec![(3, true)] } else { vec![] }
                );
                assert!(scratch.learned.is_empty());
                let woke_head = disjunction && limit > 0;
                assert_eq!(scratch.heads, if woke_head { vec![2] } else { vec![] });
                assert_eq!(scratch.pending, if woke_head { vec![4] } else { vec![0] });
                assert_eq!(quota.calls, (limit + 1).min(2));
                assert_eq!(quota.remaining, 0);
            }
        }
    }
}

#[test]
fn a_processed_false_body_still_removes_support() {
    let theory = chain_body(true);
    let index = Narrower::new(&theory);
    let producers =
        super::super::producers(&theory, RegionLimits::default(), &Cancellation::default())
            .unwrap()
            .producers
            .expect("the implication is a producer");
    let subject = Subject::from(OriginalSubject::new(&theory, Some(&producers)));
    let (mut knowledge, mut scratch) = processed_body(&index);
    let mut closure = Closure {
        known: compact_mut(&mut knowledge),
        lists: &mut scratch,
    };
    let mut work = Work::new(3);
    assert!(matches!(
        closure.revisit(subject, &index, 2, false, &mut work),
        Ok(Step::Unchanged)
    ));
    assert_eq!(work.spent, 2);
    assert_eq!(closure.lists.next_recheck(), Some(2));
    assert!(matches!(
        closure.recheck(&index, &producers, 2, &mut work),
        Ok(Step::Changed)
    ));
    assert_eq!(work.spent, 3);
    assert!(bit(closure.known.atom_never, 2));
    assert_eq!(closure.lists.nodes, [(3, false)]);
    assert_eq!(closure.lists.learned, [2]);
    assert!(closure.lists.heads.is_empty());
    assert!(closure.lists.pending.iter().all(|word| *word == 0));
}

#[test]
fn a_frozen_false_root_remains_a_constant() {
    let theory = chain_body(true);
    let index = Narrower::new(&theory);
    let candidate = Interpretation::new(&theory, vec![2]).unwrap();
    let mut workspace = EvaluationWorkspace::default();
    let evaluation = workspace
        .evaluate(
            &candidate,
            EvaluationLimits::default(),
            &Cancellation::default(),
        )
        .result
        .unwrap();
    assert!(!evaluation.truth()[2]);
    assert!(evaluation.truth()[4]);
    let subject = Subject::from(FrozenSubject::new(&theory, evaluation.truth()));
    let (mut knowledge, mut scratch) = processed_body(&index);
    let mut expected = knowledge.clone();
    let known = compact_mut(&mut expected);
    known.unknown.decrement(0);
    known.unknown.decrement(1);
    let mut work = Work::new(2);
    let result = Closure {
        known: compact_mut(&mut knowledge),
        lists: &mut scratch,
    }
    .revisit(subject, &index, 2, false, &mut work);
    assert!(matches!(result, Ok(Step::Unchanged)));
    assert_eq!(work.spent, 2);
    same_knowledge(&knowledge, &expected);
    // The falsum event still updates ranking and its unmasked parent. It
    // neither teaches operands nor applies original-theory support pruning.
    assert!(scratch.nodes.is_empty() && scratch.learned.is_empty() && scratch.heads.is_empty());
    assert!(scratch.pending.iter().all(|word| *word == 0));
}

#[test]
fn witnessed_units_keep_the_node_and_parent_permits() {
    for disjunction in [false, true] {
        let theory = chain(disjunction);
        let index = Narrower::new(&theory);
        let opposite = if disjunction { 2 } else { 4 };
        let witness = if disjunction { 4 } else { 2 };
        let subject = Subject::from(OriginalSubject::new(&theory, None));
        for stop in [Stop::WorkLimit, Stop::Cancelled, Stop::Deadline] {
            for limit in 0..=2 {
                let (mut knowledge, mut scratch) =
                    state(&index, [opposite, opposite, opposite - 1, witness]);
                assert_eq!(scratch.nodes.pop(), Some((2, !disjunction)));
                let mut expected = knowledge.clone();
                let mut quota = Quota {
                    remaining: limit,
                    calls: 0,
                    stop,
                };
                let mut work = Work::reserved(&mut quota);
                let result = Closure {
                    known: compact_mut(&mut knowledge),
                    lists: &mut scratch,
                }
                .revisit(subject, &index, 2, !disjunction, &mut work);
                assert_eq!(work.spent, limit);
                work.settle();
                let expected_known = compact_mut(&mut expected);
                if limit > 0 {
                    if disjunction {
                        expected_known.atom_never[0] |= 1 << 1;
                    } else {
                        expected_known.atom_sure[0] |= 1 << 1;
                    }
                }
                if limit == 2 {
                    expected_known.neutral_operands.add(0, 1);
                    assert!(matches!(result, Ok(Step::Changed)));
                } else {
                    assert!(result == Err(stop));
                }
                assert_eq!(scratch.nodes, [(7, disjunction)]);
                assert_eq!(scratch.learned, if limit == 0 { vec![] } else { vec![1] });
                assert!(scratch.heads.is_empty() && scratch.pending.iter().all(|word| *word == 0));
                // All full-state fields are checked, including unchanged seen
                // and unknown counts, without reusing stopped knowledge.
                same_knowledge(&knowledge, &expected);
                assert_eq!(quota.calls, (limit + 1).min(2));
            }
        }
    }
}

fn narrow(
    theory: &Theory,
    index: &Narrower,
    frozen: Option<&[bool]>,
    region: &mut Region,
    knowledge: &mut Knowledge,
) {
    let mut scratch = NarrowingScratch::default();
    let result = if let Some(truth) = frozen {
        index.narrow_frozen_known(
            FrozenSubject::new(theory, truth),
            region,
            knowledge,
            &mut scratch,
            RegionLimits::default(),
            &Cancellation::default(),
        )
    } else {
        index.narrow_known(
            OriginalSubject::new(theory, None),
            region,
            knowledge,
            &mut scratch,
            RegionLimits::default(),
            &Cancellation::default(),
        )
    }
    .unwrap();
    assert!(matches!(result.0, Narrowing::Fixed { .. }));
    assert!(scratch.nodes.is_empty() && scratch.learned.is_empty() && scratch.heads.is_empty());
    assert!(scratch.pending.iter().all(|word| *word == 0));
}

fn inherited(theory: &Theory, index: &Narrower, frozen: Option<&[bool]>, first: bool) {
    for wide in [false, true] {
        let mut parent = Region::all_open(3);
        let disjunction = index.chains[0].disjunction;
        assert!(if first { parent.hold(2) } else { parent.cut(2) });
        let fresh = index.knowledge();
        let fresh = if wide { native(&fresh) } else { fresh };
        let mut knowledge = fresh.clone();
        narrow(theory, index, frozen, &mut parent, &mut knowledge);
        let saved = knowledge.clone();
        let mut child = parent.clone();
        for atom in 0..2 {
            assert!(if disjunction {
                child.cut(atom)
            } else {
                child.hold(atom)
            });
        }
        let mut from_fresh = child.clone();
        let mut from_parent = knowledge.clone();
        let mut from_nothing = fresh;
        narrow(theory, index, frozen, &mut child, &mut from_parent);
        narrow(theory, index, frozen, &mut from_fresh, &mut from_nothing);
        assert_eq!(child, from_fresh);
        same_knowledge(&from_parent, &from_nothing);
        same_knowledge(&knowledge, &saved);
    }
}

#[test]
fn inherited_chain_knowledge_agrees_with_fresh_closure() {
    for disjunction in [false, true] {
        let theory = chain(disjunction);
        let index = Narrower::new(&theory);
        // One first decision supplies an absorbing witness; its opposite
        // lets the child learn every operand at the same polarity.
        for first in [false, true] {
            inherited(&theory, &index, None, first);
            for atoms in [vec![0, 1, 2], vec![0, 1]] {
                let candidate = Interpretation::new(&theory, atoms).unwrap();
                let mut workspace = EvaluationWorkspace::default();
                let attempt = workspace.evaluate(
                    &candidate,
                    EvaluationLimits::default(),
                    &Cancellation::default(),
                );
                let evaluation = attempt.result.unwrap();
                inherited(&theory, &index, Some(evaluation.truth()), first);
            }
        }
    }
}
