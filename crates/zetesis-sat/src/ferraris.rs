use zetesis_ferraris::{Interpretation, Theory, models, models_reduct};

use crate::encoding;
use crate::search::{Budget, Cursor, Quota, increment};
use crate::timing::{self, Phase};
use crate::{
    AdmissionLimits, Cancellation, Cnf, Incomplete, ProjectionLimits, ProjectionStatistics,
    SearchLimits, SearchStatistics, Solve,
};

mod batch;
pub use batch::{BatchError, BatchLimits, BatchStatistics, BatchVerdict};

mod completion;
pub use completion::{CompletionExecutor, CompletionScratch, CompletionStatistics};

mod certified;
mod conditions;
pub use certified::{
    CertificateError, CertificateLimits, CertificateOrder, CertificatePlanStatistics,
    CertifiedStatistics,
};

mod candidate_support;
pub use candidate_support::{SupportStatistics, SupportStatus};

mod reduct_query;

mod regions;

mod original_index;
pub(crate) use original_index::IndexedTheory;
use original_index::OriginalIndex;

mod parallel_regions;
use parallel_regions::ParallelRegions;
mod region_proposals;
use region_proposals::RegionProposals;
pub(crate) use regions::ReductQuery;
use regions::RegionSearch;
pub use regions::{RegionCounts, RegionFrontierStatistics, RegionSearchStatistics, SearchMethod};

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
    /// Outer clause queries started, including a final UNSAT query. Direct
    /// positive proposals and region traversal do not increment this counter.
    pub candidate_queries: u64,
    /// Successful permanent restrictions and bound updates, cumulatively.
    /// This is an installation count, not the population of retained indexes.
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
    /// These are separate from deterministic semantic work counters. Under
    /// several workers they are the workers' own intervals summed with the
    /// coordinator's, such as its build of the original index before the
    /// workers start, which may exceed the wall time of the enumeration.
    pub phase_timings: Option<crate::SearchPhaseTimings>,
    /// Optional complete-theory certificate attempt and checks.
    pub certified: Option<CertifiedStatistics>,
    /// Initial necessary disjunctive support restriction on outer candidates.
    /// Standalone membership checks do not construct this optional restriction,
    /// and the regions proposer has the support cut in its narrowing instead.
    pub support: Option<SupportStatistics>,
    /// The regions proposer's statistics; absent under the clauses proposer.
    pub regions: Option<RegionSearchStatistics>,
    /// Original-region callback attempts; source work is accounted by its owner.
    pub region_filter: Option<crate::RegionFilterStatistics>,
    /// Actual persistent-reduct construction and query work, including failures.
    pub reduct: crate::ReductStatistics,
}

fn verification(limits: Limits) -> zetesis_ferraris::Limits {
    zetesis_ferraris::Limits {
        max_work: limits.max_verification_work,
        max_subsets: 0,
    }
}

/// Check stability through the proper-subset query of the frozen reduct by
/// the default method, [`SearchMethod::default`]: a region tree narrowed by
/// the reduct's readings. [`check_with`] chooses the method. No enumeration
/// of all subsets or invocation of an external solver occurs.
/// `max_candidates` applies only to [`StableModels`].
#[must_use]
pub fn check(
    theory: &Theory,
    candidate: &Interpretation,
    limits: Limits,
    cancellation: &Cancellation,
) -> Check {
    check_with(
        theory,
        candidate,
        SearchMethod::default(),
        limits,
        cancellation,
    )
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
    cancellation: &Cancellation,
) -> Check {
    let mut budget = Budget {
        quota: crate::search::LocalQuota,
        limits: limits.search,
        cancellation,
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
            // The one standalone holder: its index is built here, inside the
            // reduct phase, and charged to the reduct's region counts.
            let mut original = OriginalIndex::new(theory);
            original
                .ensure(|work| {
                    budget.charge(work)?;
                    statistics.reduct.regions.work = statistics
                        .reduct
                        .regions
                        .work
                        .checked_add(work)
                        .ok_or(Incomplete::CounterOverflow)?;
                    Ok(())
                })
                .and_then(|index| {
                    crate::prepared_reduct::State::new(method).check(
                        theory,
                        Some(index),
                        candidate,
                        limits,
                        budget,
                        statistics,
                    )
                })
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
        budget.cancellation.poll()?;
        if !theory.same_instance(candidate.theory()) {
            return Err(Incomplete::WrongTheory);
        }
        models(theory, candidate, verification(limits), budget.cancellation)
            .map_err(Incomplete::from)
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
            budget.cancellation,
        )?
    {
        return Err(Incomplete::InvalidWitness);
    }
    increment(&mut statistics.countermodels)?;
    Ok(Check::NonMinimal(subset))
}

