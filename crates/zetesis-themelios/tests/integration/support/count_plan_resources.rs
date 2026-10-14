//! The count-plan limit each count-plan resource is counted against, set by
//! resource: count planning is not an admission, and only these tests set its
//! limits, so the pairing lives here rather than in the shared resource view.

use zetesis_themelios::{CountPlanLimits, CountPlanResource};

/// The default count-plan limits, with `resource` bounded by `limit`, which is
/// read as `u64` whatever the field's width.
///
/// # Panics
/// Panics if `limit` does not fit the field's width.
pub(crate) fn limits(resource: CountPlanResource, limit: u64) -> CountPlanLimits {
    let size = || usize::try_from(limit).expect("a limit within usize");
    let mut limits = CountPlanLimits::default();
    match resource {
        CountPlanResource::Groups => limits.max_groups = size(),
        CountPlanResource::Members => limits.max_members = size(),
        CountPlanResource::Origins => limits.max_origins = size(),
        CountPlanResource::Attempts => limits.max_attempts = size(),
        CountPlanResource::Bytes => limits.max_bytes = limit,
        CountPlanResource::Work => limits.max_work = limit,
    }
    limits
}
