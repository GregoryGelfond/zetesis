//! The run reports the update route's steady state once a batch holds reuse
//! authority, and the last batch's decision until then.

use super::{TightUpdateAvailability, TightUpdateStatistics, record_update_availability};
use zetesis_wgpu::{TightGpuUpdateUnavailable, TightGpuUpdates};

const ADMITTED: TightGpuUpdates = TightGpuUpdates::Admitted {
    count_bits: 16,
    index_built: true,
};

#[test]
fn a_declining_batch_without_authority_reports_its_reason() {
    let mut updates = TightUpdateStatistics::default();
    record_update_availability(
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
    record_update_availability(
        &mut updates,
        false,
        TightGpuUpdates::Unavailable(TightGpuUpdateUnavailable::CandidateCeiling),
    );
    record_update_availability(&mut updates, false, TightGpuUpdates::Pending);
    assert_eq!(updates.availability, TightUpdateAvailability::Pending);
    assert!(!updates.steady_state);
}

#[test]
fn an_authority_batch_outlasts_later_batches_without_it() {
    let mut updates = TightUpdateStatistics::default();
    record_update_availability(&mut updates, true, ADMITTED);
    let steady = updates;
    assert_eq!(steady.availability, TightUpdateAvailability::Admitted);
    assert_eq!(steady.count_bits, Some(16));
    assert!(steady.steady_state);
    // An alternating count holds no authority and binds no state; its
    // `pending` must not replace the steady state.
    record_update_availability(&mut updates, false, TightGpuUpdates::Pending);
    record_update_availability(
        &mut updates,
        false,
        TightGpuUpdates::Unavailable(TightGpuUpdateUnavailable::CandidateCeiling),
    );
    assert_eq!(updates, steady);
}

#[test]
fn a_later_authority_batch_replaces_the_steady_state() {
    let mut updates = TightUpdateStatistics::default();
    record_update_availability(&mut updates, true, ADMITTED);
    record_update_availability(
        &mut updates,
        true,
        TightGpuUpdates::Unavailable(TightGpuUpdateUnavailable::Bytes),
    );
    assert_eq!(updates.availability, TightUpdateAvailability::Bytes);
    assert_eq!(updates.count_bits, None);
    assert!(updates.steady_state);
}
