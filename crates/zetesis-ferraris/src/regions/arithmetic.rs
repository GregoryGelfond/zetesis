//! Checked affine equations and count bounds for candidate preparation.
//!
//! Source recognition proposes rows; independent source reads establish their
//! consequences for the original theory's answer sets. These rows never enter
//! the frozen reduct's proper-subset search. Root preparation precedes region
//! narrowing; the later region subjects will consume the same immutable rows.

use zetesis_algebra::Quota;

mod budget;
mod cardinality_source;
mod completion;
mod conjunction;
#[cfg(test)]
mod frozen;
mod hull;
mod parity;
mod root_facts;
mod rows;
mod source;
mod storage;
mod uniform;
mod cardinality;
mod cardinality_runtime;
mod work;
pub(super) mod runtime;
mod snapshot;

pub use cardinality_runtime::CardinalityKnowledge;
pub use parity::ParityKnowledge;
pub use runtime::ArithmeticAttempt;
pub use work::PreparationWork;

#[cfg(test)]
mod tests;

use std::{fmt, mem::size_of};

use zetesis_algebra::region::Region;
use zetesis_algebra::{Cancellation, Failure, Meter, Stop};

use super::Producers;
use crate::{AdmissionError, Theory};
use budget::{Budget, BudgetError};
use storage::Storage;
use work::Phase;

/// Independent allowance for optional original-theory arithmetic preparation.
/// Exhausting this allowance loses pruning; it does not stop the solve.
#[derive(Clone, Copy, Debug)]
pub struct PreparationLimits {
    /// Logical 64-bit operations across recognition, checking and preparation.
    pub work: u64,
    /// Named simultaneous headers and capacities, excluding shared source data.
    pub bytes: usize,
}

impl PreparationLimits {
    /// Ordinary optional effort allowance, shared by both arithmetic systems.
    pub const DEFAULT_WORK: u64 = 100_000_000;
}

/// Whether a complete immutable preparation was published.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PreparationStatus {
    /// The stage has not run, including after an earlier absorbing refutation.
    #[default]
    NotPrepared,
    /// Complete checked rows and their root result are available.
    Prepared,
    /// The shared optional effort allowance declined the entire preparation.
    WorkDeclined,
    /// The named optional storage allowance declined the entire preparation.
    StorageDeclined,
    /// A mandatory resource or source failure left the preparation incomplete.
    Incomplete,
}

impl PreparationStatus {
    /// Stable receipt spelling.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::NotPrepared => "not_prepared",
            Self::Prepared => "prepared",
            Self::WorkDeclined => "work_declined",
            Self::StorageDeclined => "storage_declined",
            Self::Incomplete => "incomplete",
        }
    }
}

/// Preparation and runtime observations for affine consequences of original answers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ParityStatistics {
    /// Publication or refusal status.
    pub status: PreparationStatus,
    /// Checked source equations, including a possible zero-coefficient row.
    pub rows: u64,
    /// Checked rows over exactly one atom.
    pub one: u64,
    /// Checked rows over exactly two atoms.
    pub two: u64,
    /// Checked rows over at least three atoms.
    pub wider: u64,
    /// Constraint-root occurrences retained as provenance.
    pub source_roots: usize,
    /// Local groups whose allowed set is not itself affine.
    pub inexact_hulls: u64,
    /// Original roots not retained as residual clause sources.
    pub unsupported_clauses: u64,
    /// Heads outside complete signed normal-definition recognition.
    pub unsupported_definitions: u64,
    /// Proposals rejected by independent source checking.
    pub provenance_rejected: u64,
    /// Retained echelon pivots.
    pub rank: usize,
    /// Trusted row additions executed during elimination.
    pub row_additions: u64,
    /// Work performed by recognition and parity preparation.
    pub preparation_work: u64,
    /// Disjoint phase observations, including work abandoned on refusal.
    pub phases: PreparationWork,
    /// Maximum shared named storage request observed through this stage.
    pub preparation_bytes: u128,
    /// Published system header and actual capacities, excluding source theory.
    pub retained_bytes: u128,
    /// A checked zero row with constant one refuted the unconstrained root.
    pub root_refuted: bool,
    /// Echelon rows that force an original atom at the root.
    pub units: usize,
    /// Completed runtime batches, including unchanged batches and absorbing
    /// refutations. Repeated batches on one region count separately; failed
    /// attempts contribute work and applied decisions but no completed batch.
    pub region_passes: u64,
    /// Fresh held or cut decisions applied by runtime parity, including a
    /// prefix retained in the receipt of a failed attempt. Excludes root units.
    pub forced: u64,
    /// Runtime batches that returned a checked semantic refutation.
    /// Excludes root refutation and every typed failure.
    pub refuted: u64,
}

