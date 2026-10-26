use zetesis_ferraris::{Interpretation, Theory, models, models_reduct};
use zetesis_objective::{PlanIdentity, Score};

use crate::budget::{Budget, Quota, increment};
use crate::timing::{self, Phase};
use crate::{Cancellation, Incomplete, SearchLimits, SearchStatistics};

mod batch;
pub use batch::{
    BatchError, BatchLimits, BatchStatistics, BatchVerdict, Lookahead, LookaheadDeclined,
    SlateProposal, SlateTicket,
};

mod completion;
pub use completion::{CompletionExecutor, CompletionStatistics};

mod certified;
mod conditions;
pub use certified::{
    CertificateError, CertificateLimits, CertificateOrder, CertificatePlanStatistics,
    CertifiedStatistics,
};

mod regions;

mod learned;
pub use learned::LearnedStore;

mod original_index;
mod passes;
pub(crate) use original_index::IndexedTheory;
use original_index::OriginalIndex;

mod parallel_regions;
pub use parallel_regions::{Engagement, QueryDivision};
mod region_frontier;
use parallel_regions::ParallelRegions;
mod region_proposals;
use region_proposals::RegionProposals;
use regions::RegionSearch;
pub use regions::{
    FrontierOrder, GeneratorOrderStatistics, GeneratorOrderStatus, QueryDivisionStatistics,
    RegionCounts, RegionFrontierStatistics, RegionSearchStatistics, RegionWalkEngagement,
    RegionWorkerStatistics,
};
pub(crate) use regions::{Inline, Publication, Queried, ReductQuery};

/// Whole-operation ceilings for a membership check or stable-model enumeration.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Cumulative region search, certificate and query work and decisions
    /// across a run.
    pub search: SearchLimits,
    /// Maximum classical candidates checked during enumeration; not model count.
    pub max_candidates: u64,
    /// Per-call work ceiling for independent original/reduct formula evaluation.
    pub max_verification_work: u64,
    /// Retained capacity of each proper-subset query's workspace.
    /// Original candidate generator ordering uses the same ceiling independently
    /// for its transient graph and retained ranks; a named byte refusal declines
    /// that optional preference, with its work prefix retained in search work.
    pub max_reduct_bytes: u64,
    /// Additional retained native-extrema narrowing index bytes per owner.
    /// This excludes legacy Boolean indexes and transient construction peak.
    /// Successful preparation admits actual capacity before publishing an index.
    pub max_native_narrowing_bytes: u64,
    /// Allowances of the rows a walk learns: each tier's rows and capture,
    /// the shared store's rows, and what and when a tier shares. Exhausting
    /// one disables that learning and never ends the enumeration.
    pub learning: zetesis_ferraris::LearningLimits,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            search: SearchLimits::default(),
            max_candidates: 1_000_000,
            max_verification_work: 100_000_000,
            max_reduct_bytes: crate::reduct::DEFAULT_BYTES,
            max_native_narrowing_bytes: u64::MAX,
            learning: zetesis_ferraris::LearningLimits::DEFAULT,
        }
    }
}

impl Limits {
    /// What these limits leave to a later stream of the same run once an
    /// earlier stream has spent `spent`: the cumulative work, decision and
    /// candidate ceilings less what it charged, at least zero. Per-call and
    /// capacity ceilings are each stream's own and stay as they are. A run
    /// that searches in passes constructs each later pass's stream with this,
    /// so its passes together stay within the run's limits.
    #[must_use]
    pub const fn remaining_after(self, spent: &Statistics) -> Self {
        Self {
            search: SearchLimits {
                max_work: self.search.max_work.saturating_sub(spent.search.work),
                max_decisions: self
                    .search
                    .max_decisions
                    .saturating_sub(spent.search.decisions),
            },
            max_candidates: self.max_candidates.saturating_sub(spent.candidates),
            ..self
        }
    }
}

