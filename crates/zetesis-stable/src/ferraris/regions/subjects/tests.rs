//! Arithmetic adapters preserve absence, progress and typed receipts.

use super::*;
use crate::budget::LocalQuota;
use crate::{SearchLimits, SearchStatistics};
use zetesis_ferraris::{Node, PreparationLimits};
use zetesis_theory_support::theories::theory;

fn fixture() -> (Theory, PreparedArithmetic) {
    use zetesis_ferraris::{AdmissionLimits, FormulaNodes, FormulaParts, NodeView};
    let parts = FormulaParts::new(
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
        vec![],
    )
    .unwrap();
    let mut nodes = FormulaNodes::new(parts);
    let mut edit = nodes.transaction();
    let bounds = edit
        .push_threshold_family(&[0, 1], &[1, 1], &[1, 2], usize::MAX, usize::MAX)
        .unwrap();
    let upper = edit
        .push(NodeView::Implies(bounds[1], 2), usize::MAX, usize::MAX)
        .unwrap();
    edit.commit();
    // Both equivalent presentations are deliberate: parity clauses and a
    // checked counting pair give the late-order test two real subjects.
    let theory = Theory::new(
        2,
        nodes.into_parts(),
        vec![7, 8, bounds[0], upper],
        AdmissionLimits::default(),
    )
    .unwrap();
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
    assert!(attempt.cardinality.families > 0);
    (
        theory,
        PreparedArithmetic::new(
            attempt.result.unwrap().unwrap(),
            attempt.parity.rank,
            attempt.cardinality.families,
        ),
    )
}

fn budget(cancellation: &Cancellation, work: u64) -> Budget<'_> {
    Budget {
        quota: LocalQuota,
        limits: SearchLimits {
            max_work: work,
            ..SearchLimits::default()
        },
        cancellation,
        statistics: SearchStatistics::default(),
    }
}

#[test]
fn absent_late_owners_create_no_subject() {
    let theory = theory(0, vec![], vec![]);
    let narrower = Narrower::new(&theory);
    let reading = CandidateReading {
        original: OriginalSubject::new(&theory, None),
        narrower: &narrower,
        arithmetic: None,
    };
    assert!(late(&reading, None).iter().all(Option::is_none));
}

#[test]
fn prepared_arithmetic_uses_the_declared_late_order() {
    let (theory, prepared) = fixture();
    let narrower = Narrower::new(&theory);
    let reading = CandidateReading {
        original: OriginalSubject::new(&theory, None),
        narrower: &narrower,
        arithmetic: Some(&prepared),
    };
    let subjects = late(&reading, None);
    assert!(matches!(subjects[0], Some(CandidateSubject::Parity(_))));
    assert!(matches!(
        subjects[1],
        Some(CandidateSubject::Cardinality(_))
    ));
    assert!(subjects[2].is_none());
}

#[test]
fn parity_progress_keeps_its_decisions_and_receipt() {
    let (_, prepared) = fixture();
    let mut subject = CandidateSubject::Parity(prepared.owner.parity());
    let mut region = Region::all_open(2);
    assert!(region.hold(0));
    let mut knowledge = CandidateKnowledge::default();
    let mut scratch = NarrowingScratch::default();
    let cancellation = Cancellation::default();
    let mut budget = budget(&cancellation, u64::MAX);
    let mut counts = RegionCounts::default();
    let attempt = subject.narrow(
        &mut region,
        &mut knowledge,
        &mut scratch,
        Pass::First,
        &mut SubjectContext {
            budget: &mut budget,
            counts: &counts,
        },
    );
    assert_eq!(attempt.result, Ok(SubjectStep::Changed));
    assert_eq!(
        (
            attempt.counts.held,
            attempt.counts.cut,
            attempt.counts.propagations
        ),
        (0, 1, 1)
    );
    assert!(attempt.counts.work > 0);
    assert_eq!(attempt.account(&mut counts), Ok(SubjectStep::Changed));
    assert_eq!(counts.work, budget.statistics.work);
    assert!(region.is_held(0) && region.is_cut(1));
    assert_eq!(budget.statistics.decisions, 0);
}

#[test]
fn arithmetic_refusal_is_not_a_semantic_refutation() {
    let (_, prepared) = fixture();
    let mut subject = CandidateSubject::Parity(prepared.owner.parity());
    let mut region = Region::all_open(2);
    let before = region.clone();
    let cancellation = Cancellation::default();
    let mut budget = budget(&cancellation, 0);
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
    assert_eq!(attempt.account(&mut counts), Err(Incomplete::WorkLimit));
    assert_eq!(region, before);
    assert_eq!(counts, RegionCounts::default());
    assert_eq!(budget.statistics.work, 0);
}

#[test]
fn cancelled_child_copy_publishes_its_zero_receipt() {
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let mut budget = budget(&cancellation, 0);
    let source = CandidateKnowledge::default();
    let attempt = copy_knowledge(&source, Some(source.clone()), &mut budget);
    assert!(matches!(attempt.result, Err(Incomplete::Cancelled)));
    assert_eq!(attempt.counts, RegionCounts::default());
    assert_eq!(budget.statistics.work, 0);
}

mod source_pass;

mod arithmetic_receipts;
