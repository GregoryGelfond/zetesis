//! A requested stop keeps its precedence over the certificate's resource refusals.

use super::{TightError, TightResource, Work, bytes};
use zetesis_cpu::{Cancellation, Stop};

#[test]
fn a_requested_stop_precedes_the_work_limit_between_boundaries() {
    let tick = |cancellation: &Cancellation| {
        Work {
            used: 7,
            max: 7,
            cancellation: cancellation.polling(),
        }
        .tick()
    };
    let cancellation = Cancellation::default();
    assert_eq!(
        tick(&cancellation),
        Err(TightError::Limit(TightResource::Work))
    );
    cancellation.cancel();
    assert_eq!(
        tick(&cancellation),
        Err(TightError::Stopped(Stop::Cancelled))
    );
}

#[test]
fn a_requested_stop_precedes_a_byte_refusal() {
    let cancellation = Cancellation::default();
    assert_eq!(bytes(8, 8, cancellation.polling()), Ok(8));
    assert_eq!(
        bytes(9, 8, cancellation.polling()),
        Err(TightError::Limit(TightResource::Bytes))
    );
    cancellation.cancel();
    assert_eq!(
        bytes(9, 8, cancellation.polling()),
        Err(TightError::Stopped(Stop::Cancelled))
    );
}
