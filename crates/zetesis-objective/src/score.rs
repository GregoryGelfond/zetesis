use std::cmp::Ordering;

use zetesis_core::Value;

use crate::Statistics;

/// One active globally deduplicated key: priority, numeric weight, then tuple.
/// Its existence only records a condition satisfied in the supplied relation.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Contribution {
    pub(crate) priority: i32,
    pub(crate) weight: i32,
    pub(crate) tuple: Vec<Value>,
}
impl Contribution {
    /// Objective priority.
    #[must_use]
    pub const fn priority(&self) -> i32 {
        self.priority
    }
    /// The contribution's numeric weight, including zero and negative values.
    #[must_use]
    pub const fn weight(&self) -> i32 {
        self.weight
    }
    /// Explicit scalar tuple, excluding implicit priority and weight components.
    #[must_use]
    pub fn tuple(&self) -> &[Value] {
        &self.tuple
    }
}

/// Costs in the admitted program's fixed descending priority slots. Structural
/// equality preserves objective presence and slots; use [`Self::compare_costs`]
/// to compare costs with missing slots interpreted as zero.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Score {
    pub(crate) present: bool,
    pub(crate) costs: Vec<(i32, i64)>,
}
impl Score {
    /// Whether the caller admitted an objective, even if all costs are zero.
    #[must_use]
    pub const fn is_present(&self) -> bool {
        self.present
    }
    /// Distinct descending priority/cost pairs, including inactive zero slots.
    #[must_use]
    pub fn costs(&self) -> &[(i32, i64)] {
        &self.costs
    }
    /// Lexicographic numeric comparison at descending priorities, treating every
    /// missing priority as zero. `Less` is better for minimization. This compares
    /// costs only; objective absence remains observable through [`Self::is_present`].
    #[must_use]
    pub fn compare_costs(&self, other: &Self) -> Ordering {
        let mut left = 0;
        let mut right = 0;
        while left < self.costs.len() || right < other.costs.len() {
            let (a, b) = match (self.costs.get(left), other.costs.get(right)) {
                (Some(&(priority_a, a)), Some(&(priority_b, b))) if priority_a == priority_b => {
                    left += 1;
                    right += 1;
                    (a, b)
                }
                (Some(&(priority_a, a)), Some(&(priority_b, _))) if priority_a > priority_b => {
                    left += 1;
                    (a, 0)
                }
                (_, Some(&(_, b))) => {
                    right += 1;
                    (0, b)
                }
                (Some(&(_, a)), None) => {
                    left += 1;
                    (a, 0)
                }
                (None, None) => break,
            };
            let order = a.cmp(&b);
            if order != Ordering::Equal {
                return order;
            }
        }
        Ordering::Equal
    }
}

/// A complete score and its canonical contribution evidence.
#[derive(Clone, Debug)]
pub struct Evaluation {
    pub(crate) score: Score,
    pub(crate) contributions: Vec<Contribution>,
    pub(crate) statistics: Statistics,
}
impl Evaluation {
    /// Fixed-priority costs for the supplied model relation.
    #[must_use]
    pub const fn score(&self) -> &Score {
        &self.score
    }
    /// Transfer the completed score without cloning its priority vector.
    /// Contribution and work evidence are discarded by this explicit conversion.
    #[must_use]
    pub fn into_score(self) -> Score {
        self.score
    }
    /// Distinct active keys in ascending (priority, weight, tuple) order.
    /// Reusing this evaluator on a possible-positive relation discovers keys in
    /// that relation only; the result is not a stable-model or optimization bound.
    #[must_use]
    pub fn contributions(&self) -> &[Contribution] {
        &self.contributions
    }
    /// Exact logical accounting for this completed evaluation.
    #[must_use]
    pub const fn statistics(&self) -> Statistics {
        self.statistics
    }
}
