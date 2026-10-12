//! A requested stop keeps its precedence over the certificate's resource refusals.

use super::{Charged, TightError, TightResource, Work, bytes};
use zetesis_cpu::{Cancellation, Refusal, Stop};

#[test]
fn a_requested_stop_precedes_the_work_limit_between_boundaries() {
    let tick = |cancellation: &Cancellation| {
        Work {
            used: Charged(7),
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
fn a_byte_refusal_settles_to_a_requested_stop() {
    assert_eq!(bytes(8, 8), Ok(8));
    let refusal = bytes(9, 8);
    assert_eq!(refusal, Err(TightError::Limit(TightResource::Bytes)));
    let cancellation = Cancellation::default();
    assert_eq!(cancellation.settle(refusal), refusal);
    cancellation.cancel();
    assert_eq!(
        cancellation.settle(refusal),
        Err(TightError::Stopped(Stop::Cancelled))
    );
}

#[test]
fn certificate_errors_classify_resource_refusals() {
    for refusal in [
        TightError::Limit(TightResource::Work),
        TightError::Limit(TightResource::Bytes),
        TightError::Stopped(Stop::Allocation),
    ] {
        assert!(refusal.is_resource_refusal(), "{refusal:?}");
    }
    for shape in [
        TightError::UnsupportedRoot { root: 0 },
        TightError::UnsupportedBody { root: 0, body: 1 },
        TightError::PositiveCycle { atom: 0 },
        TightError::RankShape,
        TightError::RankOrder { head: 0 },
        TightError::Stopped(Stop::Cancelled),
        TightError::Stopped(Stop::WrongProgram),
    ] {
        assert!(!shape.is_resource_refusal(), "{shape:?}");
    }
}
