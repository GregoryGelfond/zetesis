//! Failed bodies wake support from their node events, before support quiescence.

use zetesis_cpu::{Cancellation, Stop};

use crate::{
    AdmissionLimits, EvaluationLimits, EvaluationWorkspace, FormulaParts, FrozenSubject,
    Interpretation, Knowledge, Narrower, Narrowing, NarrowingQuota, NarrowingScratch, Node,
    OriginalSubject, Producers, Region, RegionLimits, Theory,
};

use super::super::{Closure, Step, Subject, Work};

fn theory(atoms: usize, nodes: Vec<Node>, roots: Vec<usize>) -> Theory {
    Theory::new(
        atoms,
        FormulaParts::new(nodes, vec![]).unwrap(),
        roots,
        AdmissionLimits::default(),
    )
    .unwrap()
}

fn prepare(theory: &Theory) -> (Narrower, Producers) {
    let producers =
        super::super::producers(theory, RegionLimits::default(), &Cancellation::default())
            .unwrap()
            .producers
            .expect("the fixture belongs to the support fragment");
    (Narrower::new(theory), producers)
}

/// {a}; {b}; h <- not a; k <- not a, with two distinct nodes for a and
/// two distinct negative bodies supplying k. One body is also shared by h.
fn aliased_bodies() -> Theory {
    theory(
        4,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::atom(2),
            Node::atom(3),
            Node::atom(0),
            Node::falsum(),
            Node::implies(0, 5),
            Node::implies(4, 5),
            Node::implies(6, 1),
            Node::implies(6, 2),
            Node::implies(7, 2),
            Node::or_pair([0, 6]),
            Node::implies(3, 5),
            Node::or_pair([3, 12]),
        ],
        vec![8, 9, 10, 11, 13],
    )
}

fn close(
    theory: &Theory,
    index: &Narrower,
    producers: &Producers,
    region: &mut Region,
    knowledge: &mut Knowledge,
    scratch: &mut NarrowingScratch,
) {
    assert!(matches!(
        index
            .narrow_known(
                OriginalSubject::new(theory, Some(producers)),
                region,
                knowledge,
                scratch,
                RegionLimits::default(),
                &Cancellation::default(),
            )
            .unwrap()
            .0,
        Narrowing::Fixed { .. }
    ));
    assert!(scratch.nodes.is_empty());
    assert!(scratch.heads.is_empty());
    assert!(scratch.pending.iter().all(|word| *word == 0));
}

/// The complete state, including counters and seen decisions, agrees; no
/// pending effect is hidden behind agreement of the published region alone.
fn same_knowledge(left: &Knowledge, right: &Knowledge) {
    let (left, right) = (super::compact(left), super::compact(right));
    assert_eq!(left.masks.slices(), right.masks.slices());
    for (left, right) in [
        (&left.sure_operands, &right.sure_operands),
        (&left.never_operands, &right.never_operands),
        (&left.unknown, &right.unknown),
    ] {
        assert_eq!(left.len(), right.len());
        assert!((0..left.len()).all(|at| left.get(at) == right.get(at)));
    }
    assert_eq!(left.seeded, right.seeded);
}

