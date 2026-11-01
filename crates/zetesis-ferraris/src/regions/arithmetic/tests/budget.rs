//! Source reads reserve a bounded chunk but retain only its executed charge.

use std::collections::VecDeque;

use zetesis_cpu::{Cancellation, Stop};

use crate::regions::{NarrowingQuota, Work};
use crate::{
    AdmissionError, AdmissionLimits, ExtremumCutDescriptor, ExtremumCutRule, FormulaParts, Node,
    NumericDomain, NumericRow, OperandSpan, Theory, ThresholdDescriptor,
};

use super::super::budget::{Budget, BudgetError};

fn owner() -> Theory {
    let parts = FormulaParts::from_numeric_raw(
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::falsum(),
            Node::implies(0, 1),
            Node::and_pair([0, 1]),
            Node::or_pair([0, 1]),
            Node::and_span(OperandSpan {
                start: 0,
                length: 3,
            }),
            Node::or_span(OperandSpan {
                start: 3,
                length: 3,
            }),
            Node::threshold(0),
            Node::extremum_cut(0),
        ],
        vec![0, 1, 0, 1, 0, 1],
        vec![0, 1],
        vec![2, 1],
        vec![
            NumericRow {
                start: 0,
                length: 1,
                domain: NumericDomain::Weights,
            },
            NumericRow {
                start: 1,
                length: 1,
                domain: NumericDomain::Ordinal { rank_count: 3 },
            },
        ],
        vec![ThresholdDescriptor { row: 0, bound: 1 }],
        vec![ExtremumCutDescriptor {
            row: 1,
            rule: ExtremumCutRule::Prefix(2),
        }],
    )
    .unwrap();
    Theory::new(2, parts, vec![], AdmissionLimits::default()).unwrap()
}

#[test]
fn node_reads_charge_the_actual_canonical_path() {
    let theory = owner();
    let cancellation = Cancellation::default();
    let mut budget = Budget::new(Work::new(1_000), 1_000, 0, &cancellation);
    for (id, expected) in [4, 4, 5, 5, 5, 5, 12, 12, 21, 24].into_iter().enumerate() {
        let before = budget.work();
        assert_eq!(
            budget.node(&theory, id),
            theory.view().node(id).map_err(BudgetError::Source)
        );
        assert_eq!(budget.work() - before, expected, "node {id}");
    }
}

#[test]
fn absent_node_retains_its_typed_failure_prefix() {
    let theory = owner();
    let cancellation = Cancellation::default();
    let mut budget = Budget::new(Work::new(100), 100, 0, &cancellation);
    assert_eq!(
        budget.node(&theory, theory.nodes().len()),
        Err(BudgetError::Source(AdmissionError::Edge))
    );
    assert_eq!(budget.work(), 3);
}

#[test]
fn optional_work_admits_the_whole_view_bound() {
    let theory = owner();
    let cancellation = Cancellation::default();
    let mut budget = Budget::new(Work::new(100), 23, 0, &cancellation);
    // Even the four-operation atom read first needs its maximum chunk. An
    // absent ID must likewise remain unread when that chunk is not admitted.
    for id in [0, usize::MAX] {
        assert_eq!(budget.node(&theory, id), Err(BudgetError::WorkDeclined));
        assert_eq!(budget.work(), 0);
    }
}

#[test]
fn global_refusal_precedes_optional_decline() {
    let theory = owner();
    let cancellation = Cancellation::default();
    let mut budget = Budget::new(Work::new(23), 0, 0, &cancellation);
    assert_eq!(
        budget.node(&theory, 0),
        Err(BudgetError::Stopped(Stop::WorkLimit))
    );
    assert_eq!(budget.work(), 0);
}

#[test]
fn cancellation_precedes_both_work_refusals() {
    let theory = owner();
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let mut budget = Budget::new(Work::new(0), 0, 0, &cancellation);
    assert_eq!(
        budget.node(&theory, 0),
        Err(BudgetError::Stopped(Stop::Cancelled))
    );
    assert_eq!(budget.charge(1), Err(BudgetError::Stopped(Stop::Cancelled)));
    assert_eq!(budget.work(), 0);
}

