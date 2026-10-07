//! Candidates proposed by regions: the coverage tree over the theory's
//! atoms, narrowed by the theory's readings.
//!
//! The root region leaves every atom open. Each region is narrowed to the
//! fixed point of `zetesis_ferraris::Narrower::narrow_known` over the
//! original theory, from its parent's knowledge, with
//! its producers for the support cut, and over every candidate-only
//! restriction, without producers, since a restriction is not a rule of the
//! program and supports nothing. A region no reading refutes is split on
//! the atom its narrowing prefers, cut branch first; a region with every atom
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
    FrozenSubject, Interpretation, Knowledge, Narrower, NarrowingAttempt, NarrowingQuota,
    NarrowingScratch, Producers, RegionLimits, Theory,
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
    /// revisited, and support rechecks.
    pub propagations: u64,
    /// Atoms the readings held.
    pub held: u64,
    /// Atoms the readings cut.
    pub cut: u64,
    /// Node reads, root tests and producer checks, and for the candidate
    /// tree the producer extraction, the indexing of each restriction and,
    /// once a region walk first needs it, the indexing of the theory, one
    /// unit per node and operand occurrence (kept even if building the index
    /// then fails); included in search work. A run decided by a positive
    /// certificate walks no
    /// region and indexes no theory. Enumeration queries share the walk's
    /// original index. A standalone membership query includes its own index
    /// construction in its reduct counts.
    pub work: u64,
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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RegionSearchStatistics {
    /// The walk's counts.
    pub counts: RegionCounts,
    /// Whether the theory lies in the producer fragment, so the support
    /// cut applies.
    pub producers: bool,
    /// Frontier ownership measured by parallel candidate production. Other
    /// schedules do not currently measure it; absence does not mean zero bytes.
    pub frontier: Option<RegionFrontierStatistics>,
}

#[derive(Debug)]
pub(crate) struct RegionSearch {
    producers: Option<Producers>,
    /// Each region carries what is known about it under the theory and
    /// under each restriction, in order; a restriction added after a region
    /// was reached gets fresh knowledge when the region is next narrowed.
    traversal: Traversal<CandidateKnowledge>,
    /// Each restriction with its own index.
    restrictions: Conditions<(Theory, Narrower)>,
    /// The worklists every narrowing of this walk reuses.
    scratch: NarrowingScratch,
    statistics: RegionSearchStatistics,
    pub(super) filter: Option<crate::region_filter::Filter>,
}

/// What opening a region search over a theory establishes: its producers,
/// when it lies in the producer fragment, and the statistics of the
/// extraction, charged to the budget. The original theory's index is not
/// built here: the enumeration's `OriginalIndex` builds it when a walk first
/// needs it, and its work is then recorded in these counts.
pub(crate) struct Opened {
    pub(crate) producers: Option<Producers>,
    pub(crate) statistics: RegionSearchStatistics,
}

