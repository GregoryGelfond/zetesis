//! Admission at named storage boundaries in countermodel preparation.

use super::super::super::{PreparationStatus, budget::BudgetError, storage::Storage};
use super::*;
use std::mem::{size_of, size_of_val};

#[test]
fn a_declined_guard_accounts_for_its_temporary_conjuncts() {
    // One rule per prospective guard never publishes a group. Its long
    // conjunction must still be collected before that decision.
    let width = 32;
    let clause = (0..width).map(|i| (i, true)).collect();
    let fixture = Saturation::build(width, &[clause], &[]);
    let (_, root, _) = fixture.closed();
    let producers = fixture.producers();
    let cancellation = Cancellation::default();
    let run = |limit| {
        let mut meter = Meter::local(u64::MAX);
        let mut budget = Budget::new(&mut meter, u64::MAX, limit, &cancellation);
        let result = propose(
            &fixture.theory,
            &producers,
            &fixture.candidate,
            &fixture.truth,
            &root,
            &mut Storage::default(),
            &mut budget,
        );
        (result, budget.peak_bytes(), budget.work())
    };
    let (result, peak, work) = run(usize::MAX);
    let proposals = result.unwrap();
    assert!(
        proposals.guards.is_empty() && proposals.parity.is_empty() && proposals.wide.is_empty()
    );
    let retained = super::super::proposals::retained(&proposals);
    assert!(peak >= retained + (width * size_of::<usize>()) as u128);
    let peak_limit = usize::try_from(peak).unwrap();
    let (result, refused_peak, _) = run(peak_limit - 1);
    assert_eq!(
        result.unwrap_err(),
        BudgetError::StorageDeclined {
            required: peak,
            limit: peak_limit - 1
        }
    );
    assert_eq!(refused_peak, peak);
    let (result, admitted_peak, admitted_work) = run(peak_limit);
    assert!(result.unwrap().guards.is_empty());
    assert_eq!((admitted_peak, admitted_work), (peak, work));
}

#[test]
fn probe_copies_overlap_retained_proposals_before_the_first_probe() {
    // The extra atom pairs enlarge the frozen knowledge and packed masks,
    // while these two mixed clauses still retain just the guard.
    let clauses = [false, true].map(|flip| (0..9).map(|i| (i, (i % 2 == 0) == flip)).collect());
    let fixture = Saturation::build(512, &clauses, &[]);
    let proposals = fixture.propose();
    assert_eq!(proposals.guards, [fixture.valid]);
    assert!(proposals.parity.is_empty() && proposals.wide.is_empty());
    let (_, root, knowledge) = fixture.closed();
    let probe_copy = size_of::<super::super::probes::Probe<'_>>() as u128
        + (root.retained_bytes() - size_of_val(&root) as u128)
        + (knowledge.retained_bytes() - size_of_val(&knowledge) as u128);
    let simultaneous = super::super::proposals::retained(&proposals) + probe_copy;
    // Both owners are still live at the very first copy admission.
    let limits = FrozenPreparationLimits {
        bytes: usize::try_from(simultaneous - 1).unwrap(),
        ..unbounded()
    };
    let (declined, charged) = fixture.prepare(limits, u64::MAX, &Cancellation::default());
    assert!(declined.result.unwrap().is_none());
    assert_eq!(
        declined.statistics.status,
        PreparationStatus::StorageDeclined
    );
    assert_eq!(declined.statistics.guards, 1);
    assert_eq!(declined.statistics.probes, 0);
    assert_eq!(declined.peak_bytes, simultaneous);
    assert_eq!(declined.statistics.preparation_bytes, declined.peak_bytes);
    assert_eq!(
        (declined.statistics.preparation_work, declined.work),
        (charged, charged)
    );
    let installed = fixture.prepared().result.unwrap().unwrap();
    assert_eq!(installed.units(), &[fixture.valid]);
    assert!(installed.parity().is_none() && installed.cardinality().is_none());
}

#[test]
fn the_two_round_preparation_observes_its_exact_peak_before_publishing() {
    let mut clauses = two_chains();
    clauses.remove(0);
    let fixture = Saturation::build(7, &clauses, &[]);
    let complete = fixture.prepared();
    let peak = usize::try_from(complete.peak_bytes).unwrap();
    assert_eq!(complete.statistics.rounds, 2);
    let (declined, charged) = fixture.prepare(
        FrozenPreparationLimits {
            bytes: peak - 1,
            ..unbounded()
        },
        u64::MAX,
        &Cancellation::default(),
    );
    assert!(declined.result.unwrap().is_none());
    assert_eq!(
        declined.statistics.status,
        PreparationStatus::StorageDeclined
    );
    assert_eq!(declined.peak_bytes, complete.peak_bytes);
    assert_eq!(declined.statistics.preparation_bytes, complete.peak_bytes);
    assert_eq!(
        (declined.work, declined.statistics.preparation_work),
        (charged, charged)
    );
    let (admitted, charged) = fixture.prepare(
        FrozenPreparationLimits {
            bytes: peak,
            ..unbounded()
        },
        u64::MAX,
        &Cancellation::default(),
    );
    assert_eq!(admitted.statistics, complete.statistics);
    assert_eq!(admitted.work, charged);
    let installed = admitted.result.unwrap().unwrap();
    assert_eq!(installed.units(), &[fixture.valid]);
    assert!(installed.parity().is_some());
}

