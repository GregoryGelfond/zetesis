//! Ordinary record encoding retains memory and control boundaries without fuel.

use super::{model_limits, write_model_record};
use crate::RunError;
use zetesis_cpu::{Cancellation, Stop};
use zetesis_themelios::{OutputSelection, observation};

#[test]
fn ordinary_record_work_uses_its_counter_extent() {
    let limits = model_limits(1024);
    assert_eq!(limits.max_work, u64::MAX);
    assert_eq!(limits.max_bytes, 1024);
    assert_eq!(
        limits.max_depth,
        observation::ViewLimits::default().max_depth
    );
    // Typed library callers still retain a finite operation allowance.
    assert!(observation::ViewLimits::default().max_work < limits.max_work);
}

#[test]
fn ordinary_record_encoding_observes_cancellation() {
    let model = zetesis_core::Model::default();
    let selection = OutputSelection::default();
    let cancellation = Cancellation::default();
    let view = observation::ObservationProgram::default()
        .view(
            &model,
            &selection,
            None,
            observation::Limits::default(),
            &cancellation,
        )
        .unwrap();
    let mut atoms = observation::json::AtomTable::new(0);
    let mut output = Vec::new();
    cancellation.cancel();
    assert!(matches!(
        write_model_record(&mut output, 1, &view, &mut atoms, 1024, &cancellation),
        Err(RunError::JsonRecord(observation::ViewError::Stopped(
            Stop::Cancelled
        )))
    ));
    assert!(output.is_empty());
    assert_eq!(atoms.len(), 0);
}
