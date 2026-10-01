//! Recorder imports preserve bounded, independent measurement prefixes.

use std::time::Duration;

use super::{PhaseMeasurement, Recorder, SearchPhaseTimings, SolvePhase};

#[test]
fn disabled_details_never_start_a_phase_clock() {
    for recorder in [Recorder::new(false), Recorder::stages_only()] {
        for phase in SolvePhase::ALL {
            let span = recorder.start(phase);
            assert!(span.started.is_none(), "{phase:?}");
            drop(span);
        }
    }
}

#[test]
fn stages_only_ignores_detailed_search_imports() {
    let recorder = Recorder::stages_only();
    recorder.search(
        SearchPhaseTimings::default(),
        SearchPhaseTimings {
            candidates: PhaseMeasurement {
                calls: 2,
                elapsed: Duration::from_nanos(7),
                overflowed: false,
            },
            ..SearchPhaseTimings::default()
        },
    );
    assert!(
        recorder
            .snapshot()
            .unwrap()
            .get(SolvePhase::CandidateGeneration)
            .is_none()
    );
}

#[test]
fn disabled_recorder_and_returned_errors_do_not_invent_semantic_completion() {
    let disabled = Recorder::new(false);
    assert_eq!(
        disabled.measure(SolvePhase::AdmissionMaterialization, || Err::<(), _>(7)),
        Err(7)
    );
    disabled.search(
        SearchPhaseTimings::default(),
        SearchPhaseTimings {
            candidates: PhaseMeasurement {
                calls: 1,
                ..PhaseMeasurement::default()
            },
            ..SearchPhaseTimings::default()
        },
    );
    assert!(disabled.snapshot().is_none());
    let active = Recorder::new(true);
    assert_eq!(
        active.measure(SolvePhase::AdmissionMaterialization, || Err::<(), _>(7)),
        Err(7)
    );
    assert_eq!(
        active
            .snapshot()
            .unwrap()
            .get(SolvePhase::AdmissionMaterialization)
            .unwrap()
            .calls,
        1
    );
    assert!(
        active
            .snapshot()
            .unwrap()
            .get(SolvePhase::CandidateGeneration)
            .is_none()
    );
}

#[test]
fn repeated_search_snapshots_do_not_double_count() {
    let active = Recorder::new(true);
    let measurement = PhaseMeasurement {
        calls: 2,
        elapsed: Duration::from_nanos(7),
        overflowed: false,
    };
    let timing = SearchPhaseTimings {
        candidates: measurement,
        ..SearchPhaseTimings::default()
    };
    active.search(SearchPhaseTimings::default(), timing);
    active.search(timing, timing);
    assert_eq!(
        active
            .snapshot()
            .unwrap()
            .get(SolvePhase::CandidateGeneration),
        Some(measurement)
    );
    assert!(
        active
            .snapshot()
            .unwrap()
            .get(SolvePhase::ExactReductMembership)
            .is_none()
    );
    active.search(
        timing,
        SearchPhaseTimings {
            reduct: PhaseMeasurement {
                overflowed: true,
                ..PhaseMeasurement::default()
            },
            ..timing
        },
    );
    assert!(
        active
            .snapshot()
            .unwrap()
            .get(SolvePhase::ExactReductMembership)
            .unwrap()
            .overflowed
    );
}

#[test]
fn search_imports_preserve_independent_prefixes() {
    let active = Recorder::new(true);
    let first = SearchPhaseTimings {
        candidates: PhaseMeasurement {
            calls: 2,
            elapsed: Duration::from_nanos(7),
            overflowed: false,
        },
        ..SearchPhaseTimings::default()
    };
    let second = SearchPhaseTimings {
        candidates: PhaseMeasurement {
            calls: 3,
            elapsed: Duration::from_nanos(11),
            overflowed: false,
        },
        ..SearchPhaseTimings::default()
    };
    let advanced = SearchPhaseTimings {
        candidates: PhaseMeasurement {
            calls: 4,
            elapsed: Duration::from_nanos(13),
            overflowed: false,
        },
        ..SearchPhaseTimings::default()
    };
    active.search(SearchPhaseTimings::default(), first);
    active.search(SearchPhaseTimings::default(), second);
    active.search(first, advanced);
    assert_eq!(
        active
            .snapshot()
            .unwrap()
            .get(SolvePhase::CandidateGeneration),
        Some(PhaseMeasurement {
            calls: 7,
            elapsed: Duration::from_nanos(24),
            overflowed: false,
        })
    );
}

