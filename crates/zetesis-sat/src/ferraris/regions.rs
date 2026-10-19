//! Candidates proposed by regions: the coverage tree over the theory's
//! atoms, narrowed by the theory's readings.
//!
//! The root region leaves every atom open. Each region is narrowed to the
//! fixed point of `zetesis_ferraris::Narrower::narrow_known` over the
//! original theory, from its parent's knowledge, with
//! its producers for the support cut, and over every candidate-only
//! restriction, without producers, since a restriction is not a rule of the
//! program and supports nothing. A region no reading refutes is split on
//! the atom its narrowing prefers, held branch first; a region with every atom
//! decided is a leaf, and a leaf is a classical model of the theory and the
//! restrictions, because at a full decision every root is sure or
//! never and a root never refutes. The leaf is the proposal; the
//! reduct decides it as it decides a proposal from the clauses.
//!
//! The narrowing's node reads are charged as search work and each split as
//! a decision. A restriction narrows the regions still to visit; visited
//! regions were covered under the original theory, and a restriction only
//! removes candidates, so no restart and no exclusion index is needed
//! (`Search.CoverageTree`, `FormulaBounds`).

use zetesis_cpu::Stop;
use zetesis_cpu::regions::{Counting, Narrowing, Region, Traversal, Visit};
use zetesis_ferraris::{
    ChoiceHeads, FrozenSubject, GeneratorOrder, GeneratorOrderError, Interpretation, Knowledge,
    Narrower, NarrowingAttempt, NarrowingQuota, NarrowingScratch, OriginalSubject, Producers,
    RegionLimits, SupportExplanationStatistics, Theory,
};

use super::conditions::{Bound, CandidateKnowledge, Conditions};
use super::original_index::IndexedTheory;
use crate::search::{Budget, Quota};
use crate::{Cancellation, Incomplete};

/// How classical candidates are proposed to the reduct.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SearchMethod {
    /// Regions of the candidate space narrowed by the theory's readings;
    /// every leaf is a classical model and no clause form is built. The
    /// default.
    #[default]
    Regions,
    /// A chronological search over a clause form of the theory, with exact
    /// exclusion of every candidate already proposed.
    Clauses,
}

impl SearchMethod {
    /// Stable spelling for configuration and execution reports.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Regions => "regions",
            Self::Clauses => "clauses",
        }
    }
}

/// What a walk over a region tree counted, cumulatively: the candidate
/// tree's walk, or the proper-subset queries' walks together.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RegionCounts {
    /// Regions narrowed, the root included.
    pub regions: usize,
    /// Regions refuted by readings or an original-candidate filter. Frozen
    /// proper-subset queries use only their reduct readings.
    pub refuted: usize,
    /// Regions with every atom decided: the classical candidates proposed,
    /// or the proper-subset models and the candidate itself.
    pub leaves: usize,
    /// Propagation events: nodes and atoms learned and their neighbours
    /// revisited, support rechecks, and fresh source-consequence mask updates.
    pub propagations: u64,
    /// Atoms the readings or original source consequences held.
    pub held: u64,
    /// Atoms the readings or original source consequences cut.
    pub cut: u64,
    /// Node reads, root tests and producer checks, and for the candidate
    /// tree the producer extraction, optional generator-order preparation,
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
        };
        *self = sum;
        Ok(())
    }
}

/// Owned storage of queued, inactive candidate regions at frontier mutations.
///
/// Bytes include the frontier vector header, every allocated entry slot and
/// each queued region's and knowledge's owned vector capacities. They exclude
/// active regions, temporary split copies, shared theory and indexes, candidate
/// batches, measurement bookkeeping, thread stacks and allocator/driver storage.
/// This observation is neither total search memory nor a byte admission limit.
/// Peaks need not occur at the same mutation; an empty frontier can retain slots.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RegionFrontierStatistics {
    /// Currently queued inactive regions.
    pub regions: usize,
    /// Currently allocated region entry slots, including unused slots.
    pub capacity: usize,
    /// Current logical retained bytes in the stated ownership scope.
    pub retained_bytes: u128,
    /// Maximum number of simultaneously queued regions.
    pub peak_regions: usize,
    /// Maximum allocated entry capacity.
    pub peak_capacity: usize,
    /// Maximum logical retained bytes at a frontier mutation.
    pub peak_retained_bytes: u128,
}

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
    /// Frontier ownership measured by parallel candidate production. Other
    /// schedules do not currently measure it; absence does not mean zero bytes.
    pub frontier: Option<RegionFrontierStatistics>,
    /// The parallel region walk's workers. The scalar walk starts no worker,
    /// and the device route's pool starts at construction, so both report
    /// `None`.
    pub workers: Option<RegionWorkerStatistics>,
}