/// Native answer-set enumeration over classical candidates with exact semantic blocking.
/// A complete positive certificate can restrict proposals to the unique possible
/// answer; larger classical models need not be enumerated or refuted individually.
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
    cancellation: Cancellation,
    statistics: Statistics,
    terminal: bool,
    exhausted: bool,
    pending_error: Option<Incomplete>,
    batch: batch::State,
    certificate: Option<certified::Certificate>,
    certificate_workspace: certified::Workspace,
    determined_candidates: Option<certified::DeterminedCandidates>,
    bound_generation: u64,
    reduct: crate::prepared_reduct::State,
    /// The original theory's index, built when a region walk first needs it
    /// and shared by that walk and every membership query of its candidates.
    index: OriginalIndex,
}
impl StableModels {
    /// Enumerate by the default method, [`SearchMethod::default`]: regions
    /// of the candidate space narrowed by the theory's readings, with no
    /// clause form built. [`Self::with_method`] chooses the method; under
    /// [`SearchMethod::Clauses`] the original theory is encoded once,
    /// retaining its immutable instance identity, with a complete ordinary
    /// disjunctive support restriction on the outer CNF, and construction and
    /// failed encoding work stay charged. Original-model and frozen-reduct
    /// checks always use the original theory, whichever the method.
    ///
    /// # Errors
    /// Refuses admission, work limits, cancellation or allocation.
    pub fn new(
        theory: &Theory,
        limits: Limits,
        cancellation: Cancellation,
    ) -> Result<Self, Incomplete> {
        Self::with_method(theory, SearchMethod::default(), limits, cancellation)
    }

    /// Enumerate by regions with several workers walking the tree at once,
    /// each with its own knowledge, budget lease and reduct query state, sharing
    /// the immutable original index and stealing from per-worker deques of
    /// regions still to visit. Deque growth can return an allocation refusal.
    /// The models arrive in the schedule's
    /// order, which is not a property of the result and differs between
    /// runs; the family is exact. One worker is the scalar regions method.
    ///
    /// # Errors
    /// Refuses admission, work limits, cancellation or allocation.
    pub fn with_region_workers(
        theory: &Theory,
        workers: std::num::NonZeroUsize,
        limits: Limits,
        cancellation: Cancellation,
    ) -> Result<Self, Incomplete> {
        if workers.get() == 1 {
            return Self::with_method(theory, SearchMethod::Regions, limits, cancellation);
        }
        let mut budget = Budget {
            quota: crate::search::LocalQuota,
            limits: limits.search,
            cancellation: &cancellation,
            statistics: SearchStatistics::default(),
        };
        let parallel =
            ParallelRegions::new(theory, workers, limits, cancellation.clone(), &mut budget)?;
        let statistics = Statistics {
            search: budget.statistics,
            ..Default::default()
        };
        Ok(Self {
            theory: theory.clone(),
            proposer: Proposer::Parallel(Box::new(parallel)),
            limits,
            cancellation,
            statistics,
            terminal: false,
            exhausted: false,
            pending_error: None,
            batch: batch::State::default(),
            certificate: None,
            certificate_workspace: certified::Workspace::default(),
            determined_candidates: None,
            bound_generation: 0,
            reduct: crate::prepared_reduct::State::new(SearchMethod::Regions),
            index: OriginalIndex::new(theory),
        })
    }