#[test]
fn regressing_search_snapshots_mark_incomplete() {
    for (first, second) in [(5, 3), (3, 5)] {
        let active = Recorder::new(true);
        let previous = SearchPhaseTimings {
            candidates: PhaseMeasurement {
                calls: first,
                elapsed: Duration::from_nanos(second),
                overflowed: false,
            },
            ..SearchPhaseTimings::default()
        };
        let current = SearchPhaseTimings {
            candidates: PhaseMeasurement {
                calls: second,
                elapsed: Duration::from_nanos(first),
                overflowed: false,
            },
            ..SearchPhaseTimings::default()
        };
        active.search(SearchPhaseTimings::default(), previous);
        active.search(previous, current);
        let retained = active
            .snapshot()
            .unwrap()
            .get(SolvePhase::CandidateGeneration)
            .unwrap();
        assert!(retained.overflowed);
        assert_eq!(retained.calls, previous.candidates.calls);
        assert_eq!(retained.elapsed, previous.candidates.elapsed);
    }
}

#[test]
fn search_sum_overflow_preserves_the_known_prefix() {
    for prefix in [
        PhaseMeasurement {
            calls: u64::MAX,
            elapsed: Duration::ZERO,
            overflowed: false,
        },
        PhaseMeasurement {
            calls: 1,
            elapsed: Duration::MAX,
            overflowed: false,
        },
    ] {
        let active = Recorder::new(true);
        active.search(
            SearchPhaseTimings::default(),
            SearchPhaseTimings {
                candidates: prefix,
                ..SearchPhaseTimings::default()
            },
        );
        let additional = SearchPhaseTimings {
            candidates: PhaseMeasurement {
                calls: 1,
                elapsed: Duration::from_nanos(1),
                overflowed: false,
            },
            ..SearchPhaseTimings::default()
        };
        active.search(SearchPhaseTimings::default(), additional);
        active.search(SearchPhaseTimings::default(), additional);
        let retained = active
            .snapshot()
            .unwrap()
            .get(SolvePhase::CandidateGeneration)
            .unwrap();
        assert_eq!(
            retained,
            PhaseMeasurement {
                overflowed: true,
                ..prefix
            }
        );
    }
}

#[test]
fn upstream_overflow_preserves_its_known_prefix() {
    let active = Recorder::new(true);
    let incomplete = SearchPhaseTimings {
        reduct: PhaseMeasurement {
            calls: 2,
            elapsed: Duration::from_nanos(7),
            overflowed: true,
        },
        ..SearchPhaseTimings::default()
    };
    active.search(SearchPhaseTimings::default(), incomplete);
    active.search(incomplete, incomplete);
    assert_eq!(
        active
            .snapshot()
            .unwrap()
            .get(SolvePhase::ExactReductMembership),
        Some(incomplete.reduct)
    );
}

#[test]
fn poisoned_phase_bookkeeping_retains_only_incomplete_prefixes() {
    let recorder = Recorder::new(true);
    recorder.measure(SolvePhase::ExecutionSetup, || ());
    let prefix = recorder
        .snapshot()
        .unwrap()
        .get(SolvePhase::ExecutionSetup)
        .unwrap();
    let failure = std::panic::catch_unwind(|| {
        let _record = recorder.measurements.lock().unwrap();
        panic!("injected bookkeeping failure");
    });
    assert!(failure.is_err());
    assert_eq!(
        recorder.snapshot().unwrap().get(SolvePhase::ExecutionSetup),
        Some(PhaseMeasurement {
            overflowed: true,
            ..prefix
        })
    );
    assert_eq!(recorder.measure(SolvePhase::CandidateSetup, || 17), 17);
    recorder.search(
        SearchPhaseTimings::default(),
        SearchPhaseTimings {
            candidates: PhaseMeasurement {
                calls: 1,
                elapsed: Duration::from_nanos(7),
                overflowed: false,
            },
            ..SearchPhaseTimings::default()
        },
    );
    let snapshot = recorder.snapshot().unwrap();
    for phase in [SolvePhase::CandidateSetup, SolvePhase::CandidateGeneration] {
        assert_eq!(
            snapshot.get(phase),
            Some(PhaseMeasurement {
                overflowed: true,
                ..PhaseMeasurement::default()
            })
        );
    }
    assert!(snapshot.get(SolvePhase::ExactReductMembership).is_none());
}
