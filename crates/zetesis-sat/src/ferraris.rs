use zetesis_ferraris::{Interpretation, Theory, models, models_reduct};

use crate::encoding;
use crate::search::{Budget, Cursor, Quota, increment};
use crate::timing::{self, Phase};
use crate::{
    AdmissionLimits, Cnf, Control, Incomplete, ProjectionLimits, ProjectionStatistics,
    SearchLimits, SearchStatistics, Solve,
};

#[path = "batch.rs"]
mod batch;
pub use batch::{BatchError, BatchLimits, BatchStatistics, BatchVerdict};

#[path = "completion.rs"]
mod completion;
pub use completion::{CompletionExecutor, CompletionScratch, CompletionStatistics};

#[path = "certified.rs"]
mod certified;
pub use certified::{
    CertificateError, CertificateLimits, CertificateOrder, CertificatePlanStatistics,
    CertifiedStatistics,
};

#[path = "candidate_support.rs"]
mod candidate_support;
pub use candidate_support::{SupportStatistics, SupportStatus};

#[path = "reduct_query.rs"]
mod reduct_query;

#[path = "regions.rs"]
mod regions;
pub(crate) use regions::ReductQuery;
use regions::RegionSearch;
pub use regions::{RegionQueryStatistics, RegionSearchStatistics, SearchMethod};

/// Whole-operation ceilings for a membership check or stable-model enumeration.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Original candidate CNF, including submitted candidate restrictions.
    /// Exact candidate exclusions have their own `projections` population.
    pub admission: AdmissionLimits,
    /// Immutable prepared reduct CNF, or the explicit fresh-check reduct CNF.
    /// These dimensions are independent of candidate/restriction admission.
    pub reduct_admission: AdmissionLimits,
    /// Distinct exclusion keys, logical trie nodes and named retained capacity.
    pub projections: ProjectionLimits,
    /// Cumulative encoding, certificate and search work/decisions across a run.
    pub search: SearchLimits,
    /// Maximum classical candidates checked during enumeration; not model count.
    pub max_candidates: u64,
    /// Per-call work ceiling for independent original/reduct formula evaluation.
    pub max_verification_work: u64,
    /// Named immutable reduct preparation and retained per-query capacity,
    /// each bounded independently. Aggregate completion also admits the shared
    /// prepared owner once alongside all worker/transient/result storage.
    pub max_reduct_bytes: u64,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            admission: AdmissionLimits::default(),
            reduct_admission: AdmissionLimits::default(),
            projections: ProjectionLimits::default(),
            search: SearchLimits::default(),
            max_candidates: 1_000_000,
            max_verification_work: 100_000_000,
            max_reduct_bytes: crate::ReductPreparationLimits::DEFAULT_BYTES,
        }
    }
}

/// Low-level membership verdict data. Native [`check`] returns a verdict for its
/// supplied subject; the enum itself carries no binding to that subject or its
/// theory and can also be constructed by callers. For a record that retains the
/// subject of an actual native check, use [`crate::check_interpretation`].
#[derive(Clone, Debug)]
pub enum Check {
    /// Original model whose proper-subset frozen-reduct query was proved UNSAT.
    Stable,
    /// The candidate does not satisfy the original theory.
    NotModel,
    /// Independently validated proper-subset model of the frozen reduct.
    NonMinimal(Interpretation),
    /// A present atom has no producer with a true body under a complete tight
    /// plan. By the support law the candidate is not an answer set, and the
    /// candidate without that atom is a proper-subset model of its reduct.
    Unsupported {
        /// First unsupported present atom in ascending order.
        atom: usize,
    },
    /// No semantic membership decision can be made within available resources.
    Inconclusive(Incomplete),
}
impl Check {
    /// Whether this verdict is the `Stable` variant. This inspects data only;
    /// it does not run a check or authenticate a caller-constructed verdict.
    #[must_use]
    pub fn accepted(&self) -> bool {
        matches!(self, Self::Stable)
    }
}

/// Cumulative accounting across candidate and countermodel search.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Statistics {
    /// Encoding, certificate and Boolean-kernel work, including incomplete attempts.
    pub search: SearchStatistics,
    /// Exact history retained across candidate queries and restrictions.
    /// Its work is included in `search.work`, never added to it.
    pub projections: ProjectionStatistics,
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
    /// Optional complete-theory certificate attempt and checks.
    pub certified: Option<CertifiedStatistics>,
    /// Initial necessary disjunctive support restriction on outer candidates.
    /// Standalone membership checks do not construct this optional restriction,
    /// and the regions proposer has the support cut in its narrowing instead.
    pub support: Option<SupportStatistics>,
    /// The regions proposer's receipts; absent under the clauses proposer.
    pub regions: Option<RegionSearchStatistics>,
    /// Actual persistent-reduct construction and query work, including failures.
    pub reduct: crate::ReductStatistics,
}

