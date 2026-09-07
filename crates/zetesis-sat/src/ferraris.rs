use zetesis_ferraris::{Interpretation, Theory, models, models_reduct};

use crate::encoding;
use crate::search::{Budget, Cursor, Quota, increment, query};
use crate::timing::{self, Phase};
use crate::{AdmissionLimits, Cnf, Control, Incomplete, SearchLimits, SearchStatistics, Solve};

#[path = "batch.rs"]
mod batch;
pub use batch::{BatchError, BatchLimits, BatchStatistics, BatchVerdict};

#[path = "completion.rs"]
mod completion;
pub use completion::{CompletionExecutor, CompletionScratch, CompletionStatistics};

/// Whole-operation ceilings for a membership check or stable-model enumeration.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Each candidate/reduct CNF, including accumulated candidate blocking clauses.
    pub admission: AdmissionLimits,
    /// Cumulative encoding and SAT work/decisions across all queries in a run.
    pub search: SearchLimits,
    /// Maximum classical candidates checked during enumeration; not model count.
    pub max_candidates: u64,
    /// Per-call work ceiling for independent original/reduct formula evaluation.
    pub max_verification_work: u64,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            admission: AdmissionLimits::default(),
            search: SearchLimits::default(),
            max_candidates: 1_000_000,
            max_verification_work: 100_000_000,
        }
    }
}

/// A membership result for one original semantic interpretation.
#[derive(Clone, Debug)]
pub enum Check {
    /// Original model whose proper-subset frozen-reduct query was proved UNSAT.
    Stable,
    /// The candidate does not satisfy the original theory.
    NotModel,
    /// Independently validated proper-subset model of the frozen reduct.
    NonMinimal(Interpretation),
    /// No semantic membership decision can be made within available resources.
    Inconclusive(Incomplete),
}
impl Check {
    /// True only after a completed minimality proof.
    #[must_use]
    pub fn accepted(&self) -> bool {
        matches!(self, Self::Stable)
    }
}

/// Cumulative accounting across candidate and countermodel search.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Statistics {
    /// Encoding operations and SAT-kernel work, including incomplete queries.
    pub search: SearchStatistics,
    /// Outer classical SAT queries started, including a final UNSAT query.
    pub candidate_queries: u64,
    /// Successfully appended candidate-only restrictions. Exhaustion then
    /// covers their intersection with the original classical candidate region.
    pub candidate_restrictions: u64,
    /// Classical candidates admitted for membership checking, including retained
    /// pending proposals in the batched protocol.
    pub candidates: u64,
    /// Frozen reduct SAT queries started.
    pub countermodel_queries: u64,
    /// Independently checked countermodels found.
    pub countermodels: u64,
    /// Verified stable models returned by the iterator.
    pub stable_models: u64,
    /// Coarse host timings, absent unless explicitly enabled after construction.
    /// These are separate from deterministic semantic work counters.
    pub phase_timings: Option<crate::SearchPhaseTimings>,
}

fn verification(limits: Limits) -> zetesis_ferraris::Limits {
    zetesis_ferraris::Limits {
        max_work: limits.max_verification_work,
        max_subsets: 0,
    }
}

/// Check stability through a classical proper-subset query of the frozen reduct.
/// No enumeration of all subsets or invocation of an external solver occurs.
/// `max_candidates` applies only to [`StableModels`].
#[must_use]
pub fn check(
    theory: &Theory,
    candidate: &Interpretation,
    limits: Limits,
    control: &Control,
) -> Check {
    let mut budget = Budget {
        quota: crate::search::LocalQuota,
        limits: limits.search,
        control,
        statistics: SearchStatistics::default(),
    };
    let mut statistics = Statistics::default();
    match membership(theory, candidate, limits, &mut budget, &mut statistics) {
        Ok(result) => result,
        Err(error) => Check::Inconclusive(error),
    }
}

fn membership(
    theory: &Theory,
    candidate: &Interpretation,
    limits: Limits,
    budget: &mut Budget<'_, impl Quota>,
    statistics: &mut Statistics,
) -> Result<Check, Incomplete> {
    let started = timing::start(statistics.phase_timings.as_ref());
    let original = (|| {
        budget.control.poll()?;
        if !theory.same_instance(candidate.theory()) {
            return Err(Incomplete::WrongTheory);
        }
        models(theory, candidate, verification(limits), budget.control).map_err(Incomplete::from)
    })();
    timing::finish(
        &mut statistics.phase_timings,
        Phase::OriginalValidation,
        started,
    );
    if !original? {
        return Ok(Check::NotModel);
    }
    let started = timing::start(statistics.phase_timings.as_ref());
    let result = reduct_membership(theory, candidate, limits, budget, statistics);
    timing::finish(&mut statistics.phase_timings, Phase::Reduct, started);
    result
}