/// Preparation and runtime observations for independently checked count partitions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CardinalityStatistics {
    /// Publication or refusal status.
    pub status: PreparationStatus,
    /// Checked bound occurrences, retaining their guards and provenance.
    pub rows: usize,
    /// Checked lower-bound occurrences.
    pub lower: usize,
    /// Checked upper-bound occurrences.
    pub upper: usize,
    /// Checked bounds with at least one signed guard occurrence.
    pub guarded: usize,
    /// Connected support carriers considered by the bounded proposal policy.
    pub carriers: u64,
    /// Candidate pairs submitted to the independent partition checker.
    pub attempts: u64,
    /// Candidate pairs rejected by that checker.
    pub refused: u64,
    /// Checked pairs with two disjoint covers of the same atom set.
    pub families: u64,
    /// Checked row occurrences across retained pairs.
    pub occurrences: usize,
    /// Proposals rejected by independent source checking.
    pub provenance_rejected: u64,
    /// Work performed by cardinality recognition, checking and preparation.
    pub preparation_work: u64,
    /// Disjoint phase observations, including work abandoned on refusal.
    pub phases: PreparationWork,
    /// Maximum shared named storage request observed through this stage.
    pub preparation_bytes: u128,
    /// Published system header and actual capacities, excluding source theory.
    pub retained_bytes: u128,
    /// A checked lower count exceeded its matching upper count at the root.
    pub root_refuted: bool,
    /// Completed runtime batches, including unchanged batches and absorbing
    /// refutations. Repeated batches on one region count separately; failed
    /// attempts contribute work but no completed batch.
    pub region_passes: u64,
    /// Runtime batches that returned a checked semantic refutation.
    /// Excludes root refutation and every typed failure.
    pub refuted: u64,
}

/// A mandatory failure: never an optional decline or a semantic refutation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreparationError<E = Stop> {
    /// The cancellation token, global work quota or allocator refused work.
    Stopped(E),
    /// A source identity or admitted formula read failed.
    Source(AdmissionError),
}

impl<E: fmt::Display> fmt::Display for PreparationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stopped(error) => error.fmt(formatter),
            Self::Source(error) => write!(formatter, "arithmetic source: {error}"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for PreparationError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Stopped(error) => Some(error),
            Self::Source(error) => Some(error),
        }
    }
}

/// Complete checked parity evidence on exactly one original theory.
#[derive(Debug)]
pub struct ParitySystem {
    recognized: rows::Recognized,
    prepared: parity::Prepared,
}

impl ParitySystem {
    /// Owned headers and actual vector capacities; shared theory data excluded.
    #[must_use]
    pub fn retained_bytes(&self) -> u128 {
        (size_of::<Self>() - size_of::<rows::Recognized>() - size_of::<parity::Prepared>()) as u128
            + self.recognized.retained_bytes()
            + self.prepared.retained_bytes()
    }
    /// Whether the original atom coordinates belong to this exact admission.
    #[must_use]
    pub fn belongs_to(&self, theory: &Theory) -> bool {
        self.recognized.facts.belongs_to(theory)
    }

