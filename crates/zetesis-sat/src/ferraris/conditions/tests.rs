use super::*;
use crate::ferraris::{Limits, StableModels};
use crate::search::{Budget, LocalQuota};
use crate::{Cancellation, SearchLimits, SearchStatistics};
use zetesis_cpu::regions::{Narrowing, Region};
use zetesis_ferraris::Node;
use zetesis_theory_support::theories::theory;

fn guard(conjunction: bool) -> Theory {
    theory(
        2,
        vec![
            Node::Atom(0),
            Node::Atom(1),
            if conjunction {
                Node::And(0, 1)
            } else {
                Node::Or(0, 1)
            },
        ],
        vec![2],
    )
}

#[test]
fn a_new_generation_discards_old_dag_knowledge() {
    let original = theory(2, vec![], vec![]);
    let narrower = Narrower::new(&original);
    let mut knowledge = CandidateKnowledge::default();
    let mut conditions = Conditions::<(Theory, Narrower)> {
        bound: Some(Bound::prepare(&guard(false), 1).unwrap()),
        ..Conditions::default()
    };
    let mut region = Region::all_open(2);
    let cancellation = Cancellation::default();
    let mut budget = Budget {
        quota: LocalQuota,
        limits: SearchLimits::default(),
        cancellation: &cancellation,
        statistics: SearchStatistics::default(),
    };
    let mut counts = super::super::regions::RegionCounts::default();
    super::super::regions::narrow(
        (&original, &narrower, None),
        &conditions,
        &mut region,
        &mut knowledge,
        &mut zetesis_ferraris::NarrowingScratch::default(),
        &mut budget,
        &mut counts,
    )
    .unwrap();
    assert!(!region.is_held(0) && !region.is_held(1));
    // Equal node counts and coordinates are insufficient: OR and AND learn
    // different consequences even though both have already asserted root 2.
    conditions.bound = Some(Bound::prepare(&guard(true), 2).unwrap());
    assert_ne!(
        super::super::regions::narrow(
            (&original, &narrower, None),
            &conditions,
            &mut region,
            &mut knowledge,
            &mut zetesis_ferraris::NarrowingScratch::default(),
            &mut budget,
            &mut counts
        )
        .unwrap(),
        Narrowing::Refuted
    );
    assert!(region.is_held(0) && region.is_held(1));
}

#[test]
fn only_active_snapshots_retain_a_superseded_bound() {
    let bound = Bound::prepare(&guard(false), 1).unwrap();
    let retired = Arc::downgrade(&bound.index);
    let mut knowledge = CandidateKnowledge::default();
    knowledge.bound(&bound).unwrap();
    let mut conditions = Conditions::<Arc<(Theory, Narrower)>> {
        bound: Some(bound),
        ..Conditions::default()
    };
    let active = conditions.clone();
    conditions.bound = Some(Bound::prepare(&guard(true), 2).unwrap());
    assert!(retired.upgrade().is_some());
    drop(active);
    assert!(retired.upgrade().is_none());
    // Mutable knowledge can remain queued without retaining the retired DAG.
    assert_eq!(knowledge.bound_generation.unwrap(), 1);
}

#[test]
fn bound_knowledge_allocations_are_included_once() {
    let bound = Bound::prepare(&guard(false), 1).unwrap();
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
            Node::Atom(0),
            Node::False,
            Node::Implies(0, 1),
            Node::Or(0, 2),
        ],
        vec![3],
    );
    let mut search =
        StableModels::new(&original, Limits::default(), Cancellation::default()).unwrap();
    let first = theory(1, vec![Node::Atom(0)], vec![0]);
    search.tighten_candidate_bound(&first).unwrap();
    search.bound_generation = u64::MAX;
    let before = search.statistics();
    let stronger = theory(1, vec![Node::False], vec![0]);
    assert_eq!(
        search.tighten_candidate_bound(&stronger),
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
        2 => StableModels::with_region_workers(original, workers, limits, cancellation),
        _ => unreachable!(),
    }
    .unwrap()
}

#[test]
fn refused_preparation_does_not_advance_the_bound_generation() {
    let original = theory(2, vec![], vec![]);
    let first = guard(false);
    for route in 0..3 {
        let mut measured = search(route, &original, Limits::default(), Cancellation::default());
        measured.tighten_candidate_bound(&first).unwrap();
        let ceiling = measured.statistics().search.work;
        let mut limits = Limits::default();
        limits.search.max_work = ceiling;
        let mut limited = search(route, &original, limits, Cancellation::default());
        limited.tighten_candidate_bound(&first).unwrap();
        assert_eq!(
            limited.tighten_candidate_bound(&guard(true)),
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
    for route in 0..3 {
        let cancellation = Cancellation::default();
        let mut search = search(route, &original, Limits::default(), cancellation.clone());
        search.tighten_candidate_bound(&guard(false)).unwrap();
        let before = search.statistics().search.work;
        cancellation.cancel();
        assert_eq!(
            search.tighten_candidate_bound(&guard(true)),
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
    let mut knowledge = CandidateKnowledge::default();
    let mut conditions = Conditions::<(Theory, Narrower)> {
        bound: Some(Bound::prepare(&guard(false), 1).unwrap()),
        ..Conditions::default()
    };
    let mut region = Region::all_open(2);
    let cancellation = Cancellation::default();
    let mut budget = Budget {
        quota: LocalQuota,
        limits: SearchLimits::default(),
        cancellation: &cancellation,
        statistics: SearchStatistics::default(),
    };
    let mut counts = super::super::regions::RegionCounts::default();
    super::super::regions::narrow(
        (&original, &narrower, None),
        &conditions,
        &mut region,
        &mut knowledge,
        &mut zetesis_ferraris::NarrowingScratch::default(),
        &mut budget,
        &mut counts,
    )
    .unwrap();
    let permanent = theory(
        2,
        vec![Node::Atom(0), Node::False, Node::Implies(0, 1)],
        vec![2],
    );
    let permanent_index = Narrower::new(&permanent);
    conditions.permanent.push((permanent, permanent_index));
    super::super::regions::narrow(
        (&original, &narrower, None),
        &conditions,
        &mut region,
        &mut knowledge,
        &mut zetesis_ferraris::NarrowingScratch::default(),
        &mut budget,
        &mut counts,
    )
    .unwrap();
    assert!(region.is_cut(0) && region.is_held(1));
}

#[test]
fn allocated_bytes_include_unused_knowledge_slots() {
    let bound = Bound::prepare(&guard(false), 1).unwrap();
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
