//! Candidates proposed by regions: the coverage tree over the theory's
//! atoms, narrowed by the theory's readings.
//!
//! Checked original-answer arithmetic prepares the root before the walk. A
//! refuted root starts no walk; otherwise its proved units seed the root and
//! every remaining atom stays open. Each region is narrowed to the
//! fixed point of `zetesis_ferraris::Narrower::narrow_known` over the
//! original theory, from its parent's knowledge, with
//! its producers for the support cut, and over every candidate-only
//! restriction, without producers, since a restriction is not a rule of the
//! program and supports nothing. A region no reading refutes is split on
//! the atom its narrowing prefers, held branch first; a region with every atom
//! decided is a leaf, and a leaf is a classical model of the theory and the
//! restrictions, because at a full decision every root is sure or
//! never and a root never refutes. The leaf is the proposal, which the reduct
//! decides.
//!
//! The narrowing's node reads are charged as search work and each split as
//! a decision. A restriction narrows the regions still to visit; visited
//! regions were covered under the original theory, and a restriction only
//! removes candidates, so no restart and no exclusion index is needed
//! (`Search.CoverageTree`, `FormulaBounds`).

use std::cell::Cell;
use std::sync::Arc;

use zetesis_algebra::{Failure, Meter, Stop};
use zetesis_algebra::{
    region::{Entry, Narrowing, Region},
    walk::{self, Counting, Visit, Walk},
};
use zetesis_ferraris::{
    ChoiceHeads, FrozenPreparation, FrozenSubject, GeneratorOrder, GeneratorOrderError,
    Interpretation, LearningLimits, LearningOwner, Narrower, NarrowingAttempt, NarrowingScratch,
    OriginalSubject, PreparationError, PreparationLimits, Producers, RegionLimits, RootPreparation,
    SupportExplanationStatistics, Theory,
};

use super::conditions::{Bound, CandidateKnowledge, Conditions, CountermodelKnowledge};
use super::learned::{LearnedStore, Tier};
use super::original_index::IndexedTheory;
use crate::budget::{Budget, Quota};
use crate::subject::{SubjectAttempt, SubjectStep, joint};
use crate::{Cancellation, Incomplete};

pub(crate) mod arithmetic_counts;
pub(super) mod depth;
mod subjects;
pub(super) use subjects::copy_knowledge;
use subjects::{CandidateSubject, CountermodelSubject};

/// What a walk over a region tree counted, cumulatively: the candidate
/// tree's walk, or the proper-subset queries' walks together.
///
/// Runtime arithmetic attribution is retained internally and projected into
/// [`RegionSearchStatistics`]. It participates in equality, debug output and
/// checked merging along with the public dimensions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct RegionCounts {
    /// Regions narrowed, the root included. A producer's leaf that finds its
    /// slate's output spoken for is narrowed, and counted, again by the slate
    /// that proposes it.
    pub regions: usize,
    /// Regions refuted by readings or an original-candidate filter. Frozen
    /// proper-subset queries use only their reduct readings.
    pub refuted: usize,
    /// The classical candidates proposed, or the proper-subset models and the
    /// candidate itself: regions with every atom decided, each counted once,
    /// when it is proposed.
    pub leaves: usize,
    /// Propagation events: nodes and atoms learned and their neighbours
    /// revisited, support rechecks, and fresh source-consequence mask updates.
    pub propagations: u64,
    /// Atoms the readings or original source consequences held.
    pub held: u64,
    /// Atoms the readings or original source consequences cut.
    pub cut: u64,
    /// Node reads, root tests and producer checks, and for the candidate
    /// tree the producer extraction, optional arithmetic and generator-order preparation,
    /// the indexing of each restriction and,
    /// once a region walk first needs it, the indexing of the theory, one
    /// unit per node and operand occurrence (kept even if building the index
    /// then fails); included in search work. A run decided by a positive
    /// certificate walks no
    /// region and indexes no theory. Enumeration queries share the walk's
    /// original index. A standalone membership query includes its own index
    /// construction in its reduct counts. Each fresh source-consequence mask
    /// update costs one unit; source join work is accounted separately.
    pub work: u64,
    /// Charged producer rechecks, included in `work` on every outcome.
    pub support_work: u64,
    /// Charged possible-foundedness preparation, closure and publication,
    /// included in `work` on every outcome. This is not a propagation-event count.
    pub founded_work: u64,
    /// Bounded support-explanation capture and reuse; its work is included in `work`.
    /// The byte peak is the largest single worker ledger/capture footprint, not
    /// the sum across workers or total process memory.
    pub support_explanations: SupportExplanationStatistics,
    // Private attribution travels with the existing receipt. Public arithmetic
    // observations are projected by the original-region statistics owner.
    pub(crate) arithmetic: arithmetic_counts::ArithmeticCounts,
}

impl RegionCounts {
    /// Add another walk's counts to these.
    ///
    /// # Errors
    /// A sum beyond its counter's width is the counter refusal, with these
    /// counts unchanged.
    pub fn add(&mut self, other: Self) -> Result<(), Incomplete> {
        let sum = Self {
            regions: self
                .regions
                .checked_add(other.regions)
                .ok_or(Incomplete::CounterOverflow)?,
            refuted: self
                .refuted
                .checked_add(other.refuted)
                .ok_or(Incomplete::CounterOverflow)?,
            leaves: self
                .leaves
                .checked_add(other.leaves)
                .ok_or(Incomplete::CounterOverflow)?,
            propagations: self
                .propagations
                .checked_add(other.propagations)
                .ok_or(Incomplete::CounterOverflow)?,
            held: self
                .held
                .checked_add(other.held)
                .ok_or(Incomplete::CounterOverflow)?,
            cut: self
                .cut
                .checked_add(other.cut)
                .ok_or(Incomplete::CounterOverflow)?,
            work: self
                .work
                .checked_add(other.work)
                .ok_or(Incomplete::CounterOverflow)?,
            support_work: self
                .support_work
                .checked_add(other.support_work)
                .ok_or(Incomplete::CounterOverflow)?,
            founded_work: self
                .founded_work
                .checked_add(other.founded_work)
                .ok_or(Incomplete::CounterOverflow)?,
            support_explanations: SupportExplanationStatistics {
                attempts: self
                    .support_explanations
                    .attempts
                    .checked_add(other.support_explanations.attempts)
                    .ok_or(Incomplete::CounterOverflow)?,
                certified: self
                    .support_explanations
                    .certified
                    .checked_add(other.support_explanations.certified)
                    .ok_or(Incomplete::CounterOverflow)?,
                matched: self
                    .support_explanations
                    .matched
                    .checked_add(other.support_explanations.matched)
                    .ok_or(Incomplete::CounterOverflow)?,
                cut: self
                    .support_explanations
                    .cut
                    .checked_add(other.support_explanations.cut)
                    .ok_or(Incomplete::CounterOverflow)?,
                forced: self
                    .support_explanations
                    .forced
                    .checked_add(other.support_explanations.forced)
                    .ok_or(Incomplete::CounterOverflow)?,
                refuted: self
                    .support_explanations
                    .refuted
                    .checked_add(other.support_explanations.refuted)
                    .ok_or(Incomplete::CounterOverflow)?,
                declined: self
                    .support_explanations
                    .declined
                    .checked_add(other.support_explanations.declined)
                    .ok_or(Incomplete::CounterOverflow)?,
                work: self
                    .support_explanations
                    .work
                    .checked_add(other.support_explanations.work)
                    .ok_or(Incomplete::CounterOverflow)?,
                peak_bytes: self
                    .support_explanations
                    .peak_bytes
                    .max(other.support_explanations.peak_bytes),
            },
            arithmetic: self.arithmetic.checked_add(other.arithmetic)?,
        };
        *self = sum;
        Ok(())
    }
}

