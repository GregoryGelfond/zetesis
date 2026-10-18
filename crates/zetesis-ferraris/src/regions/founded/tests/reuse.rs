//! Only successfully closed inherited knowledge can omit a foundation pass.

use super::*;
use crate::regions::Width;
use crate::{Knowledge, NarrowingStatistics};
use std::collections::BTreeSet;

mod cyclic;
mod output;
mod projection;

struct Program {
    owner: Theory,
    index: Narrower,
    producers: Producers,
}

impl Program {
    fn new(owner: Theory) -> Self {
        Self {
            index: Narrower::new(&owner),
            producers: producers(&owner),
            owner,
        }
    }

    fn attempt(
        &self,
        region: &mut Region,
        knowledge: &mut Knowledge,
        scratch: &mut NarrowingScratch,
        quota: &mut dyn NarrowingQuota,
    ) -> NarrowingAttempt {
        self.index.propagate_known_reserved(
            OriginalSubject::new(&self.owner, Some(&self.producers)),
            region,
            knowledge,
            scratch,
            &Cancellation::default(),
            quota,
        )
    }

    fn close(
        &self,
        region: &mut Region,
        knowledge: &mut Knowledge,
        scratch: &mut NarrowingScratch,
    ) -> (Narrowing, NarrowingStatistics) {
        let attempt = self.attempt(region, knowledge, scratch, &mut Allowance(u64::MAX));
        (attempt.result.unwrap(), attempt.statistics)
    }

    fn closed(&self) -> (Region, Knowledge, NarrowingScratch) {
        let mut region = Region::all_open(self.owner.atom_count());
        let mut knowledge = self.index.knowledge();
        let mut scratch = NarrowingScratch::default();
        assert!(matches!(
            self.close(&mut region, &mut knowledge, &mut scratch).0,
            Narrowing::Fixed { .. }
        ));
        assert!(completed(&knowledge));
        (region, knowledge, scratch)
    }

    fn candidates(&self, force_recomputation: bool) -> (BTreeSet<usize>, usize) {
        let mut pending = vec![(
            Region::all_open(self.owner.atom_count()),
            self.index.knowledge(),
        )];
        let mut scratch = NarrowingScratch::default();
        let mut candidates = BTreeSet::new();
        while let Some((mut region, mut knowledge)) = pending.pop() {
            if force_recomputation {
                invalidate(&mut knowledge);
            }
            if self.close(&mut region, &mut knowledge, &mut scratch).0 == Narrowing::Refuted {
                continue;
            }
            let open = region.open().next();
            if let Some(atom) = open {
                let (cut, held) = region.split(atom);
                pending.push((cut, knowledge.clone()));
                pending.push((held, knowledge));
            } else {
                let mask = (0..self.owner.atom_count())
                    .filter(|&atom| region.is_held(atom))
                    .fold(0, |mask, atom| mask | (1 << atom));
                assert!(candidates.insert(mask));
            }
        }
        (candidates, scratch.founded.passes)
    }
}

fn completed(knowledge: &Knowledge) -> bool {
    match &knowledge.width {
        Width::Compact16(known) => known.founded_closed,
        Width::Compact32(known) => known.founded_closed,
        Width::Native(known) => known.founded_closed,
    }
}

fn invalidate(knowledge: &mut Knowledge) {
    match &mut knowledge.width {
        Width::Compact16(known) => known.founded_closed = false,
        Width::Compact32(known) => known.founded_closed = false,
        Width::Native(known) => known.founded_closed = false,
    }
}

/// a or b; a <- b; b <- a. Holding an atom learns only true nodes.
fn seeded_cycle() -> Theory {
    theory(
        2,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::or_pair([0, 1]),
            Node::implies(0, 1),
            Node::implies(1, 0),
        ],
        vec![2, 3, 4],
    )
}

/// {c}; a <- b; b <- a; a <- not c. Holding c falsifies the only
/// external body but cuts no atom until possible-foundedness runs again.
fn negative_seed() -> Theory {
    theory(
        3,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::atom(2),
            Node::falsum(),
            Node::implies(2, 3),
            Node::or_pair([2, 4]),
            Node::implies(1, 0),
            Node::implies(0, 1),
            Node::implies(4, 0),
        ],
        vec![5, 6, 7, 8],
    )
}

#[test]
fn unchanged_knowledge_omits_repeated_passes() {
    let program = Program::new(seeded_cycle());
    let (mut region, mut knowledge, mut scratch) = program.closed();
    assert_eq!(scratch.founded.passes, 1);
    let bytes = knowledge.retained_bytes();
    for _ in 0..3 {
        let (result, statistics) = program.close(&mut region, &mut knowledge, &mut scratch);
        assert_eq!(result, Narrowing::Fixed { changed: false });
        assert_eq!(statistics.work, 0);
        assert_eq!(scratch.founded.passes, 1);
    }
    let mut child = region;
    assert!(child.hold(0));
    let mut inherited = knowledge.clone();
    let mut other_worker = NarrowingScratch::default();
    program.close(&mut child, &mut inherited, &mut other_worker);
    assert!(child.is_held(0) && child.is_held(1));
    assert_eq!(other_worker.founded.passes, 0);
    assert_eq!(other_worker.founded.retained_bytes(), 0);
    assert_eq!(inherited.retained_bytes(), bytes);
}

#[test]
fn a_held_atom_can_invalidate_completed_foundation() {
    let program = Program::new(negative_seed());
    let (mut child, mut knowledge, mut scratch) = program.closed();
    assert_eq!(scratch.founded.passes, 1);
    assert!(child.hold(2));
    let (result, _) = program.close(&mut child, &mut knowledge, &mut scratch);
    assert_eq!(result, Narrowing::Fixed { changed: true });
    assert!(child.is_cut(0) && child.is_cut(1) && child.is_held(2));
    // The first changed pass cuts the cycle; a second pass completes after
    // ordinary propagation incorporates those cuts and their false nodes.
    assert_eq!(scratch.founded.passes, 3);
    assert!(completed(&knowledge));
}

