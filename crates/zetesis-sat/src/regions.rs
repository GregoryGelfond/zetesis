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
//! impossible and an impossible root refutes. The leaf is the proposal; the
//! reduct decides it as it decides a proposal from the clauses.
//!
//! The narrowing's node reads are charged as search work and each split as
//! a decision. A restriction narrows the regions still to visit; visited
//! regions were covered under the original theory, and a restriction only
//! removes candidates, so no restart and no exclusion index is needed
//! (`Search.CoverageTree`, `FormulaBounds`).

use zetesis_cpu::Stop;
use zetesis_cpu::regions::{Counting, Narrowing, Region, Traversal, Visit};
use zetesis_ferraris::{Interpretation, Knowledge, Narrower, Producers, RegionLimits, Theory};

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
    /// Regions the readings refuted.
    pub refuted: usize,
    /// Regions with every atom decided: the classical candidates proposed,
    /// or the proper-subset models and the candidate itself.
    pub leaves: usize,
    /// Propagation events: nodes and atoms learned and their neighbours
    /// revisited, and support rechecks.
    pub propagations: u64,
    /// Atoms the readings held.
    pub forced: u64,
    /// Atoms the readings cut.
    pub cut: u64,
    /// Node reads, root tests and producer checks, and for the candidate
    /// tree the indexing of the theory and each restriction and the
    /// producer extraction; included in search work.
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
            forced: self
                .forced
                .checked_add(other.forced)
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
}

#[derive(Debug)]
pub(crate) struct RegionSearch {
    producers: Option<Producers>,
    narrower: Narrower,
    /// Each region carries what is known about it under the theory and
    /// under each restriction, in order; a restriction added after a region
    /// was reached gets fresh knowledge when the region is next narrowed.
    traversal: Traversal<Vec<Knowledge>>,
    /// Each restriction with its own index.
    restrictions: Vec<(Theory, Narrower)>,
    statistics: RegionSearchStatistics,
}

impl RegionSearch {
    /// Extract the producers and open the root region.
    pub(crate) fn new(theory: &Theory, budget: &mut Budget<'_>) -> Result<Self, Incomplete> {
        let extraction =
            zetesis_ferraris::producers(theory, limits(budget), budget.control).map_err(stopped)?;
        budget.charge(extraction.work)?;
        let narrower = Narrower::new(theory);
        budget.charge(narrower.work())?;
        Ok(Self {
            statistics: RegionSearchStatistics {
                counts: RegionCounts {
                    work: extraction.work + narrower.work(),
                    ..Default::default()
                },
                producers: extraction.producers.is_some(),
            },
            producers: extraction.producers,
            traversal: Traversal::with_state(
                Region::undecided(theory.atom_count()),
                Counting::Never,
                vec![narrower.knowledge()],
            ),
            narrower,
            restrictions: Vec::new(),
        })
    }

