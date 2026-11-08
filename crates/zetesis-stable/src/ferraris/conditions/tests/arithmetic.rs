//! Original arithmetic snapshots have one lazy owner and one charged copy.

use super::*;
use zetesis_algebra::{Failure, Meter, Stop, region::SubjectStep};
use zetesis_ferraris::{NarrowingScratch, PreparationLimits, RootPreparation};

/// Exactly one of two atoms, expressed by its two original constraints.
fn prepared() -> (Theory, RootPreparation) {
    let theory = theory(
        2,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::falsum(),
            Node::implies(0, 2),
            Node::implies(1, 2),
            Node::and_pair([0, 1]),
            Node::and_pair([3, 4]),
            Node::implies(5, 2),
            Node::implies(6, 2),
        ],
        vec![7, 8],
    );
    let attempt = RootPreparation::prepare_reserved(
        &theory,
        None,
        PreparationLimits {
            work: u64::MAX,
            bytes: usize::MAX,
        },
        &Cancellation::default(),
        &mut Meter::local(u64::MAX),
    );
    assert_eq!(attempt.parity.rank, 1);
    let prepared = attempt.result.unwrap().unwrap();
    assert!(!prepared.root_refuted());
    assert!(prepared.cardinality().is_some());
    (theory, prepared)
}

fn initialize(prepared: &RootPreparation, knowledge: &mut CandidateKnowledge) {
    let mut region = Region::all_open(2);
    let mut scratch = NarrowingScratch::default();
    let cancellation = Cancellation::default();
    assert_eq!(
        prepared
            .parity()
            .inspect_reserved(
                &mut region,
                knowledge.parity(prepared.parity()),
                &mut scratch,
                &cancellation,
                &mut Meter::local(u64::MAX),
            )
            .result,
        Ok(SubjectStep::Unchanged)
    );
    let system = prepared.cardinality().unwrap();
    assert_eq!(
        system
            .inspect_reserved(
                &mut region,
                knowledge.cardinality(system),
                &mut scratch,
                &cancellation,
                &mut Meter::local(u64::MAX),
            )
            .result,
        Ok(SubjectStep::Unchanged)
    );
}

fn local_budget(cancellation: &Cancellation) -> Budget<'_> {
    Budget {
        quota: LocalQuota,
        limits: SearchLimits {
            max_work: u64::MAX,
            ..SearchLimits::default()
        },
        cancellation,
        statistics: SearchStatistics::default(),
    }
}

#[test]
fn arithmetic_slots_are_created_only_when_requested() {
    let (_, prepared) = prepared();
    let mut knowledge = CandidateKnowledge::default();
    assert!(knowledge.parity.is_none() && knowledge.cardinality.is_none());
    assert_eq!(knowledge.allocated_bytes(), 0);
    let _ = knowledge.parity(prepared.parity());
    assert!(knowledge.parity.is_some() && knowledge.cardinality.is_none());
    assert_eq!(knowledge.allocated_bytes(), 0);
    let _ = knowledge.cardinality(prepared.cardinality().unwrap());
    assert!(knowledge.cardinality.is_some());
    assert_eq!(knowledge.allocated_bytes(), 0);
}

#[test]
fn arithmetic_payloads_are_counted_without_inline_headers() {
    let (theory, prepared) = prepared();
    let mut knowledge = CandidateKnowledge::default();
    knowledge.permanent(0, &Narrower::new(&theory)).unwrap();
    let formula_bytes = knowledge.allocated_bytes();
    initialize(&prepared, &mut knowledge);
    let arithmetic_bytes = knowledge.parity.as_ref().unwrap().retained_bytes()
        - size_of::<ParityKnowledge>() as u128
        + knowledge.cardinality.as_ref().unwrap().retained_bytes()
        - size_of::<CardinalityKnowledge>() as u128;
    assert!(arithmetic_bytes > 0);
    assert_eq!(
        knowledge.allocated_bytes(),
        formula_bytes + arithmetic_bytes
    );
}

#[test]
fn changing_a_bound_preserves_original_arithmetic_slots() {
    let (_, prepared) = prepared();
    let mut knowledge = CandidateKnowledge::default();
    initialize(&prepared, &mut knowledge);
    let before = (
        knowledge.parity.as_ref().unwrap().retained_bytes(),
        knowledge.cardinality.as_ref().unwrap().retained_bytes(),
    );
    for generation in [1, 2] {
        let bound = Bound::prepare(
            &guard(generation == 2),
            generation,
            u64::MAX,
            &Cancellation::default(),
        )
        .unwrap();
        knowledge.bound(&bound).unwrap();
    }
    assert_eq!(knowledge.bound_generation, Some(2));
    assert_eq!(
        before,
        (
            knowledge.parity.as_ref().unwrap().retained_bytes(),
            knowledge.cardinality.as_ref().unwrap().retained_bytes(),
        )
    );
    let mut region = Region::all_open(2);
    assert!(region.hold(0));
    let attempt = prepared.parity().inspect_reserved(
        &mut region,
        knowledge.parity(prepared.parity()),
        &mut NarrowingScratch::default(),
        &Cancellation::default(),
        &mut Meter::local(u64::MAX),
    );
    assert_eq!(attempt.result, Ok(SubjectStep::Changed));
    assert!(region.is_cut(1));
}

