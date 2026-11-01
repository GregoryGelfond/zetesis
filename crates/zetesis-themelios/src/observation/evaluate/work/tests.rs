//! Observation admission retains its control, source and receipt boundaries.

use std::time::Instant;

use super::{ErrorKind, Resource, Statistics, Work};
use crate::observation::evaluate::test_support::work;
use crate::{ProgramSite, StatementId};
use themelios_base::source::SourceId;
use themelios_base::span::{ByteOffset, Location, Span};
use zetesis_algebra::{Cancellation, Stop};

fn resumed_work(cancellation: &Cancellation, limit: u64) -> Work<'_> {
    let mut work = work(cancellation);
    work.statistics = Statistics {
        work: 7,
        bindings: 11,
        term_storage_bytes: 13,
        peak_term_storage_bytes: 17,
    };
    work.limits.max_work = limit;
    work.local_bytes = 19;
    work.site = ProgramSite::statement(
        StatementId::new(3),
        Some(Location {
            source: SourceId::new(23),
            span: Span::empty(ByteOffset::new(29)),
        }),
    );
    work
}

#[test]
fn accepted_work_changes_only_the_work_receipt() {
    let cancellation = Cancellation::default();
    let mut work = resumed_work(&cancellation, 9);
    let before = work.statistics;
    let site = work.site;
    work.step(2).unwrap();
    assert_eq!(work.statistics, Statistics { work: 9, ..before });
    assert_eq!(work.site, site);
    assert_eq!(work.local_bytes, 19);
}

#[test]
fn zero_work_at_the_ceiling_preserves_the_full_receipt() {
    let cancellation = Cancellation::default();
    let mut work = resumed_work(&cancellation, 7);
    let before = work.statistics;
    let site = work.site;
    work.step(0).unwrap();
    assert_eq!(work.statistics, before);
    assert_eq!(work.site, site);
    assert_eq!(work.local_bytes, 19);
}

#[test]
fn resumed_work_refuses_only_the_next_complete_request() {
    let cancellation = Cancellation::default();
    for count in [0, 2] {
        let mut work = resumed_work(&cancellation, 3);
        let before = work.statistics;
        let site = work.site;
        let error = work.step(count).unwrap_err();
        assert_eq!(
            error.kind(),
            &ErrorKind::Limit {
                resource: Resource::Work,
                observed: 7 + count,
                limit: 3,
            }
        );
        assert_eq!(error.site(), site);
        assert_eq!(error.statistics(), before);
        assert_eq!(work.statistics, before);
        assert_eq!(work.site, site);
        assert_eq!(work.local_bytes, 19);
    }
}

#[test]
fn work_refusals_retain_wide_payloads_and_source_evidence() {
    let cancellation = Cancellation::default();
    for (accepted, count, observed) in [
        (u64::MAX, 1, u128::from(u64::MAX) + 1),
        (0, u128::MAX, u128::MAX),
    ] {
        let mut work = resumed_work(&cancellation, u64::MAX);
        work.statistics.work = accepted;
        let before = work.statistics;
        let site = work.site;
        let error = work.step(count).unwrap_err();
        assert_eq!(
            error.kind(),
            &ErrorKind::Limit {
                resource: Resource::Work,
                observed,
                limit: u128::from(u64::MAX),
            }
        );
        assert_eq!(error.site(), site);
        assert_eq!(error.statistics(), before);
        assert_eq!(work.statistics, before);
        assert_eq!(work.local_bytes, 19);
    }
}

#[test]
fn changed_limits_apply_at_the_next_work_boundary() {
    let cancellation = Cancellation::default();
    let mut work = resumed_work(&cancellation, 8);
    work.step(1).unwrap();
    let before = work.statistics;
    work.limits.max_work = 7;
    let error = work.step(0).unwrap_err();
    assert_eq!(
        error.kind(),
        &ErrorKind::Limit {
            resource: Resource::Work,
            observed: 8,
            limit: 7,
        }
    );
    assert_eq!(error.statistics(), before);
    assert_eq!(work.statistics, before);
}

#[test]
fn every_work_request_observes_cancellation_before_admission() {
    let cancellation = Cancellation::default();
    cancellation.cancel();
    for count in [0, 1] {
        let mut work = resumed_work(&cancellation, 0);
        let before = work.statistics;
        let site = work.site;
        let error = work.step(count).unwrap_err();
        assert_eq!(error.kind(), &ErrorKind::Stopped(Stop::Cancelled));
        assert_eq!(error.site(), site);
        assert_eq!(error.statistics(), before);
        assert_eq!(work.statistics, before);
        assert_eq!(work.local_bytes, 19);
    }
}

#[test]
fn every_work_request_observes_expiry_before_admission() {
    let cancellation = Cancellation::with_deadline(Instant::now()).unwrap();
    for count in [0, 1] {
        let mut work = resumed_work(&cancellation, 0);
        let before = work.statistics;
        let site = work.site;
        let error = work.step(count).unwrap_err();
        assert_eq!(error.kind(), &ErrorKind::Stopped(Stop::Deadline));
        assert_eq!(error.site(), site);
        assert_eq!(error.statistics(), before);
        assert_eq!(work.statistics, before);
        assert_eq!(work.local_bytes, 19);
    }
}

#[test]
#[should_panic(expected = "observation work producers fit u128 with the accepted prefix")]
fn artificial_overflow_violates_the_private_producer_invariant() {
    // Production counts are bounded by usize lengths. This intentionally invalid
    // private input checks the approved invariant boundary, not legacy wrapping.
    let cancellation = Cancellation::default();
    let mut work = resumed_work(&cancellation, u64::MAX);
    work.step(u128::MAX).unwrap();
}

#[test]
fn named_limits_retain_wide_demand_and_existing_receipts() {
    let cancellation = Cancellation::default();
    let work = resumed_work(&cancellation, 0);
    let before = work.statistics;
    work.check(Resource::Depth, u128::MAX, u128::MAX).unwrap();
    let error = work
        .check(Resource::Depth, u128::MAX, u128::MAX - 1)
        .unwrap_err();
    assert_eq!(
        error.kind(),
        &ErrorKind::Limit {
            resource: Resource::Depth,
            observed: u128::MAX,
            limit: u128::MAX - 1,
        }
    );
    assert_eq!(error.site(), work.site);
    assert_eq!(error.statistics(), before);
    assert_eq!(work.statistics, before);
    assert_eq!(work.local_bytes, 19);
}
