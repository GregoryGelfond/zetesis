//! A construction refusal between metered boundaries reports itself without
//! observing a requested stop; settled where a metered operation returns, as
//! construction's own refusals are, it becomes the stop.

use super::{Budget, ErrorKind, Group, Limits, Resource, Statistics, dimensions};
use zetesis_algebra::{Cancellation, Meter, Refusal, Stop};

/// Run `test` on a construction budget whose meter has admitted `charged`
/// operations; with `stop`, a stop is requested once it has. The metered exit
/// settles what `test` returns.
fn with_budget<T>(
    limits: Limits,
    charged: u64,
    stop: bool,
    test: impl FnOnce(&mut Budget<'_, '_>) -> Result<T, ErrorKind>,
) -> Result<T, ErrorKind> {
    let cancellation = Cancellation::default();
    let mut statistics = Statistics::default();
    cancellation
        .metered(limits.max_work, |meter: &mut Meter<'_>| {
            for _ in 0..charged {
                meter.charge().unwrap();
            }
            if stop {
                cancellation.cancel();
            }
            test(&mut Budget {
                limits,
                meter,
                statistics: &mut statistics,
            })
        })
        .result
}

#[test]
fn a_requested_stop_precedes_the_work_limit_between_boundaries() {
    // Seven charged operations is not a metered boundary. The tick's own
    // report is returned as a value so the exit leaves it alone.
    let limits = Limits {
        max_work: 7,
        ..Limits::default()
    };
    let tick = |stop| with_budget(limits, 7, stop, |budget| Ok(budget.tick())).unwrap();
    assert_eq!(tick(false), Err(ErrorKind::Limit(Resource::Work)));
    assert_eq!(tick(true), Err(ErrorKind::Stopped(Stop::Cancelled)));
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
    // the refusal itself is reported; the metered exit reports the stop.
    let settled = with_budget(limits, 1, true, |budget| {
        let refusal = dimensions(&groups, budget);
        assert_eq!(refusal, Err(ErrorKind::Limit(Resource::Members)));
        refusal
    });
    assert_eq!(settled, Err(ErrorKind::Stopped(Stop::Cancelled)));
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

#[test]
fn byte_refusal_retains_planned_dimensions_before_coverage_work() {
    let groups = [Group {
        members: &[0],
        upper: 1,
    }];
    let premises = super::Premises {
        atom_count: 1,
        members: &[0],
        lower: 1,
        groups: &groups,
    };
    let cancellation = Cancellation::default();
    let complete = super::Plan::new(premises, Limits::default(), &cancellation).unwrap();
    let planned = complete.statistics();
    assert!(planned.construction_bytes > planned.resident_bytes);
    let refused = super::Plan::new(
        premises,
        Limits {
            max_bytes: planned.construction_bytes - 1,
            ..Limits::default()
        },
        &cancellation,
    )
    .unwrap_err();
    assert_eq!(refused.kind(), ErrorKind::Limit(Resource::Bytes));
    // Only the one group's dimensions have been visited. The coverage-cell
    // initialization and declared/member traversals have not started.
    assert_eq!(refused.statistics(), Statistics { work: 1, ..planned });
    let exact = super::Plan::new(
        premises,
        Limits {
            max_bytes: planned.construction_bytes,
            ..Limits::default()
        },
        &cancellation,
    )
    .unwrap();
    assert_eq!(exact.statistics(), planned);
}