#[test]
fn metered_copy_preserves_recycled_formula_storage() {
    let (theory, prepared) = prepared();
    let mut source = CandidateKnowledge::default();
    source.permanent(0, &Narrower::new(&theory)).unwrap();
    initialize(&prepared, &mut source);
    let mut retired = source.clone();
    let old_entries = retired.entries.as_ptr();
    retired.bound_generation = None;
    let mut destination = Some(retired);
    let mut meter = Meter::local(u64::MAX);
    source
        .copy_reserved(&mut destination, &Cancellation::default(), &mut meter)
        .unwrap();
    let target = destination.as_mut().unwrap();
    assert_eq!(target.entries.as_ptr(), old_entries);
    assert_ne!(target.entries.as_ptr(), source.entries.as_ptr());
    assert_eq!(target.bound_generation, source.bound_generation);
    assert_eq!(target.allocated_bytes(), source.allocated_bytes());
    let mut child = Region::all_open(2);
    assert!(child.hold(0));
    assert_eq!(
        prepared
            .parity()
            .inspect_reserved(
                &mut child,
                target.parity(prepared.parity()),
                &mut NarrowingScratch::default(),
                &Cancellation::default(),
                &mut Meter::local(u64::MAX),
            )
            .result,
        Ok(SubjectStep::Changed)
    );
    let mut sibling = Region::all_open(2);
    assert!(sibling.cut(0));
    assert_eq!(
        prepared
            .parity()
            .inspect_reserved(
                &mut sibling,
                source.parity(prepared.parity()),
                &mut NarrowingScratch::default(),
                &Cancellation::default(),
                &mut Meter::local(u64::MAX),
            )
            .result,
        Ok(SubjectStep::Changed)
    );
    assert!(child.is_cut(1) && sibling.is_held(1));
}

#[test]
fn an_absent_parent_slot_discards_retired_arithmetic() {
    let (_, prepared) = prepared();
    let source = CandidateKnowledge::default();
    let mut retired = CandidateKnowledge::default();
    initialize(&prepared, &mut retired);
    let mut destination = Some(retired);
    source
        .copy_reserved(
            &mut destination,
            &Cancellation::default(),
            &mut Meter::local(u64::MAX),
        )
        .unwrap();
    let copied = destination.unwrap();
    assert!(copied.parity.is_none() && copied.cardinality.is_none());
    assert_eq!(copied.allocated_bytes(), 0);
}

#[test]
fn copied_arithmetic_components_have_one_work_receipt() {
    let (_, prepared) = prepared();
    let mut source = CandidateKnowledge::default();
    initialize(&prepared, &mut source);
    let cancellation = Cancellation::default();
    let mut component_work = Meter::local(u64::MAX);
    source
        .parity
        .as_ref()
        .unwrap()
        .copy_reserved(&mut None, &cancellation, &mut component_work)
        .unwrap();
    source
        .cardinality
        .as_ref()
        .unwrap()
        .copy_reserved(&mut None, &cancellation, &mut component_work)
        .unwrap();
    let mut budget = local_budget(&cancellation);
    let attempt = super::super::super::regions::copy_knowledge(&source, None, &mut budget);
    assert!(attempt.result.is_ok());
    // Two fresh inline None tags and one source-presence read per component.
    assert_eq!(attempt.counts.work, component_work.charged() + 4);
    assert_eq!(attempt.counts.work, budget.statistics.work);
    assert_eq!(attempt.counts.propagations, 0);
    assert_eq!(budget.statistics.decisions, 0);
}

#[test]
fn every_copy_cutoff_invalidates_only_its_destination() {
    let (_, prepared) = prepared();
    let mut source = CandidateKnowledge::default();
    initialize(&prepared, &mut source);
    let cancellation = Cancellation::default();
    let mut complete = Meter::local(u64::MAX);
    source
        .copy_reserved(&mut None, &cancellation, &mut complete)
        .unwrap();
    let retained = source.allocated_bytes();
    for limit in 0..complete.charged() {
        let mut destination = None;
        let mut meter = Meter::local(limit);
        let result = source.copy_reserved(&mut destination, &cancellation, &mut meter);
        assert!(matches!(result, Err(Failure::Stopped(Stop::WorkLimit))));
        assert!(destination.is_none(), "allowance {limit}");
        assert!(meter.charged() <= limit);
        assert_eq!(source.allocated_bytes(), retained);
    }
}

#[test]
fn cancelled_copy_has_no_semantic_work() {
    let (_, prepared) = prepared();
    let mut source = CandidateKnowledge::default();
    initialize(&prepared, &mut source);
    let mut destination = Some(source.clone());
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let mut meter = Meter::local(0);
    assert_eq!(
        source.copy_reserved(&mut destination, &cancellation, &mut meter),
        Err(Failure::Stopped(Stop::Cancelled))
    );
    assert!(destination.is_none());
    assert_eq!(meter.charged(), 0);
    assert!(source.allocated_bytes() > 0);
}
