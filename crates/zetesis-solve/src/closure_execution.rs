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
    /// The route that ran, whose units the counters use, carrying the
    /// counters only it produces.
    pub route: ClosureRoute,
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
}

/// The independent CPU closure route a receipt sums, with the counters only
/// it produces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClosureRoute {
    /// Source joins without a complete ground rule store, counted in join
    /// and copy units, with the joins' own counters.
    Lazy(ClosureJoinStatistics),
    /// Rule passes over a materialized static program, counted in eager
    /// scan units.
    Eager,
}

impl ClosureRoute {
    /// Stable spelling for execution reports.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Lazy(_) => "lazy",
            Self::Eager => "eager",
        }
    }
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
    /// Derived heads recorded as a bit of a dense relation, for which no atom
    /// was built before the model was assembled.
    pub dense_heads: u64,
    /// Blocks of body rows joined into a head's pending marks a word at a
    /// time, in place of binding their rows one by one.
    pub block_steps: u64,
    /// Largest named closure envelope any completed check admitted or
    /// reserved; not an observed allocation peak and not RSS.
    pub peak_closure_bytes: usize,
}

impl ClosureExecutionStatistics {
    /// The empty receipt of the route about to run.
    pub(crate) const fn new(route: ClosureRoute) -> Self {
        Self {
            route,
            completed_checks: 0,
            stopped_checks: 0,
            rounds: 0,
            work: 0,
            derived_atoms: 0,
        }
    }

    pub(crate) fn stopped(&mut self) -> Result<(), crate::SolveError> {
        self.stopped_checks = add(self.stopped_checks, 1)?;
        Ok(())
    }

    /// Sum one completed lazy check. Every field is checked before any is
    /// replaced, so an overflow leaves the receipt as it was.
    pub(crate) fn completed_lazy(&mut self, check: &Statistics) -> Result<(), crate::SolveError> {
        let ClosureRoute::Lazy(joins) = self.route else {
            unreachable!("a lazy check is summed into a lazy receipt")
        };
        let next = Self {
            completed_checks: add(self.completed_checks, 1)?,
            rounds: add(self.rounds, check.rounds)?,
            work: add(self.work, check.work)?,
            derived_atoms: add(self.derived_atoms, count(check.derived_atoms)?)?,
            route: ClosureRoute::Lazy(ClosureJoinStatistics {
                catalog_work: add(joins.catalog_work, check.catalog_work)?,
                bindings: add(joins.bindings, check.bindings)?,
                tuple_probes: add(joins.tuple_probes, check.tuple_probes)?,
                dense_heads: add(joins.dense_heads, check.dense_heads)?,
                block_steps: add(joins.block_steps, check.block_steps)?,
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
        if self.route != ClosureRoute::Eager {
            unreachable!("an eager check is summed into an eager receipt")
        }
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
