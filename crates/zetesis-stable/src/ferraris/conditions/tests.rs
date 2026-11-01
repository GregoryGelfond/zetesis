use super::*;
use crate::budget::{Budget, LocalQuota};
use crate::ferraris::{Limits, StableModels};
use crate::{Cancellation, SearchLimits, SearchStatistics};
use zetesis_algebra::region::{Entry, Narrowing, Region};
use zetesis_ferraris::Node;
use zetesis_objective::PlanIdentity;
use zetesis_theory_support::theories::theory;

fn guard(conjunction: bool) -> Theory {
    theory(
        2,
        vec![
            Node::atom(0),
            Node::atom(1),
            if conjunction {
                Node::and_pair([0, 1])
            } else {
                Node::or_pair([0, 1])
            },
        ],
        vec![2],
    )
}

#[test]
fn a_new_generation_discards_old_dag_knowledge() {
    let original = theory(2, vec![], vec![]);
    let narrower = Narrower::new(&original);
    let mut conditions = Conditions::<(Theory, Narrower)> {
        bound: Some(
            Bound::prepare(&guard(false), 1, u64::MAX, &crate::Cancellation::default()).unwrap(),
        ),
        ..Conditions::default()
    };
    let mut entry = Entry::new(Region::all_open(2), CandidateKnowledge::default());
    let cancellation = Cancellation::default();
    let mut budget = Budget {
        quota: LocalQuota,
        limits: SearchLimits::default(),
        cancellation: &cancellation,
        statistics: SearchStatistics::default(),
    };
    let mut counts = super::super::regions::RegionCounts::default();
    super::super::regions::narrow(
        (
            zetesis_ferraris::OriginalSubject::new(&original, None),
            &narrower,
        ),
        &conditions,
        &mut entry,
        &mut zetesis_ferraris::NarrowingScratch::default(),
        &mut budget,
        &mut counts,
        None,
    )
    .unwrap();
    assert!(!entry.region.is_held(0) && !entry.region.is_held(1));
    // Equal node counts and coordinates are insufficient: OR and AND learn
    // different consequences even though both have already asserted root 2.
    conditions.bound =
        Some(Bound::prepare(&guard(true), 2, u64::MAX, &crate::Cancellation::default()).unwrap());
    assert_ne!(
        super::super::regions::narrow(
            (
                zetesis_ferraris::OriginalSubject::new(&original, None),
                &narrower
            ),
            &conditions,
            &mut entry,
            &mut zetesis_ferraris::NarrowingScratch::default(),
            &mut budget,
            &mut counts,
            None,
        )
        .unwrap(),
        Narrowing::Refuted
    );
    assert!(entry.region.is_held(0) && entry.region.is_held(1));
}

#[test]
fn only_active_snapshots_retain_a_superseded_bound() {
    let bound =
        Bound::prepare(&guard(false), 1, u64::MAX, &crate::Cancellation::default()).unwrap();
    let retired = Arc::downgrade(&bound.index);
    let mut knowledge = CandidateKnowledge::default();
    knowledge.bound(&bound).unwrap();
    let mut conditions = Conditions::<Arc<(Theory, Narrower)>> {
        bound: Some(bound),
        ..Conditions::default()
    };
    let active = conditions.clone();
    conditions.bound =
        Some(Bound::prepare(&guard(true), 2, u64::MAX, &crate::Cancellation::default()).unwrap());
    assert!(retired.upgrade().is_some());
    drop(active);
    assert!(retired.upgrade().is_none());
    // Mutable knowledge can remain queued without retaining the retired DAG.
    assert_eq!(knowledge.bound_generation.unwrap(), 1);
}

#[test]
fn bound_knowledge_allocations_are_included_once() {
    let bound =
        Bound::prepare(&guard(false), 1, u64::MAX, &crate::Cancellation::default()).unwrap();
    let mut knowledge = CandidateKnowledge::default();
    let payload =
        knowledge.bound(&bound).unwrap().retained_bytes() - size_of::<Knowledge>() as u128;
    let capacity = knowledge.entries.capacity();
    assert_eq!(
        knowledge.allocated_bytes(),
        capacity as u128 * size_of::<Knowledge>() as u128 + payload
    );
}