fn verification(limits: Limits) -> zetesis_ferraris::Limits {
    zetesis_ferraris::Limits {
        max_work: limits.max_verification_work,
        max_subsets: 0,
    }
}

/// Check stability through a classical proper-subset query of the frozen reduct
/// on the clause kernel. No enumeration of all subsets or invocation of an
/// external solver occurs. `max_candidates` applies only to [`StableModels`].
#[must_use]
pub fn check(
    theory: &Theory,
    candidate: &Interpretation,
    limits: Limits,
    control: &Control,
) -> Check {
    check_with(theory, candidate, SearchMethod::Clauses, limits, control)
}

/// Check stability by the chosen method: the proper-subset query of the
/// frozen reduct as a region tree, or as a clause query on the kernel. The
/// verdict is the same either way; a countermodel is validated independently
/// of the method that proposed it.
#[must_use]
pub fn check_with(
    theory: &Theory,
    candidate: &Interpretation,
    method: SearchMethod,
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
    match fresh_membership(
        theory,
        candidate,
        method,
        limits,
        &mut budget,
        &mut statistics,
        &mut reduct_query::Workspace::default(),
    ) {
        Ok(result) => result,
        Err(error) => Check::Inconclusive(error),
    }
}

fn fresh_membership(
    theory: &Theory,
    candidate: &Interpretation,
    method: SearchMethod,
    limits: Limits,
    budget: &mut Budget<'_, impl Quota>,
    statistics: &mut Statistics,
    workspace: &mut reduct_query::Workspace,
) -> Result<Check, Incomplete> {
    let original = original_model(theory, candidate, limits, budget, statistics);
    if !original? {
        return Ok(Check::NotModel);
    }
    let started = timing::start(statistics.phase_timings.as_ref());
    let result = match method {
        SearchMethod::Clauses => {
            reduct_membership(theory, candidate, limits, budget, statistics, workspace)
        }
        SearchMethod::Regions => {
            let mut state = crate::prepared_reduct::State::new(method);
            state.check(theory, candidate, limits, budget, statistics)
        }
    };
    timing::finish(&mut statistics.phase_timings, Phase::Reduct, started);
    result
}

pub(crate) fn original_model(
    theory: &Theory,
    candidate: &Interpretation,
    limits: Limits,
    budget: &Budget<'_, impl Quota>,
    statistics: &mut Statistics,
) -> Result<bool, Incomplete> {
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
    original
}

fn reduct_membership(
    theory: &Theory,
    candidate: &Interpretation,
    limits: Limits,
    budget: &mut Budget<'_, impl Quota>,
    statistics: &mut Statistics,
    workspace: &mut reduct_query::Workspace,
) -> Result<Check, Incomplete> {
    let (reduct, search) = workspace.encode(theory, candidate, limits.reduct_admission, budget)?;
    increment(&mut statistics.countermodel_queries)?;
    let result = search.query(reduct, budget);
    checked_reduct_result(theory, candidate, limits, budget, statistics, result)
}

pub(crate) fn checked_reduct_result(
    theory: &Theory,
    candidate: &Interpretation,
    limits: Limits,
    budget: &mut Budget<'_, impl Quota>,
    statistics: &mut Statistics,
    result: Solve,
) -> Result<Check, Incomplete> {
    match result {
        Solve::Unsat => Ok(Check::Stable),
        Solve::Inconclusive(error) => Err(error),
        Solve::Sat(assignment) => {
            let subset = encoding::interpretation(theory, &assignment, budget)?;
            checked_countermodel(theory, candidate, subset, limits, budget, statistics)
        }
    }
}

