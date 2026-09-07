use super::*;

fn recorder(start: Instant) -> StageRecorder {
    StageRecorder {
        started: Some(start),
        state: Cell::new(State::default()),
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
fn nested_stages_and_repeated_active_snapshots_have_an_exact_partition() {
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
    assert_eq!(first, recorder.snapshot_at(start, at(start, 10)));
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
    assert_eq!(result.get(SolveStage::Solving).unwrap().calls, 1);
    assert_eq!(result.grounding_mode, GroundingMode::Eager);
}

#[test]
fn same_stage_children_count_calls_without_double_charging_their_parent() {
    let start = Instant::now();
    let recorder = recorder(start);
    let mut first = recorder.enter_at(SolveStage::Solving, Some(at(start, 1)));
    let mut second = recorder.enter_at(SolveStage::Solving, Some(at(start, 3)));
    finish(&mut second, at(start, 8));
    finish(&mut first, at(start, 10));
    let snapshot = recorder.snapshot_at(start, at(start, 11));
    assert_eq!(
        snapshot.get(SolveStage::Solving),
        Some(StageMeasurement {
            calls: 2,
            elapsed: Duration::from_nanos(9),
            overflowed: false,
        })
    );
    assert_eq!(snapshot.unattributed, Some(Duration::from_nanos(2)));
}

#[test]
fn lazy_and_mixed_modes_never_invent_standalone_lazy_duration() {
    for lazy_first in [true, false] {
        let recorder = StageRecorder::new(true);
        if lazy_first {
            recorder.mark_lazy_grounding();
            let lazy = recorder.snapshot().unwrap();
            assert_eq!(lazy.grounding_mode, GroundingMode::LazyInterleaved);
            assert!(lazy.get(SolveStage::Grounding).is_none());
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
fn disabled_error_and_unwind_paths_preserve_application_results() {
    let disabled = StageRecorder::new(false);
    let _span = disabled.enter(SolveStage::Grounding);
    disabled.mark_lazy_grounding();
    assert!(!disabled.enabled());
    assert!(disabled.snapshot().is_none());
    assert!(
        disabled
            .state
            .get()
            .measurements
            .iter()
            .all(Option::is_none)
    );
    let enabled = StageRecorder::new(true);
    let failure = || {
        let _span = enabled.enter(SolveStage::Solving);
        Err::<(), _>(7)
    };
    assert_eq!(failure(), Err(7));
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _outer = enabled.enter(SolveStage::Solving);
        let _inner = enabled.enter(SolveStage::Grounding);
        panic!("controlled unwind");
    }));
    assert!(unwind.is_err());
    let result = enabled.snapshot().unwrap();
    assert!(result.is_complete());
    assert_eq!(result.get(SolveStage::Solving).unwrap().calls, 2);
    assert_eq!(enabled.state.get().active, None);
}

#[test]
fn bad_nesting_and_overflow_make_the_partition_unavailable() {
    let recorder = StageRecorder::new(true);
    let outer = recorder.enter(SolveStage::Solving);
    let inner = recorder.enter(SolveStage::Grounding);
    drop(outer);
    drop(inner);
    assert!(!recorder.snapshot().unwrap().is_complete());
    let start = Instant::now();
    let recorder = super::tests::recorder(start);
    let mut state = recorder.state.get();
    state.measurements[SolveStage::Solving as usize] = Some(StageMeasurement {
        calls: u64::MAX,
        elapsed: Duration::ZERO,
        overflowed: false,
    });
    recorder.state.set(state);
    drop(recorder.enter(SolveStage::Solving));
    let result = recorder.snapshot().unwrap();
    assert!(!result.is_complete());
    assert!(result.get(SolveStage::Solving).unwrap().overflowed);
    let mut duration = StageMeasurement {
        elapsed: Duration::MAX,
        ..StageMeasurement::default()
    };
    duration.add(Duration::from_nanos(1));
    assert!(duration.overflowed);
}
