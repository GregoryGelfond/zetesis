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

mod counts;
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
    cardinality, hull, parity,
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
    /// What the root's first closure charged: with one pass over the index,
    /// what one probe is leased, since a probe narrows that closed root again
    /// with a few more decisions.
    pub closure_work: u64,
}

/// The lease of one candidate's preparation: crossing it declines the whole
/// preparation and installs nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrozenPreparationLimits {
    /// Word operations, charged to the caller's account as performed: the
    /// floor, which must cover the proposal pass and the rows' loading, the
    /// one ceiling on the preparation's work, held by its lease across every
    /// phase. The probes are leased on top, each the root closure's own cost
    /// plus one pass over the index (`ClosedQuery::closure_work`), as a probe
    /// narrows that closed root again with a few more decisions.
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
    /// Probing rounds run: the first by the frozen reading alone, each later
    /// one under the rows the round before established.
    pub rounds: u64,
    /// The rows refute the closed root under the units.
    pub root_refuted: bool,
    /// Work charged to the account, on every outcome.
    pub preparation_work: u64,
    /// Work leased for the probes beyond the floor: each unit probe and
    /// each pattern probe at the root closure's cost plus one index pass.
    pub leased: u64,
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
            extended: 0,
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
        statistics.leased = lease.extended;
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
    extended: u64,
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

impl<Q: Quota + ?Sized> Leased<'_, Q> {
    /// Lease `additional` operations more, as a probing round earns them.
    fn extend(&mut self, additional: u64) {
        self.remaining = self.remaining.saturating_add(additional);
        self.extended = self.extended.saturating_add(additional);
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
        // The lease is the one ceiling on work; this budget bounds bytes.
        let mut budget = Budget::new(&mut meter, u64::MAX, limits.bytes, cancellation);
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
    let mut groups = Vec::new();
    let allowance = query.closure_work.saturating_add(probe.narrower_work());
    let open_guards = proposals
        .guards
        .iter()
        .filter(|&&guard| query.region.is_open(guard))
        .count() as u64;
    lease.extend(open_guards.saturating_mul(allowance));
    {
        let mut meter = Meter::reserved(lease);
        for &guard in &proposals.guards {
            // A guard the root already cuts needs no probe: its clauses hold
            // there unconditionally. An open guard is probed held; one whose
            // narrowing reaches the candidate, or is refuted, is a unit.
            if query.region.is_open(guard) {
                statistics.probes += 1;
                let refuted = probe
                    .run(&[(guard, true)], None, scratch, cancellation, &mut meter)
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
            }
            groups.extend(proposals.parity.iter().filter(|group| group.guard == guard));
        }
        meter.refund();
    }
    statistics.probed_groups = groups.len() as u64;
    let (parity, cardinality) = probe_rounds(
        theory,
        &groups,
        &mut probe,
        scratch,
        &mut Probing {
            lease,
            allowance,
            limits,
            cancellation,
        },
        statistics,
        peak,
    )?;
    statistics.root_refuted = parity.as_ref().is_some_and(ParitySystem::root_refuted)
        || cardinality
            .as_ref()
            .is_some_and(CardinalitySystem::root_refuted);
    if units.is_empty() && parity.is_none() && cardinality.is_none() {
        // Nothing to install: no unit, no row, no bound.
        return Ok(None);
    }
    let candidate = query.candidate.try_clone().map_err(BudgetError::Source)?;
    Ok(Some(FrozenPreparation {
        owner: theory.clone(),
        candidate,
        units,
        parity,
        cardinality,
    }))
}

/// The systems a preparation installs: the parity rows and the count
/// bounds, each when any was found.
type Systems = (Option<ParitySystem>, Option<CardinalitySystem>);

/// What a probing round may spend: the lease it draws on, the allowance each
/// probe is leased, the floor's byte limit for the loading, and the
/// cancellation the loading polls. The lease is the one ceiling on work; the
/// loading's budget bounds bytes.
struct Probing<'l, 'q, Q: Quota + ?Sized> {
    lease: &'l mut Leased<'q, Q>,
    allowance: u64,
    limits: FrozenPreparationLimits,
    cancellation: &'l Cancellation,
}

/// The probing rounds a preparation may run: the first by the frozen reading
/// alone, the second under the rows it established. A later round tightens
/// a hull the round before left inexact, one probe per pattern of each such
/// group at the lease allowance each, and the cap bounds a preparation's
/// probing at two rounds' probes; what the rounds established is loaded,
/// sound for the class as before, the remaining hulls inexact.
const MAX_ROUNDS: u64 = 2;