/// Applicability of the optional original generator preference.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GeneratorOrderStatus {
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
    traversal: Traversal<CandidateKnowledge>,
    /// Each restriction with its own index.
    restrictions: Conditions<(Theory, Narrower)>,
    /// The worklists every narrowing of this walk reuses.
    scratch: NarrowingScratch,
    statistics: RegionSearchStatistics,
    filter: Option<RetainedFilter>,
    native_byte_limit: u64,
}

/// What a region walk reads of the original theory beyond the theory itself:
/// its producers, when it lies in the producer fragment, and the heads of its
/// atomic choice roots, which every split ranks first.
#[derive(Debug)]
pub(crate) struct OriginalParts {
    producers: Option<Producers>,
    choice_heads: ChoiceHeads,
    generator_order: Option<GeneratorOrder>,
}

impl OriginalParts {
    /// The walk's subject over `theory`, the theory these parts came from.
    pub(crate) fn subject<'a>(&'a self, theory: &'a Theory) -> OriginalSubject<'a> {
        OriginalSubject::new(theory, self.producers.as_ref())
            .with_choices(&self.choice_heads)
            .with_generator_order(self.generator_order.as_ref())
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
}

/// Extract original producers, then prepare the optional shared generator
/// order under the caller's independent preparation-byte allowance. Work uses
/// the same streaming quota as narrowing, including every refused prefix.
pub(crate) fn open(
    theory: &Theory,
    order_byte_limit: u64,
    budget: &mut Budget<'_>,
) -> Result<Opened, Incomplete> {
    let extraction = zetesis_ferraris::producers(theory, limits(budget), budget.cancellation)
        .map_err(stopped)?;
    budget.charge(extraction.work)?;
    let cancellation = budget.cancellation;
    let mut quota = BudgetQuota {
        budget,
        failure: None,
    };
    let attempt = GeneratorOrder::prepare_reserved(
        OriginalSubject::new(theory, extraction.producers.as_ref()),
        usize::try_from(order_byte_limit).unwrap_or(usize::MAX),
        cancellation,
        &mut quota,
    );
    let (generator_order, status) = match attempt.result {
        Ok(order) => {
            let status = if order.is_some() {
                GeneratorOrderStatus::Prepared
            } else {
                GeneratorOrderStatus::NotApplicable
            };
            (order, status)
        }
        Err(GeneratorOrderError::Storage { .. }) => (None, GeneratorOrderStatus::StorageDeclined),
        Err(GeneratorOrderError::Stopped(stop)) => {
            return Err(quota.failure.take().unwrap_or_else(|| stopped(stop)));
        }
    };
    let generator_statistics = GeneratorOrderStatistics {
        status,
        work: attempt.work,
        peak_bytes: attempt.peak_bytes,
        retained_bytes: generator_order
            .as_ref()
            .map_or(0, GeneratorOrder::retained_bytes),
    };
    Ok(Opened {
        statistics: RegionSearchStatistics {
            counts: RegionCounts {
                work: extraction
                    .work
                    .checked_add(attempt.work)
                    .ok_or(Incomplete::CounterOverflow)?,
                ..Default::default()
            },
            producers: extraction.producers.is_some(),
            generator_order: generator_statistics,
            frontier: None,
            workers: None,
        },
        parts: OriginalParts {
            producers: extraction.producers,
            choice_heads: extraction.choice_heads,
            generator_order,
        },
    })
}

/// The interpretation a leaf proposes: the atoms the region holds.
pub(crate) fn leaf_interpretation(
    theory: &Theory,
    region: &Region,
) -> Result<Interpretation, Incomplete> {
    let mut selected = crate::search::storage(theory.atom_count())?;
    selected.extend(region.held());
    Interpretation::new(theory, selected).map_err(|error| match error {
        zetesis_ferraris::AdmissionError::Allocation => Incomplete::Allocation,
        _ => Incomplete::InvalidWitness,
    })
}

impl RegionSearch {
    /// Extract the producers and queue the root region with no knowledge;
    /// its first narrowing creates each knowledge slot.
    pub(crate) fn new(
        theory: &Theory,
        native_byte_limit: u64,
        order_byte_limit: u64,
        budget: &mut Budget<'_>,
    ) -> Result<Self, Incomplete> {
        let Opened { parts, statistics } = open(theory, order_byte_limit, budget)?;
        Ok(Self {
            statistics,
            parts,
            traversal: Traversal::with_state(
                Region::all_open(theory.atom_count()),
                Counting::Never,
                CandidateKnowledge::default(),
            )
            .with_held_first(),
            restrictions: Conditions::default(),
            scratch: NarrowingScratch::default(),
            filter: None,
            native_byte_limit,
        })
    }