// Keep the public solver receipt name while the generic frontier owns its fields.
pub use zetesis_algebra::walk::frontier::Statistics as RegionFrontierStatistics;

/// What the region proposer did, cumulatively: the counts of its walk over
/// the candidate tree, and whether the theory lies in the producer
/// fragment.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RegionSearchStatistics {
    /// The walk's counts.
    pub counts: RegionCounts,
    /// Whether the theory lies in the producer fragment, so the support
    /// cut applies.
    pub producers: bool,
    /// Shared original generator-order preparation; included in `counts.work`.
    pub generator_order: GeneratorOrderStatistics,
    /// Checked original-answer affine preparation, including declined prefixes.
    pub parity: zetesis_ferraris::ParityStatistics,
    /// Checked original-answer counting preparation, including declined prefixes.
    pub cardinality: zetesis_ferraris::CardinalityStatistics,
    /// Frontier ownership measured by parallel candidate production. Other
    /// schedules do not currently measure it; absence does not mean zero bytes.
    pub frontier: Option<RegionFrontierStatistics>,
    /// The parallel region walk's workers. The scalar walk starts no worker,
    /// and the device route's pool starts at construction, so both report
    /// `None`.
    pub workers: Option<RegionWorkerStatistics>,
    /// The order in which the parallel candidate producers take regions,
    /// fixed for the stream before its first proposal. Routes without
    /// producers report `None`.
    pub frontier_order: Option<FrontierOrder>,
}

impl RegionSearchStatistics {
    /// Project this receipt once without mutating the preparation observations.
    /// Repeated snapshots neither consume nor add their cumulative activity.
    pub(super) fn with_runtime_arithmetic(mut self) -> Self {
        let activity = self.counts.arithmetic;
        self.parity.region_passes = activity.parity_passes;
        self.parity.forced = activity.parity_forced;
        self.parity.refuted = activity.parity_refuted;
        self.cardinality.region_passes = activity.cardinality_passes;
        self.cardinality.refuted = activity.cardinality_refuted;
        self
    }
}

/// The order in which the parallel candidate producers take regions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum FrontierOrder {
    /// Each producer continues on its own deque and steals a peer's oldest
    /// region only when its own deque is empty: the order for an enumeration,
    /// which visits every region whatever the order.
    PerProducer,
    /// Every producer publishes to one shared deque and takes the region any
    /// producer published most recently: the order for a caller that tightens
    /// candidate bounds as it commits answers, which keeps production where the
    /// latest splits are and leaves older, shallower regions for tighter
    /// bounds.
    Shared,
}

impl FrontierOrder {
    /// Stable spelling in execution receipts.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::PerProducer => "per_producer",
            Self::Shared => "shared",
        }
    }
}

/// Applicability of the optional original generator preference.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum GeneratorOrderStatus {
    /// An earlier conclusive preparation made this optional order unnecessary.
    NotPrepared,
    /// Mandatory preparation stopped; its executed prefix remains recorded.
    Incomplete,
    /// The complete producer fragment or a generating head is absent.
    #[default]
    NotApplicable,
    /// An immutable dependency order was prepared for original candidates.
    Prepared,
    /// Its named simultaneous storage exceeded the inherited allowance.
    StorageDeclined,
}

impl GeneratorOrderStatus {
    /// Stable spelling in execution receipts.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::NotPrepared => "not_prepared",
            Self::Incomplete => "incomplete",
            Self::NotApplicable => "not_applicable",
            Self::Prepared => "prepared",
            Self::StorageDeclined => "storage_declined",
        }
    }
}

/// One original candidate owner's generator-order preparation, never summed
/// across its workers. Active restrictions still use their existing primary
/// preference. Prepared does not assert a split occurred or a class certificate.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GeneratorOrderStatistics {
    /// Whether a reusable order was published or why it declined.
    pub status: GeneratorOrderStatus,
    /// Preparation work, including a declined prefix; already in reading work.
    pub work: u64,
    /// Maximum named simultaneous preparation request or capacity observed.
    pub peak_bytes: u128,
    /// Published order header and vector capacities; shared theory excluded.
    pub retained_bytes: u128,
}

/// The parallel region walk's workers: how many, when they were to start,
/// and whether and when they did.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RegionWorkerStatistics {
    /// The workers the walk was configured with.
    pub configured: usize,
    /// The engagement delay in force, `configured × c` by default; `None` for
    /// a delay whose deadline overflows, which never engages.
    pub engagement_delay: Option<std::time::Duration>,
    /// Whether and when the walk engaged its workers, and what it did before.
    pub engagement: RegionWalkEngagement,
    /// The candidates' proper-subset queries the workers divided among
    /// themselves.
    pub query_division: QueryDivisionStatistics,
}

/// The proper-subset queries a parallel region walk published for any of its
/// workers to serve, and who served their regions. The counts are current
/// while the workers run. A query's regions narrowed before its publication
/// are its publisher's, counted with every query's regions in the reduct
/// statistics' regions, as are the regions served here.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct QueryDivisionStatistics {
    /// The division the walk ran with.
    pub division: super::QueryDivision,
    /// Queries whose pending regions were published.
    pub published: u64,
    /// Published regions a worker narrowed. A region taken once its query
    /// was decided is dropped without a narrowing and not counted.
    pub served: u64,
    /// The served regions a worker took from a peer's deque.
    pub stolen: u64,
    /// The workers that served at least one published region.
    pub servers: usize,
    /// The most published regions any one worker served.
    pub peak_served: u64,
}

/// Whether a parallel region walk engaged its workers. The record is frozen
/// once the walk ends.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegionWalkEngagement {
    /// No pull has begun the walk. A run whose candidates a positive
    /// certificate determined never walks.
    NotBegun,
    /// The walk has run on the calling thread only. `elapsed` runs from the
    /// walk's beginning to the end of its last pull that stepped, the caller's
    /// time between pulls included; `walking` is the coordinator's own time
    /// inside those pulls.
    Unengaged {
        /// From the walk's beginning to the end of its last pull that stepped.
        elapsed: std::time::Duration,
        /// The coordinator's own time inside the pulls that stepped.
        walking: std::time::Duration,
    },
    /// The workers started `after` the walk's beginning, once the coordinator
    /// had spent `walking` inside its pulls; `launched` threads were spawned
    /// (fewer than configured only after a refused reservation or spawn), and
    /// the spawn loop took `spawn`.
    Engaged {
        /// When the workers started, from the walk's beginning.
        after: std::time::Duration,
        /// The coordinator's own time inside its pulls before then.
        walking: std::time::Duration,
        /// The threads actually spawned.
        launched: usize,
        /// The spawn loop's wall duration.
        spawn: std::time::Duration,
    },
}

