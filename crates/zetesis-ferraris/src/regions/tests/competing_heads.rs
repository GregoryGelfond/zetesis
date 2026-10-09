//! A singleton head has no competing support to wake.

use zetesis_cpu::{Cancellation, Stop};

use crate::{
    AdmissionLimits, FormulaParts, Knowledge, Narrower, Narrowing, NarrowingAttempt,
    NarrowingQuota, NarrowingScratch, Node, OriginalSubject, Producers, Region, RegionLimits,
    Theory,
};

use super::super::{Closure, SplitSelection, Step, Subject, Work};
use super::{
    compact_mut,
    counters::{native, same_knowledge},
    shared_occurrences,
};

fn singleton_heads() -> Theory {
    // Three choices, duplicate rules b -> a, and c -> (a or a) with distinct
    // nodes naming a. Head extraction, not syntactic disjunction, decides
    // whether a different atom can compete.
    Theory::new(
        3,
        FormulaParts::new(
            vec![
                Node::atom(0),
                Node::atom(1),
                Node::atom(2),
                Node::atom(0),
                Node::falsum(),
                Node::implies(0, 4),
                Node::or_pair([0, 5]),
                Node::implies(1, 4),
                Node::or_pair([1, 7]),
                Node::implies(2, 4),
                Node::or_pair([2, 9]),
                Node::implies(1, 0),
                Node::implies(1, 0),
                Node::or_pair([0, 3]),
                Node::implies(2, 13),
            ],
            vec![],
        )
        .unwrap(),
        vec![6, 8, 10, 11, 12, 14],
        AdmissionLimits::default(),
    )
    .unwrap()
}

fn extract(theory: &Theory) -> Producers {
    super::super::producers(theory, RegionLimits::default(), &Cancellation::default())
        .unwrap()
        .producers
        .unwrap()
}

#[test]
fn only_distinct_extracted_heads_enable_competition() {
    let single = extract(&singleton_heads());
    assert!(!single.has_competing_heads);
    assert_eq!(single.rules.len(), 6);
    assert!(single.rules.iter().all(|rule| rule.heads.len() == 1));
    let multi = extract(&shared_occurrences());
    assert!(multi.has_competing_heads);
    assert_eq!(multi.rules[0].heads, [0, 1]);
    // Applicability belongs to each immutable producer set, not the last
    // theory prepared or the theory size; cloning preserves that boundary.
    assert!(!single.clone().has_competing_heads);
    assert!(multi.clone().has_competing_heads);
}

struct Snapshot {
    attempt: NarrowingAttempt,
    region: Region,
    knowledge: Knowledge,
    scratch: NarrowingScratch,
}

fn narrow(
    theory: &Theory,
    index: &Narrower,
    producers: &Producers,
    initial: (&Region, &Knowledge),
    work: Work<'_>,
) -> Snapshot {
    let mut region = initial.0.clone();
    let mut knowledge = initial.1.clone();
    let mut scratch = NarrowingScratch::default();
    let subject = Subject::from(OriginalSubject::new(theory, Some(producers)));
    let attempt = index.narrow_with(
        (subject, SplitSelection::Choose),
        &mut region,
        &mut knowledge,
        &mut scratch,
        work,
        &Cancellation::default(),
    );
    Snapshot {
        attempt,
        region,
        knowledge,
        scratch,
    }
}

fn same(left: &Snapshot, right: &Snapshot) {
    assert_eq!(left.attempt.result, right.attempt.result);
    assert_eq!(left.attempt.statistics, right.attempt.statistics);
    assert_eq!(left.region, right.region);
    same_knowledge(&left.knowledge, &right.knowledge);
    assert_eq!(left.scratch.learned, right.scratch.learned);
    assert_eq!(left.scratch.nodes, right.scratch.nodes);
    assert_eq!(left.scratch.heads, right.scratch.heads);
    assert_eq!(left.scratch.pending, right.scratch.pending);
}

fn region(mut code: usize) -> Region {
    let mut region = Region::all_open(3);
    for atom in 0..3 {
        match code % 3 {
            1 => assert!(region.cut(atom)),
            2 => assert!(region.hold(atom)),
            _ => {}
        }
        code /= 3;
    }
    region
}

#[test]
fn single_head_guard_preserves_complete_closure() {
    let theory = singleton_heads();
    let index = Narrower::new(&theory);
    let producers = extract(&theory);
    let mut scanning = producers.clone();
    // A conservative true flag runs the previous competitor scan verbatim.
    scanning.has_competing_heads = true;
    let compact = index.knowledge();
    for knowledge in [compact.clone(), native(&compact)] {
        for code in 0..27 {
            let region = region(code);
            same(
                &narrow(
                    &theory,
                    &index,
                    &producers,
                    (&region, &knowledge),
                    Work::new(u64::MAX),
                ),
                &narrow(
                    &theory,
                    &index,
                    &scanning,
                    (&region, &knowledge),
                    Work::new(u64::MAX),
                ),
            );
        }
    }
}