/// Low-level membership verdict data. Native [`check`] returns a verdict for its
/// supplied subject; the enum itself carries no binding to that subject or its
/// theory and can also be constructed by callers. For a record that retains the
/// subject of an actual native check, use [`crate::check_interpretation`].
#[derive(Clone, Debug)]
pub enum Check {
    /// Original model whose frozen reduct has no proper-subset model.
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
    /// Region search, certificate and query work, including incomplete attempts.
    pub search: SearchStatistics,
    /// Successful permanent restrictions and bound updates, cumulatively.
    /// This is an installation count, not the population of retained indexes.
    pub candidate_restrictions: u64,
    /// Owned candidates proposed for membership checking, including retained
    /// pending proposals in the batched protocol.
    pub candidates: u64,
    /// Frozen-reduct proper-subset queries started.
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
    /// The region proposer's statistics, on an enumeration's receipt.
    pub regions: Option<RegionSearchStatistics>,
    /// Original-region callback attempts; source work is accounted by its owner.
    pub region_filter: Option<crate::RegionFilterStatistics>,
    /// Actual reduct-query work and storage, including failures.
    pub reduct: crate::ReductStatistics,
}

fn verification(limits: Limits) -> zetesis_ferraris::Limits {
    zetesis_ferraris::Limits {
        max_work: limits.max_verification_work,
        max_subsets: 0,
    }
}

/// Check stability through the proper-subset query of the frozen reduct, a
/// region tree narrowed by the reduct's readings. A countermodel is validated
/// independently before it refutes the candidate. No enumeration of all
/// subsets or invocation of an external solver occurs.
/// `max_candidates` applies only to [`StableModels`].
#[must_use]
pub fn check(
    theory: &Theory,
    candidate: &Interpretation,
    limits: Limits,
    cancellation: &Cancellation,
) -> Check {
    let mut budget = Budget {
        quota: crate::budget::LocalQuota,
        limits: limits.search,
        cancellation,
        statistics: SearchStatistics::default(),
    };
    let mut statistics = Statistics::default();
    match fresh_membership(theory, candidate, limits, &mut budget, &mut statistics) {
        Ok(result) => result,
        Err(error) => Check::Inconclusive(error),
    }
}