/// Validate a proposed countermodel independently: it must be a proper
/// subset of the candidate and model the candidate's frozen reduct. Either
/// proposer, the clause query or the region query, is held to this.
pub(crate) fn checked_countermodel(
    theory: &Theory,
    candidate: &Interpretation,
    subset: Interpretation,
    limits: Limits,
    budget: &mut Budget<'_, impl Quota>,
    statistics: &mut Statistics,
) -> Result<Check, Incomplete> {
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

/// Native all-model search over classical candidates with exact semantic blocking.
/// SAT assignments include Tseitin variables, but returned interpretations and
/// exact exclusions contain only the original theory's atom universe.
///
/// Work and candidate limits are cumulative. An incomplete result is emitted
/// once and terminates the iterator without marking it exhausted. If blocking
/// storage fails after a stable model has been proved, that model is returned
/// first and the pending failure is returned on the next call.
#[derive(Debug)]
pub struct StableModels {
    theory: Theory,
    proposer: Proposer,
    limits: Limits,
    control: Control,
    statistics: Statistics,
    terminal: bool,
    exhausted: bool,
    pending_error: Option<Incomplete>,
    batch: batch::State,
    certification: Option<certified::Certification>,
    reduct: crate::prepared_reduct::State,
}
impl StableModels {
    /// Enumerate with the clauses proposer: encode the original theory once,
    /// retaining its immutable instance identity, and try a complete ordinary
    /// disjunctive support restriction on the outer CNF. Rich asserted heads
    /// and optional formula/CNF shape limits retain general candidate search.
    /// Construction and failed encoding work stay charged. The optional
    /// formula bounds map SAT variables to atoms, literal units to nodes and
    /// clause units to roots; final encoding uses remaining CNF limits.
    /// Original-model and frozen-reduct checks always use the original theory.
    ///
    /// # Errors
    /// Refuses encoding/history admission, work limits, cancellation or allocation.
    pub fn new(theory: &Theory, limits: Limits, control: Control) -> Result<Self, Incomplete> {
        Self::with_method(theory, SearchMethod::Clauses, limits, control)
    }

    /// Enumerate by the chosen method. Under [`SearchMethod::Regions`] no
    /// clause form of the theory is built: the theory's producers are
    /// extracted for the support cut and the root region is opened, both
    /// charged as search work, and the reduct's proper-subset query is a
    /// region tree too. The verdict on every candidate is the same either
    /// way.
    ///
    /// # Errors
    /// Refuses admission, work limits, cancellation or allocation.
    pub fn with_method(
        theory: &Theory,
        method: SearchMethod,
        limits: Limits,
        control: Control,
    ) -> Result<Self, Incomplete> {
        let mut budget = Budget {
            quota: crate::search::LocalQuota,
            limits: limits.search,
            control: &control,
            statistics: SearchStatistics::default(),
        };
        let (proposer, support) = match method {
            SearchMethod::Clauses => {
                let mut cnf = encoding::encode(theory, None, limits.admission, &mut budget)?;
                let support = candidate_support::restrict(&mut cnf, theory, limits, &mut budget)?;
                let cursor = Cursor::projected(theory.atom_count(), limits.projections)?;
                (
                    Proposer::Clauses(Box::new(ClauseProposer { cnf, cursor })),
                    Some(support),
                )
            }
            SearchMethod::Regions => (
                Proposer::Regions(Box::new(RegionSearch::new(theory, &mut budget)?)),
                None,
            ),
        };
        let statistics = Statistics {
            search: budget.statistics,
            support,
            ..Default::default()
        };
        Ok(Self {
            theory: theory.clone(),
            proposer,
            limits,
            control,
            statistics,
            terminal: false,
            exhausted: false,
            pending_error: None,
            batch: batch::State::default(),
            certification: None,
            reduct: crate::prepared_reduct::State::new(method),
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

    /// Append a classical candidate-only constraint. The clauses proposer adds
    /// it to the CNF and restarts the outer cursor; original clauses and
    /// earlier restrictions stay in the CNF, and the separate exact projection
    /// index retains every earlier exclusion without turning it into watched
    /// CNF storage. The regions proposer narrows the regions still to visit by
    /// it and continues. Original theory and reduct acceptance stay unchanged.
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
        let result = self.proposer.restrict(restriction, &mut budget);
        self.statistics.search = budget.statistics;
        if result.is_ok() {
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
    pub fn statistics(&self) -> Statistics {
        match &self.proposer {
            Proposer::Clauses(clauses) => Statistics {
                projections: clauses.cursor.projection_statistics(),
                ..self.statistics
            },
            Proposer::Regions(regions) => Statistics {
                regions: Some(regions.statistics()),
                ..self.statistics
            },
        }
    }

    fn advance(&mut self) -> Result<Option<Interpretation>, Incomplete> {
        let mut budget = Budget {
            quota: crate::search::LocalQuota,
            limits: self.limits.search,
            control: &self.control,
            statistics: self.statistics.search,
        };
        let result = advance(
            Membership {
                theory: &self.theory,
                limits: self.limits,
                certificate: self.certification.as_ref(),
                reduct: &mut self.reduct,
            },
            &mut self.proposer,
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

struct Membership<'a> {
    theory: &'a Theory,
    limits: Limits,
    certificate: Option<&'a certified::Certification>,
    reduct: &'a mut crate::prepared_reduct::State,
}

/// How classical candidates are proposed: from a clause form of the theory
/// by the retained cursor, or from regions narrowed by the theory's readings.
#[derive(Debug)]
enum Proposer {
    Clauses(Box<ClauseProposer>),
    Regions(Box<RegionSearch>),
}

/// The clause form of the theory and the retained cursor over it.
#[derive(Debug)]
struct ClauseProposer {
    cnf: Cnf,
    cursor: Cursor,
}

impl Proposer {
    /// The next classical candidate, or `None` when the proposer has
    /// covered the candidate space. A proposal is refused, not returned,
    /// once the candidate ceiling is reached; the caller admits it.
    fn propose(
        &mut self,
        theory: &Theory,
        limits: Limits,
        budget: &mut Budget<'_>,
        statistics: &mut Statistics,
    ) -> Result<Option<Interpretation>, Incomplete> {
        let proposal = match self {
            Self::Clauses(clauses) => {
                increment(&mut statistics.candidate_queries)?;
                match clauses.cursor.query(&clauses.cnf, budget) {
                    Solve::Sat(assignment) => {
                        if statistics.candidates >= limits.max_candidates {
                            return Err(Incomplete::CandidateLimit);
                        }
                        Some(encoding::interpretation(theory, &assignment, budget)?)
                    }
                    Solve::Unsat => None,
                    Solve::Inconclusive(error) => return Err(error),
                }
            }
            Self::Regions(regions) => {
                let proposal = regions.propose(theory, budget)?;
                if proposal.is_some() && statistics.candidates >= limits.max_candidates {
                    return Err(Incomplete::CandidateLimit);
                }
                proposal
            }
        };
        Ok(proposal)
    }

    /// Exclude a proposed candidate from every later proposal. Regions need
    /// nothing: a leaf is never visited twice.
    fn exclude(
        &mut self,
        candidate: &Interpretation,
        budget: &mut Budget<'_>,
    ) -> Result<(), Incomplete> {
        match self {
            Self::Clauses(clauses) => clauses.cursor.exclude(&clauses.cnf, candidate, budget),
            Self::Regions(_) => Ok(()),
        }
    }

    /// Restrict every later proposal to the classical models of `restriction`.
    fn restrict(
        &mut self,
        restriction: &Theory,
        budget: &mut Budget<'_>,
    ) -> Result<(), Incomplete> {
        match self {
            Self::Clauses(clauses) => {
                encoding::restrict(&mut clauses.cnf, restriction, budget)?;
                clauses.cursor.restart();
                Ok(())
            }
            Self::Regions(regions) => regions.restrict(restriction),
        }
    }
}

fn advance(
    membership_input: Membership<'_>,
    proposer: &mut Proposer,
    budget: &mut Budget<'_>,
    statistics: &mut Statistics,
    pending_error: &mut Option<Incomplete>,
) -> Result<Option<Interpretation>, Incomplete> {
    let Membership {
        theory,
        limits,
        certificate,
        reduct,
    } = membership_input;
    loop {
        let started = timing::start(statistics.phase_timings.as_ref());
        let proposal = proposer.propose(theory, limits, budget, statistics);
        timing::finish(&mut statistics.phase_timings, Phase::Candidates, started);
        let Some(candidate) = proposal? else {
            return Ok(None);
        };
        increment(&mut statistics.candidates)?;
        let result = if let Some(certificate) = certificate {
            match certified::classify(
                certificate,
                &candidate,
                limits,
                budget.control,
                statistics,
                &mut budget.statistics,
            )? {
                certified::Verdict::Stable => Check::Stable,
                certified::Verdict::NotModel => Check::NotModel,
                certified::Verdict::Unsupported { atom } => Check::Unsupported { atom },
            }
        } else {
            reduct.check(theory, &candidate, limits, budget, statistics)?
        };
        if matches!(result, Check::NotModel | Check::Inconclusive(_)) {
            return Err(Incomplete::InvalidWitness);
        }
        let started = timing::start(statistics.phase_timings.as_ref());
        let blocking = proposer.exclude(&candidate, budget);
        timing::finish(&mut statistics.phase_timings, Phase::Candidates, started);
        if matches!(result, Check::Stable) {
            increment(&mut statistics.stable_models)?;
            *pending_error = blocking.err();
            return Ok(Some(candidate));
        }
        blocking?;
    }
}

#[cfg(test)]
#[path = "../tests/support/prepared_owner.rs"]
mod prepared_owner_tests;
