use super::*;

fn recorder(start: Instant) -> StageRecorder {
    StageRecorder {
        started: Some(start),
        state: Mutex::new(State::default()),
    }
}
fn finish(span: &mut StageSpan<'_>, now: Instant) {
    span.recorder.restore(span.previous, span.depth, now);
    span.enabled = false;
}
fn at(start: Instant, nanos: u64) -> Instant {
    start + Duration::from_nanos(nanos)
}

#[test]
fn nested_stages_partition_elapsed_time() {
    let start = Instant::now();
    let recorder = recorder(start);
    let mut outer = recorder.enter_at(SolveStage::Solving, Some(at(start, 2)));
    let mut inner = recorder.enter_at(SolveStage::Grounding, Some(at(start, 7)));
    let first = recorder.snapshot_at(start, at(start, 10));
    assert_eq!(
        first.get(SolveStage::Solving).unwrap().elapsed,
        Duration::from_nanos(5)
    );
    assert_eq!(
        first.get(SolveStage::Grounding).unwrap().elapsed,
        Duration::from_nanos(3)
    );
    finish(&mut inner, at(start, 12));
    finish(&mut outer, at(start, 20));
    let result = recorder.snapshot_at(start, at(start, 22));
    assert_eq!(
        result.get(SolveStage::Solving).unwrap().elapsed,
        Duration::from_nanos(13)
    );
    assert_eq!(
        result.get(SolveStage::Grounding).unwrap().elapsed,
        Duration::from_nanos(5)
    );
    assert_eq!(result.unattributed, Some(Duration::from_nanos(4)));
    assert!(result.is_complete());
}

#[test]
fn active_snapshots_do_not_charge_time_twice() {
    let start = Instant::now();
    let recorder = recorder(start);
    let mut outer = recorder.enter_at(SolveStage::Solving, Some(at(start, 2)));
    let mut inner = recorder.enter_at(SolveStage::Grounding, Some(at(start, 7)));
    let first = recorder.snapshot_at(start, at(start, 10));
    assert_eq!(first, recorder.snapshot_at(start, at(start, 10)));
    finish(&mut inner, at(start, 12));
    finish(&mut outer, at(start, 20));
    // Later accounting must still contain the active prefix exactly once.
    let result = recorder.snapshot_at(start, at(start, 22));
    assert_eq!(
        result.get(SolveStage::Solving).unwrap().elapsed,
        Duration::from_nanos(13)
    );
    assert_eq!(
        result.get(SolveStage::Grounding).unwrap().elapsed,
        Duration::from_nanos(5)
    );
    assert_eq!(result.unattributed, Some(Duration::from_nanos(4)));
    assert!(result.is_complete());
}

#[test]
fn entering_grounding_records_eager_mode() {
    let recorder = StageRecorder::new(true);
    drop(recorder.enter(SolveStage::Grounding));
    assert_eq!(
        recorder.snapshot().unwrap().grounding_mode,
        GroundingMode::Eager
    );
}

#[test]
fn span_entries_are_counted() {
    for (child, expected_calls) in [(SolveStage::Grounding, 1), (SolveStage::Solving, 2)] {
        let start = Instant::now();
        let recorder = recorder(start);
        let mut outer = recorder.enter_at(SolveStage::Solving, Some(at(start, 1)));
        let mut inner = recorder.enter_at(child, Some(at(start, 3)));
        finish(&mut inner, at(start, 8));
        finish(&mut outer, at(start, 10));
        assert_eq!(
            recorder
                .snapshot_at(start, at(start, 11))
                .get(SolveStage::Solving)
                .unwrap()
                .calls,
            expected_calls
        );
    }
}

#[test]
fn same_stage_nesting_partitions_elapsed_time() {
    let start = Instant::now();
    let recorder = recorder(start);
    let mut first = recorder.enter_at(SolveStage::Solving, Some(at(start, 1)));
    let mut second = recorder.enter_at(SolveStage::Solving, Some(at(start, 3)));
    finish(&mut second, at(start, 8));
    finish(&mut first, at(start, 10));
    let snapshot = recorder.snapshot_at(start, at(start, 11));
    let measurement = snapshot.get(SolveStage::Solving).unwrap();
    assert_eq!(measurement.elapsed, Duration::from_nanos(9));
    assert!(!measurement.overflowed);
    assert_eq!(snapshot.unattributed, Some(Duration::from_nanos(2)));
}

#[test]
fn lazy_grounding_has_no_separate_duration() {
    let recorder = StageRecorder::new(true);
    recorder.mark_lazy_grounding();
    let lazy = recorder.snapshot().unwrap();
    assert_eq!(lazy.grounding_mode, GroundingMode::LazyInterleaved);
    assert!(lazy.get(SolveStage::Grounding).is_none());
}

#[test]
fn mixed_grounding_retains_eager_measurements() {
    // Both encounter orders describe the same mixed route.
    for lazy_first in [true, false] {
        let recorder = StageRecorder::new(true);
        if lazy_first {
            recorder.mark_lazy_grounding();
        }
        drop(recorder.enter(SolveStage::Grounding));
        recorder.mark_lazy_grounding();
        let mixed = recorder.snapshot().unwrap();
        assert_eq!(mixed.grounding_mode, GroundingMode::Mixed);
        assert_eq!(mixed.get(SolveStage::Grounding).unwrap().calls, 1);
        assert!(mixed.is_complete());
    }
}

#[test]
fn disabled_recording_has_no_measurements() {
    let disabled = StageRecorder::new(false);
    let _span = disabled.enter(SolveStage::Grounding);
    disabled.mark_lazy_grounding();
    assert!(!disabled.enabled());
    assert!(disabled.snapshot().is_none());
    assert!(
        disabled
            .state
            .lock()
            .unwrap()
            .measurements
            .iter()
            .all(Option::is_none)
    );
}