    /// Produce classical candidates in bounded Rayon rounds, leaving membership
    /// to this iterator or its injected batch checker. Unlike
    /// [`Self::with_region_workers`], producers never decide answer-set membership.
    /// This permits the same original region traversal to feed a device oracle.
    ///
    /// Each batch joins its producers before checking begins. Native search
    /// work and decision allowances are cumulative; an external checker's costs
    /// belong to its own contract. Candidate order is schedule-dependent, and a
    /// failed round retains its completed proposals.
    /// One worker uses the ordinary scalar region traversal.
    ///
    /// # Errors
    /// Refuses admission, work limits, cancellation, allocation or pool creation.
    pub fn with_region_producers(
        theory: &Theory,
        workers: std::num::NonZeroUsize,
        limits: Limits,
        cancellation: Cancellation,
    ) -> Result<Self, Incomplete> {
        if workers.get() == 1 {
            return Self::with_method(theory, SearchMethod::Regions, limits, cancellation);
        }
        let mut budget = Budget {
            quota: crate::search::LocalQuota,
            limits: limits.search,
            cancellation: &cancellation,
            statistics: SearchStatistics::default(),
        };
        let proposals = RegionProposals::new(theory, workers, &mut budget)?;
        let statistics = Statistics {
            search: budget.statistics,
            ..Default::default()
        };
        Ok(Self {
            theory: theory.clone(),
            proposer: Proposer::Proposals(Box::new(proposals)),
            limits,
            cancellation,
            statistics,
            terminal: false,
            exhausted: false,
            pending_error: None,
            batch: batch::State::default(),
            certificate: None,
            certificate_workspace: certified::Workspace::default(),
            determined_candidates: None,
            bound_generation: 0,
            reduct: crate::prepared_reduct::State::new(SearchMethod::Regions),
            index: OriginalIndex::new(theory),
        })
    }

    /// Enumerate by the chosen method. Under [`SearchMethod::Regions`] no
    /// clause form of the theory is built: the theory's producers are
    /// extracted for the support cut, charged as search work, and the root
    /// region is queued; the theory's index is built, and charged, when a
    /// region walk first needs it, so a run decided by a positive
    /// certificate builds none. The reduct's proper-subset query is a region
    /// tree too, over the same index. The verdict on every candidate is the
    /// same either way.
    ///
    /// # Errors
    /// Refuses admission, work limits, cancellation or allocation.
    pub fn with_method(
        theory: &Theory,
        method: SearchMethod,
        limits: Limits,
        cancellation: Cancellation,
    ) -> Result<Self, Incomplete> {
        let mut budget = Budget {
            quota: crate::search::LocalQuota,
            limits: limits.search,
            cancellation: &cancellation,
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
            cancellation,
            statistics,
            terminal: false,
            exhausted: false,
            pending_error: None,
            batch: batch::State::default(),
            certificate: None,
            certificate_workspace: certified::Workspace::default(),
            determined_candidates: None,
            bound_generation: 0,
            reduct: crate::prepared_reduct::State::new(method),
            index: OriginalIndex::new(theory),
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
    /// it and continues. A positive cursor tests the least interpretation
    /// against these same restrictions. Original theory and reduct acceptance stay unchanged.
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
            cancellation: &self.cancellation,
            statistics: self.statistics.search,
        };
        let result = self.proposer.restrict(restriction, &mut budget);
        self.statistics.search = budget.statistics;
        if result.is_ok() {
            self.statistics.candidate_restrictions = count;
        }
        result
    }

    /// Install a successively stronger candidate-only bound.
    ///
    /// # Caller obligation
    /// Each bound must use the original atom count and semantic index meanings.
    /// Every classical model of this bound must satisfy the previous bound,
    /// if any. This logical implication is **not checked**. Violating it can
    /// omit models because regions pruned earlier are not reopened. The first
    /// bound has no implication obligation. Bounds never support original atoms
    /// and never change the original theory or its reduct.
    ///
    /// Region search retains only the latest bound separately from permanent
    /// [`Self::restrict_candidates`] constraints. Active worker snapshots may
    /// finish under an older bound, and already pending candidates are retained;
    /// their original membership checks still apply. An optimizer must compare
    /// every returned model with its current incumbent. Exhaustion covers the
    /// remaining constrained family, not the original unrestricted world view.
    /// The optional clauses method appends each bound instead of retiring it;
    /// the implication obligation makes that conjunction equivalent.
    ///
    /// # Errors
    /// Refuses a different atom count, a closed iterator, pending blocking error,
    /// generation overflow, resource exhaustion or cancellation. Failed setup
    /// leaves the active bound and generation unchanged; admitted work stays
    /// charged. Region knowledge retains its existing infallible allocation
    /// contract; this operation does not make all search allocation fallible.
    pub fn tighten_candidate_bound(&mut self, restriction: &Theory) -> Result<(), Incomplete> {
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
        let generation = self
            .bound_generation
            .checked_add(1)
            .ok_or(Incomplete::CounterOverflow)?;
        let count = self
            .statistics
            .candidate_restrictions
            .checked_add(1)
            .ok_or(Incomplete::CounterOverflow)?;
        let mut budget = Budget {
            quota: crate::search::LocalQuota,
            limits: self.limits.search,
            cancellation: &self.cancellation,
            statistics: self.statistics.search,
        };
        let result = self.proposer.tighten(restriction, generation, &mut budget);
        self.statistics.search = budget.statistics;
        if result.is_ok() {
            self.bound_generation = generation;
            self.statistics.candidate_restrictions = count;
        }
        result
    }

