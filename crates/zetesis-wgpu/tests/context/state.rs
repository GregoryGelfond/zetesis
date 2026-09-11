use super::*;

fn state() -> (mpsc::SyncSender<String>, Faults) {
    let (sender, receiver) = mpsc::sync_channel(1);
    (
        sender,
        Faults {
            receiver,
            invalidated: false,
        },
    )
}
fn error(kind: GpuErrorKind, detail: &str) -> GpuError {
    GpuError::new(kind, detail)
}

#[test]
fn callbacks_retain_only_the_first_fault() {
    let (sender, mut faults) = state();
    assert!(faults.check().is_ok());
    sender
        .try_send("first actual callback message".into())
        .unwrap();
    assert!(matches!(
        sender.try_send("later message".into()),
        Err(mpsc::TrySendError::Full(_))
    ));
    let first = faults.check().unwrap_err();
    assert_eq!(first.kind(), GpuErrorKind::Device);
    assert_eq!(first.detail(), "first actual callback message");
}

#[test]
fn observed_fault_permanently_invalidates_context() {
    let (sender, mut faults) = state();
    sender.try_send("device lost".into()).unwrap();
    assert_eq!(faults.check().unwrap_err().kind(), GpuErrorKind::Device);
    assert!(
        faults
            .check()
            .unwrap_err()
            .detail()
            .contains("earlier execution failure")
    );
    assert!(faults.complete(Ok(()), Ok(42)).is_err());
}

#[test]
fn completion_respects_failure_precedence() {
    for callback in [false, true] {
        for validation in [false, true] {
            for execution in [false, true] {
                let (sender, mut faults) = state();
                if callback {
                    sender.try_send("callback".into()).unwrap();
                }
                let validation_result = if validation {
                    Err(error(GpuErrorKind::Validation, "scope"))
                } else {
                    Ok(())
                };
                let outcome = if execution {
                    Err(error(GpuErrorKind::Readback, "decode"))
                } else {
                    Ok(42)
                };
                let result = faults.complete(validation_result, outcome);
                let expected = if validation {
                    Some(GpuErrorKind::Validation)
                } else if callback {
                    Some(GpuErrorKind::Device)
                } else if execution {
                    Some(GpuErrorKind::Readback)
                } else {
                    None
                };
                if let Some(kind) = expected {
                    assert_eq!(result.unwrap_err().kind(), kind);
                } else {
                    assert_eq!(result.unwrap(), 42);
                    assert!(!faults.invalidated);
                }
                // Health is polled even when a scope or execution result fails.
                if callback {
                    assert!(matches!(
                        faults.receiver.try_recv(),
                        Err(mpsc::TryRecvError::Empty)
                    ));
                }
            }
        }
    }
}

#[test]
fn cancellation_never_hides_a_scope_failure() {
    let (_, mut faults) = state();
    let error = faults
        .complete::<()>(
            Err(error(GpuErrorKind::Validation, "scope")),
            Err(GpuError::interrupted(zetesis_cpu::Stop::Cancelled)),
        )
        .unwrap_err();
    assert_eq!(error.kind(), GpuErrorKind::Validation);
    assert_eq!(error.interruption, None);
}

#[test]
fn cancellation_never_hides_a_device_fault() {
    let (sender, mut faults) = state();
    sender.try_send("device lost".into()).unwrap();
    let error = faults
        .complete::<()>(
            Ok(()),
            Err(GpuError::interrupted(zetesis_cpu::Stop::Cancelled)),
        )
        .unwrap_err();
    assert_eq!(error.interruption, None);
    assert_eq!(error.detail(), "device lost");
}

#[test]
fn cancellation_survives_successful_scope_completion() {
    let (_, mut faults) = state();
    let error = faults
        .complete::<()>(
            Ok(()),
            Err(GpuError::interrupted(zetesis_cpu::Stop::Cancelled)),
        )
        .unwrap_err();
    assert_eq!(error.interruption, Some(zetesis_cpu::Stop::Cancelled));
    assert!(faults.invalidated);
}

