//! A candidate's inner subjects: parity rows and count bounds of its
//! countermodel class, recognized by probing the closed query root under the
//! candidate's frozen reduct.
//!
//! Proposals are syntactic (`proposals`); nothing rests on them. Every row's
//! provenance is a refuted probe (`probes`): a frozen narrowing from a copy of
//! the closed root with exactly a guard and a pattern decided, the premise of
//! `Zetesis.Algebra.entailed_by_refutations`. A saturation guard whose probe
//! reaches the candidate is a unit (`Zetesis.Algebra.saturation_unit`). The
//! preparation is built from the theory and the candidate, under its own
//! lease of the caller's account, and belongs to that candidate alone.

mod probes;
mod proposals;

#[cfg(test)]
mod tests;

use std::mem::size_of;

use zetesis_algebra::words::Words;
use zetesis_algebra::{Cancellation, Countermodels, Failure, Meter, Quota, Refusal, Stop};

use super::{
    CardinalityStatistics, CardinalitySystem, ParityStatistics, ParitySystem, PreparationError,
    PreparationLimits, PreparationStatus,
    budget::{Budget, BudgetError},
    hull, parity,
    root_facts::CheckedRootFacts,
    rows::{self, Source},
    source::{self, Support},
    storage::Storage,
    work::Phase,
};
use crate::{
    FrozenSubject, Interpretation, Knowledge, Narrower, NarrowingScratch, Producers, Region, Theory,
};

/// A candidate's query root after its first frozen closure, with what reads it.
#[derive(Clone, Copy)]
pub struct ClosedQuery<'a> {
    /// The index of the candidate's theory.
    pub narrower: &'a Narrower,
    /// The theory's producers when its roots are rules; none proposes nothing.
    pub producers: Option<&'a Producers>,
    /// The candidate's frozen reduct.
    pub subject: FrozenSubject<'a>,
    /// The candidate whose countermodels the query walks.
    pub candidate: &'a Interpretation,
    /// The query root, closed by the frozen narrowing.
    pub region: &'a Region,
    /// The closed root's knowledge.
    pub knowledge: &'a Knowledge<Countermodels>,
}

/// The lease of one candidate's preparation: crossing it declines the whole
/// preparation and installs nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrozenPreparationLimits {
    /// Word operations, charged to the caller's account as performed.
    pub work: u64,
    /// Peak simultaneous named bytes.
    pub bytes: usize,
}

impl FrozenPreparationLimits {
    /// The root preparation's work allowance, per candidate, with `bytes`.
    #[must_use]
    pub const fn per_candidate(bytes: usize) -> Self {
        Self {
            work: PreparationLimits::DEFAULT_WORK,
            bytes,
        }
    }
}

/// Observations of one candidate's preparation, on every outcome.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrozenPreparationStatistics {
    /// Publication or refusal status.
    pub status: PreparationStatus,
    /// Candidate guards proposed.
    pub guards: u64,
    /// Guards whose probe reached the candidate: atoms cut at the root.
    pub saturation_units: u64,
    /// Parity groups proposed.
    pub groups: u64,
    /// Parity groups probed, under a unit guard.
    pub probed_groups: u64,
    /// Probes run, unit probes included.
    pub probes: u64,
    /// Patterns whose probe was refuted.
    pub refuted_patterns: u64,
    /// Probed groups whose allowed patterns are not themselves affine.
    pub inexact_hulls: u64,
    /// The rows refute the closed root under the units.
    pub root_refuted: bool,
    /// Work charged to the account, on every outcome.
    pub preparation_work: u64,
    /// Peak named bytes observed, the probes' retired copies included.
    pub preparation_bytes: u128,
    /// The parity system's own observations.
    pub parity: ParityStatistics,
    /// The counting system's own observations.
    pub cardinality: CardinalityStatistics,
}

/// Preparation result and the executed prefix on every outcome.
pub struct FrozenPreparationAttempt<E = Stop> {
    /// `None` means the lease declined or nothing was proposed; no partial
    /// system escapes.
    pub result: Result<Option<FrozenPreparation>, PreparationError<E>>,
    /// Work settled once against the supplied account.
    pub work: u64,
    /// Maximum named simultaneous capacity or request, not process RSS.
    pub peak_bytes: u128,
    /// What the preparation observed.
    pub statistics: FrozenPreparationStatistics,
}