fn reduct_membership(
    theory: &Theory,
    candidate: &Interpretation,
    limits: Limits,
    budget: &mut Budget<'_, impl Quota>,
    statistics: &mut Statistics,
) -> Result<Check, Incomplete> {
    let reduct = encoding::encode(theory, Some(candidate), limits.admission, budget)?;
    increment(&mut statistics.countermodel_queries)?;
    match query(&reduct, budget) {
        Solve::Unsat => Ok(Check::Stable),
        Solve::Inconclusive(error) => Err(error),
        Solve::Sat(assignment) => {
            let subset = encoding::interpretation(theory, &assignment, budget)?;
            let mut proper = false;
            for atom in 0..theory.atom_count() {
                budget.tick()?;
                if subset.contains(atom) && !candidate.contains(atom) {
                    return Err(Incomplete::InvalidWitness);
                }
                proper |= candidate.contains(atom) && !subset.contains(atom);
            }
            if !proper
                || !models_reduct(
                    theory,
                    candidate,
                    &subset,
                    verification(limits),
                    budget.control,
                )?
            {
                return Err(Incomplete::InvalidWitness);
            }
            increment(&mut statistics.countermodels)?;
            Ok(Check::NonMinimal(subset))
        }
    }
}

/// Native all-model search over classical candidates with exact semantic blocking.
/// SAT assignments include Tseitin variables, but returned interpretations and
/// blocking clauses contain only the original theory's atom universe.
///
/// Work and candidate limits are cumulative. An incomplete result is emitted
/// once and terminates the iterator without marking it exhausted. If blocking
/// storage fails after a stable model has been proved, that model is returned
/// first and the pending failure is returned on the next call.
#[derive(Debug)]
pub struct StableModels {
    theory: Theory,
    candidate_cnf: Cnf,
    candidate_cursor: Cursor,
    limits: Limits,
    control: Control,
    statistics: Statistics,
    terminal: bool,
    exhausted: bool,
    pending_error: Option<Incomplete>,
    batch: batch::State,
}
impl StableModels {
    /// Encode the original theory once, retaining its immutable instance identity.
    ///
    /// # Errors
    /// Refuses encoding admission, work limits, cancellation or allocation.
    pub fn new(theory: &Theory, limits: Limits, control: Control) -> Result<Self, Incomplete> {
        let mut budget = Budget {
            quota: crate::search::LocalQuota,
            limits: limits.search,
            control: &control,
            statistics: SearchStatistics::default(),
        };
        let candidate_cnf = encoding::encode(theory, None, limits.admission, &mut budget)?;
        let statistics = Statistics {
            search: budget.statistics,
            ..Default::default()
        };
        Ok(Self {
            theory: theory.clone(),
            candidate_cnf,
            candidate_cursor: Cursor::projected(theory.atom_count()),
            limits,
            control,
            statistics,
            terminal: false,
            exhausted: false,
            pending_error: None,
            batch: batch::State::default(),
        })
    }
    /// Enable coarse host timing from this point onward. Repeated calls retain
    /// existing measurements. No clock is read while timing is disabled, and
    /// initial CNF construction is deliberately outside these intervals.
    pub fn enable_phase_timing(&mut self) {
        self.statistics.phase_timings.get_or_insert_default();
    }

    /// Original immutable theory used for every independent reduct check.
    #[must_use]
    pub fn theory(&self) -> &Theory {
        &self.theory
    }

