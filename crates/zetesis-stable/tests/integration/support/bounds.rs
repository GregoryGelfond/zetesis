//! Candidate bounds as an optimizer installs them: one objective plan, each
//! bound at a strictly better cost than the last.

use zetesis_ferraris::Theory;
use zetesis_objective::{PlanIdentity, Score};
use zetesis_stable::{Incomplete, StableModels};

/// Successive bounds of one plan, each one cost better than the last installed.
pub(crate) struct Bounds {
    plan: PlanIdentity,
    cost: i64,
}

impl Bounds {
    pub(crate) fn new() -> Self {
        Self {
            plan: PlanIdentity::fresh(),
            cost: 0,
        }
    }

    /// Install `restriction` as this plan's bound one cost better than the last.
    pub(crate) fn improve(
        &mut self,
        search: &mut StableModels,
        restriction: &Theory,
    ) -> Result<(), Incomplete> {
        let result = search.improve_candidate_bound(self.plan, &cost(self.cost - 1), restriction);
        if result.is_ok() {
            self.cost -= 1;
        }
        result
    }

    pub(crate) const fn plan(&self) -> PlanIdentity {
        self.plan
    }

    /// The cost of the last installed bound.
    pub(crate) const fn cost(&self) -> i64 {
        self.cost
    }
}

/// A one-priority score at `value`.
pub(crate) fn cost(value: i64) -> Score {
    Score::from_costs(vec![(0, value)]).expect("one priority")
}
