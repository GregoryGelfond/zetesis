//! A support recheck is queued at most once until it runs, and a support that
//! falls after an atom's recheck has it rechecked again.

use zetesis_cpu::{Cancellation, Stop};

use super::super::{Closure, Step, Work};

use crate::{
    AdmissionLimits, Narrower, Narrowing, NarrowingQuota, NarrowingScratch, NarrowingStatistics,
    Node, OriginalSubject, Producers, Region, RegionLimits, Theory,
};

/// Atom 0 with one producer per body atom `1..=bodies`, `a ← b_i`, each body
/// a free choice `{b_i}`, written `b_i ∨ ¬b_i`.
fn shared_head(bodies: usize) -> Theory {
    let mut nodes: Vec<Node> = (0..=bodies).map(Node::atom).collect();
    nodes.push(Node::falsum());
    let falsum = nodes.len() - 1;
    let mut roots = Vec::new();
    for body in 1..=bodies {
        nodes.push(Node::implies(body, 0));
        roots.push(nodes.len() - 1);
        nodes.push(Node::implies(body, falsum));
        nodes.push(Node::or_pair([body, nodes.len() - 1]));
        roots.push(nodes.len() - 1);
    }
    Theory::new(
        bodies + 1,
        crate::FormulaParts::new(nodes, vec![]).unwrap(),
        roots,
        AdmissionLimits::default(),
    )
    .unwrap()
}

fn producers(theory: &Theory) -> Producers {
    super::super::producers(theory, RegionLimits::default(), &Cancellation::default())
        .unwrap()
        .producers
        .expect("every root is a rule of the support fragment")
}

fn narrow(
    theory: &Theory,
    producers: &Producers,
    narrower: &Narrower,
    region: &mut Region,
    knowledge: &mut crate::Knowledge,
    scratch: &mut NarrowingScratch,
) -> (Narrowing, NarrowingStatistics) {
    narrower
        .narrow_known(
            OriginalSubject::new(theory, Some(producers)),
            region,
            knowledge,
            scratch,
            RegionLimits::default(),
            &Cancellation::default(),
        )
        .unwrap()
}

/// The work of narrowing a held head with `bodies` producers, all but two of
/// whose bodies fail: each failure queues the head's recheck, which reads
/// every producer before reaching its two surviving witnesses.
fn held_head_work(bodies: usize) -> u64 {
    let theory = shared_head(bodies);
    let producers = producers(&theory);
    let narrower = Narrower::new(&theory);
    let mut region = Region::all_open(theory.atom_count());
    region.hold(0);
    // Keep the surviving witnesses last so each recheck needs the full scan.
    for body in 1..=bodies - 2 {
        region.cut(body);
    }
    let (narrowing, statistics) = narrow(
        &theory,
        &producers,
        &narrower,
        &mut region,
        &mut narrower.knowledge(),
        &mut NarrowingScratch::default(),
    );
    assert_eq!(narrowing, Narrowing::Fixed { changed: false });
    statistics.work
}

#[test]
fn a_repeatedly_queued_recheck_runs_once() {
    // One recheck per failed body makes the work quadratic in the bodies
    // (doubling them multiplies it by about 3.4 here); queued once, the
    // head's rechecks keep it linear.
    let (small, large) = (held_head_work(20), held_head_work(40));
    assert!(
        large * 2 < small * 5,
        "work {small} at 20 bodies, {large} at 40"
    );
}

#[test]
fn a_support_that_falls_after_a_recheck_is_rechecked() {
    let theory = shared_head(2);
    let producers = producers(&theory);
    let narrower = Narrower::new(&theory);
    let mut scratch = NarrowingScratch::default();
    let mut knowledge = narrower.knowledge();
    let mut region = Region::all_open(theory.atom_count());
    // The first narrowing rechecks the head: two producers support it.
    narrow(
        &theory,
        &producers,
        &narrower,
        &mut region,
        &mut knowledge,
        &mut scratch,
    );
    assert_eq!(region.decision(0), None);
    // Both bodies then fail, with the same scratch: the head loses its
    // support and must be rechecked again, and so cut.
    region.cut(1);
    region.cut(2);
    let (narrowing, _) = narrow(
        &theory,
        &producers,
        &narrower,
        &mut region,
        &mut knowledge,
        &mut scratch,
    );
    assert_eq!(narrowing, Narrowing::Fixed { changed: true });
    assert_eq!(region.decision(0), Some(false));
}