fn fresh_membership(
    theory: &Theory,
    candidate: &Interpretation,
    limits: Limits,
    budget: &mut Budget<'_, impl Quota>,
    statistics: &mut Statistics,
) -> Result<Check, Incomplete> {
    let original = original_model(theory, candidate, limits, budget, statistics);
    if !original? {
        return Ok(Check::NotModel);
    }
    let started = timing::start(statistics.phase_timings.as_ref());
    // The one standalone holder: its index is built here, inside the reduct
    // phase, and charged to the reduct's region counts.
    let mut original = OriginalIndex::new(theory, limits.max_native_narrowing_bytes);
    let result = original
        .ensure(budget.cancellation, |work| {
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
            crate::reduct::State::default().check(
                theory,
                Some(index),
                candidate,
                limits,
                budget,
                statistics,
            )
        });
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

/// Validate a proposed countermodel independently: it must be a proper
/// subset of the candidate and model the candidate's frozen reduct. Every
/// proposer, the frozen-positive closure and the region query alike, is held
/// to this.
pub(crate) fn checked_countermodel(
    theory: &Theory,
    candidate: &Interpretation,
    subset: Interpretation,
    limits: Limits,
    budget: &mut Budget<'_, impl Quota>,
    statistics: &mut Statistics,
) -> Result<Check, Incomplete> {
    validated_countermodel(theory, candidate, subset, limits, budget, statistics)
        .map(Check::NonMinimal)
}

/// [`checked_countermodel`]'s validation, returning the countermodel it
/// accepts: a served region of a published query records it as its query's
/// witness rather than as a verdict.
pub(crate) fn validated_countermodel(
    theory: &Theory,
    candidate: &Interpretation,
    subset: Interpretation,
    limits: Limits,
    budget: &mut Budget<'_, impl Quota>,
    statistics: &mut Statistics,
) -> Result<Interpretation, Incomplete> {
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
    Ok(subset)
}

/// Native answer-set enumeration over classical candidates, each region leaf
/// proposed once. A complete positive certificate can restrict proposals to the
/// unique possible answer; larger classical models need not be enumerated or
/// refuted individually. Returned interpretations contain only the original
/// theory's atom universe.
///
/// Work and candidate limits are cumulative. An incomplete result is emitted
/// once and terminates the iterator without marking it exhausted.
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
    /// The plan and score of the installed candidate bound, which every later
    /// bound must strictly improve on.
    active_bound: Option<(PlanIdentity, Score)>,
    /// The final receipts of the passes before this stream's, which every
    /// statistics read reports this stream's after ([`Self::follow`]).
    earlier: Option<Box<passes::Earlier>>,
    reduct: crate::reduct::State,
    /// The original theory's index, built when a region walk first needs it
    /// and shared by that walk and every membership query of its candidates.
    index: OriginalIndex,
}
impl StableModels {
    /// Enumerate by regions of the candidate space narrowed by the theory's
    /// readings, with no clause form built: the theory's producers are
    /// extracted for the support cut, charged as search work, and the root
    /// region is queued; the theory's index is built, and charged, when a
    /// region walk first needs it, so a run decided by a positive
    /// certificate builds none. The reduct's proper-subset query is a region
    /// tree too, over the same index. Original-model and frozen-reduct checks
    /// always use the original theory.
    ///
    /// # Errors
    /// Refuses admission, work limits, cancellation or allocation.
    pub fn new(
        theory: &Theory,
        limits: Limits,
        cancellation: Cancellation,
    ) -> Result<Self, Incomplete> {
        let mut budget = Budget {
            quota: crate::budget::LocalQuota,
            limits: limits.search,
            cancellation: &cancellation,
            statistics: SearchStatistics::default(),
        };
        let proposer = Proposer::Regions(Box::new(RegionSearch::new(
            theory,
            limits.max_native_narrowing_bytes,
            limits.max_reduct_bytes,
            limits.learning,
            &mut budget,
        )?));
        let statistics = Statistics {
            search: budget.statistics,
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
            active_bound: None,
            earlier: None,
            reduct: crate::reduct::State::default(),
            index: OriginalIndex::new(theory, limits.max_native_narrowing_bytes),
        })
    }

    /// Enumerate by regions with up to `workers` walking the tree at once:
    /// the walk begins on the calling thread and starts the workers once it
    /// has run, in wall time since it began, for longer than starting them
    /// costs (see [`Self::with_region_workers_engaging`]). Each worker has its
    /// own knowledge, budget lease and reduct query state, sharing the
    /// immutable original index and stealing from per-worker deques of
    /// regions still to visit. A candidate's proper-subset query that runs
    /// long while a worker is idle is divided among the workers
    /// ([`Self::set_query_division`]). Deque growth can return an allocation
    /// refusal.
    /// The models arrive in the scalar walk's order until the workers start,
    /// and in the schedule's order after, which is not a property of the
    /// result and differs between runs; the family is exact. One worker is
    /// the scalar regions method.
    ///
    /// # Errors
    /// Refuses admission, work limits, cancellation or allocation.
    pub fn with_region_workers(
        theory: &Theory,
        workers: std::num::NonZeroUsize,
        limits: Limits,
        cancellation: Cancellation,
    ) -> Result<Self, Incomplete> {
        Self::with_region_workers_engaging(
            theory,
            workers,
            Engagement::default(),
            limits,
            cancellation,
        )
    }

    /// Enumerate by regions with up to `workers` walking the tree at once,
    /// starting them as `engagement` says.
    ///
    /// The walk begins at the first pull on the calling thread, and returns the
    /// models it verifies there in the scalar walk's order. Once its engagement
    /// delay has passed, it starts `workers` threads, which continue from its
    /// remaining frontier. The delay is wall time since the walk began, the
    /// caller's time between pulls included, checked between the walk's steps.
    /// Models then arrive in the schedule's order, which is not a property of the
    /// result; the family is exact under every engagement.
    ///
    /// The enumeration owns the threads and joins them when it is exhausted,
    /// stopped or dropped. Until it engages it owns one worker's workspace instead,
    /// released when the walk ends, engages, stops or is dropped. One worker is the
    /// scalar regions method, and `engagement` is then ignored.
    ///
    /// # Errors
    /// Refuses admission, work limits, cancellation or allocation, as
    /// [`Self::with_region_workers`] does. Threads are created at engagement, not
    /// here. A thread creation the host refuses then stops the enumeration with
    /// `Incomplete::Allocation`, after the models already returned: the threads
    /// already started are joined and the models they queued are discarded. A
    /// refused thread creation never establishes exhaustion.
    ///
    /// # Costs
    /// A walk that ends within its delay starts no thread. Against
    /// `Engagement::Immediate`, a walk that engages finishes at most about its
    /// delay plus one step later. Until engagement the walk advances only inside
    /// pulls, so the caller's per-model work does not overlap it.
    pub fn with_region_workers_engaging(
        theory: &Theory,
        workers: std::num::NonZeroUsize,
        engagement: Engagement,
        limits: Limits,
        cancellation: Cancellation,
    ) -> Result<Self, Incomplete> {
        if workers.get() == 1 {
            return Self::new(theory, limits, cancellation);
        }
        let mut budget = Budget {
            quota: crate::budget::LocalQuota,
            limits: limits.search,
            cancellation: &cancellation,
            statistics: SearchStatistics::default(),
        };
        let parallel = ParallelRegions::new(
            theory,
            workers,
            engagement,
            limits,
            cancellation.clone(),
            &mut budget,
        )?;
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
            active_bound: None,
            earlier: None,
            reduct: crate::reduct::State::default(),
            index: OriginalIndex::new(theory, limits.max_native_narrowing_bytes),
        })
    }

    /// Produce classical candidates in bounded Rayon runs, one for each slate
    /// of candidates the stream checks and commits together, leaving
    /// membership to this iterator or its injected batch checker. Unlike
    /// [`Self::with_region_workers`], producers never decide answer-set membership.
    /// This permits the same original region traversal to feed a device oracle.
    ///
    /// Each slate joins its producers before checking begins. Native search
    /// work and decision allowances are cumulative; an external checker's costs
    /// belong to its own contract. Candidate order is schedule-dependent, and a
    /// stopped slate retains its completed proposals.
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
            return Self::new(theory, limits, cancellation);
        }
        let mut budget = Budget {
            quota: crate::budget::LocalQuota,
            limits: limits.search,
            cancellation: &cancellation,
            statistics: SearchStatistics::default(),
        };
        let proposals = RegionProposals::new(
            theory,
            workers,
            limits.max_native_narrowing_bytes,
            limits.max_reduct_bytes,
            limits.learning,
            &mut budget,
        )?;
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
            active_bound: None,
            earlier: None,
            reduct: crate::reduct::State::default(),
            index: OriginalIndex::new(theory, limits.max_native_narrowing_bytes),
        })
    }

    /// Divide a parallel region walk's proper-subset queries among its workers
    /// as `division` says, [`QueryDivision::WhenIdle`] by default. Answer sets
    /// are the same under every division; what differs is which worker
    /// narrows a query's regions. A walk keeps the division it began with, so
    /// a setting after the first pull is ignored, as a late timing enable is;
    /// the walk's statistics name the division it ran with. Other proposers
    /// divide no query and ignore it.
    pub fn set_query_division(&mut self, division: QueryDivision) {
        if let Proposer::Parallel(parallel) = &mut self.proposer {
            parallel.set_query_division(division);
        }
    }

    /// Enable coarse host timing from this point onward. Repeated calls retain
    /// existing measurements. No timing clock is read while timing is
    /// disabled; a parallel region walk reads the clock between its
    /// coordinator's steps until its workers start, to keep its engagement
    /// deadline. A walk's timing is fixed when the walk begins.
    pub fn enable_phase_timing(&mut self) {
        self.statistics.phase_timings.get_or_insert_default();
    }

    /// Original immutable theory used for every independent reduct check.
    #[must_use]
    pub fn theory(&self) -> &Theory {
        &self.theory
    }

    /// Append a classical candidate-only constraint. The region proposer
    /// narrows the regions still to visit by it and continues; a positive
    /// cursor tests the least interpretation against these same restrictions.
    /// Original theory and reduct acceptance stay unchanged.
    ///
    /// The restriction must use the original semantic atom count and index
    /// meanings. Its separate immutable instance is expected. Restrictions
    /// accumulate by conjunction; this method cannot widen a candidate region.
    /// Exhaustion afterwards proves coverage only of that constrained region.
    /// An optimizer must separately justify that omitted candidates are dominated.
    ///
    /// # Errors
    /// Refuses a different atom count, a closed iterator, a pending failure,
    /// capacity, work, cancellation or allocation; charged work remains
    /// cumulative.
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
            quota: crate::budget::LocalQuota,
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

    /// Install the bound an objective plan compiled at an incumbent's `score`:
    /// a candidate-only restriction to the interpretations costing less than
    /// the incumbent, or at most as much, under that `plan`.
    ///
    /// # Checked
    /// The first bound is accepted as given. Every later bound must come from
    /// the same `plan`, and its `score` must be strictly better than the active
    /// bound's (`Score::compare_costs` is `Less`); otherwise it is refused with
    /// [`Incomplete::CandidateBoundPlan`] or
    /// [`Incomplete::CandidateBoundNotImproving`], and the active bound stays.
    /// A plan's bound at a strictly better score keeps only interpretations its
    /// bound at the worse score keeps too, strict or not, so regions pruned
    /// under an earlier bound never hold a candidate a later bound admits.
    /// `restriction` must be the bound `plan` compiled at `score`; this
    /// operation checks the plan and the score, not the restriction's formula.
    /// It must use the original atom count and semantic index meanings. Bounds
    /// never support original atoms and never change the original theory or
    /// its reduct.
    ///
    /// Region search retains only the latest bound separately from permanent
    /// [`Self::restrict_candidates`] constraints. Active worker snapshots may
    /// finish under an older bound, and already pending candidates are retained;
    /// their original membership checks still apply. An optimizer must compare
    /// every returned model with its current incumbent. Exhaustion covers the
    /// remaining constrained family, not the original unrestricted world view.
    ///
    /// # Errors
    /// Refuses a closed iterator, a pending error, a different atom count,
    /// another plan or a score that does not improve, generation overflow,
    /// resource exhaustion or cancellation. Failed setup leaves the active
    /// bound and generation unchanged; admitted work stays charged. Region
    /// knowledge retains its existing infallible allocation contract; this
    /// operation does not make all search allocation fallible.
    pub fn improve_candidate_bound(
        &mut self,
        plan: PlanIdentity,
        score: &Score,
        restriction: &Theory,
    ) -> Result<(), Incomplete> {
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
        if let Some((active, previous)) = &self.active_bound {
            if *active != plan {
                return Err(Incomplete::CandidateBoundPlan);
            }
            if score.compare_costs(previous) != std::cmp::Ordering::Less {
                return Err(Incomplete::CandidateBoundNotImproving);
            }
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
            quota: crate::budget::LocalQuota,
            limits: self.limits.search,
            cancellation: &self.cancellation,
            statistics: self.statistics.search,
        };
        let result = self.proposer.tighten(restriction, generation, &mut budget);
        self.statistics.search = budget.statistics;
        if result.is_ok() {
            self.bound_generation = generation;
            self.active_bound = Some((plan, score.clone()));
            self.statistics.candidate_restrictions = count;
        }
        result
    }

    /// Declare, before the stream's first proposal, that the caller will
    /// improve candidate bounds ([`Self::improve_candidate_bound`]) as it
    /// commits answers, as an optimizing search does. Region producers
    /// ([`Self::with_region_producers`]) then share one frontier
    /// ([`FrontierOrder::Shared`]) for the whole stream: every producer takes
    /// the region any producer published most recently, so a slate fills from
    /// the area the latest splits opened, and older, shallower regions wait for
    /// the tighter bounds that refute them near the root. Without the
    /// declaration each producer keeps its own deque, the order an enumeration
    /// prefers. Other routes are unaffected, and answer sets are unchanged
    /// either way.
    ///
    /// # Errors
    /// Refuses a closed or started stream, so one order governs every slate
    /// and the region statistics' `frontier_order` names the order the
    /// stream's producers take regions in.
    pub fn expect_candidate_bounds(&mut self) -> Result<(), Incomplete> {
        if self.terminal {
            return Err(Incomplete::ClosedEnumerator);
        }
        if self.started() {
            return Err(Incomplete::LateCandidateBounds);
        }
        if let Proposer::Proposals(proposals) = &mut self.proposer {
            proposals.share_frontier();
        }
        // A slate produced ahead would be produced under a bound its
        // predecessor's answers may tighten, so such a stream never proposes ahead.
        self.batch.bounds_expected = true;
        Ok(())
    }

    /// Whether candidate traversal has begun: an error or batch is pending, a
    /// candidate was proposed or queried, or a region was visited. Choices
    /// that must govern the whole traversal refuse a started stream.
    fn started(&self) -> bool {
        let statistics = self.own_statistics();
        self.pending_error.is_some()
            || !self.batch.pending.is_empty()
            || statistics.candidates != 0
            || statistics
                .regions
                .is_some_and(|regions| regions.counts.regions != 0)
    }

    /// Install an original-program condition before traversal: the streamed
    /// constraints, or a test's filter. Certificates may already be prepared.
    /// The filter can narrow or refute candidate regions; it never participates
    /// in frozen-reduct queries. Exhaustion then covers the family allowed by
    /// this filter as well as explicit candidate restrictions.
    ///
    /// # Errors
    /// Refuses a closed or started stream, or replacement.
    pub(crate) fn set_region_filter(
        &mut self,
        filter: std::sync::Arc<dyn crate::RegionFilter>,
    ) -> Result<(), Incomplete> {
        if self.terminal {
            return Err(Incomplete::ClosedEnumerator);
        }
        if self.started() {
            return Err(Incomplete::LateRegionFilter);
        }
        if self.proposer.filter().is_some() {
            return Err(Incomplete::RegionFilterAlreadySet);
        }
        let filter = crate::region_filter::Filter::new(filter);
        match &mut self.proposer {
            Proposer::Regions(regions) => regions.set_filter(filter),
            Proposer::Proposals(proposals) => proposals.set_filter(filter),
            Proposer::Parallel(parallel) => parallel.set_filter(filter)?,
        }
        Ok(())
    }

    /// Check an admitted hybrid program's streamed integrity constraints over
    /// this stream's candidate regions, before traversal. The stream's theory
    /// must be the constraints' own core theory, which each checker's
    /// preparation authenticates. The check can narrow or refute candidate
    /// regions and never participates in frozen-reduct queries; exhaustion then
    /// covers the core's stable models that satisfy the constraints, as well as
    /// explicit candidate restrictions.
    ///
    /// # Errors
    /// Refuses a closed or started stream, or replacement.
    pub fn set_streamed_constraints(
        &mut self,
        constraints: std::sync::Arc<crate::StreamedConstraints>,
    ) -> Result<(), Incomplete> {
        self.set_region_filter(constraints)
    }

    /// Share learned rows through `store` with every other stream given the
    /// same store, before traversal. The stream's walks seed their tiers from
    /// it and deliver to it at their boundaries (see [`LearnedStore`]); rows
    /// only ever prune candidate regions and never decide membership or
    /// exhaustion. A stream given no store keeps its rows to each walk.
    ///
    /// # Errors
    /// Refuses a closed or started stream, or replacement.
    pub fn set_learned_store(
        &mut self,
        store: std::sync::Arc<LearnedStore>,
    ) -> Result<(), Incomplete> {
        if self.terminal {
            return Err(Incomplete::ClosedEnumerator);
        }
        if self.started() {
            return Err(Incomplete::LateLearnedStore);
        }
        if self.proposer.learned_store().is_some() {
            return Err(Incomplete::LearnedStoreAlreadySet);
        }
        match &mut self.proposer {
            Proposer::Regions(regions) => regions.set_learned_store(store),
            Proposer::Proposals(proposals) => proposals.set_learned_store(store),
            Proposer::Parallel(parallel) => parallel.set_learned_store(store)?,
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

    /// Bounds installed so far by [`Self::improve_candidate_bound`]: zero
    /// before the first, one more after each successful installation. A
    /// refused or failed tightening, a permanent [`Self::restrict_candidates`]
    /// addition and every batch operation leave it unchanged; it never
    /// decreases. It is the generation the region proposer assigns to the
    /// current bound. A scheduling observation only: membership, exhaustion
    /// and optimality never depend on it.
    #[must_use]
    pub const fn bound_generation(&self) -> u64 {
        self.bound_generation
    }

    /// Cumulative work, including an incomplete terminal attempt, after the
    /// receipts of the earlier passes this stream follows ([`Self::follow`]).
    #[must_use]
    pub fn statistics(&self) -> Statistics {
        match &self.earlier {
            Some(earlier) => passes::statistics(&earlier.statistics, &self.own_statistics()),
            None => self.own_statistics(),
        }
    }

    /// This stream's own receipts, which decide whether it has started.
    fn own_statistics(&self) -> Statistics {
        let mut statistics = match &self.proposer {
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
                    // The walk's coordinator and worker phases, plus the
                    // enumeration's index build before the walk began.
                    phase_timings: crate::timing::combined(
                        merged.phase_timings,
                        self.statistics.phase_timings,
                    ),
                    reduct: crate::ReductStatistics {
                        original_work: merged.reduct.original_work,
                        frozen_positive_attempts: merged.reduct.frozen_positive_attempts,
                        frozen_positive_completed: merged.reduct.frozen_positive_completed,
                        frozen_positive_preparation_work: merged
                            .reduct
                            .frozen_positive_preparation_work,
                        frozen_positive_work: merged.reduct.frozen_positive_work,
                        peak_workspace_bytes: merged.reduct.peak_workspace_bytes,
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

    /// Report `earlier`'s receipts with this stream's, as one search over the
    /// same theory that an optimizer runs in passes, such as a tie pass after
    /// the pass that proved its optimum: from now on [`Self::statistics`] and
    /// [`Self::batch_statistics`] report `earlier`'s final receipts, its own
    /// earlier passes included, followed by this stream's. Work and counts
    /// add, peaks take the larger, and what describes the route as it stands
    /// is this stream's. Membership, exhaustion and every limit stay this
    /// stream's own; only the receipts combine, so a run that must stay within
    /// its limits across passes constructs this stream with
    /// [`Limits::remaining_after`] `earlier`'s statistics. Calling it again
    /// replaces the earlier receipts.
    ///
    /// # Errors
    /// Refuses a stream over another theory instance.
    pub fn follow(&mut self, earlier: &StableModels) -> Result<(), Incomplete> {
        if !earlier.theory.same_instance(&self.theory) {
            return Err(Incomplete::WrongTheory);
        }
        self.earlier = Some(Box::new(passes::Earlier {
            statistics: earlier.statistics(),
            batch: earlier.batch_statistics(),
        }));
        Ok(())
    }

    fn advance(&mut self) -> Result<Option<Interpretation>, Incomplete> {
        let mut budget = Budget {
            quota: crate::budget::LocalQuota,
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
    reduct: &'a mut crate::reduct::State,
    index: &'a mut OriginalIndex,
    determined_candidates: Option<&'a mut certified::DeterminedCandidates>,
}

/// The component that proposes classical candidates from regions narrowed by
/// the theory's readings: the scalar walk, workers that walk the tree in
/// parallel and decide its leaves, or bounded producers. No proposal needs
/// excluding afterwards: each region proposer proposes a leaf once, since only
/// the leaf's emission resolves its region, and a producer's leaf that finds
/// the slate's output spoken for returns unresolved and is proposed later.
#[derive(Debug)]
enum Proposer {
    Regions(Box<RegionSearch>),
    /// The coordinator and, after engagement, workers walk the region tree
    /// and decide its leaves.
    Parallel(Box<ParallelRegions>),
    /// Bounded parallel production, before any membership operation.
    Proposals(Box<RegionProposals>),
}

/// Where the next proposal comes from: the positive cursor, which walks no
/// region, or the proposer's own walk with the original index it reads.
enum Walk<'a> {
    Determined(&'a mut certified::DeterminedCandidates),
    Index(&'a std::sync::Arc<IndexedTheory>),
}

/// What a proposer hands the enumeration.
enum Proposal {
    /// A classical model the reduct has yet to decide.
    Candidate(Interpretation),
    /// A stable model the coordinator or a worker has already decided.
    Stable(Interpretation),
}

impl Proposer {
    fn filter(&self) -> Option<&crate::region_filter::Filter> {
        match self {
            Self::Regions(regions) => regions.filter(),
            Self::Proposals(proposals) => proposals.filter(),
            Self::Parallel(parallel) => parallel.filter(),
        }
    }

    fn learned_store(&self) -> Option<&std::sync::Arc<LearnedStore>> {
        match self {
            Self::Regions(regions) => regions.learned_store(),
            Self::Proposals(proposals) => proposals.learned_store(),
            Self::Parallel(parallel) => parallel.learned_store(),
        }
    }

    /// Serial and joined producers are idle between pulls. Release their
    /// borrowed scratch when the stream closes, preserving candidate receipts.
    fn finish_production(&mut self) -> Result<(), Incomplete> {
        match self {
            Self::Regions(regions) => regions.finish(),
            Self::Proposals(proposals) => proposals.finish(),
            Self::Parallel(_) => Ok(()),
        }
    }

    /// Record the original index's work, charged when a region walk first
    /// needed it, once in this proposer's region counts.
    fn record_index_work(&mut self, work: u64) -> Result<(), Incomplete> {
        let counts = match self {
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
    /// reads the index `walk` carries, which [`walk_index`] built.
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
            Self::Regions(regions) => {
                let proposal =
                    regions.propose(theory, index, budget, &mut statistics.phase_timings)?;
                if proposal.is_some() && statistics.candidates >= limits.max_candidates {
                    return Err(Incomplete::CandidateLimit);
                }
                proposal
            }
            Self::Parallel(parallel) => {
                let timed = statistics.phase_timings.is_some();
                return Ok(parallel
                    .propose(index, certificate, timed, budget)?
                    .map(Proposal::Stable));
            }
            Self::Proposals(proposals) => {
                let subject = index.subject(theory)?;
                let mut output = crate::budget::storage(1)?;
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
                // A slate of one candidate has one hand, which a peer reserves
                // only after the first producer emits, and then finds the
                // output spoken for, so no second producer can stop after the
                // first emits. Batched calls retain that richer outcome.
                if let Some(error) = produced.stopped {
                    return Err(error);
                }
                output.pop()
            }
        };
        Ok(proposal.map(Proposal::Candidate))
    }

    fn tighten(
        &mut self,
        restriction: &Theory,
        generation: u64,
        budget: &mut Budget<'_>,
    ) -> Result<(), Incomplete> {
        match self {
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
        Some(walk_index(
            index,
            proposer,
            budget,
            &mut statistics.phase_timings,
        )?)
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
        let walk = match (determined_candidates.as_deref_mut(), index) {
            (Some(candidates), _) => Walk::Determined(candidates),
            (None, Some(index)) => Walk::Index(index),
            (None, None) => return Err(Incomplete::InvalidWitness),
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
        if decision == Decision::Stable {
            increment(&mut statistics.stable_models)?;
            return Ok(Some(candidate));
        }
    }
}

/// The original index a region walk reads, built and charged before the
/// walk's first step: this is the one build site of every enumeration route,
/// scalar, parallel and batched, and it runs on the coordinator before any
/// worker starts.
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
) -> Result<&'i std::sync::Arc<IndexedTheory>, Incomplete> {
    if original.get().is_some() {
        return original.get().ok_or(Incomplete::InvalidWitness);
    }
    let started = timing::start(timings.as_ref());
    let built = original.ensure(budget.cancellation, |work| {
        budget.charge(work)?;
        proposer.record_index_work(work)
    });
    timing::finish(timings, Phase::Candidates, started);
    built
}

#[cfg(test)]
pub(crate) mod fixtures;
#[cfg(test)]
mod tests;
