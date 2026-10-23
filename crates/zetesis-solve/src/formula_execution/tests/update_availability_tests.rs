//! The run reports the update route's steady state once a batch holds reuse
//! authority, and the last batch's decision until then.

use super::{TightUpdateAvailability, TightUpdateStatistics, record_update_availability};
use zetesis_wgpu::{TightGpuUpdateUnavailable, TightGpuUpdates};

const ADMITTED: TightGpuUpdates = TightGpuUpdates::Admitted {
    count_bits: 16,
    index_built: true,
};

/// Fold one batch's report; the fold refuses only a counter overflow.
fn fold(updates: &mut TightUpdateStatistics, authority: bool, report: TightGpuUpdates) {
    assert!(record_update_availability(updates, authority, report).is_ok());
}

#[test]
fn a_declining_batch_without_authority_reports_its_reason() {
    let mut updates = TightUpdateStatistics::default();
    fold(
        &mut updates,
        false,
        TightGpuUpdates::Unavailable(TightGpuUpdateUnavailable::NarrowSchedule),
    );
    assert_eq!(
        updates.availability,
        TightUpdateAvailability::NarrowSchedule
    );
    assert_eq!(updates.count_bits, None);
    assert!(!updates.steady_state);
}

#[test]
fn batches_without_authority_report_the_last_decision() {
    let mut updates = TightUpdateStatistics::default();
    fold(
        &mut updates,
        false,
        TightGpuUpdates::Unavailable(TightGpuUpdateUnavailable::CandidateCeiling),
    );
    fold(&mut updates, false, TightGpuUpdates::Pending);
    assert_eq!(updates.availability, TightUpdateAvailability::Pending);
    assert!(!updates.steady_state);
}

#[test]
fn an_authority_batch_outlasts_later_batches_without_it() {
    let mut updates = TightUpdateStatistics::default();
    fold(&mut updates, true, ADMITTED);
    let steady = updates;
    assert_eq!(steady.availability, TightUpdateAvailability::Admitted);
    assert_eq!(steady.count_bits, Some(16));
    assert!(steady.steady_state);
    // An alternating count holds no authority and binds no state; its
    // `pending` must not replace the steady state.
    fold(&mut updates, false, TightGpuUpdates::Pending);
    fold(
        &mut updates,
        false,
        TightGpuUpdates::Unavailable(TightGpuUpdateUnavailable::CandidateCeiling),
    );
    assert_eq!(updates, steady);
}

#[test]
fn a_later_authority_batch_replaces_the_steady_state() {
    let mut updates = TightUpdateStatistics::default();
    fold(&mut updates, true, ADMITTED);
    fold(
        &mut updates,
        true,
        TightGpuUpdates::Unavailable(TightGpuUpdateUnavailable::Bytes),
    );
    assert_eq!(updates.availability, TightUpdateAvailability::Bytes);
    assert_eq!(updates.count_bits, None);
    assert!(updates.steady_state);
}

const GUARD_DECLINED: TightGpuUpdates =
    TightGpuUpdates::Unavailable(TightGpuUpdateUnavailable::GuardDeclined);

#[test]
fn a_guard_declined_authority_batch_is_the_steady_state() {
    let mut updates = TightUpdateStatistics::default();
    fold(&mut updates, true, ADMITTED);
    fold(&mut updates, true, GUARD_DECLINED);
    let steady = updates;
    assert_eq!(steady.availability, TightUpdateAvailability::GuardDeclined);
    assert_eq!(steady.availability.label(), "guard_declined");
    assert!(steady.steady_state);
    // A later batch without authority leaves it standing.
    fold(&mut updates, false, TightGpuUpdates::Pending);
    assert_eq!(updates, steady);
}

#[test]
fn every_guard_declined_batch_is_counted() {
    let mut updates = TightUpdateStatistics::default();
    fold(&mut updates, true, ADMITTED);
    for authority in [true, false, true] {
        fold(&mut updates, authority, GUARD_DECLINED);
    }
    // The route re-engages; the count keeps what it declined.
    fold(&mut updates, true, ADMITTED);
    assert_eq!(updates.guard_declined_batches, 3);
    assert_eq!(updates.availability, TightUpdateAvailability::Admitted);
}