    /// Install a caller-owned original-program condition before traversal.
    /// Certificates may already be prepared. The filter can narrow or refute
    /// candidate regions; it never participates in frozen-reduct queries.
    /// Exhaustion then covers the family allowed by this filter as well as
    /// explicit candidate restrictions. See [`crate::RegionFilter`] for the
    /// caller's soundness, resource and failure-receipt obligations.
    ///
    /// # Errors
    /// Refuses a closed or started stream, replacement, or clause search.
    pub fn set_region_filter(
        &mut self,
        filter: std::sync::Arc<dyn crate::RegionFilter>,
    ) -> Result<(), Incomplete> {
        if self.terminal {
            return Err(Incomplete::ClosedEnumerator);
        }
        let statistics = self.statistics();
        if self.pending_error.is_some()
            || !self.batch.pending.is_empty()
            || statistics.candidates != 0
            || statistics.candidate_queries != 0
            || statistics
                .regions
                .is_some_and(|regions| regions.counts.regions != 0)
        {
            return Err(Incomplete::LateRegionFilter);
        }
        if self.proposer.filter().is_some() {
            return Err(Incomplete::RegionFilterAlreadySet);
        }
        let filter = crate::region_filter::Filter::new(filter);
        match &mut self.proposer {
            Proposer::Clauses(_) => return Err(Incomplete::RegionFilterUnsupported),
            Proposer::Regions(regions) => regions.set_filter(filter),
            Proposer::Proposals(proposals) => proposals.set_filter(filter),
            Proposer::Parallel(parallel) => parallel.set_filter(filter)?,
        }
        Ok(())
    }

    /// Fuse future pulls and join candidate workers without cancelling the
    /// caller's shared token. Statistics retain every joined worker's prefix;
    /// unresolved batch entries remain visible in their pending receipt.
    /// Queued worker models are discarded, not counted as delivered answers.
    /// An early stop cannot establish exhaustion; prior exhaustion is retained.
    ///
    /// # Errors
    /// Reports the first worker stop or a failure to merge joined receipts.
    /// All workers are joined even on failure; the stream remains fused.
    pub fn stop(&mut self) -> Result<(), Incomplete> {
        self.terminal = true;
        let production = self.proposer.finish_production();
        if let Proposer::Parallel(parallel) = &mut self.proposer {
            let result = parallel.stop();
            let joined = parallel.search_statistics();
            // Before the first pull, certificate preparation can have charged
            // the coordinator while the workers still hold construction totals.
            self.statistics.search.work = self.statistics.search.work.max(joined.work);
            self.statistics.search.decisions =
                self.statistics.search.decisions.max(joined.decisions);
            return result;
        }
        production
    }

    /// True only after successful coverage of every unreturned answer set
    /// satisfying all successful candidate restrictions and the original-region
    /// filter, if either was configured. A complete positive certificate can
    /// establish this without refuting larger classical models individually.
    #[must_use]
    pub const fn exhausted(&self) -> bool {
        self.exhausted
    }
    /// Cumulative work, including an incomplete terminal attempt.
    #[must_use]
    pub fn statistics(&self) -> Statistics {
        let mut statistics = match &self.proposer {
            Proposer::Clauses(clauses) => Statistics {
                projections: clauses.cursor.projection_statistics(),
                ..self.statistics
            },
            Proposer::Regions(regions) => Statistics {
                regions: Some(regions.statistics()),
                ..self.statistics
            },
            Proposer::Proposals(proposals) => Statistics {
                regions: Some(proposals.statistics()),
                ..self.statistics
            },
            Proposer::Parallel(parallel) if self.determined_candidates.is_none() => {
                let merged = parallel.merged();
                let merged = &merged;
                let mut certified = self.statistics.certified;
                if let (Some(into), Some(from)) = (certified.as_mut(), merged.certified.as_ref()) {
                    into.checks = from.checks;
                    into.stable = from.stable;
                    into.refuted = from.refuted;
                    into.failed = from.failed;
                    into.checking_work = from.checking_work;
                    into.tight_check_peak_bytes = from.tight_check_peak_bytes;
                    into.positive_check_peak_bytes = from.positive_check_peak_bytes;
                    into.stratified_check_peak_bytes = from.stratified_check_peak_bytes;
                }
                Statistics {
                    regions: Some(parallel.statistics()),
                    candidates: merged.candidates,
                    countermodel_queries: merged.countermodel_queries,
                    countermodels: merged.countermodels,
                    certified,
                    // The workers' sums, and the coordinator's own phases:
                    // the index build before the workers launched.
                    phase_timings: crate::timing::combined(
                        merged.phase_timings,
                        self.statistics.phase_timings,
                    ),
                    reduct: crate::ReductStatistics {
                        original_work: merged.reduct.original_work,
                        regions: merged.reduct.regions,
                        ..self.statistics.reduct
                    },
                    ..self.statistics
                }
            }
            Proposer::Parallel(parallel) => Statistics {
                regions: Some(parallel.statistics()),
                ..self.statistics
            },
        };
        statistics.region_filter = self
            .proposer
            .filter()
            .map(crate::region_filter::Filter::statistics);
        statistics
    }

