//! The observation construction limit, set by the resource a refusal reports:
//! only these tests set it, so the pairing lives here rather than in the shared
//! resource view, which pairs the evaluation limits.

use zetesis_themelios::observation::{ConstructionLimits, Resource};

/// Construction limits with `resource` bounded by `limit`.
///
/// # Panics
/// Panics for any resource but the construction bytes, the one resource the
/// construction limits bound.
pub(crate) fn limits(resource: Resource, limit: usize) -> ConstructionLimits {
    match resource {
        Resource::ConstructionBytes => ConstructionLimits { max_bytes: limit },
        _ => panic!("{resource:?} is not bounded by the observation construction limits"),
    }
}