/// One candidate's inner subjects: the units its query root cuts, and the
/// parity and counting systems of its countermodel class. Immutable for the
/// query's life and consulted for no other candidate.
#[derive(Debug)]
pub struct FrozenPreparation {
    owner: Theory,
    candidate: Interpretation,
    units: Vec<usize>,
    parity: Option<ParitySystem>,
    cardinality: Option<CardinalitySystem>,
}

impl FrozenPreparation {
    /// Prepare `query`'s candidate's inner subjects under `limits`, charging
    /// `quota` as performed. The query root and its knowledge are read, never
    /// changed. Optional limits publish nothing; the account's refusal and
    /// cancellation keep their typed error.
    #[must_use]
    pub fn prepare_reserved<Q: Quota + ?Sized>(
        query: ClosedQuery<'_>,
        scratch: &mut NarrowingScratch,
        limits: FrozenPreparationLimits,
        cancellation: &Cancellation,
        quota: &mut Q,
    ) -> FrozenPreparationAttempt<Failure<Q::Error>> {
        let mut lease = Leased {
            account: quota,
            remaining: limits.work,
            used: 0,
        };
        let mut statistics = FrozenPreparationStatistics::default();
        let mut peak = 0_u128;
        let result = build(
            query,
            scratch,
            limits,
            cancellation,
            &mut lease,
            &mut statistics,
            &mut peak,
        );
        let result = match result {
            Ok(prepared) => {
                statistics.status = if prepared.is_some() {
                    PreparationStatus::Prepared
                } else {
                    PreparationStatus::NotPrepared
                };
                Ok(prepared)
            }
            Err(error) => settle(error, &mut statistics),
        };
        statistics.preparation_work = lease.used;
        statistics.preparation_bytes = peak;
        FrozenPreparationAttempt {
            result,
            work: lease.used,
            peak_bytes: peak,
            statistics,
        }
    }

    /// Whether this preparation is `candidate`'s under `theory`: the same
    /// theory instance and the same atoms.
    #[must_use]
    pub fn belongs_to(&self, theory: &Theory, candidate: &Interpretation) -> bool {
        self.owner.same_instance(theory)
            && candidate.theory.same_instance(theory)
            && self.candidate.words.storage() == candidate.words.storage()
    }

    /// The atoms the query root cuts: the saturation units.
    #[must_use]
    pub fn units(&self) -> &[usize] {
        &self.units
    }

    /// The parity system of the countermodels, when any row was found.
    #[must_use]
    pub const fn parity(&self) -> Option<&ParitySystem> {
        self.parity.as_ref()
    }

    /// The counting system of the countermodels, when any bound was found.
    #[must_use]
    pub const fn cardinality(&self) -> Option<&CardinalitySystem> {
        self.cardinality.as_ref()
    }

    /// Whether a checked row refuted the closed root under the units.
    #[must_use]
    pub fn root_refuted(&self) -> bool {
        self.parity.as_ref().is_some_and(ParitySystem::root_refuted)
            || self
                .cardinality
                .as_ref()
                .is_some_and(CardinalitySystem::root_refuted)
    }
}

/// The lease's refusal: crossed, or the account's own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LeaseRefusal<E> {
    Crossed,
    Account(E),
}

impl<E: Refusal> Refusal for LeaseRefusal<E> {
    fn is_resource_refusal(&self) -> bool {
        match self {
            Self::Crossed => true,
            Self::Account(error) => error.is_resource_refusal(),
        }
    }

    fn from_stop(stop: Stop) -> Self {
        Self::Account(E::from_stop(stop))
    }
}

/// The caller's account under a lease: grants at most what the lease leaves,
/// forwarding every grant and refund to the account so the account's receipt
/// is exact.
struct Leased<'a, Q: Quota + ?Sized> {
    account: &'a mut Q,
    remaining: u64,
    used: u64,
}

