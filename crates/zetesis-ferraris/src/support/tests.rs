//! A requested stop keeps its precedence over the construction work limit.

use super::{Builder, SupportError, SupportLimits};
use crate::{AdmissionLimits, FormulaNodes};
use zetesis_cpu::{Cancellation, Stop};

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