/// Isolate one support reduction from the surrounding formula propagation.
/// Atom nodes in these fixtures have their atom's index. A refused scan must
/// preserve every learned flag and publish no pending propagation.
fn support_attempt(
    theory: &Theory,
    held: &[usize],
    failed: &[usize],
    work: &mut Work<'_>,
) -> (Result<Step, Stop>, Vec<(usize, bool)>) {
    let index = Narrower::new(theory);
    let producers = producers(theory);
    let mut knowledge = index.knowledge();
    let mut lists = NarrowingScratch::default();
    lists.prepare(theory.atom_count());
    let mut closure = Closure {
        known: super::compact_mut(&mut knowledge),
        lists: &mut lists,
    };
    for &atom in held {
        assert!(matches!(
            closure.atom(&index, None, None, atom, true),
            Step::Changed
        ));
    }
    for &body in failed {
        assert!(matches!(closure.never(body), Step::Changed));
    }
    closure.lists.nodes.clear();
    closure.lists.learned.clear();
    let before = [
        closure.known.sure.to_vec(),
        closure.known.never.to_vec(),
        closure.known.atom_sure.to_vec(),
        closure.known.atom_never.to_vec(),
    ];
    let result = closure.recheck(&index, &producers, 0, work);
    if result.is_err() {
        assert_eq!(&*closure.known.sure, before[0].as_slice());
        assert_eq!(&*closure.known.never, before[1].as_slice());
        assert_eq!(&*closure.known.atom_sure, before[2].as_slice());
        assert_eq!(&*closure.known.atom_never, before[3].as_slice());
        assert!(closure.lists.nodes.is_empty());
        assert!(closure.lists.learned.is_empty());
        assert!(closure.lists.heads.is_empty());
        assert!(closure.lists.pending.iter().all(|word| *word == 0));
    }
    (result, std::mem::take(&mut closure.lists.nodes))
}

#[test]
fn an_open_head_stops_at_its_first_supporter() {
    let mut work = Work::new(1);
    let (result, learned) = support_attempt(&shared_head(8), &[], &[], &mut work);
    assert!(matches!(result, Ok(Step::Unchanged)));
    assert!(learned.is_empty());
    assert_eq!(work.spent, 1);
}

#[test]
fn a_held_head_stops_at_its_second_supporter() {
    let mut work = Work::new(2);
    let (result, learned) = support_attempt(&shared_head(8), &[0], &[], &mut work);
    assert!(matches!(result, Ok(Step::Unchanged)));
    assert!(learned.is_empty());
    assert_eq!(work.spent, 2);
}

#[test]
fn sole_support_requires_the_complete_scan() {
    let mut work = Work::new(4);
    let (result, learned) = support_attempt(&shared_head(4), &[0], &[1, 2, 3], &mut work);
    assert!(matches!(result, Ok(Step::Changed)));
    assert_eq!(learned, [(4, true)]);
    assert_eq!(work.spent, 4);
}

#[test]
fn an_unsupported_cut_requires_the_complete_scan() {
    let mut work = Work::new(4);
    let (result, learned) = support_attempt(&shared_head(4), &[], &[1, 2, 3, 4], &mut work);
    assert!(matches!(result, Ok(Step::Changed)));
    assert_eq!(learned, [(0, false)]);
    assert_eq!(work.spent, 4);
}

#[test]
fn a_held_unsupported_head_refutes_the_region() {
    let mut work = Work::new(4);
    let (result, learned) = support_attempt(&shared_head(4), &[0], &[1, 2, 3, 4], &mut work);
    assert!(matches!(result, Ok(Step::Contradiction)));
    assert!(learned.is_empty());
    assert_eq!(work.spent, 4);
}

