//! Source reads reserve a bounded chunk but retain only its executed charge.

use zetesis_algebra::Meter;

use std::collections::VecDeque;

use zetesis_algebra::{Cancellation, Stop};

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
    let mut budget_work = Meter::local(1_000);
    let mut budget = Budget::new(&mut budget_work, 1_000, 0, &cancellation);
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
    let mut budget_work = Meter::local(100);
    let mut budget = Budget::new(&mut budget_work, 100, 0, &cancellation);
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
    let mut budget_work = Meter::local(100);
    let mut budget = Budget::new(&mut budget_work, 23, 0, &cancellation);
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
    let mut budget_work = Meter::local(23);
    let mut budget = Budget::new(&mut budget_work, 0, 0, &cancellation);
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
    let mut budget_work = Meter::local(0);
    let mut budget = Budget::new(&mut budget_work, 0, 0, &cancellation);
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

impl zetesis_algebra::Quota for Grants {
    type Error = Stop;
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
        let mut budget_work = Meter::reserved(&mut quota);
        let mut budget = Budget::new(&mut budget_work, 100, 0, &cancellation);
        budget.charge(2).unwrap();
        // Three existing spare permits and seven newly granted ones cannot
        // admit the next read. Its invalid ID must not replace the deadline.
        assert_eq!(
            budget.node(&theory, usize::MAX),
            Err(BudgetError::Refused(Stop::Deadline))
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
        let mut budget_work = Meter::reserved(&mut quota);
        let mut budget = Budget::new(&mut budget_work, 0, 0, &cancellation);
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
        let mut budget_work = Meter::reserved(&mut quota);
        let mut budget = Budget::new(&mut budget_work, 24, 0, &cancellation);
        assert_eq!(budget.node(&theory, 0), Ok(crate::NodeView::Atom(0)));
        budget.finish();
        assert_eq!(budget.work(), 4);
    }
    assert_eq!((quota.granted, quota.refunded), (24, 20));
}

#[test]
fn storage_decline_records_the_requested_peak() {
    let cancellation = Cancellation::default();
    let mut budget_work = Meter::local(0);
    let mut budget = Budget::new(&mut budget_work, 0, 12, &cancellation);
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
    let mut budget_work = Meter::local(0);
    let mut budget = Budget::new(&mut budget_work, 0, 0, &cancellation);
    assert_eq!(
        budget.admit_bytes(1),
        Err(BudgetError::Stopped(Stop::Cancelled))
    );
    assert_eq!(budget.peak_bytes(), 0);
}

// The owned payload intentionally implements neither Clone nor Copy. The
// caller refusal must travel through the real meter without a saved-error slot.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum TypedRefusal {
    Storage {
        owner: Box<str>,
        required: u128,
        limit: u128,
    },
    Control(Stop),
}

impl zetesis_algebra::Refusal for TypedRefusal {
    fn is_resource_refusal(&self) -> bool {
        match self {
            Self::Storage { .. } => true,
            Self::Control(stop) => zetesis_algebra::Refusal::is_resource_refusal(stop),
        }
    }

    fn from_stop(stop: Stop) -> Self {
        Self::Control(stop)
    }
}

pub(super) struct TypedQuota {
    steps: VecDeque<Result<u64, TypedRefusal>>,
    pub(super) granted: u64,
    pub(super) refunded: u64,
}

impl TypedQuota {
    pub(super) fn new(steps: impl IntoIterator<Item = Result<u64, TypedRefusal>>) -> Self {
        Self {
            steps: steps.into_iter().collect(),
            granted: 0,
            refunded: 0,
        }
    }
}

impl zetesis_algebra::Quota for TypedQuota {
    type Error = TypedRefusal;

    fn reserve(&mut self, wanted: u64) -> Result<u64, Self::Error> {
        let grant = self
            .steps
            .pop_front()
            .expect("the bounded script covers each reservation")?;
        assert!(grant <= wanted);
        self.granted += grant;
        Ok(grant)
    }

    fn refund(&mut self, unspent: u64) {
        self.refunded += unspent;
    }
}

#[test]
fn typed_global_refusal_precedes_optional_and_source_failures() {
    let owner_name = Box::<str>::from("arithmetic-original");
    let identity = owner_name.as_ptr();
    let mut quota = TypedQuota::new([
        Ok(5),
        Ok(7),
        Err(TypedRefusal::Storage {
            owner: owner_name,
            required: u128::MAX,
            limit: 31,
        }),
    ]);
    let cancellation = Cancellation::default();
    let theory = owner();
    {
        let mut work = Meter::reserved(&mut quota);
        let mut budget = Budget::new(&mut work, 2, 0, &cancellation);
        budget.charge(2).unwrap();
        // Optional work is exhausted and the source ID is invalid. The old
        // global-first reservation boundary must nevertheless return this
        // exact caller error, without executing the node read.
        let BudgetError::Refused(TypedRefusal::Storage {
            owner,
            required,
            limit,
        }) = budget.node(&theory, usize::MAX).unwrap_err()
        else {
            panic!("the typed reservation refusal must remain intact")
        };
        assert_eq!(owner.as_ptr(), identity);
        assert_eq!(&*owner, "arithmetic-original");
        assert_eq!((required, limit), (u128::MAX, 31));
        assert_eq!(budget.work(), 2);
        assert_eq!(budget.peak_bytes(), 0);
        budget.finish();
        assert_eq!(budget.work(), 2);
    }
    assert_eq!((quota.granted, quota.refunded), (12, 10));
}

#[test]
fn a_borrowed_meter_gives_each_preparation_its_own_prefix() {
    let theory = owner();
    let cancellation = Cancellation::default();
    let mut work = Meter::local(100);
    work.bounded(7, || ((), 7)).unwrap();
    {
        let mut budget = Budget::new(&mut work, 24, 0, &cancellation);
        assert_eq!(budget.work(), 0);
        budget
            .in_phase(super::super::work::Phase::Recognition, |budget| {
                assert_eq!(budget.node(&theory, 0)?, crate::NodeView::Atom(0));
                Ok(())
            })
            .unwrap();
        assert_eq!(budget.work(), 4);
        assert_eq!(budget.phases().total(), 4);
        budget.finish();
    }
    assert_eq!(work.charged(), 11);
    {
        let mut budget = Budget::new(&mut work, 23, 0, &cancellation);
        assert_eq!(budget.node(&theory, 0), Err(BudgetError::WorkDeclined));
        assert_eq!(budget.work(), 0);
        assert_eq!(budget.phases().total(), 0);
        budget.finish();
    }
    assert_eq!(work.charged(), 11);
}