#[test]
fn a_false_body_event_owns_its_support_wakeup() {
    // {a}; h <- not a; k <- not a. The shared body is node 4.
    let theory = theory(
        3,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::atom(2),
            Node::falsum(),
            Node::implies(0, 3),
            Node::implies(4, 1),
            Node::implies(4, 2),
            Node::or_pair([0, 4]),
        ],
        vec![5, 6, 7],
    );
    let (index, producers) = prepare(&theory);
    let subject = Subject::from(OriginalSubject::new(&theory, Some(&producers)));
    let mut knowledge = index.knowledge();
    let mut lists = NarrowingScratch::default();
    lists.prepare(3);
    let mut closure = Closure {
        known: super::compact_mut(&mut knowledge),
        lists: &mut lists,
    };
    let mut work = Work::new(u64::MAX);
    assert!(matches!(closure.never(3), Step::Changed));
    while let Some((node, value)) = closure.lists.nodes.pop() {
        closure
            .revisit(subject, &index, node, value, &mut work)
            .unwrap();
    }
    assert!(matches!(
        closure.atom(&index, Some(&producers), None, 0, true),
        Step::Changed
    ));
    assert_eq!(closure.lists.nodes.pop(), Some((0, true)));
    closure
        .revisit(subject, &index, 0, true, &mut work)
        .unwrap();
    // Parent teaching has already learned the body's falsity, but neither
    // dependent head is scheduled until its queued false event runs.
    assert!(closure.lists.nodes.contains(&(4, false)));
    assert!(!closure.lists.heads.contains(&1));
    assert!(!closure.lists.heads.contains(&2));
    while let Some((node, value)) = closure.lists.nodes.pop() {
        closure
            .revisit(subject, &index, node, value, &mut work)
            .unwrap();
    }
    assert!(closure.lists.heads.contains(&1));
    assert!(closure.lists.heads.contains(&2));
    while closure.lists.next_recheck().is_some() {}
    // Repeated parent teaching cannot reschedule an unchanged failed body.
    for _ in 0..2 {
        closure
            .teach_operands(subject, &index, 4, &mut work)
            .unwrap();
        assert!(closure.lists.heads.is_empty());
    }
    assert!(matches!(closure.never(4), Step::Unchanged));
    assert!(closure.lists.nodes.is_empty());
}

/// A closed parent's private state and a fresh state must give the same
/// complete region and knowledge for both children of the next split.
fn compare_children(
    index: &Narrower,
    mut parent: Region,
    atom: usize,
    mut finish: impl FnMut(&mut Region, &mut Knowledge),
    check: impl Fn(&Region, bool),
) {
    let mut inherited = index.knowledge();
    finish(&mut parent, &mut inherited);
    assert!(parent.is_open(atom));
    for held in [false, true] {
        let mut child = parent.clone();
        if held {
            child.hold(atom);
        } else {
            child.cut(atom);
        }
        let mut fresh = child.clone();
        let mut known = inherited.clone();
        let mut fresh_known = index.knowledge();
        finish(&mut child, &mut known);
        finish(&mut fresh, &mut fresh_known);
        check(&child, held);
        assert_eq!(child, fresh);
        same_knowledge(&known, &fresh_known);
    }
}

#[test]
fn shared_aliased_bodies_preserve_inherited_closure() {
    let theory = aliased_bodies();
    let (index, producers) = prepare(&theory);
    let mut scratch = NarrowingScratch::default();
    compare_children(
        &index,
        Region::all_open(4),
        0,
        |region, known| close(&theory, &index, &producers, region, known, &mut scratch),
        |region, held| {
            assert_eq!(region.decision(1), Some(!held));
            assert_eq!(region.decision(2), Some(!held));
        },
    );
}

#[test]
fn inherited_failed_bodies_need_no_replayed_event() {
    let theory = aliased_bodies();
    let (index, producers) = prepare(&theory);
    let mut parent = Region::all_open(4);
    parent.hold(0);
    let mut scratch = NarrowingScratch::default();
    compare_children(
        &index,
        parent,
        3,
        |region, known| close(&theory, &index, &producers, region, known, &mut scratch),
        |region, _| {
            assert_eq!(region.decision(1), Some(false));
            assert_eq!(region.decision(2), Some(false));
        },
    );
}

