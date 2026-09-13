//! Exercise the actual wait loop with a deterministic clock and poll operation.

use super::{Effects, GpuError, GpuErrorKind, SubmissionWait, finish_read, wait_for_submission};
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
    .outcome
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
        .outcome
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
    .outcome
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
            Ok(wgpu::PollStatus::WaitSucceeded)
        },
        || Duration::ZERO,
    )
    .outcome
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
    .outcome
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
    .outcome
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
        |_| Ok(wgpu::PollStatus::WaitSucceeded),
        || Duration::ZERO,
    )
    .outcome
    .unwrap();
    assert_eq!(controls, 2);
}

#[test]
fn only_completed_waits_record_queue_settlement() {
    for completed in [false, true] {
        let mut polls = 0;
        let result = wait_for_submission(
            Duration::ZERO,
            || Ok(()),
            |_| {
                polls += 1;
                if completed {
                    Ok(wgpu::PollStatus::WaitSucceeded)
                } else {
                    Err(wgpu::PollError::Timeout)
                }
            },
            || Duration::ZERO,
        );
        assert_eq!(polls, 1);
        assert_eq!(result.completed, completed);
        assert_eq!(result.outcome.is_ok(), completed);
    }
}

#[test]
fn poll_only_status_does_not_establish_completion() {
    let wait = wait_for_submission(
        Duration::ZERO,
        || Ok(()),
        |_| Ok(wgpu::PollStatus::Poll),
        || Duration::ZERO,
    );
    assert!(!wait.completed);
    assert_eq!(wait.outcome.unwrap_err().kind(), GpuErrorKind::Device);
    let wait = wait_for_submission(
        Duration::ZERO,
        || Ok(()),
        |_| Ok(wgpu::PollStatus::QueueEmpty),
        || Duration::ZERO,
    );
    assert!(wait.completed);
    assert!(wait.outcome.is_ok());
}

#[test]
fn late_wait_stops_need_successful_mapping_and_release() {
    for stop in [Stop::Cancelled, Stop::Deadline] {
        for completed in [false, true] {
            for map_ok in [false, true] {
                let mapped = Cell::new(false);
                let released = Cell::new(false);
                let completion = finish_read::<()>(
                    SubmissionWait {
                        outcome: Err(GpuError::interrupted(stop)),
                        completed,
                    },
                    || {
                        mapped.set(true);
                        if map_ok {
                            Ok(())
                        } else {
                            Err(GpuError::new(GpuErrorKind::Readback, "mapping"))
                        }
                    },
                    || panic!("stopped wait must not decode"),
                    || {
                        assert!(!released.replace(true));
                    },
                    || panic!("stopped wait must preserve its original stop"),
                );
                assert!(released.get());
                assert_eq!(mapped.get(), completed);
                assert_eq!(completion.outcome.unwrap_err().interruption(), Some(stop));
                assert_eq!(
                    completion.effects,
                    if completed && map_ok {
                        Effects::SubmittedAndReleased
                    } else {
                        Effects::MayBeLive
                    }
                );
            }
        }
    }
}

#[test]
fn decoded_output_is_discarded_before_a_late_stop_escapes() {
    for stop in [Stop::Cancelled, Stop::Deadline] {
        let stage = Cell::new(0);
        let dropped = Cell::new(false);
        let completion = finish_read(
            SubmissionWait {
                outcome: Ok(()),
                completed: true,
            },
            || {
                assert_eq!(stage.replace(1), 0);
                Ok(())
            },
            || {
                assert_eq!(stage.replace(2), 1);
                Ok(Output(&dropped))
            },
            || {
                assert_eq!(stage.replace(4), 3);
                assert!(dropped.get());
            },
            || {
                assert_eq!(stage.replace(3), 2);
                Err(GpuError::interrupted(stop))
            },
        );
        assert_eq!(stage.get(), 4);
        assert_eq!(completion.effects, Effects::SubmittedAndReleased);
        assert_eq!(completion.outcome.err().unwrap().interruption(), Some(stop));
    }
}

struct Output<'a>(&'a Cell<bool>);
impl Drop for Output<'_> {
    fn drop(&mut self) {
        self.0.set(true);
    }
}

#[test]
fn map_and_decoder_failures_keep_their_original_causes() {
    for map_ok in [false, true] {
        let released = Cell::new(false);
        let completion = finish_read::<()>(
            SubmissionWait {
                outcome: Ok(()),
                completed: true,
            },
            || {
                if map_ok {
                    Ok(())
                } else {
                    Err(GpuError::new(GpuErrorKind::Readback, "mapping"))
                }
            },
            || Err(GpuError::new(GpuErrorKind::Readback, "decoding")),
            || released.set(true),
            || panic!("failed read must precede late control"),
        );
        assert!(released.get());
        let error = completion.outcome.unwrap_err();
        assert_eq!(error.kind(), GpuErrorKind::Readback);
        assert_eq!(error.detail(), if map_ok { "decoding" } else { "mapping" });
        assert_eq!(
            completion.effects,
            if map_ok {
                Effects::SubmittedAndReleased
            } else {
                Effects::MayBeLive
            }
        );
    }
}