impl<Q: Quota + ?Sized> Quota for Leased<'_, Q> {
    type Error = LeaseRefusal<Q::Error>;

    fn reserve(&mut self, wanted: u64) -> Result<u64, Self::Error> {
        let capped = wanted.min(self.remaining);
        if capped == 0 {
            return Err(LeaseRefusal::Crossed);
        }
        let mut granted = self
            .account
            .reserve(capped)
            .map_err(LeaseRefusal::Account)?;
        if granted > capped {
            self.account.refund(granted - capped);
            granted = capped;
        }
        self.remaining -= granted;
        self.used += granted;
        Ok(granted)
    }

    fn refund(&mut self, unspent: u64) {
        self.account.refund(unspent);
        self.remaining += unspent;
        self.used -= unspent;
    }
}

/// A probe's meter failure as the budget reports it.
fn from_meter<E>(failure: Failure<E>) -> BudgetError<E> {
    match failure {
        Failure::Refused(error) => BudgetError::Refused(error),
        Failure::Stopped(stop) => BudgetError::Stopped(stop),
    }
}

/// Settle a refusal: a crossed lease declines, the rest keep their error.
fn settle<E>(
    error: BudgetError<LeaseRefusal<E>>,
    statistics: &mut FrozenPreparationStatistics,
) -> Result<Option<FrozenPreparation>, PreparationError<Failure<E>>> {
    match error {
        BudgetError::WorkDeclined | BudgetError::Refused(LeaseRefusal::Crossed) => {
            statistics.status = PreparationStatus::WorkDeclined;
            Ok(None)
        }
        BudgetError::StorageDeclined { .. } => {
            statistics.status = PreparationStatus::StorageDeclined;
            Ok(None)
        }
        BudgetError::Stopped(stop) => {
            statistics.status = PreparationStatus::Incomplete;
            Err(PreparationError::Stopped(Failure::Stopped(stop)))
        }
        BudgetError::Refused(LeaseRefusal::Account(error)) => {
            statistics.status = PreparationStatus::Incomplete;
            Err(PreparationError::Stopped(Failure::Refused(error)))
        }
        BudgetError::Source(source) => {
            statistics.status = PreparationStatus::Incomplete;
            Err(PreparationError::Source(source))
        }
    }
}

/// Propose, probe, and load the rows: three metered phases over one lease.
fn build<Q: Quota + ?Sized>(
    query: ClosedQuery<'_>,
    scratch: &mut NarrowingScratch,
    limits: FrozenPreparationLimits,
    cancellation: &Cancellation,
    lease: &mut Leased<'_, Q>,
    statistics: &mut FrozenPreparationStatistics,
    peak: &mut u128,
) -> Result<Option<FrozenPreparation>, BudgetError<LeaseRefusal<Q::Error>>> {
    let theory = query.subject.theory();
    let Some(producers) = query.producers else {
        return Ok(None);
    };
    let proposals = {
        let mut meter = Meter::reserved(lease);
        let mut budget = Budget::new(&mut meter, limits.work, limits.bytes, cancellation);
        let proposals = budget.in_phase(Phase::Recognition, |budget| {
            proposals::propose(
                theory,
                producers,
                query.candidate,
                query.subject.truth(),
                query.region,
                budget,
            )
        });
        *peak = (*peak).max(budget.peak_bytes());
        budget.finish();
        proposals?
    };
    statistics.guards = proposals.counts.guards;
    statistics.groups = proposals.counts.groups;
    if proposals.guards.is_empty() {
        return Ok(None);
    }
    let mut probe =
        probes::Probe::new(query.narrower, query.subject, query.region, query.knowledge);
    *peak = (*peak).max(probe.retained_bytes());
    let mut units = Vec::new();
    let mut tables: Vec<(Support, Words<u64, [u64; 4]>)> = Vec::new();
    {
        let mut meter = Meter::reserved(lease);
        for &guard in &proposals.guards {
            statistics.probes += 1;
            let refuted = probe
                .run(&[(guard, true)], scratch, cancellation, &mut meter)
                .map_err(from_meter)?;
            let reaches = refuted
                || query
                    .candidate
                    .atoms()
                    .all(|atom| probe.region().is_held(atom));
            if !reaches {
                continue;
            }
            units.push(guard);
            statistics.saturation_units += 1;
            for group in proposals.parity.iter().filter(|group| group.guard == guard) {
                statistics.probed_groups += 1;
                let mut counts = (0, 0);
                let forbidden = probe
                    .table(group, &mut counts, scratch, cancellation, &mut meter)
                    .map_err(from_meter)?;
                statistics.probes += counts.0;
                statistics.refuted_patterns += counts.1;
                tables.push((group.support, forbidden));
            }
        }
        meter.refund();
    }
    let parity = {
        let mut meter = Meter::reserved(lease);
        let mut budget = Budget::new(&mut meter, limits.work, limits.bytes, cancellation);
        let parity = load(theory, &tables, statistics, &mut budget);
        *peak = (*peak).max(budget.peak_bytes());
        budget.finish();
        parity?
    };
    statistics.root_refuted = parity.as_ref().is_some_and(ParitySystem::root_refuted);
    let candidate = query.candidate.try_clone().map_err(BudgetError::Source)?;
    Ok(Some(FrozenPreparation {
        owner: theory.clone(),
        candidate,
        units,
        parity,
        cardinality: None,
    }))
}