#[test]
fn body_failure_combines_with_competing_head_support() {
    // a or b; a <- not c; {b}; {c}. Held b removes the ordinary
    // disjunctive support for a; whether c holds decides its remaining body.
    let theory = theory(
        3,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::atom(2),
            Node::falsum(),
            Node::implies(2, 3),
            Node::or_pair([0, 1]),
            Node::implies(4, 0),
            Node::implies(1, 3),
            Node::or_pair([1, 7]),
            Node::or_pair([2, 4]),
        ],
        vec![5, 6, 8, 9],
    );
    let (index, producers) = prepare(&theory);
    let mut parent = Region::all_open(3);
    parent.hold(1);
    let mut scratch = NarrowingScratch::default();
    compare_children(
        &index,
        parent,
        2,
        |region, known| close(&theory, &index, &producers, region, known, &mut scratch),
        |region, held| assert_eq!(region.decision(0), Some(!held)),
    );
}

#[test]
fn frozen_failed_bodies_do_not_apply_original_support() {
    let theory = aliased_bodies();
    let candidate = Interpretation::new(&theory, [0, 1, 2, 3]).unwrap();
    let cancellation = Cancellation::default();
    let mut workspace = EvaluationWorkspace::default();
    let evaluated = workspace
        .evaluate(&candidate, EvaluationLimits::default(), &cancellation)
        .result
        .unwrap();
    assert!(evaluated.is_model());
    let index = Narrower::new(&theory);
    let mut scratch = NarrowingScratch::default();
    compare_children(
        &index,
        Region::all_open(4),
        1,
        |region, known| {
            assert!(matches!(
                index
                    .narrow_frozen_known(
                        FrozenSubject::new(&theory, evaluated.truth()),
                        region,
                        known,
                        &mut scratch,
                        RegionLimits::default(),
                        &cancellation,
                    )
                    .unwrap()
                    .0,
                Narrowing::Fixed { .. }
            ));
            assert!(scratch.nodes.is_empty());
            assert!(scratch.heads.is_empty());
        },
        |region, _| assert!(region.is_open(2)),
    );
}

struct Quota {
    remaining: u64,
    failure: Stop,
    spent: u64,
}

impl NarrowingQuota for Quota {
    fn reserve(&mut self, _wanted: u64) -> Result<u64, Stop> {
        if self.remaining == 0 {
            return Err(self.failure);
        }
        self.remaining -= 1;
        self.spent += 1;
        Ok(1)
    }

    fn refund(&mut self, unspent: u64) {
        self.spent -= unspent;
    }
}

#[test]
fn refused_body_propagation_keeps_exact_work_receipts() {
    let theory = aliased_bodies();
    let (index, producers) = prepare(&theory);
    let mut input = Region::all_open(4);
    input.hold(0);
    let mut expected = input.clone();
    let mut expected_known = index.knowledge();
    let mut scratch = NarrowingScratch::default();
    let (_, statistics) = index
        .narrow_known(
            OriginalSubject::new(&theory, Some(&producers)),
            &mut expected,
            &mut expected_known,
            &mut scratch,
            RegionLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    assert_eq!(expected.decision(1), Some(false));
    for failure in [Stop::WorkLimit, Stop::Cancelled, Stop::Deadline] {
        for limit in 0..statistics.work {
            let mut region = input.clone();
            let mut known = index.knowledge();
            let mut quota = Quota {
                remaining: limit,
                failure,
                spent: 0,
            };
            let attempt = index.narrow_known_reserved(
                OriginalSubject::new(&theory, Some(&producers)),
                &mut region,
                &mut known,
                &mut scratch,
                &Cancellation::default(),
                &mut quota,
            );
            assert_eq!(attempt.result, Err(failure));
            assert_eq!(attempt.statistics.work, limit);
            assert_eq!(quota.spent, limit);
            // Discard partial knowledge/region; reused scratch must not leak
            // a delayed wakeup into the next complete invocation.
            let mut retry = input.clone();
            let mut retry_known = index.knowledge();
            close(
                &theory,
                &index,
                &producers,
                &mut retry,
                &mut retry_known,
                &mut scratch,
            );
            assert_eq!(retry, expected);
            same_knowledge(&retry_known, &expected_known);
        }
    }
}