#[test]
fn publication_admits_the_candidates_bit_payload_alongside_the_retained_units() {
    let clauses = [false, true].map(|flip| (0..9).map(|i| (i, (i % 2 == 0) == flip)).collect());
    let fixture = Saturation::build(64, &clauses, &[]);
    let (narrower, region, knowledge) = fixture.closed();
    let query = ClosedQuery {
        narrower: &narrower,
        region: &region,
        knowledge: &knowledge,
        subject: fixture.subject(),
        candidate: &fixture.candidate,
        producers: None,
        closure_work: 0,
    };
    let cancellation = Cancellation::default();
    let run = |bytes| {
        let units = vec![fixture.valid];
        let mut meter = Meter::local(u64::MAX);
        let mut budget = Budget::new(&mut meter, u64::MAX, bytes, &cancellation);
        let live = size_of_val(&units) + units.capacity() * size_of::<usize>();
        let mut storage = Storage::with_live_bytes(live as u128);
        let published =
            super::super::publish(query, units, (None, None), &mut storage, &mut budget);
        (published, budget.peak_bytes())
    };
    let unit_capacity = vec![fixture.valid].capacity() * size_of::<usize>();
    let required = size_of::<FrozenPreparation>()
        + unit_capacity
        + fixture.theory.atom_count().div_ceil(64) * size_of::<u64>();
    let (refusal, peak) = run(required - 1);
    assert_eq!(
        refusal.unwrap_err(),
        BudgetError::StorageDeclined {
            required: required as u128,
            limit: required - 1
        }
    );
    assert_eq!(peak, required as u128);
    let (published, actual) = run(usize::MAX);
    let published = published.unwrap();
    let actual_expected = size_of::<FrozenPreparation>()
        + unit_capacity
        + published.candidate.words.capacity() * size_of::<u64>();
    assert_eq!(actual, actual_expected as u128);
    let (exact, again) = run(actual_expected);
    let exact = exact.unwrap();
    assert!(exact.belongs_to(&fixture.theory, &fixture.candidate));
    assert_eq!(exact.units(), [fixture.valid]);
    assert_eq!(again, actual);
}

#[test]
fn a_byte_refusal_cannot_replace_cancellation_or_a_caller_refusal() {
    let fixture = Saturation::build(3, &gate(0, 1, 2), &[]);
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let limits = FrozenPreparationLimits {
        bytes: 0,
        ..unbounded()
    };
    let (attempt, charged) = fixture.prepare(limits, u64::MAX, &cancellation);
    assert_eq!(
        attempt.result.unwrap_err(),
        super::super::super::PreparationError::Stopped(Failure::Stopped(Stop::Cancelled))
    );
    assert_eq!(attempt.statistics.status, PreparationStatus::Incomplete);
    assert_eq!(
        (attempt.work, attempt.statistics.preparation_work),
        (charged, charged)
    );
    // The first source comparison draws on the caller before the first
    // allocation request. An empty account yields its own typed refusal.
    let (refused, charged) = fixture.prepare(limits, 0, &Cancellation::default());
    assert_eq!(
        refused.result.unwrap_err(),
        super::super::super::PreparationError::Stopped(Failure::Refused(Stop::WorkLimit))
    );
    assert_eq!(refused.statistics.status, PreparationStatus::Incomplete);
    assert_eq!(
        (charged, refused.work, refused.statistics.preparation_work),
        (0, 0, 0)
    );
}

#[test]
fn a_later_probe_counts_its_snapshot_and_elimination_above_the_retained_rows() {
    // The second round reads a first-round system while it owns its mutable
    // per-probe snapshot and a Gaussian elimination workspace.
    let fixture = Saturation::build(3, &gate(0, 1, 2), &[]);
    let (narrower, region, knowledge) = fixture.closed();
    let installed = fixture.prepared().result.unwrap().unwrap();
    let parity = installed.parity().unwrap();
    let cancellation = Cancellation::default();
    let run = |limit| {
        let mut probe =
            super::super::probes::Probe::new(&narrower, fixture.subject(), &region, &knowledge);
        let live = probe.live_bytes() + parity.retained_bytes();
        let storage = Storage::with_live_bytes(live);
        let mut meter = Meter::local(u64::MAX);
        let mut budget = Budget::new(&mut meter, u64::MAX, limit, &cancellation);
        let outcome = probe.run(
            &[(fixture.valid, false)],
            Some(parity),
            &mut NarrowingScratch::default(),
            &cancellation,
            &storage,
            &mut budget,
        );
        (outcome, live, budget.peak_bytes(), budget.work())
    };
    let (outcome, live, peak, work) = run(usize::MAX);
    assert!(!outcome.unwrap());
    let snapshot =
        size_of::<crate::ParityKnowledge>() + 2 * std::mem::size_of_val(region.decision_words().0);
    assert!(
        peak > live + snapshot as u128,
        "elimination and the snapshot must overlap"
    );
    let lim = usize::try_from(peak).unwrap();
    let (outcome, _, refused_peak, _) = run(lim - 1);
    assert_eq!(
        outcome.unwrap_err(),
        BudgetError::StorageDeclined {
            required: peak,
            limit: lim - 1
        }
    );
    assert_eq!(refused_peak, peak);
    let (outcome, _, admitted_peak, admitted_work) = run(lim);
    assert!(!outcome.unwrap());
    assert_eq!((admitted_peak, admitted_work), (peak, work));
}
