use super::{CountPlanFailureKind, CountPlanLimits, CountPlanResource, CountPlanStatistics, Work};
use crate::ProgramSite;
use zetesis_algebra::{Cancellation, Stop};

#[test]
fn imported_work_retains_its_prefix_before_the_next_poll() {
    for limit in [0, 7, u64::MAX] {
        let cancellation = Cancellation::default();
        let location = ProgramSite::program();
        let mut work = Work::new(
            CountPlanLimits {
                max_work: limit,
                ..CountPlanLimits::default()
            },
            cancellation.clone(),
            location,
        );
        let initial = limit.min(2);
        work.charge(initial).unwrap();
        work.payload(11).unwrap();
        cancellation.cancel();

        // Native work must be retained before exposing the native outcome.
        // Importing a receipt must neither poll nor partially debit an overrun.
        let remaining = limit - initial;
        assert_eq!(
            work.record(remaining + 1),
            Err(CountPlanFailureKind::Limit(CountPlanResource::Work))
        );
        assert_eq!(
            work.statistics(),
            CountPlanStatistics {
                work: initial,
                storage_bytes: 11,
                ..Default::default()
            }
        );
        work.record(remaining).unwrap();
        let expected = CountPlanStatistics {
            work: limit,
            storage_bytes: 11,
            ..Default::default()
        };
        assert_eq!(work.statistics(), expected);
        assert_eq!(
            work.record(1),
            Err(CountPlanFailureKind::Limit(CountPlanResource::Work))
        );
        assert_eq!(work.statistics(), expected);
        work.record(0).unwrap();
        assert_eq!(work.statistics(), expected);

        // A fresh charge observes control even for zero work at the ceiling.
        let kind = work.charge(0).unwrap_err();
        assert_eq!(kind, CountPlanFailureKind::Stopped(Stop::Cancelled));
        let error = work.failure(kind);
        assert_eq!(error.statistics(), expected);
        assert_eq!(error.site(), location);
    }
}

#[test]
fn payload_refusal_preserves_the_complete_unpolled_prefix() {
    for limit in [7, u64::MAX] {
        let cancellation = Cancellation::default();
        let location = ProgramSite::program();
        let mut work = Work::new(
            CountPlanLimits {
                max_bytes: limit,
                ..CountPlanLimits::default()
            },
            cancellation.clone(),
            location,
        );
        work.charge(3).unwrap();
        work.payload(limit).unwrap();
        let before = work.statistics();
        cancellation.cancel();
        // Payload admission has no poll. Native u64 addition overflow must not
        // become a byte-limit refusal, and neither failure commits a prefix.
        let expected = if limit == u64::MAX {
            CountPlanFailureKind::Overflow
        } else {
            CountPlanFailureKind::Limit(CountPlanResource::Bytes)
        };
        assert_eq!(work.payload(1), Err(expected));
        assert_eq!(work.statistics(), before);
        let error = work.failure(expected);
        assert_eq!(error.statistics(), before);
        assert_eq!(error.site(), location);
        assert_eq!(work.payload(0), Ok(()));
        assert_eq!(work.statistics(), before);
    }
}
