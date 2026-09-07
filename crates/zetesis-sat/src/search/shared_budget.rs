//! Reservations shared by independent queries in one joined completion batch.

use std::sync::atomic::{AtomicU64, Ordering};

use crate::{Incomplete, SearchLimits, SearchStatistics};

pub(crate) struct SharedBudget {
    limits: SearchLimits,
    work: AtomicU64,
    decisions: AtomicU64,
}

impl SharedBudget {
    pub(crate) fn new(limits: SearchLimits, spent: SearchStatistics) -> Self {
        Self {
            limits,
            work: AtomicU64::new(spent.work),
            decisions: AtomicU64::new(spent.decisions),
        }
    }

    pub(crate) fn tick(&self) -> Result<(), Incomplete> {
        reserve(&self.work, self.limits.max_work, Incomplete::WorkLimit)
    }

    pub(crate) fn decide(&self) -> Result<(), Incomplete> {
        reserve(
            &self.decisions,
            self.limits.max_decisions,
            Incomplete::DecisionLimit,
        )
    }

    /// All workers have joined before the coordinator reads these counters.
    /// Their atomic order protects only the quota, not the workers' data.
    pub(crate) fn record(&self, statistics: &mut SearchStatistics) {
        statistics.work = self.work.load(Ordering::Relaxed);
        statistics.decisions = self.decisions.load(Ordering::Relaxed);
    }
}

fn reserve(counter: &AtomicU64, ceiling: u64, error: Incomplete) -> Result<(), Incomplete> {
    counter
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |spent| {
            (spent < ceiling).then(|| spent + 1)
        })
        .map(|_| ())
        .map_err(|_| error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn last_representable_reservation_never_wraps_or_spends_a_different_quota() {
        let mut spent = SearchStatistics {
            work: u64::MAX - 1,
            decisions: 7,
            ..SearchStatistics::default()
        };
        let shared = SharedBudget::new(
            SearchLimits {
                max_work: u64::MAX,
                max_decisions: 7,
            },
            spent,
        );
        assert_eq!(shared.tick(), Ok(()));
        assert_eq!(shared.tick(), Err(Incomplete::WorkLimit));
        assert_eq!(shared.decide(), Err(Incomplete::DecisionLimit));
        shared.record(&mut spent);
        assert_eq!(spent.work, u64::MAX);
        assert_eq!(spent.decisions, 7);
    }
}