    /// Append a classical candidate-only constraint and restart the outer cursor.
    /// All original clauses, earlier restrictions and exact semantic blocks stay
    /// present. The original theory and frozen-reduct acceptance are unchanged.
    ///
    /// The restriction must use the original semantic atom count and index
    /// meanings. Its separate immutable instance is expected. Restrictions
    /// accumulate by conjunction; this method cannot widen a candidate region.
    /// Exhaustion afterwards proves coverage only of that constrained region.
    /// An optimizer must separately justify that omitted candidates are dominated.
    ///
    /// # Errors
    /// Refuses a different atom count, a closed iterator, pending blocking
    /// failure, capacity, work, cancellation or allocation. Failed encoding
    /// restores the previous CNF and cursor; charged work remains cumulative.
    pub fn restrict_candidates(&mut self, restriction: &Theory) -> Result<(), Incomplete> {
        if self.terminal {
            return Err(Incomplete::ClosedEnumerator);
        }
        if let Some(error) = self.pending_error {
            return Err(error);
        }
        if restriction.atom_count() != self.theory.atom_count() {
            return Err(Incomplete::RestrictionUniverse {
                expected: self.theory.atom_count(),
                actual: restriction.atom_count(),
            });
        }
        let count = self
            .statistics
            .candidate_restrictions
            .checked_add(1)
            .ok_or(Incomplete::CounterOverflow)?;
        let mut budget = Budget {
            quota: crate::search::LocalQuota,
            limits: self.limits.search,
            control: &self.control,
            statistics: self.statistics.search,
        };
        let result = encoding::restrict(&mut self.candidate_cnf, restriction, &mut budget);
        self.statistics.search = budget.statistics;
        if result.is_ok() {
            self.candidate_cursor = Cursor::refined(self.theory.atom_count());
            self.statistics.candidate_restrictions = count;
        }
        result
    }

    /// True only after a completed outer query refutes every unblocked model
    /// satisfying all successful candidate restrictions, if any were added.
    #[must_use]
    pub const fn exhausted(&self) -> bool {
        self.exhausted
    }
    /// Cumulative work, including an incomplete terminal attempt.
    #[must_use]
    pub const fn statistics(&self) -> Statistics {
        self.statistics
    }

    fn advance(&mut self) -> Result<Option<Interpretation>, Incomplete> {
        let mut budget = Budget {
            quota: crate::search::LocalQuota,
            limits: self.limits.search,
            control: &self.control,
            statistics: self.statistics.search,
        };
        let result = advance(
            &self.theory,
            &mut self.candidate_cnf,
            &mut self.candidate_cursor,
            self.limits,
            &mut budget,
            &mut self.statistics,
            &mut self.pending_error,
        );
        self.statistics.search = budget.statistics;
        result
    }
}

impl Iterator for StableModels {
    type Item = Result<Interpretation, Incomplete>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.terminal {
            return None;
        }
        if !self.batch.pending.is_empty() {
            self.terminal = true;
            return Some(Err(Incomplete::PendingBatch));
        }
        if let Some(error) = self.pending_error.take() {
            self.terminal = true;
            return Some(Err(error));
        }
        match self.advance() {
            Ok(Some(model)) => Some(Ok(model)),
            Ok(None) => {
                self.terminal = true;
                self.exhausted = true;
                None
            }
            Err(error) => {
                self.terminal = true;
                Some(Err(error))
            }
        }
    }
}
impl std::iter::FusedIterator for StableModels {}

fn advance(
    theory: &Theory,
    cnf: &mut Cnf,
    cursor: &mut Cursor,
    limits: Limits,
    budget: &mut Budget<'_>,
    statistics: &mut Statistics,
    pending_error: &mut Option<Incomplete>,
) -> Result<Option<Interpretation>, Incomplete> {
    loop {
        let started = timing::start(statistics.phase_timings.as_ref());
        let proposal = (|| {
            increment(&mut statistics.candidate_queries)?;
            let assignment = match cursor.query(cnf, budget) {
                Solve::Sat(assignment) => assignment,
                Solve::Unsat => return Ok(None),
                Solve::Inconclusive(error) => return Err(error),
            };
            if statistics.candidates >= limits.max_candidates {
                return Err(Incomplete::CandidateLimit);
            }
            increment(&mut statistics.candidates)?;
            encoding::interpretation(theory, &assignment, budget).map(Some)
        })();
        timing::finish(&mut statistics.phase_timings, Phase::Candidates, started);
        let Some(candidate) = proposal? else {
            return Ok(None);
        };
        let result = membership(theory, &candidate, limits, budget, statistics)?;
        if matches!(result, Check::NotModel | Check::Inconclusive(_)) {
            return Err(Incomplete::InvalidWitness);
        }
        let started = timing::start(statistics.phase_timings.as_ref());
        let blocking = encoding::block(cnf, &candidate, budget);
        timing::finish(&mut statistics.phase_timings, Phase::Candidates, started);
        if matches!(result, Check::Stable) {
            increment(&mut statistics.stable_models)?;
            *pending_error = blocking.err();
            return Ok(Some(candidate));
        }
        blocking?;
    }
}