    /// Original atom decisions from one-atom echelon rows, in basis order.
    pub fn units(&self) -> impl Iterator<Item = (usize, bool)> + '_ {
        self.prepared
            .units
            .iter()
            .map(|unit| (unit.atom, unit.value))
    }

    /// Whether a checked row established the root contradiction `0 = 1`.
    #[must_use]
    pub const fn root_refuted(&self) -> bool {
        self.prepared.root_refuted
    }
}

/// Complete checked counting evidence on exactly one original theory.
#[derive(Debug)]
pub struct CardinalitySystem {
    owner: Theory,
    bounds: Vec<cardinality_source::Bound>,
    prepared: cardinality::Prepared,
}

impl CardinalitySystem {
    /// Owned headers and capacities; the original theory is shared, not copied.
    #[must_use]
    pub fn retained_bytes(&self) -> u128 {
        size_of::<Self>() as u128
            + self.bounds.capacity() as u128 * size_of::<cardinality_source::Bound>() as u128
            + self
                .bounds
                .iter()
                .map(|bound| {
                    bound.support.capacity() as u128 * size_of::<u64>() as u128
                        + bound.guard.capacity() as u128 * size_of::<source::Literal>() as u128
                })
                .sum::<u128>()
            + self.prepared.retained_bytes()
            - size_of::<cardinality::Prepared>() as u128
    }
    /// Whether its original atom coordinates belong to this exact admission.
    #[must_use]
    pub fn belongs_to(&self, theory: &Theory) -> bool {
        self.owner.same_instance(theory)
    }

    /// Whether a checked root count excluded every original answer set.
    #[must_use]
    pub const fn root_refuted(&self) -> bool {
        matches!(self.prepared, cardinality::Prepared::RootRefuted { .. })
    }

    /// Number of independently checked source bounds, before family selection.
    #[must_use]
    pub fn bound_count(&self) -> usize {
        self.bounds.len()
    }
}

/// The two immutable original-answer preparations, sharing one optional budget.
/// A parity refutation absorbs before cardinality preparation is necessary.
#[derive(Debug)]
pub struct RootPreparation {
    parity: ParitySystem,
    cardinality: Option<CardinalitySystem>,
    seeded_root: Option<Region>,
}

#[derive(Clone, Copy)]
enum Stage {
    Parity,
    Cardinality,
    Seeding,
}

/// Preparation result and the executed prefix on every outcome.
pub struct PreparationAttempt<E = Stop> {
    /// `None` means optional work or storage declined; no partial system escapes.
    pub result: Result<Option<RootPreparation>, PreparationError<E>>,
    /// Work settled once against the supplied global quota.
    pub work: u64,
    /// Maximum named simultaneous capacity or request, not process RSS.
    pub peak_bytes: u128,
    /// Affine-stage observations.
    pub parity: ParityStatistics,
    /// Counting-stage observations.
    pub cardinality: CardinalityStatistics,
}