    pub(crate) fn counts_mut(&mut self) -> &mut RegionCounts {
        &mut self.statistics.counts
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
            frontier: None,
            workers: None,
        }
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
            statistics,
            filter,
            native_byte_limit: _,
        } = self;
        let (formulas, narrower) = index.subject(theory)?;
        let before = traversal.statistics();
        let mut advance = |mut check: Option<crate::region_filter::Check<'_, '_>>| {
            traversal.next(|region, knowledge| -> Result<Narrowing, Incomplete> {
                narrow(
                    (parts.subject(formulas), narrower),
                    restrictions,
                    (region, knowledge),
                    scratch,
                    budget,
                    &mut statistics.counts,
                    check.as_mut(),
                )
            })
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
    (original, narrower): (OriginalSubject<'_>, &Narrower),
    restrictions: &Conditions<R>,
    (region, knowledge): (&mut Region, &mut CandidateKnowledge),
    scratch: &mut NarrowingScratch,
    budget: &mut Budget<'_, Q>,
    counts: &mut RegionCounts,
    mut filter: Option<&mut crate::region_filter::Check<'_, '_>>,
) -> Result<Narrowing, Incomplete> {
    let mut changed = false;
    let mut pass = crate::RegionPass::First;
    loop {
        let mut revisit = false;
        // A restriction is not a rule of the original theory: it is read
        // without producers, and only the original's choice heads rank splits.
        for (index, (subject, narrower)) in std::iter::once((original, narrower))
            .chain(restrictions.permanent.iter().map(|restriction| {
                let (theory, narrower) = restriction.borrow();
                (OriginalSubject::new(theory, None), narrower)
            }))
            .enumerate()
        {
            let known = knowledge.permanent(index, narrower)?;
            let attempt = BudgetQuota::narrow(budget, |cancellation, quota| {
                narrower.propagate_known_reserved(
                    subject,
                    region,
                    known,
                    scratch,
                    cancellation,
                    quota,
                )
            });
            match account(&attempt, counts)? {
                Narrowing::Refuted => return Ok(Narrowing::Refuted),
                Narrowing::Fixed { changed: moved } => {
                    changed |= moved;
                    revisit |= index != 0 && moved;
                }
            }
        }
        if let Some(bound) = &restrictions.bound {
            let (formulas, narrower) = bound.index.as_ref();
            let known = knowledge.bound(bound)?;
            let attempt = BudgetQuota::narrow(budget, |cancellation, quota| {
                narrower.propagate_known_reserved(
                    zetesis_ferraris::OriginalSubject::new(formulas, None),
                    region,
                    known,
                    scratch,
                    cancellation,
                    quota,
                )
            });
            match account(&attempt, counts)? {
                Narrowing::Refuted => return Ok(Narrowing::Refuted),
                Narrowing::Fixed { changed: moved } => {
                    changed |= moved;
                    revisit |= moved;
                }
            }
        }
        if !revisit {
            if let Some(filter) = &mut filter {
                let consequence =
                    filter.consequence(original.theory(), region, budget.cancellation, pass)?;
                match consequence {
                    crate::RegionConsequence::Refuted => return Ok(Narrowing::Refuted),
                    crate::RegionConsequence::Unchanged => {}
                    crate::RegionConsequence::Hold(_) | crate::RegionConsequence::Cut(_) => {
                        apply_consequence(consequence, region, budget, counts)?;
                        changed = true;
                        pass = crate::RegionPass::Continue;
                        continue;
                    }
                }
            }
            prefer_split(original, restrictions, region, knowledge, budget, counts)?;
            return Ok(Narrowing::Fixed { changed });
        }
    }
}

/// Rank open atoms only after all subjects agree on the current region. An
/// active restriction keeps the primary degree; the original breaks its ties.
/// Otherwise a prepared generator order takes precedence over the degree read.
fn prefer_split<Q: Quota, R>(
    original: OriginalSubject<'_>,
    conditions: &Conditions<R>,
    region: &mut Region,
    knowledge: &CandidateKnowledge,
    budget: &mut Budget<'_, Q>,
    counts: &mut RegionCounts,
) -> Result<(), Incomplete> {
    let (original_known, primary) = knowledge.preference_subjects(conditions);
    let cancellation = budget.cancellation;
    let mut quota = BudgetQuota {
        budget,
        failure: None,
    };
    let attempt = if conditions.permanent.is_empty() && conditions.bound.is_none() {
        match original.generator_order() {
            Some(order) => {
                order.preferred_atom_reserved(original.theory(), region, cancellation, &mut quota)
            }
            None => primary.preferred_atom_reserved(
                region,
                original.choices(),
                cancellation,
                &mut quota,
            ),
        }
    } else {
        primary.preferred_atom_with_secondary_reserved(
            region,
            original.choices(),
            original_known,
            cancellation,
            &mut quota,
        )
    };
    counts.work += attempt.work;
    let preferred = attempt
        .result
        .map_err(|stop| quota.failure.take().unwrap_or_else(|| stopped(stop)))?;
    if let Some(atom) = preferred {
        region.prefer(atom);
    }
    Ok(())
}

/// Charge one fresh mask update before publishing it. Its validity was checked
/// against the immutable region by the callback boundary.
fn apply_consequence<Q: Quota>(
    consequence: crate::RegionConsequence,
    region: &mut Region,
    budget: &mut Budget<'_, Q>,
    counts: &mut RegionCounts,
) -> Result<(), Incomplete> {
    let (atom, held) = match consequence {
        crate::RegionConsequence::Hold(atom) => (atom, true),
        crate::RegionConsequence::Cut(atom) => (atom, false),
        _ => return Err(Incomplete::InvalidRegionConsequence),
    };
    let mut updated = *counts;
    updated.add(RegionCounts {
        held: u64::from(held),
        cut: u64::from(!held),
        work: 1,
        propagations: 1,
        ..RegionCounts::default()
    })?;
    budget.cancellation.poll()?;
    budget.tick()?;
    let applied = if held {
        region.hold(atom)
    } else {
        region.cut(atom)
    };
    if !applied {
        return Err(Incomplete::InvalidRegionConsequence);
    }
    *counts = updated;
    Ok(())
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
        let attempt = BudgetQuota::narrow(budget, |cancellation, quota| {
            narrower.narrow_known_reserved(
                zetesis_ferraris::OriginalSubject::new(theory, None),
                &mut region,
                &mut knowledge,
                scratch,
                cancellation,
                quota,
            )
        });
        if account(&attempt, counts)? == Narrowing::Refuted {
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

/// The budget as a narrowing's quota: permits granted in batches through
/// [`Budget::reserve_up_to`], control polled once per batch, and the
/// budget's own refusal kept to be returned unchanged.
struct BudgetQuota<'b, 'a, Q: Quota> {
    budget: &'b mut Budget<'a, Q>,
    failure: Option<Incomplete>,
}

impl<'b, 'a, Q: Quota> BudgetQuota<'b, 'a, Q> {
    /// Run one narrowing on the budget, returning its receipt with the
    /// budget's refusal, or the narrowing's own stop, as the failure.
    fn narrow(
        budget: &'b mut Budget<'a, Q>,
        run: impl FnOnce(&Cancellation, &mut dyn NarrowingQuota) -> NarrowingAttempt,
    ) -> NarrowingAttempt<Incomplete> {
        #[cfg(test)]
        tests::record_narrowing();
        let cancellation = budget.cancellation;
        let mut quota = Self {
            budget,
            failure: None,
        };
        let attempt = run(cancellation, &mut quota);
        NarrowingAttempt {
            result: attempt
                .result
                .map_err(|stop| quota.failure.take().unwrap_or_else(|| stopped(stop))),
            statistics: attempt.statistics,
        }
    }
}

impl<Q: Quota> NarrowingQuota for BudgetQuota<'_, '_, Q> {
    fn reserve(&mut self, wanted: u64) -> Result<u64, Stop> {
        self.budget.reserve_up_to(wanted).map_err(|failure| {
            self.failure = Some(failure);
            Stop::WorkLimit
        })
    }

    fn refund(&mut self, unspent: u64) {
        self.budget.refund(unspent);
    }
}

/// Publish every admitted narrowing prefix before returning its result. Work
/// has passed through the local or shared budget, batch by batch, before
/// each read.
fn account(
    attempt: &NarrowingAttempt<Incomplete>,
    counts: &mut RegionCounts,
) -> Result<Narrowing, Incomplete> {
    let charges = &attempt.statistics;
    counts.add(RegionCounts {
        propagations: charges.propagations,
        held: charges.held,
        cut: charges.cut,
        work: charges.work,
        support_work: charges.support_work,
        founded_work: charges.founded_work,
        support_explanations: charges.support_explanations,
        ..RegionCounts::default()
    })?;
    attempt.result
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
    /// node truth is `truth`, for a model of its frozen reduct.
    ///
    /// # Errors
    /// Work, decision and control stops end the query without a verdict.
    pub(crate) fn check<Q: Quota>(
        self,
        subject: FrozenSubject<'_>,
        candidate: &Interpretation,
        limits: crate::Limits,
        budget: &mut Budget<'_, Q>,
        statistics: &mut crate::Statistics,
        scratch: &mut NarrowingScratch,
    ) -> Result<crate::Check, Incomplete> {
        let truth = subject.truth();
        let (theory, narrower) = self.index.subject(subject.theory())?;
        if !theory.same_instance(candidate.theory()) {
            return Err(Incomplete::WrongTheory);
        }
        let mut root = Region::all_open(theory.atom_count());
        for atom in (0..theory.atom_count()).filter(|&atom| !candidate.contains(atom)) {
            root.cut(atom);
        }
        let mut traversal = Traversal::with_state(root, Counting::Never, narrower.knowledge());
        loop {
            let before = traversal.statistics();
            let visit = traversal.next(|region, knowledge| {
                narrow_frozen(
                    narrower,
                    FrozenSubject::new(theory, truth),
                    region,
                    knowledge,
                    scratch,
                    budget,
                    &mut statistics.reduct.regions,
                )
            });
            let after = traversal.statistics();
            let counts = &mut statistics.reduct.regions;
            counts.regions += after.regions - before.regions;
            counts.refuted += after.refuted - before.refuted;
            counts.leaves += after.leaves - before.leaves;
            for _ in 0..after.splits_since(before) {
                budget.decide()?;
            }
            match visit? {
                None | Some(Visit::Counted(..)) => return Ok(crate::Check::Stable),
                Some(Visit::Leaf(region, _)) => {
                    // The candidate models its own reduct and is no
                    // countermodel; every other leaf is a proper subset.
                    if candidate.atoms().all(|atom| region.is_held(atom)) {
                        continue;
                    }
                    let subset = leaf_interpretation(theory, &region)?;
                    return crate::ferraris::checked_countermodel(
                        theory, candidate, subset, limits, budget, statistics,
                    );
                }
            }
        }
    }
}

/// Narrow one region of a proper-subset query by the frozen reduct.
fn narrow_frozen<Q: Quota>(
    narrower: &Narrower,
    subject: FrozenSubject<'_>,
    region: &mut Region,
    knowledge: &mut Knowledge,
    scratch: &mut NarrowingScratch,
    budget: &mut Budget<'_, Q>,
    counts: &mut RegionCounts,
) -> Result<Narrowing, Incomplete> {
    let attempt = BudgetQuota::narrow(budget, |cancellation, quota| {
        narrower.narrow_frozen_known_reserved(
            subject,
            region,
            knowledge,
            scratch,
            cancellation,
            quota,
        )
    });
    account(&attempt, counts)
}

#[cfg(test)]
mod tests;