/// Probe the groups in rounds and load their rows and bounds: round one by
/// the frozen reading alone, each later round under the parity rows the
/// round before established, sound for the class as their joint closure is,
/// while a round refutes more patterns than the one before, some hull is
/// still inexact and no system refutes the root; each round leases its
/// probes at `allowance` each. The systems of the last round are the
/// preparation's.
fn probe_rounds<Q: Quota + ?Sized>(
    theory: &Theory,
    groups: &[&proposals::Group],
    probe: &mut probes::Probe<'_>,
    scratch: &mut NarrowingScratch,
    probing: &mut Probing<'_, '_, Q>,
    statistics: &mut FrozenPreparationStatistics,
    peak: &mut u128,
) -> Result<Systems, BudgetError<LeaseRefusal<Q::Error>>> {
    let mut parity: Option<ParitySystem> = None;
    let mut cardinality: Option<CardinalitySystem> = None;
    let mut tables: Vec<(Support, Words<u64, [u64; 4]>)> = Vec::new();
    let mut refuted: Vec<u64> = vec![0; groups.len()];
    // Round one probes every group; a later round only those whose hull was
    // inexact, keeping the others' tables, up to `MAX_ROUNDS`.
    let mut pending: Vec<usize> = (0..groups.len()).collect();
    while !pending.is_empty() && statistics.rounds < MAX_ROUNDS {
        statistics.rounds += 1;
        let probes: u64 = pending
            .iter()
            .map(|&index| 1_u64 << groups[index].support.len)
            .sum();
        probing
            .lease
            .extend(probes.saturating_mul(probing.allowance));
        let mut progressed = false;
        {
            let mut meter = Meter::reserved(probing.lease);
            for &index in &pending {
                let mut counts = (0, 0);
                let forbidden = probe
                    .table(
                        groups[index],
                        parity.as_ref(),
                        &mut counts,
                        scratch,
                        probing.cancellation,
                        &mut meter,
                    )
                    .map_err(from_meter)?;
                statistics.probes += counts.0;
                progressed |= counts.1 > refuted[index];
                refuted[index] = counts.1;
                if tables.len() == index {
                    tables.push((groups[index].support, forbidden));
                } else {
                    tables[index].1 = forbidden;
                }
            }
            meter.refund();
        }
        let loaded = {
            let mut meter = Meter::reserved(probing.lease);
            let mut budget = Budget::new(
                &mut meter,
                u64::MAX,
                probing.limits.bytes,
                probing.cancellation,
            );
            let loaded = load(theory, &tables, statistics, &mut budget);
            *peak = (*peak).max(budget.peak_bytes());
            budget.finish();
            loaded?
        };
        parity = loaded.parity;
        cardinality = loaded.cardinality;
        pending = loaded.inexact;
        if !progressed
            || parity.as_ref().is_none_or(ParitySystem::root_refuted)
            || cardinality
                .as_ref()
                .is_some_and(CardinalitySystem::root_refuted)
        {
            break;
        }
    }
    statistics.refuted_patterns = refuted.iter().sum();
    Ok((parity, cardinality))
}

/// What a round loaded: the parity and counting systems, when any row or
/// bound was found, and the indices of the tables whose hull is inexact.
struct Loaded {
    parity: Option<ParitySystem>,
    cardinality: Option<CardinalitySystem>,
    inexact: Vec<usize>,
}

/// The systems of the probed groups: the parity system of each table's
/// affine hull, every equation checked against its table and eliminated to
/// echelon form, and the counting system of the tables' bounds; with the
/// indices of the tables whose hull is inexact.
fn load<Q: Quota + ?Sized>(
    theory: &Theory,
    tables: &[(Support, Words<u64, [u64; 4]>)],
    statistics: &mut FrozenPreparationStatistics,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<Loaded, BudgetError<Q::Error>> {
    if tables.is_empty() {
        return Ok(Loaded {
            parity: None,
            cardinality: None,
            inexact: Vec::new(),
        });
    }
    let mut storage = Storage::default();
    let mut inexact = Vec::new();
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
                if !proposed.exact {
                    inexact.push(group);
                }
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
    let parity = if recognized_equations.is_empty() {
        None
    } else {
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
        Some(ParitySystem::from_rows(recognized, prepared))
    };
    let cardinality = load_counts(theory, tables, statistics, &mut storage, budget)?;
    Ok(Loaded {
        parity,
        cardinality,
        inexact,
    })
}

/// The counting system of the tables' bounds, when any was found: the
/// bounds proposed from the tables, their families prepared and the root's
/// double count taken, as the outer preparation takes them.
fn load_counts<Q: Quota + ?Sized>(
    theory: &Theory,
    tables: &[(Support, Words<u64, [u64; 4]>)],
    statistics: &mut FrozenPreparationStatistics,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<Option<CardinalitySystem>, BudgetError<Q::Error>> {
    let mut counted = CardinalityStatistics::default();
    let start = budget.work();
    let bounds = budget.in_phase(Phase::Recognition, |budget| {
        counts::propose(theory, tables, &mut counted, storage, budget)
    })?;
    budget.charge(1)?;
    if bounds.is_empty() {
        storage.release(bounds)?;
        statistics.cardinality = counted;
        return Ok(None);
    }
    let prepared = budget.in_phase(Phase::Reduction, |budget| {
        cardinality::prepare(&bounds, theory.atom_count(), storage, budget)
    })?;
    counted.status = PreparationStatus::Prepared;
    counted.rows = bounds.len();
    counted.carriers = prepared.counts().carriers;
    counted.attempts = prepared.counts().attempts;
    counted.families = prepared.counts().accepted;
    counted.refused = prepared.counts().rejected;
    match &prepared {
        cardinality::Prepared::RootRefuted { .. } => counted.root_refuted = true,
        cardinality::Prepared::Families(families) => {
            counted.occurrences = families.occurrences.len();
        }
    }
    counted.preparation_work = budget.work() - start;
    counted.preparation_bytes = budget.peak_bytes();
    let system = CardinalitySystem::from_bounds(theory.clone(), bounds, prepared);
    counted.retained_bytes = system.retained_bytes();
    statistics.cardinality = counted;
    Ok(Some(system))
}