impl RegionWalkEngagement {
    /// Stable machine-readable identifier of the state: `not_begun`,
    /// `unengaged` or `engaged`.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::NotBegun => "not_begun",
            Self::Unengaged { .. } => "unengaged",
            Self::Engaged { .. } => "engaged",
        }
    }
}

type Checker<'a> = Option<crate::region_filter::Worker<'a>>;

self_cell::self_cell! {
    /// The original-source factory owns the preparation borrowed by this
    /// enumeration's one serial checker. Moving the stream never moves the
    /// owner relative to its checker. No checker is shared with another stream,
    /// source or frozen-reduct query. The cell is allocated only when a filter
    /// is installed; its small allocation follows the boxed-checker contract.
    struct RetainedFilter {
        owner: crate::region_filter::Filter,
        #[not_covariant]
        dependent: Checker,
    }
}

impl std::fmt::Debug for RetainedFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RetainedFilter")
            .field("factory", self.borrow_owner())
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub(crate) struct RegionSearch {
    parts: OriginalParts,
    /// Each region carries what is known about it under the theory and
    /// under each restriction, in order; a restriction added after a region
    /// was reached gets fresh knowledge when the region is next narrowed.
    traversal: Walk<CandidateKnowledge>,
    /// Each restriction with its own index.
    restrictions: Conditions<(Theory, Narrower)>,
    /// The worklists every narrowing of this walk reuses, with its tier of
    /// learned rows.
    scratch: NarrowingScratch,
    /// The tier's sharing state.
    tier: Tier,
    statistics: RegionSearchStatistics,
    filter: Option<RetainedFilter>,
    native_byte_limit: u64,
}

/// What a region walk reads of the original theory beyond the theory itself:
/// its producers, when it lies in the producer fragment, the heads of its
/// atomic choice roots, which every split ranks first, the owner of the rows
/// its walks learn, and the store its tiers share them through, if any.
#[derive(Debug)]
pub(crate) struct OriginalParts {
    producers: Option<Producers>,
    choice_heads: ChoiceHeads,
    generator_order: Option<GeneratorOrder>,
    arithmetic: Option<subjects::PreparedArithmetic>,
    learning: LearningOwner,
    learned: Option<Arc<LearnedStore>>,
}

/// Borrow the original candidate reading and its immutable arithmetic systems.
/// The original parts establish the theory association; the caller supplies
/// that theory's existing narrower. Restrictions and frozen queries do not
/// receive these systems.
#[derive(Clone, Copy)]
pub(super) struct CandidateReading<'a> {
    pub(super) original: OriginalSubject<'a>,
    pub(super) narrower: &'a Narrower,
    pub(super) arithmetic: Option<&'a subjects::PreparedArithmetic>,
}

/// Which way a walk boundary moves a tier's rows.
#[derive(Clone, Copy)]
enum Sharing {
    Seed,
    Deliver,
}

impl OriginalParts {
    /// The walk's subject over `theory`, the theory these parts came from:
    /// the carrier of the rows the walk learns. Restrictions, the bound and
    /// filters are read by other subjects, which never carry rows.
    pub(crate) fn subject(&self, theory: &Theory) -> Result<OriginalSubject<'_>, Incomplete> {
        if !self.learning.owns(theory)
            || self
                .arithmetic
                .as_ref()
                .is_some_and(|prepared| !prepared.owner.belongs_to(theory))
        {
            return Err(Incomplete::WrongTheory);
        }
        Ok(
            OriginalSubject::carrier(&self.learning, self.producers.as_ref())
                .with_choices(&self.choice_heads)
                .with_generator_order(self.generator_order.as_ref()),
        )
    }

    /// Borrow the original subjects after the existing ownership check.
    /// The narrower is the one lent by the authenticated original index.
    pub(super) fn reading<'a>(
        &'a self,
        theory: &Theory,
        narrower: &'a Narrower,
    ) -> Result<CandidateReading<'a>, Incomplete> {
        Ok(CandidateReading {
            original: self.subject(theory)?,
            narrower,
            arithmetic: self.arithmetic.as_ref(),
        })
    }

    /// The stream's learned store, once installed.
    pub(crate) const fn learned_store(&self) -> Option<&Arc<LearnedStore>> {
        self.learned.as_ref()
    }

    /// Install the stream's learned store; the caller refuses a walk that
    /// has begun and a replacement.
    pub(crate) fn set_learned_store(&mut self, store: Arc<LearnedStore>) {
        self.learned = Some(store);
    }

    /// Seed `scratch`'s tier from the store before the tier's first
    /// narrowing, or its first after [`Tier::reseed`].
    pub(crate) fn seed_tier<Q: Quota>(
        &self,
        tier: &mut Tier,
        scratch: &mut NarrowingScratch,
        budget: &mut Budget<'_, Q>,
        counts: &mut RegionCounts,
    ) -> Result<(), Incomplete> {
        if tier.seeded {
            return Ok(());
        }
        tier.seeded = true;
        self.share(Sharing::Seed, scratch, budget, counts)
    }

    /// Count one region the walk visited with `scratch`; at the owner's
    /// publication trigger, deliver the tier. The caller calls this between
    /// regions, never inside a narrowing.
    pub(crate) fn visited<Q: Quota>(
        &self,
        tier: &mut Tier,
        scratch: &mut NarrowingScratch,
        budget: &mut Budget<'_, Q>,
        counts: &mut RegionCounts,
    ) -> Result<(), Incomplete> {
        tier.visited = tier.visited.saturating_add(1);
        let every = self.learning.limits().publish_regions;
        if every == 0 || tier.visited < every {
            return Ok(());
        }
        self.deliver_tier(tier, scratch, budget, counts)
    }

    /// Deliver `scratch`'s tier to the store at a walk boundary.
    pub(crate) fn deliver_tier<Q: Quota>(
        &self,
        tier: &mut Tier,
        scratch: &mut NarrowingScratch,
        budget: &mut Budget<'_, Q>,
        counts: &mut RegionCounts,
    ) -> Result<(), Incomplete> {
        tier.visited = 0;
        self.share(Sharing::Deliver, scratch, budget, counts)
    }

    /// Move rows between `scratch`'s tier and the store, under the store's
    /// lock, charging the budget. The reads count as learning work in
    /// `counts`, admitted prefix included. A stream given no store shares
    /// nothing. Exhausting the tier's sharing allowance ends sharing, not the
    /// walk; the quota, a cancellation and an allocator refusal stop it.
    fn share<Q: Quota>(
        &self,
        sharing: Sharing,
        scratch: &mut NarrowingScratch,
        budget: &mut Budget<'_, Q>,
        counts: &mut RegionCounts,
    ) -> Result<(), Incomplete> {
        let Some(store) = &self.learned else {
            return Ok(());
        };
        let mut quota = BudgetQuota { budget };
        let mut meter = Meter::reserved(&mut quota);
        let attempt = {
            let mut rows = store.lock();
            match sharing {
                Sharing::Seed => scratch.seed_learning(&self.learning, &mut rows, &mut meter),
                Sharing::Deliver => scratch.deliver_learning(&self.learning, &mut rows, &mut meter),
            }
        };
        counts.add(RegionCounts {
            work: attempt.work,
            support_explanations: SupportExplanationStatistics {
                work: attempt.work,
                ..SupportExplanationStatistics::default()
            },
            ..RegionCounts::default()
        })?;
        attempt.result.map_err(meter_failure)
    }
}

