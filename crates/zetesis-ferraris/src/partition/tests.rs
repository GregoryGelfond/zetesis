//! A requested stop keeps its precedence over construction refusals.

use super::{Budget, ErrorKind, Group, Limits, Resource, Statistics, dimensions};
use zetesis_cpu::{Cancellation, Stop};

/// A construction budget that has charged `work` operations.
fn budget(cancellation: &Cancellation, work: u64, limits: Limits) -> Budget<'_> {
    Budget {
        limits,
        cancellation,
        statistics: Statistics {
            work,
            ..Statistics::default()
        },
    }
}

#[test]
fn a_requested_stop_precedes_the_work_limit_between_boundaries() {
    // Seven charged operations is not a metered boundary.
    let limits = Limits {
        max_work: 7,
        ..Limits::default()
    };
    let cancellation = Cancellation::default();
    assert_eq!(
        budget(&cancellation, 7, limits).tick(),
        Err(ErrorKind::Limit(Resource::Work))
    );
    cancellation.cancel();
    assert_eq!(
        budget(&cancellation, 7, limits).tick(),
        Err(ErrorKind::Stopped(Stop::Cancelled))
    );
}

#[test]
fn a_requested_stop_precedes_the_member_limit() {
    let groups = [Group {
        members: &[0, 1],
        upper: 1,
    }];
    let limits = Limits {
        max_members: 1,
        ..Limits::default()
    };
    // The group's charge starts from one operation, between boundaries, so
    // only the refusal can observe the stop.
    let measure =
        |cancellation: &Cancellation| dimensions(&groups, &mut budget(cancellation, 1, limits));
    let cancellation = Cancellation::default();
    assert_eq!(
        measure(&cancellation),
        Err(ErrorKind::Limit(Resource::Members))
    );
    cancellation.cancel();
    assert_eq!(
        measure(&cancellation),
        Err(ErrorKind::Stopped(Stop::Cancelled))
    );
}
