use super::*;
use crate::ferraris::regions::arithmetic_counts::ArithmeticCounts;

#[test]
fn parity_application_records_the_real_changed_batch() {
    let (_, prepared) = fixture();
    let mut subject = CandidateSubject::Parity(prepared.owner.parity());
    let cancellation = Cancellation::default();
    let mut budget = budget(&cancellation, u64::MAX);
    let mut region = Region::all_open(2);
    assert!(region.hold(0));
    let mut counts = RegionCounts::default();
    let attempt = subject.narrow(
        &mut region,
        &mut CandidateKnowledge::default(),
        &mut NarrowingScratch::default(),
        Pass::First,
        &mut SubjectContext {
            budget: &mut budget,
            counts: &counts,
        },
    );
    assert_eq!(attempt.account(&mut counts), Ok(SubjectStep::Changed));
    assert!(region.is_cut(1));
    assert_eq!(
        counts.arithmetic,
        ArithmeticCounts {
            parity_passes: 1,
            parity_forced: 1,
            ..ArithmeticCounts::default()
        }
    );
    assert_eq!(counts.work, budget.statistics.work);
}

#[test]
fn parity_refutation_counts_a_batch_but_not_a_root_refutation() {
    let (_, prepared) = fixture();
    assert!(!prepared.owner.root_refuted());
    let mut subject = CandidateSubject::Parity(prepared.owner.parity());
    let cancellation = Cancellation::default();
    let mut budget = budget(&cancellation, u64::MAX);
    let mut region = Region::all_open(2);
    assert!(region.hold(0) && region.hold(1));
    let mut counts = RegionCounts::default();
    let attempt = subject.narrow(
        &mut region,
        &mut CandidateKnowledge::default(),
        &mut NarrowingScratch::default(),
        Pass::First,
        &mut SubjectContext {
            budget: &mut budget,
            counts: &counts,
        },
    );
    assert_eq!(attempt.account(&mut counts), Ok(SubjectStep::Refuted));
    assert_eq!(
        counts.arithmetic,
        ArithmeticCounts {
            parity_passes: 1,
            parity_refuted: 1,
            ..ArithmeticCounts::default()
        }
    );
    assert!(!prepared.owner.root_refuted());
}

#[test]
fn an_entry_cancellation_produces_no_arithmetic_activity() {
    let (_, prepared) = fixture();
    let mut subject = CandidateSubject::Parity(prepared.owner.parity());
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let mut budget = budget(&cancellation, 0);
    let mut counts = RegionCounts::default();
    let attempt = subject.narrow(
        &mut Region::all_open(2),
        &mut CandidateKnowledge::default(),
        &mut NarrowingScratch::default(),
        Pass::First,
        &mut SubjectContext {
            budget: &mut budget,
            counts: &counts,
        },
    );
    assert_eq!(attempt.account(&mut counts), Err(Incomplete::Cancelled));
    assert_eq!(counts, RegionCounts::default());
    assert_eq!(budget.statistics.work, 0);
}

#[test]
fn every_parity_refusal_preserves_its_applied_decision_receipt() {
    let (_, prepared) = fixture();
    let mut saw_failed_application = false;
    let cancellation = Cancellation::default();
    let mut full_budget = budget(&cancellation, u64::MAX);
    let mut full_region = Region::all_open(2);
    assert!(full_region.hold(0));
    let full = CandidateSubject::Parity(prepared.owner.parity()).narrow(
        &mut full_region,
        &mut CandidateKnowledge::default(),
        &mut NarrowingScratch::default(),
        Pass::First,
        &mut SubjectContext {
            budget: &mut full_budget,
            counts: &RegionCounts::default(),
        },
    );
    assert_eq!(full.result, Ok(SubjectStep::Changed));
    let complete_work = full.counts.work;
    let mut completed = false;
    // The successful attempt supplies the finite cutoff range. Every prefix
    // is checked against the actual mutation, not a duplicate charge formula.
    for maximum in 0..=complete_work {
        let mut subject = CandidateSubject::Parity(prepared.owner.parity());
        let cancellation = Cancellation::default();
        let mut budget = budget(&cancellation, maximum);
        let mut region = Region::all_open(2);
        assert!(region.hold(0));
        let mut counts = RegionCounts::default();
        let attempt = subject.narrow(
            &mut region,
            &mut CandidateKnowledge::default(),
            &mut NarrowingScratch::default(),
            Pass::First,
            &mut SubjectContext {
                budget: &mut budget,
                counts: &counts,
            },
        );
        let result = attempt.account(&mut counts);
        assert_eq!(counts.work, budget.statistics.work);
        assert_eq!(counts.arithmetic.parity_forced, u64::from(region.is_cut(1)));
        assert_eq!(counts.arithmetic.parity_refuted, 0);
        assert_eq!(counts.arithmetic.cardinality_passes, 0);
        match result {
            Ok(SubjectStep::Changed) => {
                assert_eq!(counts.arithmetic.parity_passes, 1);
                completed = true;
                break;
            }
            Err(Incomplete::WorkLimit) => {
                assert_eq!(counts.arithmetic.parity_passes, 0);
                saw_failed_application |= region.is_cut(1);
            }
            other => panic!("unexpected parity attempt: {other:?}"),
        }
    }
    assert!(completed);
    assert!(saw_failed_application);
}

#[test]
fn cardinality_completion_does_not_become_parity_activity() {
    let (_, prepared) = fixture();
    let mut subject = CandidateSubject::Cardinality(prepared.owner.cardinality().unwrap());
    let cancellation = Cancellation::default();
    let mut budget = budget(&cancellation, u64::MAX);
    let mut counts = RegionCounts::default();
    let attempt = subject.narrow(
        &mut Region::all_open(2),
        &mut CandidateKnowledge::default(),
        &mut NarrowingScratch::default(),
        Pass::First,
        &mut SubjectContext {
            budget: &mut budget,
            counts: &counts,
        },
    );
    assert_eq!(attempt.account(&mut counts), Ok(SubjectStep::Unchanged));
    assert_eq!(
        counts.arithmetic,
        ArithmeticCounts {
            cardinality_passes: 1,
            ..ArithmeticCounts::default()
        }
    );
    assert_eq!(counts.work, budget.statistics.work);
}