/// What opening a region search over a theory establishes: the parts of the
/// original theory its walk reads, and the statistics of the extraction,
/// charged to the budget. The original theory's index is not built here: the
/// enumeration's `OriginalIndex` builds it when a walk first needs it, and its
/// work is then recorded in these counts.
pub(crate) struct Opened {
    pub(crate) parts: OriginalParts,
    pub(crate) statistics: RegionSearchStatistics,
    /// Absent only after an absorbing original-answer refutation.
    pub(crate) root: Option<Region>,
}

/// Opening result with every settled preparation prefix, even on failure.
pub(crate) struct Opening {
    pub(crate) result: Result<Opened, Incomplete>,
    pub(crate) statistics: RegionSearchStatistics,
}

/// Extract original producers, checked arithmetic, and then generator order.
/// Publish the settled receipt before inspecting success or failure. A proved
/// arithmetic contradiction still absorbs before order or root allocation.
pub(crate) fn open_attempt(
    theory: &Theory,
    order_byte_limit: u64,
    learning: LearningLimits,
    budget: &mut Budget<'_>,
) -> Opening {
    let before = budget.statistics.work;
    let mut statistics = RegionSearchStatistics {
        generator_order: GeneratorOrderStatistics {
            status: GeneratorOrderStatus::NotPrepared,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut result = open_recording(theory, order_byte_limit, learning, budget, &mut statistics);
    statistics.counts.work = budget.statistics.work - before;
    if let Ok(opened) = result.as_mut() {
        opened.statistics = statistics;
    } else {
        // These objects were dropped with the failed opening; their peak and
        // work receipts remain, but no published capacity remains retained.
        statistics.generator_order.retained_bytes = 0;
        statistics.parity.retained_bytes = 0;
        statistics.cardinality.retained_bytes = 0;
    }
    Opening { result, statistics }
}

#[cfg(test)]
pub(crate) fn open(
    theory: &Theory,
    order_byte_limit: u64,
    learning: LearningLimits,
    budget: &mut Budget<'_>,
) -> Result<Opened, Incomplete> {
    open_attempt(theory, order_byte_limit, learning, budget).result
}

fn open_recording(
    theory: &Theory,
    order_byte_limit: u64,
    learning: LearningLimits,
    budget: &mut Budget<'_>,
    statistics: &mut RegionSearchStatistics,
) -> Result<Opened, Incomplete> {
    let extraction = zetesis_ferraris::producers(theory, limits(budget), budget.cancellation)
        .map_err(stopped)?;
    budget.charge(extraction.work)?;
    statistics.producers = extraction.producers.is_some();
    let cancellation = budget.cancellation;
    let mut quota = BudgetQuota { budget };
    let mut meter = Meter::reserved(&mut quota);
    let arithmetic = RootPreparation::prepare_reserved(
        theory,
        extraction.producers.as_ref(),
        PreparationLimits {
            work: PreparationLimits::DEFAULT_WORK,
            bytes: usize::try_from(order_byte_limit).unwrap_or(usize::MAX),
        },
        cancellation,
        &mut meter,
    );
    let prepared = arithmetic.result.map(|published| {
        published.map(|owner| {
            subjects::PreparedArithmetic::new(
                owner,
                arithmetic.parity.rank,
                arithmetic.cardinality.families,
            )
        })
    });
    statistics.parity = arithmetic.parity;
    statistics.cardinality = arithmetic.cardinality;
    let mut arithmetic = prepared.map_err(|error| match error {
        PreparationError::Stopped(error) => meter_failure(error),
        PreparationError::Source(source) => Incomplete::ArithmeticSource(source),
    })?;
    let refuted = arithmetic
        .as_ref()
        .is_some_and(|prepared| prepared.owner.root_refuted());
    let generator_order = if refuted {
        None
    } else {
        let attempt = GeneratorOrder::prepare_reserved(
            OriginalSubject::new(theory, extraction.producers.as_ref()),
            usize::try_from(order_byte_limit).unwrap_or(usize::MAX),
            cancellation,
            &mut meter,
        );
        // The attempt's prefix remains visible even when its result is a stop.
        statistics.generator_order.work = attempt.work;
        statistics.generator_order.peak_bytes = attempt.peak_bytes;
        let (order, status) = match attempt.result {
            Ok(order) => {
                let status = if order.is_some() {
                    GeneratorOrderStatus::Prepared
                } else {
                    GeneratorOrderStatus::NotApplicable
                };
                (order, status)
            }
            Err(GeneratorOrderError::Storage { .. }) => {
                (None, GeneratorOrderStatus::StorageDeclined)
            }
            Err(GeneratorOrderError::Stopped(stop)) => {
                statistics.generator_order.status = GeneratorOrderStatus::Incomplete;
                return Err(meter_failure(stop));
            }
        };
        statistics.generator_order.status = status;
        statistics.generator_order.retained_bytes =
            order.as_ref().map_or(0, GeneratorOrder::retained_bytes);
        order
    };
    let root = if refuted {
        None
    } else {
        Some(
            arithmetic
                .as_mut()
                .and_then(|prepared| prepared.owner.take_seeded_root())
                .unwrap_or_else(|| Region::all_open(theory.atom_count())),
        )
    };
    Ok(Opened {
        statistics: *statistics,
        parts: OriginalParts {
            producers: extraction.producers,
            choice_heads: extraction.choice_heads,
            generator_order,
            arithmetic,
            learning: LearningOwner::theory(theory, learning),
            learned: None,
        },
        root,
    })
}

/// The interpretation a leaf proposes: the atoms the region holds.
pub(crate) fn leaf_interpretation(
    theory: &Theory,
    region: &Region,
) -> Result<Interpretation, Incomplete> {
    let mut selected = crate::budget::storage(theory.atom_count())?;
    selected.extend(region.held());
    Interpretation::new(theory, selected).map_err(|error| match error {
        zetesis_ferraris::AdmissionError::Allocation => Incomplete::Allocation,
        _ => Incomplete::InvalidWitness,
    })
}

impl RegionSearch {
    /// Queue the already prepared root; its first narrowing creates each
    /// knowledge slot. Refuted roots are dispatched before this constructor.
    pub(crate) fn new(opened: Opened, native_byte_limit: u64) -> Result<Self, Incomplete> {
        let Opened {
            parts,
            statistics,
            root,
        } = opened;
        let root = root.ok_or(Incomplete::InvalidWitness)?;
        Ok(Self {
            statistics,
            parts,
            traversal: Walk::with_state(root, Counting::Never, CandidateKnowledge::default())
                .with_held_first(),
            restrictions: Conditions::default(),
            scratch: NarrowingScratch::default(),
            tier: Tier::default(),
            filter: None,
            native_byte_limit,
        })
    }

    pub(crate) fn counts_mut(&mut self) -> &mut RegionCounts {
        &mut self.statistics.counts
    }

    pub(super) fn learned_store(&self) -> Option<&Arc<LearnedStore>> {
        self.parts.learned_store()
    }

    /// The caller installs a store only before traversal and never replaces
    /// one.
    pub(super) fn set_learned_store(&mut self, store: Arc<LearnedStore>) {
        self.parts.set_learned_store(store);
    }

    pub(super) fn filter(&self) -> Option<&crate::region_filter::Filter> {
        self.filter.as_ref().map(RetainedFilter::borrow_owner)
    }

    /// The caller allows installation only before traversal and never replaces
    /// an installed factory, so no checker can outlive its source identity.
    pub(super) fn set_filter(&mut self, filter: crate::region_filter::Filter) {
        self.filter = Some(RetainedFilter::new(filter, |_| None));
    }

    /// Release the retained checker on completion, failure or explicit stop.
    /// Keep the factory and its receipts inspectable after the stream closes.
    pub(super) fn finish(&mut self) -> Result<(), Incomplete> {
        self.filter.as_mut().map_or(Ok(()), |filter| {
            filter.with_dependent_mut(|_, worker| {
                let worker = worker.take();
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(worker)))
                    .map_err(|_| Incomplete::WorkerPanicked)
            })
        })
    }

    pub(crate) fn statistics(&self) -> RegionSearchStatistics {
        let regions = self.traversal.statistics();
        RegionSearchStatistics {
            counts: RegionCounts {
                regions: regions.regions,
                refuted: regions.refuted,
                leaves: regions.leaves,
                ..self.statistics.counts
            },
            producers: self.statistics.producers,
            generator_order: self.statistics.generator_order,
            parity: self.statistics.parity,
            cardinality: self.statistics.cardinality,
            frontier: None,
            workers: None,
            frontier_order: None,
        }
        .with_runtime_arithmetic()
    }

    /// Narrow the regions still to visit by a candidate-only restriction;
    /// indexing it is charged to the budget, as the theory's was.
    pub(crate) fn restrict(
        &mut self,
        restriction: &Theory,
        budget: &mut Budget<'_>,
    ) -> Result<(), Incomplete> {
        self.restrictions
            .permanent
            .try_reserve(1)
            .map_err(|_| Incomplete::Allocation)?;
        let narrower = super::original_index::prepare(
            restriction,
            self.native_byte_limit,
            budget.cancellation,
        )?;
        budget.charge(narrower.work())?;
        self.statistics.counts.work += narrower.work();
        self.restrictions
            .permanent
            .push((restriction.clone(), narrower));
        Ok(())
    }

    /// Commit only after preparation and every charge succeeds.
    pub(crate) fn tighten(
        &mut self,
        restriction: &Theory,
        generation: u64,
        budget: &mut Budget<'_>,
    ) -> Result<(), Incomplete> {
        let bound = Bound::tightened(
            restriction,
            generation,
            self.native_byte_limit,
            budget,
            &mut self.statistics.counts.work,
        )?;
        self.restrictions.bound = Some(bound);
        Ok(())
    }

    pub(super) fn permits_determined(
        &mut self,
        candidate: &Interpretation,
        budget: &mut Budget<'_>,
        timings: &mut Option<crate::SearchPhaseTimings>,
    ) -> Result<bool, Incomplete> {
        permits(
            &self.restrictions,
            self.filter.as_ref().map(RetainedFilter::borrow_owner),
            candidate,
            &mut self.scratch,
            budget,
            &mut self.statistics.counts,
            timings,
        )
    }

    /// The next leaf, a classical model of the theory and the restrictions,
    /// or `None` once the tree is covered. The walk reads the original
    /// theory through `index`, the enumeration's one shared index.
    pub(crate) fn propose(
        &mut self,
        theory: &Theory,
        index: &IndexedTheory,
        budget: &mut Budget<'_>,
        timings: &mut Option<crate::SearchPhaseTimings>,
    ) -> Result<Option<Interpretation>, Incomplete> {
        match self.next_leaf(theory, index, budget, timings) {
            Ok(Some(candidate)) => Ok(Some(candidate)),
            Ok(None) => {
                // The walk's end is a tier boundary. A stop while delivering
                // ends the walk incomplete, as any read's does.
                let delivered = self.parts.deliver_tier(
                    &mut self.tier,
                    &mut self.scratch,
                    budget,
                    &mut self.statistics.counts,
                );
                if let Err(error) = delivered {
                    let _ = self.finish();
                    return Err(error);
                }
                self.finish()?;
                Ok(None)
            }
            Err(error) => {
                // Teardown cannot replace the failure that stopped this pull.
                let _ = self.finish();
                Err(error)
            }
        }
    }

    fn next_leaf(
        &mut self,
        theory: &Theory,
        index: &IndexedTheory,
        budget: &mut Budget<'_>,
        timings: &mut Option<crate::SearchPhaseTimings>,
    ) -> Result<Option<Interpretation>, Incomplete> {
        let Self {
            parts,
            traversal,
            restrictions,
            scratch,
            tier,
            statistics,
            filter,
            native_byte_limit: _,
        } = self;
        let (formulas, narrower) = index.subject(theory)?;
        parts.seed_tier(tier, scratch, budget, &mut statistics.counts)?;
        let before = traversal.statistics();
        let mut advance = |mut check: Option<crate::region_filter::Check<'_, '_>>| {
            let mut context = (&mut *budget, &mut statistics.counts);
            traversal.next_with_copy(
                &mut context,
                |(budget, counts), entry| -> Result<Narrowing, Incomplete> {
                    let narrowing = narrow(
                        parts.reading(formulas, narrower)?,
                        restrictions,
                        entry,
                        scratch,
                        budget,
                        counts,
                        check.as_mut(),
                    )?;
                    parts.visited(tier, scratch, budget, counts)?;
                    Ok(narrowing)
                },
                |(budget, counts), known| {
                    let attempt = copy_knowledge(known, None, budget);
                    counts.add(attempt.counts)?;
                    attempt.result
                },
            )
        };
        let visit = if let Some(filter) = filter {
            filter.with_dependent_mut(|filter, worker| {
                advance(Some(crate::region_filter::Check {
                    filter,
                    worker,
                    timings,
                }))
            })
        } else {
            advance(None)
        };
        let after = traversal.statistics();
        for _ in 0..after.splits_since(before) {
            budget.decide()?;
        }
        match visit? {
            None | Some(Visit::Counted(..)) => Ok(None),
            Some(Visit::Leaf(region, _)) => leaf_interpretation(theory, &region).map(Some),
        }
    }
}