#[test]
fn generation_overflow_preserves_the_active_bound() {
    let original = theory(
        1,
        vec![
            Node::atom(0),
            Node::falsum(),
            Node::implies(0, 1),
            Node::or_pair([0, 2]),
        ],
        vec![3],
    );
    let mut search =
        StableModels::new(&original, Limits::default(), Cancellation::default()).unwrap();
    let plan = PlanIdentity::fresh();
    let first = theory(1, vec![Node::atom(0)], vec![0]);
    bound(&mut search, plan, 1, &first).unwrap();
    search.bound_generation = u64::MAX;
    let before = search.statistics();
    let stronger = theory(1, vec![Node::falsum()], vec![0]);
    assert_eq!(
        bound(&mut search, plan, 0, &stronger),
        Err(Incomplete::CounterOverflow)
    );
    assert_eq!(search.bound_generation, u64::MAX);
    assert_eq!(
        search.statistics().candidate_restrictions,
        before.candidate_restrictions
    );
    assert_eq!(
        search.next().unwrap().unwrap().atoms().collect::<Vec<_>>(),
        vec![0]
    );
    assert!(search.next().is_none());
    assert!(search.exhausted());
}

/// Install `restriction` as `plan`'s candidate bound at cost `value`.
fn bound(
    search: &mut StableModels,
    plan: PlanIdentity,
    value: i64,
    restriction: &Theory,
) -> Result<(), Incomplete> {
    let score = zetesis_objective::Score::from_costs(vec![(0, value)]).unwrap();
    search.improve_candidate_bound(plan, &score, restriction)
}

fn search(
    route: usize,
    original: &Theory,
    limits: Limits,
    cancellation: Cancellation,
) -> StableModels {
    let workers = std::num::NonZeroUsize::new(2).unwrap();
    match route {
        0 => StableModels::new(original, limits, cancellation),
        1 => StableModels::with_region_producers(original, workers, limits, cancellation),
        // Workers started at the first pull, then a walk that never starts them.
        2 => StableModels::with_region_workers_engaging(
            original,
            workers,
            super::super::Engagement::Immediate,
            limits,
            cancellation,
        ),
        3 => StableModels::with_region_workers_engaging(
            original,
            workers,
            super::super::Engagement::After(std::time::Duration::MAX),
            limits,
            cancellation,
        ),
        _ => unreachable!(),
    }
    .unwrap()
}

#[test]
fn refused_preparation_does_not_advance_the_bound_generation() {
    let original = theory(2, vec![], vec![]);
    let first = guard(false);
    for route in 0..4 {
        let plan = PlanIdentity::fresh();
        let mut measured = search(route, &original, Limits::default(), Cancellation::default());
        bound(&mut measured, plan, 1, &first).unwrap();
        let ceiling = measured.statistics().search.work;
        let mut limits = Limits::default();
        limits.search.max_work = ceiling;
        let mut limited = search(route, &original, limits, Cancellation::default());
        bound(&mut limited, plan, 1, &first).unwrap();
        assert_eq!(
            bound(&mut limited, plan, 0, &guard(true)),
            Err(Incomplete::WorkLimit)
        );
        assert_eq!(limited.bound_generation, 1);
        assert_eq!(limited.statistics().candidate_restrictions, 1);
        assert!(!limited.exhausted());
    }
}

#[test]
fn cancellation_does_not_advance_the_bound_generation() {
    let original = theory(2, vec![], vec![]);
    for route in 0..4 {
        let cancellation = Cancellation::default();
        let plan = PlanIdentity::fresh();
        let mut search = search(route, &original, Limits::default(), cancellation.clone());
        bound(&mut search, plan, 1, &guard(false)).unwrap();
        let before = search.statistics().search.work;
        cancellation.cancel();
        assert_eq!(
            bound(&mut search, plan, 0, &guard(true)),
            Err(Incomplete::Cancelled)
        );
        assert_eq!(search.bound_generation, 1);
        assert_eq!(search.statistics().search.work, before);
        assert_eq!(search.statistics().candidate_restrictions, 1);
        assert!(!search.exhausted());
    }
}

