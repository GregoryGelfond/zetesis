//! The objective-bound limit each refusal of an objective bound is counted
//! against, set by resource: bounding is not an admission, and only these tests
//! set its limits, so the pairing lives here rather than in the shared resource
//! view, which pairs the objective plan's limits.

use zetesis_ferraris::AggregateErrorKind;
use zetesis_reference_support::resource_limits;
use zetesis_themelios::objective_bound::{ObjectiveBoundLimits, ObjectiveBoundResource};

/// The default objective-bound limits, with `resource` bounded by `limit`.
///
/// # Panics
/// Panics for a resource the plan's limits bound
/// (`resource_limits::objective_plan`): only the bound's work has a field of
/// its own here; its aggregate ceilings are set by [`aggregate`].
pub(crate) fn limits(resource: ObjectiveBoundResource, limit: u64) -> ObjectiveBoundLimits {
    match resource {
        ObjectiveBoundResource::Work => ObjectiveBoundLimits {
            max_work: limit,
            ..ObjectiveBoundLimits::default()
        },
        _ => panic!("{resource:?} is bounded by the objective plan's limits"),
    }
}

/// The default objective-bound limits, with the aggregate ceiling that a
/// refusal of `kind` reports bounded by `limit`.
///
/// # Panics
/// Panics as `resource_limits::aggregate` does.
pub(crate) fn aggregate(kind: AggregateErrorKind, limit: u64) -> ObjectiveBoundLimits {
    ObjectiveBoundLimits {
        aggregate: resource_limits::aggregate(kind, limit),
        ..ObjectiveBoundLimits::default()
    }
}