impl Drop for RegionSearch {
    fn drop(&mut self) {
        // Explicit finish reports teardown failure; Drop has no result channel.
        let _ = self.finish();
    }
}

/// Narrow a region by the theory and every restriction until all are closed
/// under the same decisions, or one refutes it, each from what the region
/// already knows under it. Each narrowing runs to its own fixed point. Only a
/// later subject's new decisions can invalidate an earlier subject's closure;
/// when no later subject changes the region, the round has reached their joint
/// fixed point. Every charged read spends a budget permit reserved in batches
/// of at most `NARROWING_BATCH`,
/// unspent permits are refunded, and even a failed narrowing contributes its
/// admitted prefix to the counts. Streamed original constraints may decide one
/// open atom, returning to propagation before the next source pass. These passes
/// share one allowance. Every source decision decreases the number of open atoms;
/// no source clause is added to the theory or its frozen reduct.
///
/// Select the split once after the joint fixed point. Original atomic choice
/// heads rank first. The active bound, latest permanent restriction, or original
/// theory supplies the primary unknown-parent count, in that precedence. The
/// original count breaks restriction-count ties, followed by the lower atom.
/// Selection borrows existing knowledge, charging two count reads per open atom
/// when a restriction is active and one otherwise. A failed attempt installs no
/// partial preference.
pub(super) fn narrow<Q: Quota, R: std::borrow::Borrow<(Theory, Narrower)>>(
    reading: CandidateReading<'_>,
    restrictions: &Conditions<R>,
    entry: &mut Entry<CandidateKnowledge>,
    scratch: &mut NarrowingScratch,
    budget: &mut Budget<'_, Q>,
    counts: &mut RegionCounts,
    filter: Option<&mut crate::region_filter::Check<'_, '_>>,
) -> Result<Narrowing, Incomplete> {
    let original = reading.original;
    let narrower = reading.narrower;
    let (region, knowledge) = entry.parts_mut();
    let eager = || {
        std::iter::once(CandidateSubject::Original {
            reading: original,
            narrower,
        })
        .chain(
            restrictions
                .permanent
                .iter()
                .enumerate()
                .map(|(index, owner)| {
                    let (theory, narrower) = owner.borrow();
                    CandidateSubject::Restriction {
                        index: index + 1,
                        theory,
                        narrower,
                    }
                }),
        )
        .chain(restrictions.bound.iter().map(CandidateSubject::Bound))
    };
    let mut late = subjects::late(&reading, filter);
    let result = joint(eager, &mut late, region, knowledge, scratch, budget, counts)?;
    if result == Narrowing::Refuted {
        return Ok(result);
    }
    let preferred = prefer_split(original, restrictions, region, knowledge, budget, counts)?;
    if let Some(atom) = preferred {
        entry.prefer(atom);
    }
    Ok(result)
}

