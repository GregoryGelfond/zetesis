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

use std::sync::Arc;

use zetesis_cpu::Stop;
use zetesis_cpu::regions::{Counting, Narrowing, Region, Traversal, Visit};
use zetesis_ferraris::{
    Interpretation, Knowledge, Narrower, NarrowingAttempt, Producers, RegionLimits, Theory,
};

use crate::Incomplete;
use crate::search::{Budget, Quota};

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
    /// tree the indexing of the theory and each restriction and the
    /// producer extraction; included in search work. Enumeration queries
    /// share that already charged original index. A standalone membership
    /// query includes its own index construction in its reduct counts.
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

/// One immutable original-theory index. Construction binds the index to the
/// exact admitted instance; equal independently admitted DAGs are not its subject.
/// Candidate and reduct traversals share this owner, never their mutable knowledge.
#[derive(Debug)]
pub(crate) struct IndexedTheory {
    theory: Theory,
    narrower: Narrower,
}

impl IndexedTheory {
    pub(crate) fn new(theory: &Theory) -> Self {
        Self {
            theory: theory.clone(),
            narrower: Narrower::new(theory),
        }
    }

    pub(crate) fn theory(&self) -> &Theory {
        &self.theory
    }

    pub(crate) fn narrower(&self) -> &Narrower {
        &self.narrower
    }

    /// Authenticate before borrowing the indexed subject for any traversal.
    pub(crate) fn subject(&self, theory: &Theory) -> Result<(&Theory, &Narrower), Incomplete> {
        if self.theory.same_instance(theory) {
            Ok((&self.theory, &self.narrower))
        } else {
            Err(Incomplete::WrongTheory)
        }
    }
}

#[derive(Debug)]
pub(crate) struct RegionSearch {
    producers: Option<Producers>,
    index: Arc<IndexedTheory>,
    /// Each region carries what is known about it under the theory and
    /// under each restriction, in order; a restriction added after a region
    /// was reached gets fresh knowledge when the region is next narrowed.
    traversal: Traversal<Vec<Knowledge>>,
    /// Each restriction with its own index.
    restrictions: Vec<(Theory, Narrower)>,
    statistics: RegionSearchStatistics,
    pub(super) filter: Option<crate::region_filter::Filter>,
}

/// What opening a region search over a theory establishes: its producers,
/// when it lies in the producer fragment, its index, and the statistics of
/// the extraction and the indexing, both charged to the budget.
pub(crate) struct Opened {
    pub(crate) producers: Option<Producers>,
    pub(crate) index: Arc<IndexedTheory>,
    pub(crate) statistics: RegionSearchStatistics,
}