#[test]
fn single_head_guard_preserves_inherited_children() {
    let theory = singleton_heads();
    let index = Narrower::new(&theory);
    let producers = extract(&theory);
    let mut scanning = producers.clone();
    scanning.has_competing_heads = true;
    let parent = narrow(
        &theory,
        &index,
        &producers,
        (&Region::all_open(3), &index.knowledge()),
        Work::new(u64::MAX),
    );
    assert!(matches!(parent.attempt.result, Ok(Narrowing::Fixed { .. })));
    for held in [false, true] {
        let mut child = parent.region.clone();
        assert!(child.is_open(1));
        assert!(if held { child.hold(1) } else { child.cut(1) });
        same(
            &narrow(
                &theory,
                &index,
                &producers,
                (&child, &parent.knowledge),
                Work::new(u64::MAX),
            ),
            &narrow(
                &theory,
                &index,
                &scanning,
                (&child, &parent.knowledge),
                Work::new(u64::MAX),
            ),
        );
    }
}

struct Quota {
    remaining: u64,
    stop: Stop,
    requests: Vec<u64>,
    refunded: u64,
}

impl NarrowingQuota for Quota {
    fn reserve(&mut self, wanted: u64) -> Result<u64, Stop> {
        self.requests.push(wanted);
        if self.remaining == 0 {
            return Err(self.stop);
        }
        self.remaining -= 1;
        Ok(1)
    }

    fn refund(&mut self, unspent: u64) {
        self.refunded += unspent;
    }
}

#[test]
fn single_head_guard_preserves_refused_prefixes() {
    let theory = singleton_heads();
    let index = Narrower::new(&theory);
    let producers = extract(&theory);
    let mut scanning = producers.clone();
    scanning.has_competing_heads = true;
    let mut region = Region::all_open(3);
    assert!(region.hold(1));
    let knowledge = index.knowledge();
    let initial = (&region, &knowledge);
    let complete = narrow(&theory, &index, &producers, initial, Work::new(u64::MAX));
    assert!(matches!(
        complete.attempt.result,
        Ok(Narrowing::Fixed { .. })
    ));
    let required = complete.attempt.statistics.work;
    assert!(required > 0);
    for stop in [Stop::WorkLimit, Stop::Cancelled, Stop::Deadline] {
        for limit in 0..=required {
            let mut guarded = Quota {
                remaining: limit,
                stop,
                requests: vec![],
                refunded: 0,
            };
            let mut reference = Quota {
                remaining: limit,
                stop,
                requests: vec![],
                refunded: 0,
            };
            let actual = narrow(
                &theory,
                &index,
                &producers,
                initial,
                Work::reserved(&mut guarded),
            );
            let expected = narrow(
                &theory,
                &index,
                &scanning,
                initial,
                Work::reserved(&mut reference),
            );
            same(&actual, &expected);
            assert_eq!(actual.attempt.statistics.work, limit);
            if limit < required {
                assert_eq!(actual.attempt.result, Err(stop));
            } else {
                assert_eq!(actual.attempt.result, complete.attempt.result);
            }
            assert_eq!(guarded.requests, reference.requests);
            assert_eq!(guarded.remaining, reference.remaining);
            assert_eq!(guarded.refunded, reference.refunded);
        }
    }
}

#[test]
fn multiple_heads_keep_ordered_competitor_wakeups() {
    let theory = shared_occurrences();
    let index = Narrower::new(&theory);
    let producers = extract(&theory);
    assert!(producers.has_competing_heads);
    let mut knowledge = index.knowledge();
    let mut lists = NarrowingScratch::default();
    lists.prepare(theory.atom_count());
    let mut closure = Closure {
        known: compact_mut(&mut knowledge),
        lists: &mut lists,
    };
    assert!(matches!(closure.sure(0), Step::Changed));
    let mut work = Work::new(0);
    assert!(matches!(
        closure
            .teach_operands(
                OriginalSubject::new(&theory, Some(&producers)).into(),
                &index,
                0,
                &mut work,
            )
            .unwrap(),
        Step::Changed
    ));
    // Competitor b precedes a's own newly-held support check. Both nodes
    // naming a become true, and the scan adds no charged operation.
    assert_eq!(closure.lists.heads, [1, 0]);
    assert_eq!(closure.lists.nodes, [(0, true), (1, true)]);
    assert_eq!(closure.lists.learned, [0]);
    assert_eq!(closure.lists.pending, [3]);
    assert_eq!(work.spent, 0);
}