/// Rank open atoms only after all subjects agree on the current region. An
/// active restriction keeps the primary degree; the original breaks its ties.
/// Otherwise a prepared generator order takes precedence over the degree read.
fn prefer_split<Q: Quota, R>(
    original: OriginalSubject<'_>,
    conditions: &Conditions<R>,
    region: &Region,
    knowledge: &CandidateKnowledge,
    budget: &mut Budget<'_, Q>,
    counts: &mut RegionCounts,
) -> Result<Option<usize>, Incomplete> {
    let (original_known, primary) = knowledge.preference_subjects(conditions);
    let cancellation = budget.cancellation;
    let mut quota = BudgetQuota { budget };
    let mut meter = Meter::reserved(&mut quota);
    let attempt = if conditions.permanent.is_empty() && conditions.bound.is_none() {
        match original.generator_order() {
            Some(order) => {
                order.preferred_atom_reserved(original.theory(), region, cancellation, &mut meter)
            }
            None => primary.preferred_atom_reserved(
                region,
                original.choices(),
                cancellation,
                &mut meter,
            ),
        }
    } else {
        primary.preferred_atom_with_secondary_reserved(
            region,
            original.choices(),
            original_known,
            cancellation,
            &mut meter,
        )
    };
    counts.work += attempt.work;
    let preferred = attempt.result.map_err(meter_failure)?;
    Ok(preferred)
}

/// Check only accumulated candidate conditions on a certified singleton.
/// No original-theory propagation or search decision is required. Existing
/// restriction owners and their accounted narrowing primitive are reused.
pub(super) fn permits<R: std::borrow::Borrow<(Theory, Narrower)>>(
    restrictions: &Conditions<R>,
    filter: Option<&crate::region_filter::Filter>,
    candidate: &Interpretation,
    scratch: &mut NarrowingScratch,
    budget: &mut Budget<'_>,
    counts: &mut RegionCounts,
    timings: &mut Option<crate::SearchPhaseTimings>,
) -> Result<bool, Incomplete> {
    budget.cancellation.poll()?;
    if restrictions.is_empty() && filter.is_none() {
        return Ok(true);
    }
    let mut region = Region::all_open(candidate.theory().atom_count());
    for atom in 0..candidate.theory().atom_count() {
        budget.tick()?;
        if candidate.contains(atom) {
            region.hold(atom);
        } else {
            region.cut(atom);
        }
    }
    for (theory, narrower) in restrictions.iter() {
        let mut knowledge = narrower.knowledge();
        let attempt = BudgetQuota::narrow(budget, |cancellation, meter| {
            narrower.narrow_known_reserved(
                zetesis_ferraris::OriginalSubject::new(theory, None),
                &mut region,
                &mut knowledge,
                scratch,
                cancellation,
                meter,
            )
        });
        if SubjectAttempt::from_narrowing(&attempt).account(counts)? == SubjectStep::Refuted {
            return Ok(false);
        }
    }
    if let Some(filter) = filter {
        let mut worker = None;
        if filter.check(
            &mut worker,
            candidate.theory(),
            &region,
            budget.cancellation,
            timings,
        )? == crate::RegionFeasibility::Refuted
        {
            return Ok(false);
        }
    }
    budget.cancellation.poll()?;
    Ok(true)
}

/// The existing budget as the common meter's quota. Reservation polls control
/// once per batch; a refusal travels with its original typed payload.
struct BudgetQuota<'b, 'a, Q: Quota> {
    budget: &'b mut Budget<'a, Q>,
}