/// The parity system of the probed groups: each table's affine hull, every
/// equation checked against its table, eliminated to echelon form.
fn load<Q: Quota + ?Sized>(
    theory: &Theory,
    tables: &[(Support, Words<u64, [u64; 4]>)],
    statistics: &mut FrozenPreparationStatistics,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<Option<ParitySystem>, BudgetError<Q::Error>> {
    if tables.is_empty() {
        return Ok(None);
    }
    let mut storage = Storage::default();
    let (facts, mut recognized_equations, mut counts) =
        budget.in_phase(Phase::Recognition, |budget| {
            storage.admit_record(
                (size_of::<rows::Recognized>()
                    - size_of::<CheckedRootFacts>()
                    - 2 * size_of::<Vec<source::Clause>>()) as u128,
                budget,
            )?;
            let facts = CheckedRootFacts::none(theory, &mut storage, budget)?;
            let mut equations = storage.reserve(0, budget)?;
            let mut counts = rows::Counts::default();
            for (group, (support, forbidden)) in tables.iter().enumerate() {
                let fixed = size_of::<hull::Hull>() as u128;
                storage.admit_record(fixed, budget)?;
                let proposed =
                    storage.with_record(hull::SCRATCH_BYTES as u128, budget, |_, budget| {
                        rows::propose(*support, forbidden.storage(), &mut counts, budget)
                    })?;
                for &local in proposed.equations() {
                    let words = rows::materialize(theory, *support, local, &mut storage, budget)?;
                    budget.charge(3)?; // constant, probed-source tag and group
                    let candidate = rows::Equation {
                        words,
                        constant: local.constant,
                        source: Source::Probed { group },
                    };
                    let valid = source::implied_by_table(
                        forbidden.storage(),
                        support.len,
                        local.coefficients,
                        local.constant,
                        budget,
                    )?;
                    rows::retain(
                        valid,
                        candidate,
                        local.coefficients,
                        &mut equations,
                        &mut counts,
                        &mut storage,
                        budget,
                    )?;
                }
                storage.release_record(fixed)?;
            }
            Ok((facts, equations, counts))
        })?;
    statistics.inexact_hulls = counts.inexact_hulls;
    budget.charge(1)?;
    if recognized_equations.is_empty() {
        return Ok(None);
    }
    let prepared = budget.in_phase(Phase::Reduction, |budget| {
        parity::prepare(
            &mut recognized_equations,
            theory.atom_count(),
            &mut storage,
            budget,
        )
    })?;
    counts.groups = tables.len() as u64;
    let recognized = rows::Recognized {
        facts,
        clauses: Vec::new(),
        equations: recognized_equations,
        counts,
    };
    statistics.parity = ParityStatistics::from_preparation(&recognized, &prepared, budget);
    Ok(Some(ParitySystem::from_rows(recognized, prepared)))
}
