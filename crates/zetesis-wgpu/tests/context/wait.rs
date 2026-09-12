//! Exercise the actual wait loop with a deterministic clock and poll operation.

use super::{GpuError, GpuErrorKind, wait_for_submission};
use std::{cell::Cell, time::Duration};
use zetesis_cpu::{Control, Stop};

#[test]
fn wait_quanta_respect_the_remaining_timeout() {
    let elapsed = Cell::new(Duration::ZERO);
    let mut waits = Vec::new();
    let error = wait_for_submission(
        Duration::from_millis(120),
        || Ok(()),
        |wait| {
            waits.push(wait);
            elapsed.set(elapsed.get() + wait);
            Err(wgpu::PollError::Timeout)
        },
        || elapsed.get(),
    )
    .unwrap_err();
    assert_eq!(waits, [50, 50, 20].map(Duration::from_millis));
    assert_eq!(error.kind(), GpuErrorKind::Timeout);
    assert_eq!(error.interruption(), None);
}

#[test]
fn control_refusal_prevents_a_device_wait() {
    for control in [Control::with_deadline(std::time::Instant::now()), {
        let control = Control::default();
        control.cancel();
        control
    }] {
        let stop = control.poll().unwrap_err();
        let error = wait_for_submission(
            Duration::from_secs(1),
            || control.poll().map_err(GpuError::interrupted),
            |_| panic!("stopped control must precede device polling"),
            || Duration::ZERO,
        )
        .unwrap_err();
        assert_eq!(error.kind(), GpuErrorKind::Interrupted);
        assert_eq!(error.interruption(), Some(stop));
    }
}

#[test]
fn cancellation_is_observed_between_pending_waits() {
    let control = Control::default();
    let mut polls = 0;
    let error = wait_for_submission(
        Duration::from_secs(1),
        || control.poll().map_err(GpuError::interrupted),
        |wait| {
            assert_eq!(wait, Duration::from_millis(50));
            polls += 1;
            control.cancel();
            Err(wgpu::PollError::Timeout)
        },
        || Duration::ZERO,
    )
    .unwrap_err();
    assert_eq!(polls, 1);
    assert_eq!(error.interruption(), Some(Stop::Cancelled));
}

#[test]
fn a_completed_poll_does_not_hide_new_cancellation() {
    let control = Control::default();
    let error = wait_for_submission(
        Duration::from_secs(1),
        || control.poll().map_err(GpuError::interrupted),
        |_| {
            control.cancel();
            Ok(())
        },
        || Duration::ZERO,
    )
    .unwrap_err();
    assert_eq!(error.interruption(), Some(Stop::Cancelled));
}

#[test]
fn a_device_poll_failure_precedes_later_control_refusal() {
    let control = Control::default();
    let error = wait_for_submission(
        Duration::from_secs(1),
        || control.poll().map_err(GpuError::interrupted),
        |_| {
            control.cancel();
            Err(wgpu::PollError::WrongSubmissionIndex(2, 1))
        },
        || Duration::ZERO,
    )
    .unwrap_err();
    assert_eq!(error.kind(), GpuErrorKind::Device);
    assert_eq!(error.interruption(), None);
}

#[test]
fn zero_timeout_performs_only_a_nonblocking_poll() {
    let mut polls = 0;
    let error = wait_for_submission(
        Duration::ZERO,
        || Ok(()),
        |wait| {
            assert_eq!(wait, Duration::ZERO);
            polls += 1;
            Err(wgpu::PollError::Timeout)
        },
        || Duration::ZERO,
    )
    .unwrap_err();
    assert_eq!(polls, 1);
    assert_eq!(error.kind(), GpuErrorKind::Timeout);
}

#[test]
fn successful_completion_keeps_the_wait_result() {
    let mut controls = 0;
    wait_for_submission(
        Duration::from_secs(1),
        || {
            controls += 1;
            Ok(())
        },
        |_| Ok(()),
        || Duration::ZERO,
    )
    .unwrap();
    assert_eq!(controls, 2);
}