#[test]
fn a_late_permanent_condition_keeps_bound_knowledge_separate() {
    let original = theory(2, vec![], vec![]);
    let narrower = Narrower::new(&original);
    let mut conditions = Conditions::<(Theory, Narrower)> {
        bound: Some(
            Bound::prepare(&guard(false), 1, u64::MAX, &crate::Cancellation::default()).unwrap(),
        ),
        ..Conditions::default()
    };
    let mut entry = Entry::new(Region::all_open(2), CandidateKnowledge::default());
    let cancellation = Cancellation::default();
    let mut budget = Budget {
        quota: LocalQuota,
        limits: SearchLimits::default(),
        cancellation: &cancellation,
        statistics: SearchStatistics::default(),
    };
    let mut counts = super::super::regions::RegionCounts::default();
    super::super::regions::narrow(
        (
            zetesis_ferraris::OriginalSubject::new(&original, None),
            &narrower,
        ),
        &conditions,
        &mut entry,
        &mut zetesis_ferraris::NarrowingScratch::default(),
        &mut budget,
        &mut counts,
        None,
    )
    .unwrap();
    let permanent = theory(
        2,
        vec![Node::atom(0), Node::falsum(), Node::implies(0, 1)],
        vec![2],
    );
    let permanent_index = Narrower::new(&permanent);
    conditions.permanent.push((permanent, permanent_index));
    super::super::regions::narrow(
        (
            zetesis_ferraris::OriginalSubject::new(&original, None),
            &narrower,
        ),
        &conditions,
        &mut entry,
        &mut zetesis_ferraris::NarrowingScratch::default(),
        &mut budget,
        &mut counts,
        None,
    )
    .unwrap();
    assert!(entry.region.is_cut(0) && entry.region.is_held(1));
}

#[test]
fn allocated_bytes_include_unused_knowledge_slots() {
    let bound =
        Bound::prepare(&guard(false), 1, u64::MAX, &crate::Cancellation::default()).unwrap();
    let mut knowledge = CandidateKnowledge::default();
    knowledge.permanent(0, &bound.index.1).unwrap();
    knowledge.entries.reserve_exact(7);
    let expected = knowledge.entries[0].retained_bytes()
        + (knowledge.entries.capacity() - 1) as u128 * size_of::<Knowledge>() as u128;
    assert_eq!(knowledge.allocated_bytes(), expected);
}

#[test]
fn the_first_knowledge_slot_is_reserved_exactly() {
    // A root that knows only the original theory holds one slot, as a
    // one-element vector does; the frontier's byte receipts count it. This
    // pins `try_reserve_exact`, whose contract permits a larger capacity:
    // should the standard library ever give one, this fails loudly rather
    // than letting the receipts drift.
    let original = guard(false);
    let narrower = Narrower::new(&original);
    let mut knowledge = CandidateKnowledge::default();
    knowledge.permanent(0, &narrower).unwrap();
    assert_eq!(knowledge.entries.capacity(), 1);
    let permanent = guard(true);
    knowledge.permanent(1, &Narrower::new(&permanent)).unwrap();
    assert_eq!(knowledge.entries.len(), 2);
}

#[test]
fn copying_knowledge_replaces_the_bound_generation() {
    let bound =
        Bound::prepare(&guard(false), 1, u64::MAX, &crate::Cancellation::default()).unwrap();
    let mut source = CandidateKnowledge::default();
    source.bound(&bound).unwrap();
    let mut destination = CandidateKnowledge::default();
    destination
        .bound(&Bound::prepare(&guard(true), 2, u64::MAX, &crate::Cancellation::default()).unwrap())
        .unwrap();
    destination.clone_from(&source);
    assert_eq!(destination.bound_generation, source.bound_generation);

    source = CandidateKnowledge::default();
    destination.clone_from(&source);
    assert_eq!(destination.bound_generation, None);
    assert!(destination.entries.is_empty());
}