impl RootPreparation {
    /// Prepare root consequences from the exact original theory and its already
    /// complete producer inventory. This never extracts another inventory.
    /// Optional limits publish no partial rows or units; global refusals keep
    /// their typed error. Spare global work grants are refunded on every exit.
    #[must_use]
    pub fn prepare_reserved<Q: Quota + ?Sized>(
        theory: &Theory,
        producers: Option<&Producers>,
        limits: PreparationLimits,
        cancellation: &Cancellation,
        meter: &mut Meter<'_, Q>,
    ) -> PreparationAttempt<Failure<Q::Error>> {
        let mut budget = Budget::new(meter, limits.work, limits.bytes, cancellation);
        let mut parity = ParityStatistics::default();
        let mut cardinality = CardinalityStatistics::default();
        let mut stage = Stage::Parity;
        let result = budget.in_phase(Phase::Recognition, |budget| {
            build(
                theory,
                producers,
                &mut parity,
                &mut cardinality,
                &mut stage,
                budget,
            )
        });
        // The outer observation settles after build returns, including a
        // failed prefix. Cardinality's complete subtotal is retained separately.
        match stage {
            Stage::Parity => parity.phases = budget.phases(),
            Stage::Cardinality => cardinality.phases = budget.phases().since(parity.phases),
            Stage::Seeding => parity.phases = budget.phases().since(cardinality.phases),
        }
        let result = match result {
            Ok(value) => Ok(Some(value)),
            Err(error) => {
                let status = match error {
                    BudgetError::WorkDeclined => PreparationStatus::WorkDeclined,
                    BudgetError::StorageDeclined { .. } => PreparationStatus::StorageDeclined,
                    BudgetError::Stopped(_) | BudgetError::Refused(_) | BudgetError::Source(_) => {
                        PreparationStatus::Incomplete
                    }
                };
                match stage {
                    Stage::Parity => {
                        parity.preparation_work = budget.work();
                        parity.preparation_bytes = budget.peak_bytes();
                    }
                    Stage::Cardinality => {
                        cardinality.preparation_work = budget.work() - parity.preparation_work;
                        cardinality.preparation_bytes = budget.peak_bytes();
                        cardinality.status = status;
                    }
                    Stage::Seeding => {
                        parity.preparation_work = budget.work() - cardinality.preparation_work;
                        parity.preparation_bytes = budget.peak_bytes();
                        cardinality.status = status;
                    }
                }
                // Earlier completed stages belong to this one atomic bundle;
                // none is published if its later preparation is declined.
                parity.status = status;
                parity.retained_bytes = 0;
                cardinality.retained_bytes = 0;
                match error {
                    BudgetError::WorkDeclined | BudgetError::StorageDeclined { .. } => Ok(None),
                    BudgetError::Stopped(stop) => {
                        Err(PreparationError::Stopped(Failure::Stopped(stop)))
                    }
                    BudgetError::Refused(error) => {
                        Err(PreparationError::Stopped(Failure::Refused(error)))
                    }
                    BudgetError::Source(source) => Err(PreparationError::Source(source)),
                }
            }
        };
        budget.finish();
        PreparationAttempt {
            result,
            work: budget.work(),
            peak_bytes: budget.peak_bytes(),
            parity,
            cardinality,
        }
    }

    /// Whether either checked preparation refuted the unconstrained root.
    #[must_use]
    pub fn root_refuted(&self) -> bool {
        self.parity.root_refuted()
            || self
                .cardinality
                .as_ref()
                .is_some_and(CardinalitySystem::root_refuted)
    }

    /// The checked equations; their owner cannot be replaced independently.
    #[must_use]
    pub const fn parity(&self) -> &ParitySystem {
        &self.parity
    }

    /// Checked counting families, absent after an earlier parity refutation.
    #[must_use]
    pub const fn cardinality(&self) -> Option<&CardinalitySystem> {
        self.cardinality.as_ref()
    }

    /// Both systems retain this exact original admission and atom universe.
    #[must_use]
    pub fn belongs_to(&self, theory: &Theory) -> bool {
        self.parity.belongs_to(theory)
            && self
                .cardinality
                .as_ref()
                .is_none_or(|system| system.belongs_to(theory))
    }

    /// Transfer the root seeded by P0's unique one-atom rows, at most once.
    /// `None` means there were no units, preparation refuted the root, or the
    /// caller already took it. Check `root_refuted` before creating a walk.
    pub fn take_seeded_root(&mut self) -> Option<Region> {
        self.seeded_root.take()
    }
}