fn support_theory(atoms: usize, nodes: Vec<Node>, roots: Vec<usize>) -> Theory {
    Theory::new(
        atoms,
        crate::FormulaParts::new(nodes, vec![]).unwrap(),
        roots,
        AdmissionLimits::default(),
    )
    .unwrap()
}

#[test]
fn duplicate_producers_remain_distinct_supporters() {
    let theory = support_theory(
        2,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::implies(1, 0),
            Node::implies(1, 0),
        ],
        vec![2, 3],
    );
    let mut work = Work::new(2);
    let (result, learned) = support_attempt(&theory, &[0], &[], &mut work);
    assert!(matches!(result, Ok(Step::Unchanged)));
    assert!(learned.is_empty());
    assert_eq!(work.spent, 2);
}

#[test]
fn a_competing_head_cannot_supply_support() {
    // c -> (a or b), d -> a: held b blocks the first producer of held a.
    let theory = support_theory(
        4,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::atom(2),
            Node::atom(3),
            Node::or_pair([0, 1]),
            Node::implies(2, 4),
            Node::implies(3, 0),
        ],
        vec![5, 6],
    );
    let mut work = Work::new(2);
    let (result, learned) = support_attempt(&theory, &[0, 1], &[], &mut work);
    assert!(matches!(result, Ok(Step::Changed)));
    assert_eq!(learned, [(3, true)]);
    assert_eq!(work.spent, 2);
}

#[test]
fn choice_support_prevents_a_spurious_sole_body() {
    // {a}, b -> a: the bodyless choice and ordinary rule both support a.
    let theory = support_theory(
        2,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::falsum(),
            Node::implies(0, 2),
            Node::or_pair([0, 3]),
            Node::implies(1, 0),
        ],
        vec![4, 5],
    );
    let mut work = Work::new(2);
    let (result, learned) = support_attempt(&theory, &[0], &[], &mut work);
    assert!(matches!(result, Ok(Step::Unchanged)));
    assert!(learned.is_empty());
    assert_eq!(work.spent, 2);
}

#[test]
fn a_bodyless_sole_support_forces_no_body() {
    let theory = support_theory(
        2,
        vec![Node::atom(0), Node::atom(1), Node::implies(1, 0)],
        vec![0, 2],
    );
    let mut work = Work::new(2);
    let (result, learned) = support_attempt(&theory, &[0], &[1], &mut work);
    assert!(matches!(result, Ok(Step::Unchanged)));
    assert!(learned.is_empty());
    assert_eq!(work.spent, 2);
}

#[test]
fn a_newly_held_head_rechecks_its_sole_support() {
    let theory = shared_head(3);
    let producers = producers(&theory);
    let narrower = Narrower::new(&theory);
    let mut region = Region::all_open(theory.atom_count());
    let mut knowledge = narrower.knowledge();
    let mut scratch = NarrowingScratch::default();
    region.cut(1);
    region.cut(2);
    narrow(
        &theory,
        &producers,
        &narrower,
        &mut region,
        &mut knowledge,
        &mut scratch,
    );
    assert_eq!(region.decision(0), None);
    assert_eq!(region.decision(3), None);
    region.hold(0);
    narrow(
        &theory,
        &producers,
        &narrower,
        &mut region,
        &mut knowledge,
        &mut scratch,
    );
    assert_eq!(region.decision(3), Some(true));
}

#[test]
fn later_support_withdrawal_forces_the_remaining_body() {
    let theory = shared_head(3);
    let producers = producers(&theory);
    let narrower = Narrower::new(&theory);
    let mut region = Region::all_open(theory.atom_count());
    let mut knowledge = narrower.knowledge();
    let mut scratch = NarrowingScratch::default();
    region.hold(0);
    narrow(
        &theory,
        &producers,
        &narrower,
        &mut region,
        &mut knowledge,
        &mut scratch,
    );
    assert_eq!(region.decision(3), None);
    region.cut(1);
    region.cut(2);
    narrow(
        &theory,
        &producers,
        &narrower,
        &mut region,
        &mut knowledge,
        &mut scratch,
    );
    assert_eq!(region.decision(3), Some(true));
}

