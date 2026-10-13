//! The construction work limit observes a requested stop before refusing, a
//! node refusal between metered boundaries reports itself, and settled where a
//! metered operation returns, as construction's own refusals are, it becomes a
//! requested stop.

use super::{Builder, SupportError, SupportLimits};
use crate::{AdmissionError, AdmissionLimits, FormulaNodes, NodeView};
use zetesis_cpu::{Cancellation, Refusal, Stop};

/// Run `test` on a builder whose meter has admitted seven operations, which is
/// not a metered boundary; with `stop`, a stop is requested once it has. The
/// metered exit settles what `test` returns.
fn with_builder<T>(
    limits: SupportLimits,
    stop: bool,
    test: impl FnOnce(&mut Builder<'_, '_>) -> Result<T, SupportError>,
) -> Result<T, SupportError> {
    let cancellation = Cancellation::default();
    cancellation
        .metered(limits.max_work, |meter| {
            for _ in 0..7 {
                meter.charge().unwrap();
            }
            if stop {
                cancellation.cancel();
            }
            test(&mut Builder {
                limits,
                meter,
                nodes: FormulaNodes::default(),
            })
        })
        .result
}

#[test]
fn a_requested_stop_precedes_the_work_limit_between_boundaries() {
    let limits = SupportLimits {
        admission: AdmissionLimits::default(),
        max_work: 7,
    };
    // The tick's own report, returned as a value so the exit leaves it alone.
    let tick = |stop| with_builder(limits, stop, |builder| Ok(builder.tick())).unwrap();
    assert_eq!(tick(false), Err(SupportError::Stopped(Stop::WorkLimit)));
    assert_eq!(tick(true), Err(SupportError::Stopped(Stop::Cancelled)));
}

#[test]
fn a_node_refusal_settles_to_a_requested_stop() {
    let limits = SupportLimits {
        admission: AdmissionLimits {
            max_nodes: 0,
            ..AdmissionLimits::default()
        },
        max_work: u64::MAX,
    };
    let settled = with_builder(limits, true, |builder| {
        let refusal = builder.push(NodeView::False);
        assert_eq!(refusal, Err(SupportError::Admission(AdmissionError::Limit)));
        refusal
    });
    assert_eq!(settled, Err(SupportError::Stopped(Stop::Cancelled)));
}

#[test]
fn support_errors_classify_resource_refusals() {
    for refusal in [
        SupportError::Admission(AdmissionError::Limit),
        SupportError::Admission(AdmissionError::Allocation),
        SupportError::Stopped(Stop::WorkLimit),
    ] {
        assert!(refusal.is_resource_refusal(), "{refusal:?}");
    }
    for shape in [
        SupportError::Admission(AdmissionError::Root),
        SupportError::Stopped(Stop::Cancelled),
    ] {
        assert!(!shape.is_resource_refusal(), "{shape:?}");
    }
}
