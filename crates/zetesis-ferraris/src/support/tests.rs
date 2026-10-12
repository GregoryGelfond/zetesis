//! A requested stop keeps its precedence over construction refusals.

use super::{Builder, SupportError, SupportLimits};
use crate::{AdmissionError, AdmissionLimits, FormulaNodes};
use zetesis_cpu::{Cancellation, Refusal, Stop};

#[test]
fn a_requested_stop_precedes_the_work_limit_between_boundaries() {
    // Seven charged operations is not a metered boundary.
    let tick = |cancellation: &Cancellation| {
        Builder {
            limits: SupportLimits {
                admission: AdmissionLimits::default(),
                max_work: 7,
            },
            cancellation,
            work: 7,
            nodes: FormulaNodes::default(),
        }
        .tick()
    };
    let cancellation = Cancellation::default();
    assert_eq!(
        tick(&cancellation),
        Err(SupportError::Stopped(Stop::WorkLimit))
    );
    cancellation.cancel();
    assert_eq!(
        tick(&cancellation),
        Err(SupportError::Stopped(Stop::Cancelled))
    );
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