/// Open a region search over the theory: extract its producers, charging
/// the extraction.
pub(crate) fn open(theory: &Theory, budget: &mut Budget<'_>) -> Result<Opened, Incomplete> {
    let extraction = zetesis_ferraris::producers(theory, limits(budget), budget.cancellation)
        .map_err(stopped)?;
    budget.charge(extraction.work)?;
    Ok(Opened {
        statistics: RegionSearchStatistics {
            counts: RegionCounts {
                work: extraction.work,
                ..Default::default()
            },
            producers: extraction.producers.is_some(),
            frontier: None,
        },
        producers: extraction.producers,
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
    pub(crate) fn new(theory: &Theory, budget: &mut Budget<'_>) -> Result<Self, Incomplete> {
        let Opened {
            producers,
            statistics,
        } = open(theory, budget)?;
        Ok(Self {
            statistics,
            producers,
            traversal: Traversal::with_state(
                Region::all_open(theory.atom_count()),
                Counting::Never,
                CandidateKnowledge::default(),
            ),
            restrictions: Conditions::default(),
            scratch: NarrowingScratch::default(),
            filter: None,
        })
    }

    pub(crate) fn counts_mut(&mut self) -> &mut RegionCounts {
        &mut self.statistics.counts
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
            frontier: None,
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
        let narrower = Narrower::try_new(restriction).map_err(stopped)?;
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
        budget.cancellation.poll()?;
        let bound = Bound::prepare(restriction, generation)?;
        budget.charge(bound.work())?;
        let work = self
            .statistics
            .counts
            .work
            .checked_add(bound.work())
            .ok_or(Incomplete::CounterOverflow)?;
        self.statistics.counts.work = work;
        budget.cancellation.poll()?;
        self.restrictions.bound = Some(bound);
        Ok(())
    }

    pub(super) fn permits_positive(
        &mut self,
        candidate: &Interpretation,
        budget: &mut Budget<'_>,
        timings: &mut Option<crate::SearchPhaseTimings>,
    ) -> Result<bool, Incomplete> {
        permits(
            &self.restrictions,
            self.filter.as_ref(),
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
        let Self {
            producers,
            traversal,
            restrictions,
            scratch,
            statistics,
            filter,
        } = self;
        let (formulas, narrower) = index.subject(theory)?;
        let factory = filter.as_ref();
        let mut worker = None;
        let before = traversal.statistics();
        let visit = traversal.next(|region, knowledge| -> Result<Narrowing, Incomplete> {
            let narrowing = narrow(
                (formulas, narrower, producers.as_ref()),
                restrictions,
                region,
                knowledge,
                scratch,
                budget,
                &mut statistics.counts,
            )?;
            if narrowing != Narrowing::Refuted
                && let Some(filter) = factory
                && filter.check(&mut worker, theory, region, budget.cancellation, timings)?
                    == crate::RegionFeasibility::Refuted
            {
                return Ok(Narrowing::Refuted);
            }
            Ok(narrowing)
        });
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

/// Narrow a region by the theory and every restriction until none decides
/// an atom, or one refutes it, each from what the region already knows
/// under it. Each narrowing runs to its own fixed point, so the joint fixed
/// point is reached when a full round changes nothing. Every charged read
/// spends a budget permit reserved in batches of at most `NARROWING_BATCH`,
/// unspent permits are refunded, and even a failed narrowing contributes its
/// admitted prefix to the counts.
pub(super) fn narrow<Q: Quota, R: std::borrow::Borrow<(Theory, Narrower)>>(
    (theory, narrower, producers): (&Theory, &Narrower, Option<&Producers>),
    restrictions: &Conditions<R>,
    region: &mut Region,
    knowledge: &mut CandidateKnowledge,
    scratch: &mut NarrowingScratch,
    budget: &mut Budget<'_, Q>,
    counts: &mut RegionCounts,
) -> Result<Narrowing, Incomplete> {
    let mut changed = false;
    loop {
        let mut round = false;
        for (index, (formulas, narrower)) in std::iter::once((theory, narrower))
            .chain(restrictions.permanent.iter().map(|restriction| {
                let (theory, narrower) = restriction.borrow();
                (theory, narrower)
            }))
            .enumerate()
        {
            let producers = if index == 0 { producers } else { None };
            let known = knowledge.permanent(index, narrower)?;
            let attempt = BudgetQuota::narrow(budget, |cancellation, quota| {
                narrower.narrow_known_reserved(
                    zetesis_ferraris::OriginalSubject::new(formulas, producers),
                    region,
                    known,
                    scratch,
                    cancellation,
                    quota,
                )
            });
            match account(&attempt, counts)? {
                Narrowing::Refuted => return Ok(Narrowing::Refuted),
                Narrowing::Fixed { changed: moved } => round |= moved,
            }
        }
        if let Some(bound) = &restrictions.bound {
            let (formulas, narrower) = bound.index.as_ref();
            let known = knowledge.bound(bound)?;
            let attempt = BudgetQuota::narrow(budget, |cancellation, quota| {
                narrower.narrow_known_reserved(
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
                Narrowing::Fixed { changed: moved } => round |= moved,
            }
        }
        changed |= round;
        if !round {
            return Ok(Narrowing::Fixed { changed });
        }
    }
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
    let charges = attempt.statistics;
    counts.propagations += charges.propagations;
    counts.held += charges.held;
    counts.cut += charges.cut;
    counts.work += charges.work;
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