#[test]
fn recycled_siblings_receive_only_parent_authority() {
    let program = Program::new(negative_seed());
    let (parent, knowledge, mut scratch) = program.closed();
    let mut recycled = knowledge.clone();
    for held in [true, false, true] {
        let mut child = parent.clone();
        if held {
            child.hold(2);
        } else {
            child.cut(2);
        }
        recycled.clone_from(&knowledge);
        let before = scratch.founded.passes;
        program.close(&mut child, &mut recycled, &mut scratch);
        if held {
            assert!(scratch.founded.passes > before);
        } else {
            assert_eq!(scratch.founded.passes, before);
        }
        assert_eq!(child.decision(0), Some(!held));
        assert_eq!(child.decision(1), Some(!held));
        assert_eq!(child.decision(2), Some(held));
        assert!(completed(&knowledge) && completed(&recycled));
    }
}

#[test]
fn foreign_worker_scratch_supplies_no_completion() {
    let first = Program::new(seeded_cycle());
    let (mut first_region, first_known, mut scratch) = first.closed();
    let second = Program::new(cycle());
    let mut second_region = Region::all_open(2);
    let mut recycled = first_known.clone();
    recycled.clone_from(&second.index.knowledge());
    assert!(!completed(&recycled));
    let before = scratch.founded.passes;
    second.close(&mut second_region, &mut recycled, &mut scratch);
    assert!(second_region.is_cut(0) && second_region.is_cut(1));
    assert_eq!(scratch.founded.passes, before + 2);
    assert!(
        scratch
            .founded
            .owner
            .as_ref()
            .unwrap()
            .same_instance(&second.owner)
    );
    // Restoring the first subject's inherited knowledge needs no support
    // values from a worker whose last completed pass belonged to the second.
    recycled.clone_from(&first_known);
    first_region.hold(0);
    let before = scratch.founded.passes;
    first.close(&mut first_region, &mut recycled, &mut scratch);
    assert!(first_region.is_held(0) && first_region.is_held(1));
    assert_eq!(scratch.founded.passes, before);
    // The new narrowing discards foreign projection authority even though
    // inherited completion itself survives and needs no foundation pass.
    assert!(scratch.founded.owner.is_none());
}

#[test]
fn inherited_families_match_forced_recomputation() {
    let mut saved = 0;
    for owner in [seeded_cycle(), negative_seed(), projected_bodies(), cycle()] {
        let program = Program::new(owner);
        let (actual, passes) = program.candidates(false);
        let (reference, repeated) = program.candidates(true);
        assert_eq!(actual, reference);
        assert!(passes <= repeated);
        saved += repeated - passes;
        let expected: BTreeSet<_> = (0..1 << program.owner.atom_count())
            .filter(|&mask| accepted(&program.owner, mask))
            .collect();
        let answers: BTreeSet<_> = actual
            .into_iter()
            .filter(|&mask| accepted(&program.owner, mask))
            .collect();
        assert_eq!(answers, expected);
    }
    assert!(saved > 0, "the family comparison exercised an omitted pass");
}

struct RefuseAfter {
    remaining: u64,
    failure: Stop,
}

impl NarrowingQuota for RefuseAfter {
    fn reserve(&mut self, _wanted: u64) -> Result<u64, Stop> {
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

#[test]
fn refused_passes_do_not_publish_completion() {
    let program = Program::new(negative_seed());
    let (parent, inherited, mut scratch) = program.closed();
    let mut expected = parent.clone();
    expected.hold(2);
    let mut expected_known = inherited.clone();
    let (expected_result, statistics) =
        program.close(&mut expected, &mut expected_known, &mut scratch);
    let mut interrupted_passes = 0;
    for failure in [Stop::WorkLimit, Stop::Cancelled, Stop::Deadline] {
        for ceiling in 0..statistics.work {
            let (_, _, mut scratch) = program.closed();
            let before = scratch.founded.passes;
            let mut input = parent.clone();
            input.hold(2);
            let mut partial = inherited.clone();
            let attempt = program.attempt(
                &mut input,
                &mut partial,
                &mut scratch,
                &mut RefuseAfter {
                    remaining: ceiling,
                    failure,
                },
            );
            assert_eq!(attempt.result, Err(failure));
            assert_eq!(attempt.statistics.work, ceiling);
            if scratch.founded.passes > before {
                assert!(!completed(&partial));
                interrupted_passes += 1;
            }
            // Failed knowledge is discarded. Scratch retains capacity only;
            // a complete retry starts from the parent's successful knowledge.
            let mut retry = parent.clone();
            retry.hold(2);
            partial.clone_from(&inherited);
            let (actual, _) = program.close(&mut retry, &mut partial, &mut scratch);
            assert_eq!(actual, expected_result);
            assert_eq!(retry, expected);
            assert!(completed(&partial));
        }
    }
    assert!(interrupted_passes > 0);
}

#[test]
fn inherited_completion_still_polls_cancellation() {
    let program = Program::new(seeded_cycle());
    let (mut region, mut knowledge, mut scratch) = program.closed();
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let attempt = program.index.propagate_known_reserved(
        OriginalSubject::new(&program.owner, Some(&program.producers)),
        &mut region,
        &mut knowledge,
        &mut scratch,
        &cancellation,
        &mut Allowance(0),
    );
    assert_eq!(attempt.result, Err(Stop::Cancelled));
    assert_eq!(attempt.statistics.work, 0);
    assert_eq!(scratch.founded.passes, 1);
}