    fn advance(&mut self) -> Result<Option<Interpretation>, Incomplete> {
        let mut budget = Budget {
            quota: crate::search::LocalQuota,
            limits: self.limits.search,
            cancellation: &self.cancellation,
            statistics: self.statistics.search,
        };
        let result = advance(
            Membership {
                theory: &self.theory,
                limits: self.limits,
                certificate: self
                    .certificate
                    .as_ref()
                    .and_then(certified::Certificate::cpu),
                certificate_workspace: &mut self.certificate_workspace,
                reduct: &mut self.reduct,
                index: &mut self.index,
                determined_candidates: self.determined_candidates.as_mut(),
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
            let _ = self.proposer.finish_production();
            return Some(Err(Incomplete::PendingBatch));
        }
        if let Some(error) = self.pending_error.take() {
            self.terminal = true;
            let _ = self.proposer.finish_production();
            return Some(Err(error));
        }
        match self.advance() {
            Ok(Some(model)) => Some(Ok(model)),
            Ok(None) => {
                self.terminal = true;
                if let Err(error) = self.proposer.finish_production() {
                    return Some(Err(error));
                }
                self.exhausted = true;
                None
            }
            Err(error) => {
                // Terminal evidence includes every worker's settled prefix.
                // Cleanup cannot replace the interruption that stopped this pull.
                let _ = self.stop();
                Some(Err(error))
            }
        }
    }
}
impl std::iter::FusedIterator for StableModels {}

/// What the enumeration does with a candidate's membership verdict: the
/// candidate is an answer set; it is refuted, by a proper-subset model of
/// its reduct or by the support law, and the search goes on; or the verdict
/// is one no proposed candidate can have, every candidate being a classical
/// model, and the enumeration stops on an invalid witness.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Decision {
    Stable,
    Refuted,
    Invalid,
}
impl From<Check> for Decision {
    fn from(check: Check) -> Self {
        match check {
            Check::Stable => Self::Stable,
            Check::NonMinimal(_) => Self::Refuted,
            Check::NotModel | Check::Inconclusive(_) => Self::Invalid,
        }
    }
}

struct Membership<'a> {
    theory: &'a Theory,
    limits: Limits,
    /// The certificate as the enumeration owns it, so that workers can
    /// share it; the coordinator reads through it.
    certificate: Option<&'a std::sync::Arc<certified::Certification>>,
    certificate_workspace: &'a mut certified::Workspace,
    reduct: &'a mut crate::prepared_reduct::State,
    index: &'a mut OriginalIndex,
    determined_candidates: Option<&'a mut certified::DeterminedCandidates>,
}

/// The component that proposes classical candidates: it realizes the
/// [`SearchMethod`] the enumeration was asked for, from a clause form of the
/// theory by the retained cursor or from regions narrowed by the theory's
/// readings, and under several workers the regions method walked in
/// parallel, which no method names.
#[derive(Debug)]
enum Proposer {
    Clauses(Box<ClauseProposer>),
    Regions(Box<RegionSearch>),
    /// Several workers walk the region tree and decide the leaves themselves.
    Parallel(Box<ParallelRegions>),
    /// Bounded parallel production, before any membership operation.
    Proposals(Box<RegionProposals>),
}