/// Open a region search over the theory: extract its producers and index
/// it, charging both.
pub(crate) fn open(theory: &Theory, budget: &mut Budget<'_>) -> Result<Opened, Incomplete> {
    let extraction = zetesis_ferraris::producers(theory, limits(budget), budget.cancellation)
        .map_err(stopped)?;
    budget.charge(extraction.work)?;
    let index = IndexedTheory::new(theory);
    let indexed_work = index.narrower().work();
    budget.charge(indexed_work)?;
    Ok(Opened {
        statistics: RegionSearchStatistics {
            counts: RegionCounts {
                work: extraction.work + indexed_work,
                ..Default::default()
            },
            producers: extraction.producers.is_some(),
            frontier: None,
        },
        producers: extraction.producers,
        index: Arc::new(index),
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
    /// Extract the producers and open the root region.
    pub(crate) fn new(theory: &Theory, budget: &mut Budget<'_>) -> Result<Self, Incomplete> {
        let Opened {
            producers,
            index,
            statistics,
        } = open(theory, budget)?;
        Ok(Self {
            statistics,
            producers,
            traversal: Traversal::with_state(
                Region::all_open(theory.atom_count()),
                Counting::Never,
                vec![index.narrower().knowledge()],
            ),
            index,
            restrictions: Vec::new(),
            filter: None,
        })
    }

    pub(crate) fn index(&self) -> &Arc<IndexedTheory> {
        &self.index
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
            .try_reserve(1)
            .map_err(|_| Incomplete::Allocation)?;
        let narrower = Narrower::new(restriction);
        budget.charge(narrower.work())?;
        self.statistics.counts.work += narrower.work();
        self.restrictions.push((restriction.clone(), narrower));
        Ok(())
    }

    /// The next leaf, a classical model of the theory and the restrictions,
    /// or `None` once the tree is covered.
    pub(crate) fn propose(
        &mut self,
        theory: &Theory,
        budget: &mut Budget<'_>,
        timings: &mut Option<crate::SearchPhaseTimings>,
    ) -> Result<Option<Interpretation>, Incomplete> {
        let Self {
            producers,
            index,
            traversal,
            restrictions,
            statistics,
            filter,
        } = self;
        let subject = index.subject(theory)?;
        let factory = filter.as_ref();
        let mut worker = None;
        let before = traversal.statistics();
        let visit = traversal.next(|region, knowledge| -> Result<Narrowing, Incomplete> {
            let narrowed = narrow(
                subject,
                producers.as_ref(),
                restrictions,
                region,
                knowledge,
                budget,
                &mut statistics.counts,
            )?;
            if narrowed != Narrowing::Refuted
                && let Some(filter) = factory
                && filter.check(&mut worker, theory, region, budget.cancellation, timings)?
                    == crate::RegionFeasibility::Refuted
            {
                return Ok(Narrowing::Refuted);
            }
            Ok(narrowed)
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
/// acquires its budget permit first, and even a failed narrowing contributes
/// its admitted prefix to the counts.
pub(crate) fn narrow<Q: Quota, R: std::borrow::Borrow<(Theory, Narrower)>>(
    theory: (&Theory, &Narrower),
    producers: Option<&Producers>,
    restrictions: &[R],
    region: &mut Region,
    knowledge: &mut Vec<Knowledge>,
    budget: &mut Budget<'_, Q>,
    counts: &mut RegionCounts,
) -> Result<Narrowing, Incomplete> {
    let mut changed = false;
    loop {
        let mut round = false;
        for (index, (formulas, narrower)) in std::iter::once(theory)
            .chain(restrictions.iter().map(|restriction| {
                let (theory, narrower) = restriction.borrow();
                (theory, narrower)
            }))
            .enumerate()
        {
            let producers = if index == 0 { producers } else { None };
            if knowledge.len() <= index {
                knowledge
                    .try_reserve(1)
                    .map_err(|_| Incomplete::Allocation)?;
                knowledge.push(narrower.knowledge());
            }
            let cancellation = budget.cancellation;
            let attempt = narrower.narrow_known_metered(
                formulas,
                producers,
                region,
                &mut knowledge[index],
                cancellation,
                || budget.tick().map_err(NarrowingStop),
            );
            match account(attempt, counts)? {
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

/// Keep the search-level work refusal distinct from a verification refusal,
/// while preserving any error supplied by the injected budget unchanged.
struct NarrowingStop(Incomplete);

impl From<Stop> for NarrowingStop {
    fn from(stop: Stop) -> Self {
        Self(stopped(stop))
    }
}

/// Publish every admitted narrowing prefix before returning its result. Work
/// has already passed through the local or shared budget before each read.
fn account(
    attempt: NarrowingAttempt<NarrowingStop>,
    counts: &mut RegionCounts,
) -> Result<Narrowing, Incomplete> {
    let charges = attempt.statistics;
    counts.propagations += charges.propagations;
    counts.held += charges.held;
    counts.cut += charges.cut;
    counts.work += charges.work;
    attempt.result.map_err(|error| error.0)
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
/// index of the theory is shared by candidate preparation and every query.
#[derive(Debug)]
pub(crate) struct ReductQuery {
    index: Arc<IndexedTheory>,
}

impl ReductQuery {
    pub(crate) fn from_index(index: Arc<IndexedTheory>) -> Self {
        Self { index }
    }

    pub(crate) fn theory(&self) -> &Theory {
        self.index.theory()
    }

    /// Search the proper subsets of the candidate, a classical model whose
    /// node truth is `truth`, for a model of its frozen reduct.
    ///
    /// # Errors
    /// Work, decision and control stops end the query without a verdict.
    pub(crate) fn check<Q: Quota>(
        &self,
        theory: &Theory,
        candidate: &Interpretation,
        truth: &[bool],
        limits: crate::Limits,
        budget: &mut Budget<'_, Q>,
        statistics: &mut crate::Statistics,
    ) -> Result<crate::Check, Incomplete> {
        let (theory, narrower) = self.index.subject(theory)?;
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
                    theory,
                    truth,
                    region,
                    knowledge,
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
    theory: &Theory,
    truth: &[bool],
    region: &mut Region,
    knowledge: &mut Knowledge,
    budget: &mut Budget<'_, Q>,
    counts: &mut RegionCounts,
) -> Result<Narrowing, Incomplete> {
    let cancellation = budget.cancellation;
    let attempt = narrower.narrow_frozen_known_metered(
        theory,
        truth,
        region,
        knowledge,
        cancellation,
        || budget.tick().map_err(NarrowingStop),
    );
    account(attempt, counts)
}

#[cfg(test)]
#[path = "regions/tests.rs"]
mod tests;
