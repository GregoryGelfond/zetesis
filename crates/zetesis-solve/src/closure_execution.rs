//! Ordinary independent CPU closure accounting, summed over completed checks.

use zetesis_cpu::{StaticStatistics, Statistics};

/// Cumulative counters of the independent CPU closure routes, lazy and eager.
///
/// Each completed check returns its own counters and this receipt sums them.
/// A stopped check returns none, so its partial work is counted nowhere: the
/// sums describe completed checks only, and `stopped_checks` says how many
/// checks are missing from them. Units follow the route's `max_work` ceiling
/// and differ between the routes. These are work counts, not process memory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClosureExecutionStatistics {
    /// Route whose units the counters use.
    pub grounder: crate::Grounder,
    /// Checks that returned a complete closure.
    pub completed_checks: u64,
    /// Checks stopped by a resource ceiling, cancellation or deadline.
    pub stopped_checks: u64,
    /// Lazy source rounds or eager rule passes, including each check's final
    /// unchanged one.
    pub rounds: u64,
    /// Charged operations, in the route's units.
    pub work: u64,
    /// Atoms in the completed closures, summed over checks.
    pub derived_atoms: u64,
    /// Counters only the lazy route's source joins produce.
    pub joins: Option<ClosureJoinStatistics>,
}

/// Counters of the lazy route's source joins, summed over completed checks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClosureJoinStatistics {
    /// Work spent on retained typed catalogs; included in `work`.
    pub catalog_work: u64,
    /// Fully matched bindings visited by the round schedules.
    pub bindings: u64,
    /// Source rows offered to the whole-row matcher; bounded by `work`.
    pub tuple_probes: u64,
    /// Largest named closure envelope any completed check admitted or
    /// reserved; not an observed allocation peak and not RSS.
    pub peak_closure_bytes: usize,
}

impl ClosureExecutionStatistics {
    pub(crate) fn new(grounder: crate::Grounder) -> Self {
        Self {
            grounder,
            completed_checks: 0,
            stopped_checks: 0,
            rounds: 0,
            work: 0,
            derived_atoms: 0,
            joins: matches!(grounder, crate::Grounder::Lazy)
                .then_some(ClosureJoinStatistics::default()),
        }
    }

    pub(crate) fn stopped(&mut self) -> Result<(), crate::SolveError> {
        self.stopped_checks = add(self.stopped_checks, 1)?;
        Ok(())
    }

    /// Sum one completed lazy check. Every field is checked before any is
    /// replaced, so an overflow leaves the receipt as it was.
    pub(crate) fn completed_lazy(&mut self, check: &Statistics) -> Result<(), crate::SolveError> {
        let joins = self.joins.unwrap_or_default();
        let next = Self {
            completed_checks: add(self.completed_checks, 1)?,
            rounds: add(self.rounds, check.rounds)?,
            work: add(self.work, check.work)?,
            derived_atoms: add(self.derived_atoms, count(check.derived_atoms)?)?,
            joins: Some(ClosureJoinStatistics {
                catalog_work: add(joins.catalog_work, check.catalog_work)?,
                bindings: add(joins.bindings, check.bindings)?,
                tuple_probes: add(joins.tuple_probes, check.tuple_probes)?,
                peak_closure_bytes: joins.peak_closure_bytes.max(check.peak_closure_bytes),
            }),
            ..self.clone()
        };
        *self = next;
        Ok(())
    }

    /// Sum one completed eager check, with the same overflow discipline.
    pub(crate) fn completed_eager(
        &mut self,
        check: &StaticStatistics,
    ) -> Result<(), crate::SolveError> {
        let next = Self {
            completed_checks: add(self.completed_checks, 1)?,
            rounds: add(self.rounds, check.passes)?,
            work: add(self.work, check.work)?,
            derived_atoms: add(self.derived_atoms, count(check.derived_atoms)?)?,
            ..self.clone()
        };
        *self = next;
        Ok(())
    }
}

fn add(left: u64, right: u64) -> Result<u64, crate::SolveError> {
    left.checked_add(right)
        .ok_or(crate::SolveError::ClosureStatisticsOverflow)
}

fn count(atoms: usize) -> Result<u64, crate::SolveError> {
    u64::try_from(atoms).map_err(|_| crate::SolveError::ClosureStatisticsOverflow)
}