/// Where the next proposal comes from: the positive cursor, which walks no
/// region, or the proposer's own walk with the original index it reads
/// (`None` under the clause kernel, which reads none).
enum Walk<'a> {
    Determined(&'a mut certified::DeterminedCandidates),
    Index(Option<&'a std::sync::Arc<IndexedTheory>>),
}

/// What a proposer hands the enumeration.
enum Proposal {
    /// A classical model the reduct has yet to decide.
    Candidate(Interpretation),
    /// A stable model a worker has already decided.
    Stable(Interpretation),
}

/// The clause form of the theory and the retained cursor over it.
#[derive(Debug)]
struct ClauseProposer {
    cnf: Cnf,
    cursor: Cursor,
}

impl Proposer {
    fn filter(&self) -> Option<&crate::region_filter::Filter> {
        match self {
            Self::Clauses(_) => None,
            Self::Regions(regions) => regions.filter(),
            Self::Proposals(proposals) => proposals.filter(),
            Self::Parallel(parallel) => parallel.filter(),
        }
    }

    /// Serial and joined producers are idle between pulls. Release their
    /// borrowed scratch when the stream closes, preserving candidate receipts.
    fn finish_production(&mut self) -> Result<(), Incomplete> {
        match self {
            Self::Regions(regions) => regions.finish(),
            Self::Proposals(proposals) => proposals.finish(),
            Self::Clauses(_) | Self::Parallel(_) => Ok(()),
        }
    }

    /// Record the original index's work, charged when a region walk first
    /// needed it, once in this proposer's region counts. The clause kernel
    /// walks no region and indexes no original theory.
    fn record_index_work(&mut self, work: u64) -> Result<(), Incomplete> {
        let counts = match self {
            Self::Clauses(_) => return Err(Incomplete::InvalidWitness),
            Self::Regions(regions) => regions.counts_mut(),
            Self::Parallel(parallel) => parallel.counts_mut(),
            Self::Proposals(proposals) => proposals.counts_mut(),
        };
        counts.work = counts
            .work
            .checked_add(work)
            .ok_or(Incomplete::CounterOverflow)?;
        Ok(())
    }

    /// The next classical candidate, from the positive cursor when one is
    /// active and else from this proposer's own walk, or `None` when the
    /// candidate space is covered. A proposal is refused, not returned, once
    /// the candidate ceiling is reached; the caller admits it. A region walk
    /// reads the index `walk` carries, which [`walk_index`] built; its
    /// absence there is refused as an invalid witness.
    fn propose(
        &mut self,
        theory: &Theory,
        walk: Walk<'_>,
        limits: Limits,
        certificate: Option<&std::sync::Arc<certified::Certification>>,
        budget: &mut Budget<'_>,
        statistics: &mut Statistics,
    ) -> Result<Option<Proposal>, Incomplete> {
        let index = match walk {
            Walk::Determined(candidates) => {
                return candidates
                    .propose(self, theory, limits, budget, statistics)
                    .map(|candidate| candidate.map(Proposal::Candidate));
            }
            Walk::Index(index) => index,
        };
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
                let index = index.ok_or(Incomplete::InvalidWitness)?;
                let proposal =
                    regions.propose(theory, index, budget, &mut statistics.phase_timings)?;
                if proposal.is_some() && statistics.candidates >= limits.max_candidates {
                    return Err(Incomplete::CandidateLimit);
                }
                proposal
            }
            Self::Parallel(parallel) => {
                let index = index.ok_or(Incomplete::InvalidWitness)?;
                let timed = statistics.phase_timings.is_some();
                return Ok(parallel
                    .propose(index, certificate, timed, budget)?
                    .map(Proposal::Stable));
            }
            Self::Proposals(proposals) => {
                let subject = index.ok_or(Incomplete::InvalidWitness)?.subject(theory)?;
                let mut output = crate::search::storage(1)?;
                let produced = proposals.fill(
                    subject,
                    1,
                    limits.max_candidates.saturating_sub(statistics.candidates),
                    budget,
                    &mut output,
                    statistics.phase_timings.is_some(),
                );
                if let Some(timings) = statistics.phase_timings.as_mut() {
                    timings
                        .original_validation
                        .merge(produced.original_validation);
                }
                // With one reserved slot no second producer can stop after
                // the first emits. Batched calls retain that richer outcome.
                if let Some(error) = produced.stopped {
                    return Err(error);
                }
                output.pop()
            }
        };
        Ok(proposal.map(Proposal::Candidate))
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
            Self::Regions(_) | Self::Parallel(_) | Self::Proposals(_) => Ok(()),
        }
    }

    fn tighten(
        &mut self,
        restriction: &Theory,
        generation: u64,
        budget: &mut Budget<'_>,
    ) -> Result<(), Incomplete> {
        match self {
            Self::Clauses(_) => self.restrict(restriction, budget),
            Self::Regions(regions) => regions.tighten(restriction, generation, budget),
            Self::Parallel(parallel) => parallel.tighten(restriction, generation, budget),
            Self::Proposals(proposals) => proposals.tighten(restriction, generation, budget),
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
            Self::Regions(regions) => regions.restrict(restriction, budget),
            Self::Parallel(parallel) => parallel.restrict(restriction, budget),
            Self::Proposals(proposals) => proposals.restrict(restriction, budget),
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
        certificate_workspace,
        reduct,
        index,
        mut determined_candidates,
    } = membership_input;
    let index = if determined_candidates.is_none() {
        walk_index(index, proposer, budget, &mut statistics.phase_timings)?
    } else {
        None
    };
    loop {
        // The parallel walk's workers time their own phases; the wait for
        // their models is not a phase.
        let started = match proposer {
            Proposer::Parallel(_) if determined_candidates.is_none() => None,
            _ => timing::start(statistics.phase_timings.as_ref()),
        };
        let walk = match determined_candidates.as_deref_mut() {
            Some(candidates) => Walk::Determined(candidates),
            None => Walk::Index(index),
        };
        let proposal = proposer.propose(theory, walk, limits, certificate, budget, statistics);
        timing::finish(&mut statistics.phase_timings, Phase::Candidates, started);
        let candidate = match proposal? {
            None => return Ok(None),
            Some(Proposal::Stable(model)) => {
                // Decided by a worker; the coordinator only counts it.
                increment(&mut statistics.stable_models)?;
                return Ok(Some(model));
            }
            Some(Proposal::Candidate(candidate)) => candidate,
        };
        increment(&mut statistics.candidates)?;
        let decision: Decision = if let Some(certificate) = certificate {
            certified::classify(
                certificate,
                certificate_workspace,
                &candidate,
                limits,
                budget.cancellation,
                statistics,
                &mut budget.statistics,
            )?
            .into()
        } else {
            reduct
                .check(
                    theory,
                    index.map(std::sync::Arc::as_ref),
                    &candidate,
                    limits,
                    budget,
                    statistics,
                )?
                .into()
        };
        if decision == Decision::Invalid {
            return Err(Incomplete::InvalidWitness);
        }
        let started = timing::start(statistics.phase_timings.as_ref());
        let blocking = proposer.exclude(&candidate, budget);
        timing::finish(&mut statistics.phase_timings, Phase::Candidates, started);
        if decision == Decision::Stable {
            increment(&mut statistics.stable_models)?;
            *pending_error = blocking.err();
            return Ok(Some(candidate));
        }
        blocking?;
    }
}