fn build<Q: Quota + ?Sized>(
    theory: &Theory,
    producers: Option<&Producers>,
    parity_statistics: &mut ParityStatistics,
    cardinality_statistics: &mut CardinalityStatistics,
    stage: &mut Stage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<RootPreparation, BudgetError<Q::Error>> {
    let mut storage = Storage::default();
    // The returned owner has fixed optional slots even when a root refutation
    // leaves them empty. Admit them before any conclusive arithmetic step.
    // The two parity records are admitted by their own constructors below.
    storage.admit_record(
        (size_of::<RootPreparation>()
            - size_of::<rows::Recognized>()
            - size_of::<parity::Prepared>()) as u128,
        budget,
    )?;
    let mut recognized = budget.in_phase(Phase::Recognition, |budget| {
        rows::recognize(theory, producers, &mut storage, budget)
    })?;
    let prepared = budget.in_phase(Phase::Reduction, |budget| {
        parity::prepare(
            &mut recognized.equations,
            theory.atom_count(),
            &mut storage,
            budget,
        )
    })?;
    *parity_statistics = ParityStatistics::from_preparation(&recognized, &prepared, budget);
    let parity = ParitySystem {
        recognized,
        prepared,
    };
    if parity.root_refuted() {
        return Ok(RootPreparation {
            parity,
            cardinality: None,
            seeded_root: None,
        });
    }
    *stage = Stage::Cardinality;
    let start = budget.work();
    // Transfer the prepaid slots to the constructors that account their own
    // headers. The shared theory token and optional-tag padding remain live.
    storage.release_record(size_of::<Vec<cardinality_source::Bound>>() as u128)?;
    let bounds = budget.in_phase(Phase::Recognition, |budget| {
        checked_bounds(theory, cardinality_statistics, &mut storage, budget)
    })?;
    storage.release_record(size_of::<cardinality::Prepared>() as u128)?;
    let prepared = budget.in_phase(Phase::Reduction, |budget| {
        cardinality::prepare(&bounds, theory.atom_count(), &mut storage, budget)
    })?;
    cardinality_statistics.carriers = prepared.counts().carriers;
    cardinality_statistics.attempts = prepared.counts().attempts;
    cardinality_statistics.families = prepared.counts().accepted;
    cardinality_statistics.refused = prepared.counts().rejected;
    cardinality_statistics.status = PreparationStatus::Prepared;
    cardinality_statistics.rows = bounds.len();
    match &prepared {
        cardinality::Prepared::RootRefuted { .. } => cardinality_statistics.root_refuted = true,
        cardinality::Prepared::Families(families) => {
            cardinality_statistics.occurrences = families.occurrences.len();
        }
    }
    cardinality_statistics.preparation_work = budget.work() - start;
    cardinality_statistics.phases = budget.phases().since(parity_statistics.phases);
    cardinality_statistics.preparation_bytes = budget.peak_bytes();
    let cardinality = CardinalitySystem {
        owner: theory.clone(),
        bounds,
        prepared,
    };
    cardinality_statistics.retained_bytes = cardinality.retained_bytes();
    if cardinality.root_refuted() {
        return Ok(RootPreparation {
            parity,
            cardinality: Some(cardinality),
            seeded_root: None,
        });
    }
    *stage = Stage::Seeding;
    let start = budget.work();
    let seeded_root = budget.in_phase(Phase::RootSeed, |budget| {
        seed_root(theory, &parity, &mut storage, budget)
    })?;
    parity_statistics.preparation_work += budget.work() - start;
    parity_statistics.preparation_bytes = budget.peak_bytes();
    Ok(RootPreparation {
        parity,
        cardinality: Some(cardinality),
        seeded_root,
    })
}

impl ParityStatistics {
    fn from_preparation<Q: Quota + ?Sized>(
        recognized: &rows::Recognized,
        prepared: &parity::Prepared,
        budget: &Budget<'_, '_, Q>,
    ) -> Self {
        Self {
            status: PreparationStatus::Prepared,
            rows: recognized.counts.rows,
            one: recognized.counts.one,
            two: recognized.counts.two,
            wider: recognized.counts.wider,
            source_roots: recognized.clauses.len(),
            inexact_hulls: recognized.counts.inexact_hulls,
            unsupported_clauses: recognized.counts.unsupported_clauses,
            unsupported_definitions: recognized.counts.unsupported_definitions,
            provenance_rejected: recognized.counts.provenance_rejected,
            rank: prepared.basis.len(),
            row_additions: prepared.counts.additions,
            preparation_work: budget.work(),
            phases: budget.phases(),
            preparation_bytes: budget.peak_bytes(),
            retained_bytes: (size_of::<ParitySystem>()
                - size_of::<rows::Recognized>()
                - size_of::<parity::Prepared>()) as u128
                + recognized.retained_bytes()
                + prepared.retained_bytes(),
            root_refuted: prepared.root_refuted,
            units: prepared.units.len(),
            region_passes: 0,
            forced: 0,
            refuted: 0,
        }
    }
}

/// Root decisions are part of preparation: failure publishes no partial seed.
/// The all-open masks are initialized once; every unit has a distinct pivot.
fn seed_root<Q: Quota + ?Sized>(
    theory: &Theory,
    parity: &ParitySystem,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<Option<Region>, BudgetError<Q::Error>> {
    budget.charge(1)?;
    if parity.prepared.units.is_empty() {
        budget.charge(1)?;
        return Ok(None);
    }
    let baseline = storage.bytes() - size_of::<Region>() as u128;
    let mut root = Region::try_all_open_with(
        theory.atom_count(),
        budget,
        &mut |budget, work| budget.charge(work),
        &mut |budget, bytes| {
            let required = baseline
                .checked_add(bytes)
                .ok_or(BudgetError::Stopped(Stop::Allocation))?;
            budget.admit_bytes(required)
        },
    )?;
    storage.admit_record(root.retained_bytes() - size_of::<Region>() as u128, budget)?;
    for unit in &parity.prepared.units {
        // Select the operation, then execute Region::decide's open-atom path:
        // bounds (1), locate (3), held/cut tests (4), polarity (1), OR (1),
        // and successful result (1). Distinct echelon pivots remain open here.
        budget.charge(1)?;
        let hold = unit.value;
        budget.charge(11)?;
        let applied = if hold {
            root.hold(unit.atom)
        } else {
            root.cut(unit.atom)
        };
        assert!(applied, "echelon units have distinct admitted pivots");
    }
    budget.charge(1)?;
    Ok(Some(root))
}

fn checked_bounds<Q: Quota + ?Sized>(
    theory: &Theory,
    statistics: &mut CardinalityStatistics,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<Vec<cardinality_source::Bound>, BudgetError<Q::Error>> {
    let mut checked = storage.reserve(0, budget)?;
    for root in 0..theory.roots().len() {
        let mut proposed = cardinality_source::read_root(theory, root, storage, budget)?;
        // Admit moving the headers out of this vector before Drain changes it.
        let move_work = u64::try_from(proposed.len())
            .ok()
            .and_then(|n| n.checked_mul(12))
            .and_then(|n| n.checked_add(1))
            .ok_or(BudgetError::Stopped(Stop::WorkLimit))?;
        budget.charge(move_work)?;
        for bound in proposed.drain(..) {
            let valid = budget.in_phase(Phase::Provenance, |budget| {
                cardinality_source::check(theory, theory, &bound, storage, budget)
            })?;
            if valid {
                budget.charge(3)?;
                match bound.side {
                    uniform::BoundSide::Lower => statistics.lower += 1,
                    uniform::BoundSide::Upper => statistics.upper += 1,
                }
                if !bound.guard.is_empty() {
                    budget.charge(1)?;
                    statistics.guarded += 1;
                }
                // Both inner headers already belong to an enclosing accounted
                // allocation. Transfer does not add another header owner.
                storage.push(&mut checked, bound, 12, budget)?;
            } else {
                budget.charge(1)?;
                statistics.provenance_rejected += 1;
                cardinality_source::release(bound, storage)?;
                debug_assert!(valid, "a proposed bound must follow from its source");
            }
        }
        storage.release(proposed)?;
    }
    Ok(checked)
}