struct Grants {
    steps: VecDeque<Result<u64, Stop>>,
    calls: usize,
    granted: u64,
    refunded: u64,
}

impl Grants {
    fn new(steps: impl IntoIterator<Item = Result<u64, Stop>>) -> Self {
        Self {
            steps: steps.into_iter().collect(),
            calls: 0,
            granted: 0,
            refunded: 0,
        }
    }
}

impl NarrowingQuota for Grants {
    fn reserve(&mut self, wanted: u64) -> Result<u64, Stop> {
        self.calls += 1;
        let grant = self.steps.pop_front().unwrap_or(Err(Stop::WorkLimit))?;
        assert!(grant <= wanted);
        self.granted += grant;
        Ok(grant)
    }

    fn refund(&mut self, unspent: u64) {
        self.refunded += unspent;
    }
}

#[test]
fn partial_reservation_refunds_without_executing_the_read() {
    let theory = owner();
    let cancellation = Cancellation::default();
    let mut quota = Grants::new([Ok(5), Ok(7), Err(Stop::Deadline)]);
    {
        let mut budget = Budget::new(Work::reserved(&mut quota), 100, 0, &cancellation);
        budget.charge(2).unwrap();
        // Three existing spare permits and seven newly granted ones cannot
        // admit the next read. Its invalid ID must not replace the deadline.
        assert_eq!(
            budget.node(&theory, usize::MAX),
            Err(BudgetError::Stopped(Stop::Deadline))
        );
        assert_eq!(budget.work(), 2);
        budget.finish();
        assert_eq!(budget.work(), 2);
        budget.finish();
    }
    assert_eq!(quota.calls, 3);
    assert_eq!((quota.granted, quota.refunded), (12, 10));
}

#[test]
fn optional_decline_refunds_all_reserved_permits() {
    let theory = owner();
    let cancellation = Cancellation::default();
    let mut quota = Grants::new([Ok(13), Ok(11)]);
    {
        let mut budget = Budget::new(Work::reserved(&mut quota), 0, 0, &cancellation);
        assert_eq!(budget.node(&theory, 0), Err(BudgetError::WorkDeclined));
        assert_eq!(budget.work(), 0);
        budget.finish();
    }
    assert_eq!((quota.granted, quota.refunded), (24, 24));
}

#[test]
fn successful_read_refunds_only_its_unused_reservation() {
    let theory = owner();
    let cancellation = Cancellation::default();
    let mut quota = Grants::new([Ok(24)]);
    {
        let mut budget = Budget::new(Work::reserved(&mut quota), 24, 0, &cancellation);
        assert_eq!(budget.node(&theory, 0), Ok(crate::NodeView::Atom(0)));
        budget.finish();
        assert_eq!(budget.work(), 4);
    }
    assert_eq!((quota.granted, quota.refunded), (24, 20));
}

#[test]
fn storage_decline_records_the_requested_peak() {
    let cancellation = Cancellation::default();
    let mut budget = Budget::new(Work::new(0), 0, 12, &cancellation);
    budget.admit_bytes(8).unwrap();
    assert_eq!(budget.peak_bytes(), 8);
    assert_eq!(
        budget.admit_bytes(16),
        Err(BudgetError::StorageDeclined {
            required: 16,
            limit: 12
        })
    );
    budget.admit_bytes(0).unwrap();
    assert_eq!(budget.peak_bytes(), 16);
    assert_eq!(budget.work(), 0);
}

#[test]
fn cancellation_precedes_storage_decline() {
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let mut budget = Budget::new(Work::new(0), 0, 0, &cancellation);
    assert_eq!(
        budget.admit_bytes(1),
        Err(BudgetError::Stopped(Stop::Cancelled))
    );
    assert_eq!(budget.peak_bytes(), 0);
}