impl<'b, 'a, Q: Quota> BudgetQuota<'b, 'a, Q> {
    /// Run one narrowing, preserving its receipt and the origin of its failure.
    fn narrow(
        budget: &'b mut Budget<'a, Q>,
        run: impl FnOnce(&Cancellation, &mut Meter<'_, Self>) -> NarrowingAttempt<Failure<Incomplete>>,
    ) -> NarrowingAttempt<Incomplete> {
        #[cfg(test)]
        tests::record_narrowing();
        let cancellation = budget.cancellation;
        let mut quota = Self { budget };
        let mut meter = Meter::reserved(&mut quota);
        let attempt = run(cancellation, &mut meter);
        NarrowingAttempt {
            result: attempt.result.map_err(meter_failure),
            statistics: attempt.statistics,
            preferred: attempt.preferred,
        }
    }
}

impl<Q: Quota> zetesis_algebra::Quota for BudgetQuota<'_, '_, Q> {
    type Error = Incomplete;

    fn reserve(&mut self, wanted: u64) -> Result<u64, Self::Error> {
        self.budget.reserve_up_to(wanted)
    }

    fn refund(&mut self, unspent: u64) {
        self.budget.refund(unspent);
    }

    fn replenish(
        &mut self,
        unused: &mut u64,
        minimum: u64,
        capacity: u64,
    ) -> Result<(), Failure<Incomplete>> {
        self.budget.replenish(unused, minimum, capacity)
    }
}

/// Preserve a caller's refusal; local narrowing stops retain their existing
/// mapping. Independent verification uses its own `From<Stop>` boundary.
fn meter_failure(error: Failure<Incomplete>) -> Incomplete {
    match error {
        Failure::Refused(error) => error,
        Failure::Stopped(stop) => stopped(stop),
    }
}

pub(crate) fn limits<Q: Quota>(budget: &Budget<'_, Q>) -> RegionLimits {
    RegionLimits {
        max_work: budget.remaining_work(),
    }
}

pub(crate) fn stopped(stop: Stop) -> Incomplete {
    match stop {
        Stop::WorkLimit => Incomplete::WorkLimit,
        other => other.into(),
    }
}

/// The proper-subset query of a classical model as a region tree: the
/// coverage tree over the subsets of the candidate, narrowed by the
/// knowledge of the frozen reduct. A leaf other than the candidate is a
/// proper-subset model of the reduct and refutes stability; a covered tree
/// with no such leaf proves it (`ReductRegions.stable_iff_no_countermodel`).
/// The reduct is read as the original DAG under the candidate's truth mask
/// (`FerrarisMask`), so no clause form and no second theory is built; the
/// query borrows the index its owner lends, the one the candidate walk reads.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ReductQuery<'a> {
    index: &'a IndexedTheory,
}

impl<'a> ReductQuery<'a> {
    pub(crate) fn new(index: &'a IndexedTheory) -> Self {
        Self { index }
    }

    /// Search the proper subsets of the candidate, a classical model whose
    /// node truth has already been evaluated, for a model of its frozen reduct.
    /// The authenticated evaluation owns the exact (theory, interpretation)
    /// relation; callers supply neither a reading nor root knowledge.
    ///
    /// # Errors
    /// Work, decision and control stops end the query without a verdict.
    pub(crate) fn check<Q: Quota>(
        self,
        evaluated: &zetesis_ferraris::FormulaEvaluation<'_>,
        limits: crate::Limits,
        budget: &mut Budget<'_, Q>,
        statistics: &mut crate::Statistics,
        scratch: &mut NarrowingScratch,
    ) -> Result<crate::Check, Incomplete> {
        match self.check_publishing(
            evaluated,
            limits,
            budget,
            (statistics, scratch),
            &mut Inline,
        )? {
            Queried::Decided(check) => Ok(check),
            Queried::Published(never) => match never {},
        }
    }

    /// Search just the supplied region, with fresh knowledge of this candidate's
    /// frozen reduct. A witness is validated independently. An exhausted region
    /// establishes absence only inside its own bounds.
    pub(super) fn check_region<Q: Quota>(
        self,
        evaluated: &zetesis_ferraris::FormulaEvaluation<'_>,
        root: Region,
        limits: crate::Limits,
        budget: &mut Budget<'_, Q>,
        workspace: (&mut crate::Statistics, &mut NarrowingScratch),
        observation: Option<(&mut depth::QueryDepth, Option<&mut depth::DepthWorkspace>)>,
    ) -> Result<Option<Interpretation>, Incomplete> {
        let result = if let Some((counts, levels)) = observation {
            self.walk_from(
                evaluated,
                limits,
                budget,
                workspace,
                (&mut Inline, depth::Recording::new(counts, levels)),
                Some(root),
            )
        } else {
            self.walk_from(
                evaluated,
                limits,
                budget,
                workspace,
                (&mut Inline, ()),
                Some(root),
            )
        };
        match result? {
            Queried::Decided(witness) => Ok(witness),
            Queried::Published(never) => match never {},
        }
    }

    /// [`Self::check`], asking `publication` before each narrowing whether to
    /// publish the query's pending regions for other workers to serve. A
    /// publication takes every pending region, the one about to be narrowed
    /// included, and ends the query here undecided; a declined one is not
    /// offered again, and the query continues inline to its verdict.
    ///
    /// # Errors
    /// Work, decision and control stops end the query without a verdict.
    pub(crate) fn check_publishing<Q: Quota, P: Publication>(
        self,
        evaluated: &zetesis_ferraris::FormulaEvaluation<'_>,
        limits: crate::Limits,
        budget: &mut Budget<'_, Q>,
        (statistics, scratch): (&mut crate::Statistics, &mut NarrowingScratch),
        publication: &mut P,
    ) -> Result<Queried<P::Receipt>, Incomplete> {
        Ok(
            match self.walk_from(
                evaluated,
                limits,
                budget,
                (statistics, scratch),
                (publication, ()),
                None,
            )? {
                Queried::Decided(witness) => {
                    Queried::Decided(witness.map_or(crate::Check::Stable, crate::Check::NonMinimal))
                }
                Queried::Published(receipt) => Queried::Published(receipt),
            },
        )
    }

