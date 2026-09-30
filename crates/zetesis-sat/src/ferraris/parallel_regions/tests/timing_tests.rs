//! Parallel timing preserves explicit incompleteness without changing search.

use std::sync::{Arc, Barrier};
use std::time::Duration;

use super::{Live, PhaseMeasurement, SearchPhaseTimings};

fn measurement(calls: u64, elapsed: Duration) -> PhaseMeasurement {
    PhaseMeasurement {
        calls,
        elapsed,
        overflowed: false,
    }
}

fn publish(live: &Live, before: PhaseMeasurement, after: PhaseMeasurement) {
    live.add_timings(
        &SearchPhaseTimings {
            candidates: before,
            ..SearchPhaseTimings::default()
        },
        &SearchPhaseTimings {
            candidates: after,
            ..SearchPhaseTimings::default()
        },
    );
}

#[test]
fn worker_timing_overflow_reaches_the_snapshot() {
    let live = Live::default();
    let after = PhaseMeasurement {
        overflowed: true,
        ..measurement(1, Duration::from_nanos(7))
    };
    publish(&live, PhaseMeasurement::default(), after);
    assert_eq!(live.timings().candidates, after);
    assert_eq!(live.timings().reduct, PhaseMeasurement::default());
}

#[test]
fn a_zero_delta_preserves_new_worker_overflow() {
    let live = Live::default();
    let before = measurement(1, Duration::from_nanos(7));
    publish(&live, PhaseMeasurement::default(), before);
    let after = PhaseMeasurement {
        overflowed: true,
        ..before
    };
    publish(&live, before, after);
    assert_eq!(live.timings().candidates, after);
}

#[test]
fn nanosecond_conversion_overflow_is_explicit() {
    let live = Live::default();
    let elapsed = Duration::from_nanos(u64::MAX)
        .checked_add(Duration::from_nanos(1))
        .unwrap();
    publish(&live, PhaseMeasurement::default(), measurement(1, elapsed));
    let result = live.timings().candidates;
    assert_eq!(result.calls, 1);
    assert_eq!(result.elapsed, Duration::from_nanos(u64::MAX));
    assert!(result.overflowed);
}

#[test]
fn aggregate_timing_calls_saturate_explicitly() {
    let live = Live::default();
    for calls in [u64::MAX, 1] {
        publish(
            &live,
            PhaseMeasurement::default(),
            measurement(calls, Duration::ZERO),
        );
    }
    let result = live.timings().candidates;
    assert_eq!(result.calls, u64::MAX);
    assert_eq!(result.elapsed, Duration::ZERO);
    assert!(result.overflowed);
}

#[test]
fn aggregate_timing_nanoseconds_saturate_explicitly() {
    let live = Live::default();
    for nanos in [u64::MAX, 1] {
        publish(
            &live,
            PhaseMeasurement::default(),
            measurement(1, Duration::from_nanos(nanos)),
        );
    }
    let result = live.timings().candidates;
    assert_eq!(result.calls, 2);
    assert_eq!(result.elapsed, Duration::from_nanos(u64::MAX));
    assert!(result.overflowed);
}

#[test]
fn aggregate_timing_overflow_remains_sticky() {
    let live = Live::default();
    for calls in [u64::MAX, 1, 0, 1] {
        publish(
            &live,
            PhaseMeasurement::default(),
            measurement(calls, Duration::ZERO),
        );
    }
    let result = live.timings().candidates;
    assert_eq!(result.calls, u64::MAX);
    assert!(result.overflowed);
}

#[test]
fn unchanged_timing_snapshots_add_no_measurement() {
    let live = Live::default();
    let first = measurement(1, Duration::from_nanos(7));
    let second = measurement(2, Duration::from_nanos(11));
    publish(&live, PhaseMeasurement::default(), first);
    publish(&live, first, second);
    publish(&live, second, second);
    assert_eq!(live.timings().candidates, second);
}

#[test]
fn joined_timing_updates_preserve_every_contribution() {
    const WORKERS: u64 = 4;
    const CALLS: u64 = 2_000;
    let live = Live::default();
    let start = Arc::new(Barrier::new(usize::try_from(WORKERS).unwrap()));
    std::thread::scope(|scope| {
        for _ in 0..WORKERS {
            let live = &live;
            let start = Arc::clone(&start);
            scope.spawn(move || {
                start.wait();
                for _ in 0..CALLS {
                    publish(
                        live,
                        PhaseMeasurement::default(),
                        measurement(1, Duration::from_nanos(7)),
                    );
                }
            });
        }
    });
    assert_eq!(
        live.timings().candidates,
        measurement(WORKERS * CALLS, Duration::from_nanos(WORKERS * CALLS * 7)),
    );
}