#[test]
fn copying_knowledge_does_not_retain_surplus_capacity() {
    let narrower = Narrower::new(&guard(false));
    let mut source = CandidateKnowledge::default();
    source.permanent(0, &narrower).unwrap();
    let mut destination = source.clone();
    destination.entries.reserve_exact(7);
    assert!(destination.entries.capacity() > source.entries.len());
    destination.clone_from(&source);
    assert_eq!(
        destination.allocated_bytes(),
        source.clone().allocated_bytes()
    );
}

#[test]
fn compatible_copies_reuse_the_knowledge_vector() {
    let narrower = Narrower::new(&guard(false));
    let mut source = CandidateKnowledge::default();
    source.permanent(0, &narrower).unwrap();
    let mut destination = source.clone();
    let before = destination.entries.as_ptr();
    destination.clone_from(&source);
    assert_eq!(destination.entries.as_ptr(), before);
    assert_ne!(destination.entries.as_ptr(), source.entries.as_ptr());
}

#[test]
fn candidate_capacity_and_entry_policy_are_counted_once() {
    use std::collections::VecDeque;
    use std::sync::Mutex;
    use zetesis_algebra::walk::frontier::Payload;

    use super::super::region_frontier::{RegionFrontier, reserve_regions};

    let original = guard(false);
    let narrower = Narrower::new(&original);
    let bound = Bound::prepare(&guard(true), 1, u64::MAX, &Cancellation::default()).unwrap();
    let mut known = CandidateKnowledge::default();
    let permanent_payload =
        known.permanent(0, &narrower).unwrap().retained_bytes() - size_of::<Knowledge>() as u128;
    let bound_payload =
        known.bound(&bound).unwrap().retained_bytes() - size_of::<Knowledge>() as u128;
    // Deliberately retain unused slot capacity; only initialized knowledge owns
    // subordinate allocations. The bound is still the last initialized slot.
    known.entries.reserve_exact(7);
    assert!(known.entries.capacity() > known.entries.len());
    let known_payload = known.entries.capacity() as u128 * size_of::<Knowledge>() as u128
        + permanent_payload
        + bound_payload;
    assert_eq!(known.allocated_bytes(), known_payload);
    let mut entry = Entry::new(Region::all_open(2), known);
    entry.prefer(1);
    let payload = 2 * size_of::<u64>() as u128 + known_payload;
    assert_eq!(entry.allocated_bytes(), payload);
    let frontier = RegionFrontier::measured(2, entry).unwrap();
    let headers = (size_of::<Vec<Mutex<VecDeque<Entry<CandidateKnowledge>>>>>()
        + frontier.queues.capacity() * size_of::<Mutex<VecDeque<Entry<CandidateKnowledge>>>>())
        as u128;
    let initial = frontier.statistics().unwrap();
    assert_eq!(
        initial.retained_bytes,
        headers
            + initial.capacity as u128 * size_of::<Entry<CandidateKnowledge>>() as u128
            + payload
    );
    let hand = frontier.take_local(0).unwrap();
    let empty = frontier.statistics().unwrap();
    assert_eq!(empty.regions, 0);
    assert_eq!(empty.capacity, initial.capacity);
    assert_eq!(empty.retained_bytes, initial.retained_bytes - payload);
    assert_eq!(empty.peak_retained_bytes, initial.retained_bytes);
    assert_eq!(hand.split_atom(), Some(1));
    frontier.requeue(1, hand, reserve_regions).unwrap();
    let moved = frontier.statistics().unwrap();
    assert_eq!(moved.regions, 1);
    assert_eq!(
        moved.capacity,
        frontier.queue(0).capacity() + frontier.queue(1).capacity()
    );
    assert_eq!(
        moved.retained_bytes,
        headers + moved.capacity as u128 * size_of::<Entry<CandidateKnowledge>>() as u128 + payload
    );
    assert_eq!(
        moved.peak_retained_bytes,
        initial.retained_bytes.max(moved.retained_bytes)
    );
    frontier.release();
    let released = frontier.statistics().unwrap();
    assert_eq!((released.regions, released.capacity), (0, 0));
    assert_eq!(released.retained_bytes, headers);
    assert_eq!(released.peak_retained_bytes, moved.peak_retained_bytes);
}