/// The original index a region walk reads, built and charged before the
/// walk's first step: this is the one build site of every enumeration route,
/// scalar, parallel and batched, and it runs on the coordinator before any
/// worker starts. The clause kernel walks no region and gets `None`.
///
/// The build is charged to `budget` first, one unit per node and operand
/// occurrence, and recorded once in the proposer's region counts in the same
/// step, so search work and
/// `regions.work` agree on every exit, a build that fails after its charge
/// was admitted included. Its time, when timed, is one call of the candidate
/// phase; a built index costs neither work nor a timed call.
fn walk_index<'i>(
    original: &'i mut OriginalIndex,
    proposer: &mut Proposer,
    budget: &mut Budget<'_>,
    timings: &mut Option<crate::SearchPhaseTimings>,
) -> Result<Option<&'i std::sync::Arc<IndexedTheory>>, Incomplete> {
    if matches!(proposer, Proposer::Clauses(_)) {
        return Ok(None);
    }
    if original.get().is_some() {
        return Ok(original.get());
    }
    let started = timing::start(timings.as_ref());
    let built = original.ensure(|work| {
        budget.charge(work)?;
        proposer.record_index_work(work)
    });
    timing::finish(timings, Phase::Candidates, started);
    built.map(Some)
}

#[cfg(test)]
mod tests;
