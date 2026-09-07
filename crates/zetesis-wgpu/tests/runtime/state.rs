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
fn first_fault_is_bounded_and_permanently_invalidates_the_shared_lifecycle() {
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
    assert!(faults.invalidated);
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
fn validation_health_and_execution_precedence_never_recover_after_failure() {
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
                    assert!(faults.invalidated);
                    assert!(faults.check().is_err());
                } else {
                    assert_eq!(result.unwrap(), 42);
                    assert!(!faults.invalidated);
                    assert_eq!(faults.complete(Ok(()), Ok(43)).unwrap(), 43);
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
