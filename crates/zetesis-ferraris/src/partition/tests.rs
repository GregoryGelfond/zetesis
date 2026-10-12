//! A requested stop keeps its precedence over construction refusals.

use super::{Budget, ErrorKind, Group, Limits, Resource, Statistics, dimensions};
use zetesis_cpu::{Cancellation, Refusal, Stop};

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
fn the_member_refusal_settles_to_a_requested_stop() {
    let groups = [Group {
        members: &[0, 1],
        upper: 1,
    }];
    let limits = Limits {
        max_members: 1,
        ..Limits::default()
    };
    // From one charged operation the measuring loop observes no boundary, so
    // the refusal itself is reported; construction settles it at its exit.
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let refusal = dimensions(&groups, &mut budget(&cancellation, 1, limits));
    assert_eq!(refusal, Err(ErrorKind::Limit(Resource::Members)));
    assert_eq!(
        cancellation.settle(refusal),
        Err(ErrorKind::Stopped(Stop::Cancelled))
    );
}

#[test]
fn construction_errors_classify_resource_refusals() {
    for refusal in [
        ErrorKind::Limit(Resource::Bytes),
        ErrorKind::Overflow,
        ErrorKind::Stopped(Stop::Allocation),
    ] {
        assert!(refusal.is_resource_refusal(), "{refusal:?}");
    }
    for shape in [
        ErrorKind::Atom(0),
        ErrorKind::DuplicateMember(0),
        ErrorKind::ForeignMember(0),
        ErrorKind::RepeatedMember(0),
        ErrorKind::MissingMember(0),
        ErrorKind::Stopped(Stop::Cancelled),
    ] {
        assert!(!shape.is_resource_refusal(), "{shape:?}");
    }
}
