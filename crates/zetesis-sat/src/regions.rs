//! Candidates proposed by regions: the coverage tree over the theory's
//! atoms, narrowed by the theory's readings.
//!
//! The root region leaves every atom open. Each region is narrowed to the
//! fixed point of `zetesis_ferraris::narrow` over the original theory, with
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
use zetesis_ferraris::{Interpretation, Narrower, Producers, RegionLimits, Theory};

use crate::Incomplete;
use crate::search::Budget;

/// How classical candidates are proposed to the reduct.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CandidateSearch {
    /// Regions of the candidate space narrowed by the theory's readings;
    /// every leaf is a classical model and no clause form is built.
    Regions,
    /// A retained chronological search over a clause form of the theory,
    /// with exact exclusion of every candidate already proposed. The
    /// default until the regions proposer is measured beside it.
    #[default]
    Clauses,
}

impl CandidateSearch {
    /// Stable spelling for configuration and execution reports.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Regions => "regions",
            Self::Clauses => "clauses",
        }
    }
}

/// What the region proposer did, cumulatively.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RegionSearchStatistics {
    /// Regions narrowed, the root included.
    pub regions: usize,
    /// Regions the readings refuted.
    pub refuted: usize,
    /// Regions with every atom decided: the classical candidates proposed.
    pub leaves: usize,
    /// Propagation events over the theory and the restrictions: nodes and
    /// atoms learned and their neighbours revisited, and support rechecks.
    pub propagations: u64,
    /// Atoms the readings held.
    pub forced: u64,
    /// Atoms the readings cut.
    pub cut: u64,
    /// Node reads, root tests and producer checks, included in search work.
    pub work: u64,
    /// Whether the theory lies in the producer fragment, so the support
    /// cut applies.
    pub producers: bool,
}

#[derive(Debug)]
pub(crate) struct RegionSearch {
    producers: Option<Producers>,
    narrower: Narrower,
    traversal: Traversal,
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
                work: extraction.work + narrower.work(),
                producers: extraction.producers.is_some(),
                ..Default::default()
            },
            producers: extraction.producers,
            narrower,
            traversal: Traversal::new(Region::undecided(theory.atom_count()), Counting::Never),
            restrictions: Vec::new(),
        })
    }

    pub(crate) fn statistics(&self) -> RegionSearchStatistics {
        let regions = self.traversal.statistics();
        RegionSearchStatistics {
            regions: regions.regions,
            refuted: regions.refuted,
            leaves: regions.decided,
            ..self.statistics
        }
    }

    /// Narrow the regions still to visit by a candidate-only restriction.
    pub(crate) fn restrict(&mut self, restriction: &Theory) -> Result<(), Incomplete> {
        self.restrictions
            .try_reserve(1)
            .map_err(|_| Incomplete::Allocation)?;
        let narrower = Narrower::new(restriction);
        self.statistics.work += narrower.work();
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
        let visit = traversal.next(|region| {
            narrow(
                (theory, narrower),
                producers.as_ref(),
                restrictions,
                region,
                budget,
                statistics,
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
            None | Some(Visit::Counted(_)) => Ok(None),
            Some(Visit::Leaf(region)) => {
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
/// an atom, or one refutes it. Each `zetesis_ferraris::narrow` call runs to
/// its own fixed point, so the joint fixed point is reached when a full
/// round changes nothing. A narrowing stopped on the work ceiling has
/// spent at least the remaining work, which is charged.
fn narrow(
    theory: (&Theory, &Narrower),
    producers: Option<&Producers>,
    restrictions: &[(Theory, Narrower)],
    region: &mut Region,
    budget: &mut Budget<'_>,
    statistics: &mut RegionSearchStatistics,
) -> Result<Narrowing, Incomplete> {
    let mut changed = false;
    loop {
        let mut round = false;
        for (index, (formulas, narrower)) in std::iter::once(theory)
            .chain(
                restrictions
                    .iter()
                    .map(|(theory, narrower)| (theory, narrower)),
            )
            .enumerate()
        {
            let producers = if index == 0 { producers } else { None };
            let result =
                narrower.narrow(formulas, producers, region, limits(budget), budget.control);
            let (narrowing, pass) = match result {
                Ok(outcome) => outcome,
                Err(Stop::WorkLimit) => {
                    let remaining = budget.remaining_work();
                    statistics.work += remaining;
                    budget.charge(remaining)?;
                    return Err(Incomplete::WorkLimit);
                }
                Err(stop) => return Err(stopped(stop)),
            };
            statistics.propagations += pass.propagations;
            statistics.forced += pass.forced;
            statistics.cut += pass.cut;
            statistics.work += pass.work;
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

fn limits(budget: &Budget<'_>) -> RegionLimits {
    RegionLimits {
        max_work: budget.remaining_work(),
        max_propagations: u64::MAX,
    }
}

fn stopped(stop: Stop) -> Incomplete {
    match stop {
        Stop::WorkLimit => Incomplete::WorkLimit,
        other => other.into(),
    }
}