#[test]
fn a_new_competing_head_rechecks_remaining_support() {
    // a or b; c -> a; {b}; {c}. Initially held a has two possible
    // producers. Holding b withdraws the disjunctive support and forces c.
    let theory = support_theory(
        3,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::atom(2),
            Node::falsum(),
            Node::or_pair([0, 1]),
            Node::implies(2, 0),
            Node::implies(1, 3),
            Node::or_pair([1, 6]),
            Node::implies(2, 3),
            Node::or_pair([2, 8]),
        ],
        vec![4, 5, 7, 9],
    );
    let producers = producers(&theory);
    let narrower = Narrower::new(&theory);
    let mut region = Region::all_open(theory.atom_count());
    let mut knowledge = narrower.knowledge();
    let mut scratch = NarrowingScratch::default();
    region.hold(0);
    narrow(
        &theory,
        &producers,
        &narrower,
        &mut region,
        &mut knowledge,
        &mut scratch,
    );
    assert_eq!(region.decision(1), None);
    assert_eq!(region.decision(2), None);
    region.hold(1);
    narrow(
        &theory,
        &producers,
        &narrower,
        &mut region,
        &mut knowledge,
        &mut scratch,
    );
    assert_eq!(region.decision(2), Some(true));
}

#[test]
fn support_refusal_retains_the_exact_charged_prefix() {
    let theory = shared_head(4);
    let cases: &[(&[usize], &[usize], u64)] = &[
        (&[], &[], 1),
        (&[0], &[], 2),
        (&[], &[1, 2, 3], 4),
        (&[0], &[1, 2, 3], 4),
        (&[], &[1, 2, 3, 4], 4),
    ];
    for &(held, failed, needed) in cases {
        for limit in 0..=needed {
            let mut work = Work::new(limit);
            let (result, _) = support_attempt(&theory, held, failed, &mut work);
            assert_eq!(work.spent, limit);
            if limit == needed {
                assert!(result.is_ok());
            } else {
                assert!(matches!(result, Err(Stop::WorkLimit)));
            }
        }
    }
}

struct SupportQuota {
    remaining: u64,
    batch: u64,
    failure: Stop,
    refunded: u64,
}

impl NarrowingQuota for SupportQuota {
    fn reserve(&mut self, wanted: u64) -> Result<u64, Stop> {
        if self.remaining == 0 {
            return Err(self.failure);
        }
        let granted = wanted.min(self.batch).min(self.remaining);
        self.remaining -= granted;
        Ok(granted)
    }

    fn refund(&mut self, unspent: u64) {
        self.refunded += unspent;
    }
}

#[test]
fn support_quota_stops_before_the_next_producer() {
    let theory = shared_head(4);
    for failure in [Stop::Cancelled, Stop::Deadline] {
        for permitted in 0..4 {
            let mut quota = SupportQuota {
                remaining: permitted,
                batch: 1,
                failure,
                refunded: 0,
            };
            let mut work = Work::reserved(&mut quota);
            let (result, _) = support_attempt(&theory, &[0], &[1, 2, 3], &mut work);
            assert!(matches!(result, Err(stop) if stop == failure));
            assert_eq!(work.spent, permitted);
            work.settle();
            assert_eq!(quota.remaining, 0);
            assert_eq!(quota.refunded, 0);
        }
    }
}

#[test]
fn support_saturation_refunds_unvisited_permits() {
    let theory = shared_head(8);
    for held in [&[][..], &[0][..]] {
        let mut quota = SupportQuota {
            remaining: 64,
            batch: 64,
            failure: Stop::WorkLimit,
            refunded: 0,
        };
        let mut work = Work::reserved(&mut quota);
        let (result, learned) = support_attempt(&theory, held, &[], &mut work);
        assert!(matches!(result, Ok(Step::Unchanged)));
        assert!(learned.is_empty());
        let spent = work.spent;
        work.settle();
        assert_eq!(quota.refunded, 64 - spent);
        assert_eq!(quota.remaining, 0);
    }
}