#[test]
fn overlapping_leases_refuse_without_consuming_faults() {
    let (sender, faults) = state();
    let lifecycle = Arc::new(Lifecycle::new(faults));
    let other = Arc::clone(&lifecycle);
    let lease = lifecycle.lease().unwrap();
    sender.try_send("pending device fault".into()).unwrap();
    let Err(error) = other.lease() else {
        panic!("overlapping operation acquired the context");
    };
    assert_eq!(error.kind(), GpuErrorKind::Busy);
    assert!(!lifecycle.faults.lock().unwrap().invalidated);
    drop(lease);
    let _lease = other.lease().unwrap();
    assert_eq!(other.check().unwrap_err().detail(), "pending device fault");
}

#[test]
fn completed_preflight_releases_a_healthy_context() {
    let (_, faults) = state();
    let lifecycle = Lifecycle::new(faults);
    let refuse = || -> Result<(), GpuError> {
        let _lease = lifecycle.lease()?;
        lifecycle.check()?;
        Err(error(GpuErrorKind::Capacity, "host preflight"))
    };
    assert_eq!(refuse().unwrap_err().kind(), GpuErrorKind::Capacity);
    let _lease = lifecycle.lease().unwrap();
    assert_eq!(lifecycle.complete(Ok(()), Ok(42)).unwrap(), 42);
}

#[test]
fn execution_failure_invalidates_every_context_client() {
    let (_, faults) = state();
    let lifecycle = Arc::new(Lifecycle::new(faults));
    let other = Arc::clone(&lifecycle);
    {
        let _lease = lifecycle.lease().unwrap();
        let failure = lifecycle
            .complete::<()>(Ok(()), Err(error(GpuErrorKind::Readback, "invalid epoch")))
            .unwrap_err();
        assert_eq!(failure.kind(), GpuErrorKind::Readback);
    }
    let _lease = other.lease().unwrap();
    assert_eq!(other.check().unwrap_err().kind(), GpuErrorKind::Device);
}

#[test]
fn unwinding_a_lease_prevents_context_reuse() {
    let (_, faults) = state();
    let lifecycle = Lifecycle::new(faults);
    let failed = std::panic::catch_unwind(|| {
        let _lease = lifecycle.lease().unwrap();
        panic!("abandoned execution");
    });
    assert!(failed.is_err());
    let _lease = lifecycle.lease().unwrap();
    assert_eq!(lifecycle.check().unwrap_err().kind(), GpuErrorKind::Device);
}

#[test]
fn poisoned_health_is_never_recovered() {
    let (_, faults) = state();
    let lifecycle = Lifecycle::new(faults);
    let failed = std::panic::catch_unwind(|| {
        let _state = lifecycle.faults.lock().unwrap();
        panic!("health update interrupted");
    });
    assert!(failed.is_err());
    lifecycle.invalidate();
    assert_eq!(lifecycle.check().unwrap_err().kind(), GpuErrorKind::Device);
    assert_eq!(
        lifecycle.complete(Ok(()), Ok(42)).unwrap_err().kind(),
        GpuErrorKind::Device
    );
}

#[test]
fn another_thread_cannot_overlap_an_active_lease() {
    let (_, faults) = state();
    let lifecycle = Lifecycle::new(faults);
    let lease = lifecycle.lease().unwrap();
    std::thread::scope(|scope| {
        scope
            .spawn(|| {
                let Err(error) = lifecycle.lease() else {
                    panic!("parallel operation acquired the context");
                };
                assert_eq!(error.kind(), GpuErrorKind::Busy);
            })
            .join()
            .unwrap();
    });
    drop(lease);
    assert!(lifecycle.lease().is_ok());
    assert!(lifecycle.check().is_ok());
}

#[test]
fn independent_lifecycles_do_not_share_invalidation() {
    let (_, first) = state();
    let (_, second) = state();
    let first = Lifecycle::new(first);
    let second = Lifecycle::new(second);
    first.invalidate();
    assert_eq!(first.check().unwrap_err().kind(), GpuErrorKind::Device);
    let _lease = second.lease().unwrap();
    assert_eq!(second.complete(Ok(()), Ok(42)).unwrap(), 42);
}