    fn walk_from<Q: Quota, P: Publication, O: depth::Trace>(
        self,
        evaluated: &zetesis_ferraris::FormulaEvaluation<'_>,
        limits: crate::Limits,
        budget: &mut Budget<'_, Q>,
        (statistics, scratch): (&mut crate::Statistics, &mut NarrowingScratch),
        (publication, mut observation): (&mut P, O),
        root: Option<Region>,
    ) -> Result<Queried<P::Receipt, Option<Interpretation>>, Incomplete> {
        let (theory, narrower) = self.index.subject(evaluated.theory())?;
        let candidate = evaluated.interpretation();
        let truth = evaluated.truth();
        if !theory.same_instance(candidate.theory()) {
            return Err(Incomplete::WrongTheory);
        }
        let root = root.unwrap_or_else(|| {
            let mut full = Region::all_open(theory.atom_count());
            for atom in (0..theory.atom_count()).filter(|&atom| !candidate.contains(atom)) {
                full.cut(atom);
            }
            full
        });
        let mut traversal = Walk::with_state(
            root,
            Counting::Never,
            CountermodelKnowledge::fresh(narrower),
        );
        let mut offered = false;
        loop {
            let before = traversal.statistics();
            let visit = traversal.next(|entry| {
                if !offered && publication.due() {
                    // Before the narrowing: the region stays pending.
                    return Err(Interrupt::Publish);
                }
                observation.enter();
                let result = narrow_inner(
                    narrower,
                    FrozenSubject::new(theory, truth),
                    None,
                    entry,
                    scratch,
                    budget,
                    &mut statistics.reduct.regions,
                )
                .map_err(Interrupt::Stopped)?;
                observation.narrowed(result);
                Ok(result)
            });
            let after = traversal.statistics();
            observation.returned(after, visit.is_err());
            let counts = &mut statistics.reduct.regions;
            counts.regions += after.regions - before.regions;
            counts.refuted += after.refuted - before.refuted;
            counts.leaves += after.leaves - before.leaves;
            for _ in 0..after.splits_since(before) {
                budget.decide()?;
            }
            match visit {
                Err(Interrupt::Stopped(error)) => return Err(error),
                Err(Interrupt::Publish) => {
                    offered = true;
                    if let Some(receipt) = publication.publish(truth, candidate, &mut traversal) {
                        return Ok(Queried::Published(receipt));
                    }
                }
                Ok(None | Some(Visit::Counted(..))) => {
                    return Ok(Queried::Decided(None));
                }
                Ok(Some(Visit::Leaf(region, _))) => {
                    // The candidate models its own reduct and is no
                    // countermodel; every other leaf is a proper subset.
                    if candidate.atoms().all(|atom| region.is_held(atom)) {
                        continue;
                    }
                    let subset = leaf_interpretation(theory, &region)?;
                    return crate::ferraris::validated_countermodel(
                        theory, candidate, subset, limits, budget, statistics,
                    )
                    .map(|witness| Queried::Decided(Some(witness)));
                }
            }
        }
    }

    /// Narrow one region of a published query of `candidate`, a classical
    /// model whose node truth is `truth`, in place, as the inline query
    /// narrows its regions: from the knowledge the region carries, its
    /// parent's under the same narrower and frozen mask. The region is counted
    /// as the inline query counts it, a split charges one decision, and a leaf
    /// other than the candidate is validated before it is returned.
    ///
    /// # Errors
    /// A stopped narrowing or decision, an invalid leaf, or a countermodel's
    /// validation refused or stopped. The region is then undecided: it may
    /// hold a countermodel.
    pub(crate) fn serve<Q: Quota>(
        self,
        truth: &[bool],
        candidate: &Interpretation,
        entry: &mut Entry<CountermodelKnowledge>,
        limits: crate::Limits,
        budget: &mut Budget<'_, Q>,
        (statistics, scratch): (&mut crate::Statistics, &mut NarrowingScratch),
    ) -> Result<Served, Incomplete> {
        let (theory, narrower) = self.index.subject(candidate.theory())?;
        let narrowing = narrow_inner(
            narrower,
            FrozenSubject::new(theory, truth),
            None,
            entry,
            scratch,
            budget,
            &mut statistics.reduct.regions,
        )?;
        let counts = &mut statistics.reduct.regions;
        counts.regions += 1;
        let atom = match walk::inspect(entry, narrowing, Counting::Never, false) {
            walk::Step::Refuted => {
                counts.refuted += 1;
                return Ok(Served::Empty);
            }
            walk::Step::Leaf => {
                let region = &entry.region;
                counts.leaves += 1;
                if candidate.atoms().all(|atom| region.is_held(atom)) {
                    return Ok(Served::Empty);
                }
                let subset = leaf_interpretation(theory, region)?;
                return crate::ferraris::validated_countermodel(
                    theory, candidate, subset, limits, budget, statistics,
                )
                .map(Served::Countermodel);
            }
            walk::Step::Split(atom) => atom,
            walk::Step::Counted => unreachable!("Counting::Never cannot count a region"),
        };
        budget.decide()?;
        Ok(Served::Split(atom))
    }
}

/// Where an inline proper-subset query may hand its pending regions to other
/// workers: asked before each narrowing whether to publish, and then offered
/// the traversal.
pub(crate) trait Publication {
    /// What a published query returns to the check's caller.
    type Receipt;

    /// Whether to publish before the query's next narrowing.
    fn due(&mut self) -> bool;

    /// Take every pending region of `traversal`, the query of `candidate`
    /// under node truth `truth`, and return the receipt; or decline, leaving
    /// the traversal as it is.
    fn publish(
        &mut self,
        truth: &[bool],
        candidate: &Interpretation,
        traversal: &mut Walk<CountermodelKnowledge>,
    ) -> Option<Self::Receipt>;
}

/// The query runs to its verdict where it started.
pub(crate) struct Inline;

impl Publication for Inline {
    type Receipt = std::convert::Infallible;

    fn due(&mut self) -> bool {
        false
    }

    fn publish(
        &mut self,
        _: &[bool],
        _: &Interpretation,
        _: &mut Walk<CountermodelKnowledge>,
    ) -> Option<Self::Receipt> {
        None
    }
}

/// What a membership check whose query may be published ends in.
pub(crate) enum Queried<R, D = crate::Check> {
    /// The query reached its conclusion over the region it searched.
    Decided(D),
    /// The query's pending regions were published, and other workers decide it.
    Published(R),
}

/// What one served region of a published query came to.
pub(crate) enum Served {
    /// Refuted, or the candidate's own leaf: no countermodel lies in it.
    Empty,
    /// A validated countermodel: the candidate is not stable.
    Countermodel(Interpretation),
    /// To be split on this open atom, its decision charged.
    Split(usize),
}

/// Why the query's traversal stopped before a narrowing.
enum Interrupt {
    Stopped(Incomplete),
    Publish,
}

impl From<Stop> for Interrupt {
    fn from(stop: Stop) -> Self {
        Self::Stopped(stop.into())
    }
}

/// Narrow one region of a proper-subset query: the frozen reduct's reading
/// eager, and the candidate's prepared inner systems late, to their joint
/// fixed point. The frozen reading's split preference is installed in the
/// entry once the closure is accounted.
fn narrow_inner<Q: Quota>(
    narrower: &Narrower,
    subject: FrozenSubject<'_>,
    preparation: Option<&FrozenPreparation>,
    entry: &mut Entry<CountermodelKnowledge>,
    scratch: &mut NarrowingScratch,
    budget: &mut Budget<'_, Q>,
    counts: &mut RegionCounts,
) -> Result<Narrowing, Incomplete> {
    let (region, knowledge) = entry.parts_mut();
    let preferred = Cell::new(None);
    let eager = || {
        std::iter::once(CountermodelSubject::Frozen {
            narrower,
            reading: subject,
            preferred: &preferred,
        })
    };
    let mut late = subjects::inner_late(preparation);
    let result = joint(eager, &mut late, region, knowledge, scratch, budget, counts)?;
    if let Some(atom) = preferred.get() {
        entry.prefer(atom);
    }
    Ok(result)
}

#[cfg(test)]
mod tests;