    pub(crate) fn statistics(&self) -> RegionSearchStatistics {
        let regions = self.traversal.statistics();
        RegionSearchStatistics {
            counts: RegionCounts {
                regions: regions.regions,
                refuted: regions.refuted,
                leaves: regions.decided,
                ..self.statistics.counts
            },
            producers: self.statistics.producers,
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
    ) -> Result<Option<Interpretation>, Incomplete> {
        let Self {
            producers,
            narrower,
            traversal,
            restrictions,
            statistics,
        } = self;
        let before = traversal.statistics();
        let visit = traversal.next(|region, knowledge| {
            narrow(
                (theory, narrower),
                producers.as_ref(),
                restrictions,
                region,
                knowledge,
                budget,
                &mut statistics.counts,
            )
        });
        let after = traversal.statistics();
        // Every region visited that was neither refuted nor a leaf was split.
        let splits = (after.regions - before.regions)
            - (after.refuted - before.refuted)
            - (after.decided - before.decided);
        for _ in 0..splits {
            budget.decide()?;
        }
        match visit? {
            None | Some(Visit::Counted(..)) => Ok(None),
            Some(Visit::Leaf(region, _)) => {
                let mut selected = crate::search::storage(theory.atom_count())?;
                selected.extend(region.held());
                Interpretation::new(theory, selected)
                    .map_err(|error| match error {
                        zetesis_ferraris::AdmissionError::Allocation => Incomplete::Allocation,
                        _ => Incomplete::InvalidWitness,
                    })
                    .map(Some)
            }
        }
    }
}

/// Narrow a region by the theory and every restriction until none decides
/// an atom, or one refutes it, each from what the region already knows
/// under it. Each narrowing runs to its own fixed point, so the joint fixed
/// point is reached when a full round changes nothing. A narrowing stopped
/// on the work ceiling has spent at least the remaining work, which is
/// charged.
#[allow(clippy::too_many_arguments)]
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
            let result = narrower.narrow_known(
                formulas,
                producers,
                region,
                &mut knowledge[index],
                limits(budget),
                budget.control,
            );
            let (narrowing, pass) = match result {
                Ok(outcome) => outcome,
                Err(Stop::WorkLimit) => {
                    let remaining = budget.remaining_work();
                    counts.work += remaining;
                    budget.charge(remaining)?;
                    return Err(Incomplete::WorkLimit);
                }
                Err(stop) => return Err(stopped(stop)),
            };
            counts.propagations += pass.propagations;
            counts.forced += pass.forced;
            counts.cut += pass.cut;
            counts.work += pass.work;
            budget.charge(pass.work)?;
            match narrowing {
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

pub(crate) fn limits<Q: Quota>(budget: &Budget<'_, Q>) -> RegionLimits {
    RegionLimits {
        max_work: budget.remaining_work(),
        max_propagations: u64::MAX,
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
/// index of the theory is shared by every query.
#[derive(Debug)]
pub(crate) struct ReductQuery {
    narrower: Narrower,
}

impl ReductQuery {
    pub(crate) fn new(theory: &Theory) -> Self {
        Self {
            narrower: Narrower::new(theory),
        }
    }

    /// The indexing work, one visit per node.
    pub(crate) fn work(&self) -> u64 {
        self.narrower.work()
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
        let mut root = Region::undecided(theory.atom_count());
        for atom in (0..theory.atom_count()).filter(|&atom| !candidate.contains(atom)) {
            root.cut(atom);
        }
        let mut traversal = Traversal::with_state(root, Counting::Never, self.narrower.knowledge());
        loop {
            let before = traversal.statistics();
            let visit = traversal.next(|region, knowledge| {
                narrow_frozen(
                    &self.narrower,
                    theory,
                    truth,
                    region,
                    knowledge,
                    budget,
                    &mut statistics.reduct.regions,
                )
            });
            let after = traversal.statistics();
            let receipts = &mut statistics.reduct.regions;
            receipts.regions += after.regions - before.regions;
            receipts.refuted += after.refuted - before.refuted;
            receipts.leaves += after.decided - before.decided;
            let splits = (after.regions - before.regions)
                - (after.refuted - before.refuted)
                - (after.decided - before.decided);
            for _ in 0..splits {
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
                    let mut selected = crate::search::storage(theory.atom_count())?;
                    selected.extend(region.held());
                    let subset =
                        Interpretation::new(theory, selected).map_err(|error| match error {
                            zetesis_ferraris::AdmissionError::Allocation => Incomplete::Allocation,
                            _ => Incomplete::InvalidWitness,
                        })?;
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
    receipts: &mut RegionCounts,
) -> Result<Narrowing, Incomplete> {
    let result = narrower.narrow_frozen_known(
        theory,
        truth,
        region,
        knowledge,
        limits(budget),
        budget.control,
    );
    let (narrowing, pass) = match result {
        Ok(outcome) => outcome,
        Err(Stop::WorkLimit) => {
            let remaining = budget.remaining_work();
            receipts.work += remaining;
            budget.charge(remaining)?;
            return Err(Incomplete::WorkLimit);
        }
        Err(stop) => return Err(stopped(stop)),
    };
    receipts.propagations += pass.propagations;
    receipts.forced += pass.forced;
    receipts.cut += pass.cut;
    receipts.work += pass.work;
    budget.charge(pass.work)?;
    Ok(narrowing)
}
