//! The certificate's work limit observes a requested stop before refusing, and a
//! byte refusal settled where a metered operation returns, as the certificate's
//! own refusals are, becomes a requested stop.

use super::{Tick, TightError, TightResource, bytes};
use zetesis_cpu::{Cancellation, Refusal, Stop};

#[test]
fn a_requested_stop_precedes_the_work_limit_between_boundaries() {
    // The tick's own report, returned as a value so the exit leaves it alone.
    let tick = |stop: bool| {
        let cancellation = Cancellation::default();
        cancellation
            .metered(7, |meter| {
                for _ in 0..7 {
                    meter.charge().unwrap();
                }
                if stop {
                    cancellation.cancel();
                }
                Ok::<_, TightError>(meter.tick())
            })
            .result
            .unwrap()
    };
    assert_eq!(tick(false), Err(TightError::Limit(TightResource::Work)));
    assert_eq!(tick(true), Err(TightError::Stopped(Stop::Cancelled)));
}

#[test]
fn a_byte_refusal_settles_to_a_requested_stop() {
    assert_eq!(bytes(8, 8), Ok(8));
    let refused = |stop: bool| {
        let cancellation = Cancellation::default();
        cancellation
            .metered(u64::MAX, |_| {
                if stop {
                    cancellation.cancel();
                }
                bytes(9, 8)
            })
            .result
    };
    assert_eq!(refused(false), Err(TightError::Limit(TightResource::Bytes)));
    assert_eq!(refused(true), Err(TightError::Stopped(Stop::Cancelled)));
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