#[test]
fn error_exit_restores_recording() {
    let enabled = StageRecorder::new(true);
    let failure = || {
        let _span = enabled.enter(SolveStage::Solving);
        Err::<(), _>(7)
    };
    assert_eq!(failure(), Err(7));
    let result = enabled.snapshot().unwrap();
    assert!(result.is_complete());
    assert_eq!(result.get(SolveStage::Solving).unwrap().calls, 1);
    assert_eq!(enabled.state.lock().unwrap().active, None);
}

#[test]
fn unwind_restores_recording() {
    let enabled = StageRecorder::new(true);
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _outer = enabled.enter(SolveStage::Solving);
        let _inner = enabled.enter(SolveStage::Grounding);
        panic!("controlled unwind");
    }));
    assert!(unwind.is_err());
    let result = enabled.snapshot().unwrap();
    assert!(result.is_complete());
    assert_eq!(result.get(SolveStage::Solving).unwrap().calls, 1);
    assert_eq!(enabled.state.lock().unwrap().active, None);
}

#[test]
fn out_of_order_drop_invalidates_partition() {
    let recorder = StageRecorder::new(true);
    let outer = recorder.enter(SolveStage::Solving);
    let inner = recorder.enter(SolveStage::Grounding);
    drop(outer);
    drop(inner);
    assert!(!recorder.snapshot().unwrap().is_complete());
}

#[test]
fn call_overflow_invalidates_partition() {
    let start = Instant::now();
    let recorder = recorder(start);
    let mut state = *recorder.state.lock().unwrap();
    state.measurements[SolveStage::Solving as usize] = Some(StageMeasurement {
        calls: u64::MAX,
        elapsed: Duration::ZERO,
        overflowed: false,
    });
    *recorder.state.lock().unwrap() = state;
    drop(recorder.enter(SolveStage::Solving));
    let result = recorder.snapshot().unwrap();
    assert!(!result.is_complete());
    assert!(result.get(SolveStage::Solving).unwrap().overflowed);
}

#[test]
fn duration_overflow_is_recorded() {
    let mut duration = StageMeasurement {
        elapsed: Duration::MAX,
        ..StageMeasurement::default()
    };
    duration.add(Duration::from_nanos(1));
    assert!(duration.overflowed);
}

#[test]
fn overflowing_duration_partition_is_incomplete() {
    let start = Instant::now();
    let recorder = recorder(start);
    let mut stage = recorder.enter_at(SolveStage::Solving, Some(at(start, 2)));
    finish(&mut stage, at(start, 7));
    let mut snapshot = recorder.snapshot_at(start, at(start, 10));
    assert!(snapshot.is_complete());
    snapshot.unattributed = Some(Duration::MAX);
    assert!(!snapshot.is_complete());
}

#[test]
fn overflowed_measurement_is_incomplete() {
    let mut snapshot = StageTimings {
        driver_elapsed: Duration::from_nanos(5),
        unattributed: Some(Duration::ZERO),
        ..StageTimings::default()
    };
    snapshot.measurements[SolveStage::Solving as usize] = Some(StageMeasurement {
        calls: u64::MAX,
        elapsed: Duration::from_nanos(5),
        overflowed: true,
    });
    assert!(!snapshot.is_complete());
}

#[test]
fn sequential_threads_preserve_exclusive_attribution() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<StageRecorder>();
    assert_send_sync::<StageSpan<'static>>();

    let recorder = StageRecorder::new(true);
    std::thread::scope(|scope| {
        scope
            .spawn(|| {
                let _outer = recorder.enter(SolveStage::Solving);
                let _inner = recorder.enter(SolveStage::Grounding);
            })
            .join()
            .unwrap();
        scope
            .spawn(|| drop(recorder.enter(SolveStage::Solving)))
            .join()
            .unwrap();
    });
    let snapshot = recorder.snapshot().unwrap();
    assert!(snapshot.is_complete());
    assert_eq!(snapshot.get(SolveStage::Solving).unwrap().calls, 2);
    assert_eq!(snapshot.get(SolveStage::Grounding).unwrap().calls, 1);
}

#[test]
fn transferring_a_live_guard_makes_exclusive_attribution_unavailable() {
    let recorder = StageRecorder::new(true);
    let guard = recorder.enter(SolveStage::Solving);
    std::thread::scope(|scope| scope.spawn(|| drop(guard)).join().unwrap());
    let snapshot = recorder.snapshot().unwrap();
    assert_eq!(snapshot.unattributed, None);
    assert!(!snapshot.is_complete());
    assert_eq!(snapshot.get(SolveStage::Solving).unwrap().calls, 1);
}

#[test]
fn poisoned_bookkeeping_does_not_change_application_control() {
    let recorder = StageRecorder::new(true);
    let span = recorder.enter(SolveStage::Solving);
    let unwind = std::panic::catch_unwind(|| {
        let _bookkeeping = recorder.state.lock().unwrap();
        panic!("controlled bookkeeping poison");
    });
    assert!(unwind.is_err());
    drop(span);
    // Every subsequent operation recovers without turning timing failure into
    // application failure. The partition remains explicitly unavailable.
    recorder.mark_lazy_grounding();
    drop(recorder.enter(SolveStage::Grounding));
    let snapshot = recorder.snapshot().unwrap();
    assert_eq!(snapshot.unattributed, None);
    assert!(!snapshot.is_complete());
    assert_eq!(snapshot.get(SolveStage::Solving).unwrap().calls, 1);
    assert_eq!(snapshot.get(SolveStage::Grounding).unwrap().calls, 1);
    assert_eq!(snapshot.grounding_mode, GroundingMode::Mixed);
}
