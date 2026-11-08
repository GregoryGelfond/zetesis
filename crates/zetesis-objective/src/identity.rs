//! The identity of one objective plan, which every bound compiled from it carries.

use std::sync::atomic::{AtomicU64, Ordering};

/// One objective plan's identity within this process. Every bound compiled from
/// one plan carries the same identity, so that whoever installs successive
/// bounds can refuse to compare scores from different plans: two scores are
/// comparable only under the plan whose priority slots they read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PlanIdentity(u64);

impl PlanIdentity {
    /// An identity distinct from every other this process has issued.
    #[must_use]
    pub fn fresh() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        Self(NEXT.fetch_add(1, Ordering::Relaxed))
    }
}

#[cfg(test)]
mod tests {
    use super::PlanIdentity;

    #[test]
    fn fresh_identities_are_distinct() {
        let first = PlanIdentity::fresh();
        assert_ne!(first, PlanIdentity::fresh());
        assert_eq!(first, first);
    }
}
