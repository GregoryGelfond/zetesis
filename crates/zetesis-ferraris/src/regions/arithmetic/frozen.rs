//! A candidate's inner subjects: parity rows and count bounds of its
//! countermodel class, recognized by probing the closed query root under the
//! candidate's frozen reduct.
//!
//! Proposals are syntactic (`proposals`). A row's provenance is a refuted
//! probe (`probes`): a frozen narrowing from a copy of the closed root with
//! exactly a guard and a pattern decided, the premise of
//! `Zetesis.Algebra.entailed_by_refutations`. A pattern a clause of its group
//! forbids is refuted without a probe, on the proposal pass's reading of the
//! clause, a trusted computation that discharges the same premise
//! (`known_refutations`). A clause too wide for a group whose pattern cuts its
//! whole support bounds it from below by one on the same reading
//! (`Zetesis.AlgebraicRows.lower_of_clause`). A saturation guard whose probe
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
    rows,
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

/// The lease of one candidate's preparation: a floor, and one allowance per
/// probe run; crossing the lease declines the whole preparation and installs
/// nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrozenPreparationLimits {
    /// Word operations, charged to the caller's account as performed: the
    /// floor of the lease, which must cover the proposal pass, the
    /// refutations read off the clauses before probing, and the rows'
    /// loading. Before probing, the lease grants an allowance per scheduled
    /// probe equal to the root closure's work ([`ClosedQuery::closure_work`])
    /// plus one pass over the index. The floor and these allowances bound
    /// work across all phases. The receipt records the operations performed.
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
    /// Guards cut at the root by their probe, which reached the candidate or
    /// was refuted.
    pub saturation_units: u64,
    /// Clause-shaped rules the proposal pass read under the candidate's
    /// guards, whatever their support's width.
    pub clauses: u64,
    /// Clauses whose support exceeds the width a group may have: they propose
    /// no group, and one whose pattern cuts its whole support bounds it from
    /// below by one.
    pub unsupported_clauses: u64,
    /// Parity groups proposed.
    pub groups: u64,
    /// Parity groups probed, under a guard the root cuts: a unit's, or one
    /// the root cut already.
    pub probed_groups: u64,
    /// Probes run to their verdict, unit probes included; one the lease or
    /// the account interrupts is not counted.
    pub probes: u64,
    /// Patterns forbidden: by a probe that was refuted, or read off the
    /// clauses of the group, or of a group whose support it contains, before
    /// probing, which runs none for them.
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
/// parity and counting systems derived for that candidate within the supplied
/// closed region. Its descendants may reuse them throughout this query.
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

    /// Check the theory instance and candidate atoms. To reuse the prepared
    /// systems, the caller must also stay within the original closed region.
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
    let mut storage = Storage::default();
    let mut probing = Probing {
        lease,
        allowance: query.closure_work.saturating_add(query.narrower.work()),
        bytes: limits.bytes,
        cancellation,
        storage: &mut storage,
    };
    let proposals = probing.proposals(query, producers, statistics, peak)?;
    if proposals.guards.is_empty() {
        return Ok(None);
    }
    let proposal_bytes = probing.storage.bytes();
    let open_guards = proposals
        .guards
        .iter()
        .filter(|&&guard| query.region.is_open(guard))
        .count() as u64;
    probing
        .lease
        .extend(open_guards.saturating_mul(probing.allowance));
    let (mut probe, units, admitted) = probing.with_budget(peak, |storage, budget| {
        let copies = probes::Probe::copy_bytes(query.region, query.knowledge);
        storage.admit_record(copies, budget)?;
        let mut probe =
            probes::Probe::new(query.narrower, query.subject, query.region, query.knowledge);
        storage.release_record(copies)?;
        storage.admit_record(probe.live_bytes(), budget)?;
        let mut units = storage.reserve_bytes(0, budget)?;
        let mut admitted = Admitted {
            groups: storage.reserve_bytes(0, budget)?,
            wide: storage.reserve_bytes(0, budget)?,
        };
        for &guard in &proposals.guards {
            // A guard the root already cuts needs no probe: its clauses hold
            // there unconditionally. An open guard is probed held; one whose
            // narrowing reaches the candidate, or is refuted, is a unit.
            if query.region.is_open(guard) {
                let refuted = probe.run(
                    &[(guard, true)],
                    None,
                    scratch,
                    cancellation,
                    storage,
                    budget,
                )?;
                statistics.probes += 1;
                let reaches = refuted
                    || query
                        .candidate
                        .atoms()
                        .all(|atom| probe.region().is_held(atom));
                if !reaches {
                    continue;
                }
                storage.push_bytes(&mut units, guard, budget)?;
                statistics.saturation_units += 1;
            }
            admitted.admit_guard(&proposals, guard, storage, budget)?;
        }
        Ok((probe, units, admitted))
    })?;
    statistics.probed_groups = admitted.groups.len() as u64;
    let (parity, cardinality) = probe_rounds(
        theory,
        &admitted,
        &mut probe,
        scratch,
        &mut probing,
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
    probing.storage.release(admitted.groups)?;
    probing.storage.release(admitted.wide)?;
    probing
        .storage
        .release_record(probe.live_bytes() + proposal_bytes)?;
    drop(probe);
    drop(proposals);
    probing.with_budget(peak, |storage, budget| {
        publish(query, units, (parity, cardinality), storage, budget).map(Some)
    })
}

/// Transfer the accounted units and systems into the published record and
/// reserve an owned copy of the candidate's words. The theory stays shared.
fn publish<Q: Quota + ?Sized>(
    query: ClosedQuery<'_>,
    units: Vec<usize>,
    (parity, cardinality): Systems,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<FrozenPreparation, BudgetError<Q::Error>> {
    let mut embedded = size_of::<Vec<usize>>();
    if parity.is_some() {
        embedded += size_of::<ParitySystem>();
    }
    if cardinality.is_some() {
        embedded += size_of::<CardinalitySystem>();
    }
    storage.admit_record((size_of::<FrozenPreparation>() - embedded) as u128, budget)?;
    let requested = query.candidate.words.len() as u128 * size_of::<u64>() as u128;
    storage.admit_record(requested, budget)?;
    let candidate = query.candidate.try_clone().map_err(BudgetError::Source)?;
    storage.release_record(requested)?;
    storage.admit_record(
        candidate.words.capacity() as u128 * size_of::<u64>() as u128,
        budget,
    )?;
    Ok(FrozenPreparation {
        owner: query.subject.theory().clone(),
        candidate,
        units,
        parity,
        cardinality,
    })
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
    bytes: usize,
    cancellation: &'l Cancellation,
    storage: &'l mut Storage,
}

impl<'q, Q: Quota + ?Sized> Probing<'_, 'q, Q> {
    /// Publish the census after successful recognition, including when a
    /// later phase declines. Only the returned proposals remain live.
    fn proposals(
        &mut self,
        query: ClosedQuery<'_>,
        producers: &Producers,
        statistics: &mut FrozenPreparationStatistics,
        peak: &mut u128,
    ) -> Result<proposals::Proposals, BudgetError<LeaseRefusal<Q::Error>>> {
        let proposals = self.with_budget(peak, |storage, budget| {
            budget.in_phase(Phase::Recognition, |budget| {
                proposals::propose(
                    query.subject.theory(),
                    producers,
                    query.candidate,
                    query.subject.truth(),
                    query.region,
                    storage,
                    budget,
                )
            })
        })?;
        statistics.guards = proposals.counts.guards;
        statistics.clauses = proposals.counts.clauses;
        statistics.unsupported_clauses = proposals.counts.unsupported_clauses;
        statistics.groups = proposals.counts.groups;
        Ok(proposals)
    }

    /// Settle each phase's work and observed peak, including a refused
    /// request. Live storage crosses the phase boundary with its owners.
    fn with_budget<T>(
        &mut self,
        peak: &mut u128,
        operation: impl FnOnce(
            &mut Storage,
            &mut Budget<'_, '_, Leased<'q, Q>>,
        ) -> Result<T, BudgetError<LeaseRefusal<Q::Error>>>,
    ) -> Result<T, BudgetError<LeaseRefusal<Q::Error>>> {
        let mut meter = Meter::reserved(self.lease);
        let mut budget = Budget::new(&mut meter, u64::MAX, self.bytes, self.cancellation);
        let result = operation(self.storage, &mut budget);
        *peak = (*peak).max(budget.peak_bytes());
        budget.finish();
        result
    }
}

/// What the root admits under the guards it cuts: the groups to probe, and
/// the clauses too wide for a group whose pattern cuts their whole support,
/// each a lower bound of one over it.
struct Admitted<'p> {
    groups: Vec<&'p proposals::Group>,
    wide: Vec<&'p proposals::Wide>,
}

impl<'p> Admitted<'p> {
    /// Admit the groups and wide clauses of a guard the root cuts.
    fn admit_guard<Q: Quota + ?Sized>(
        &mut self,
        proposals: &'p proposals::Proposals,
        guard: usize,
        storage: &mut Storage,
        budget: &mut Budget<'_, '_, Q>,
    ) -> Result<(), BudgetError<Q::Error>> {
        for group in proposals.parity.iter().filter(|group| group.guard == guard) {
            storage.push_bytes(&mut self.groups, group, budget)?;
        }
        for clause in proposals.wide.iter().filter(|clause| clause.guard == guard) {
            storage.push_bytes(&mut self.wide, clause, budget)?;
        }
        Ok(())
    }
}

/// A probed group's table over its support: the patterns found forbidden,
/// those known before any probe among them.
struct ProbedGroup {
    support: Support,
    forbidden: probes::Table,
}

/// The patterns `table` forbids.
fn count_forbidden(table: &probes::Table) -> u64 {
    table
        .storage()
        .iter()
        .map(|word| u64::from(word.count_ones()))
        .sum()
}

/// The patterns of each group refuted before any probe runs: those a clause
/// of the group forbids, and those whose restriction to a group whose
/// support this one contains is a pattern that group's clauses forbid. The
/// region deciding such a pattern holds no member of the class: a member in
/// it lies in the closed root, so under the pattern it satisfies the clause's
/// body (the clause's rule and the chains to its base standing in the reduct,
/// their bodies true in the candidate, as the proposal pass reads them) and
/// holds the clause's guard, which no countermodel in the root holds, the
/// root having cut it already or the guard being a unit
/// (`saturation_unit`); the narrowing of that region, inside it, holds none
/// either, which is the premise `entailed_by_refutations` asks of a refuted
/// pattern, discharged here by the proposal pass's reading of the clause
/// rather than by a probe. The table records it without running the probe.
/// One word operation is charged to the floor per clause read, per pair of
/// groups compared and per pattern examined.
///
/// # Errors
/// The lease's refusal.
fn known_refutations<Q: Quota + ?Sized>(
    groups: &[&proposals::Group],
    storage: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<Vec<probes::Table>, BudgetError<Q::Error>> {
    let mut known = storage.reserve_bytes(groups.len(), budget)?;
    for group in groups {
        let support = &group.support.atoms[..group.support.len];
        known.push(Words::<u64, _>::new([0; 4]));
        let refuted = known.last_mut().expect("the current table was appended");
        for &pattern in &group.patterns {
            budget.charge(1)?;
            refuted.insert(usize::from(pattern));
        }
        for other in groups {
            budget.charge(1)?;
            if other.support.len > group.support.len || std::ptr::eq(*other, *group) {
                continue;
            }
            // Where `other`'s atoms sit in this support, if all of them do.
            storage.admit_record(
                size_of::<[usize; super::source::MAX_SUPPORT]>() as u128,
                budget,
            )?;
            let mut positions = [0_usize; super::source::MAX_SUPPORT];
            let contained = other.support.atoms[..other.support.len]
                .iter()
                .enumerate()
                .all(|(k, atom)| {
                    support.iter().position(|a| a == atom).is_some_and(|here| {
                        positions[k] = here;
                        true
                    })
                });
            if !contained {
                storage.release_record(size_of::<[usize; super::source::MAX_SUPPORT]>() as u128)?;
                continue;
            }
            let mask = positions[..other.support.len]
                .iter()
                .fold(0_usize, |mask, &here| mask | (1 << here));
            for &forbidden in &other.patterns {
                let value = (0..other.support.len)
                    .filter(|&k| forbidden & (1 << k) != 0)
                    .fold(0_usize, |value, k| value | (1 << positions[k]));
                for pattern in 0..(1_usize << group.support.len) {
                    budget.charge(1)?;
                    if pattern & mask == value {
                        refuted.insert(pattern);
                    }
                }
            }
            storage.release_record(size_of::<[usize; super::source::MAX_SUPPORT]>() as u128)?;
        }
    }
    Ok(known)
}

/// The probing rounds a preparation may run: the first by the frozen reading
/// alone, the second under the rows it established. A later round tightens
/// a hull the round before left inexact, one probe per pattern of each such
/// group at the lease allowance each, and the cap bounds a preparation's
/// probing at two rounds' probes; what the rounds established is loaded,
/// sound for the class as before, the remaining hulls inexact.
const MAX_ROUNDS: u64 = 2;

/// Probe the groups in rounds and load their rows and bounds: round one by
/// the frozen reading alone, leaving a group of one clause cutting its whole
/// support to that clause, whose table already bounds the support from below
/// by one; each later round under the parity rows the round before
/// established, sound for the class as their joint closure is,
/// while a round refutes more patterns than the one before, some hull is
/// still inexact, a parity row was loaded for a later round to narrow under
/// and no system refutes the root; each round leases its
/// probes at `allowance` each. The systems of the last round are the
/// preparation's.
fn probe_rounds<Q: Quota + ?Sized>(
    theory: &Theory,
    admitted: &Admitted<'_>,
    probe: &mut probes::Probe<'_>,
    scratch: &mut NarrowingScratch,
    probing: &mut Probing<'_, '_, Q>,
    statistics: &mut FrozenPreparationStatistics,
    peak: &mut u128,
) -> Result<Systems, BudgetError<LeaseRefusal<Q::Error>>> {
    let groups = &admitted.groups[..];
    let mut parity: Option<ParitySystem> = None;
    let mut cardinality: Option<CardinalitySystem> = None;
    let (mut tables, mut refuted, known, mut pending) =
        probing.with_budget(peak, |storage, budget| {
            let mut tables = storage.reserve_bytes(groups.len(), budget)?;
            tables.extend(groups.iter().map(|group| ProbedGroup {
                support: group.support,
                forbidden: Words::new([0; 4]),
            }));
            let mut refuted = storage.reserve_bytes(groups.len(), budget)?;
            refuted.resize(groups.len(), 0);
            let known = known_refutations(groups, storage, budget)?;
            let mut pending = storage.reserve_bytes(groups.len(), budget)?;
            pending.extend(0..groups.len());
            Ok((tables, refuted, known, pending))
        })?;
    while !pending.is_empty() && statistics.rounds < MAX_ROUNDS {
        statistics.rounds += 1;
        // The first round leaves a group of one clause cutting its whole
        // support to that clause: its table bounds the support from below by
        // one, and only a later round's rows could refute more of it.
        let settled_by_clause =
            |index: usize| statistics.rounds == 1 && groups[index].patterns == [0];
        // The probes this round runs: the patterns not known refuted.
        let probes: u64 = pending
            .iter()
            .filter(|&&index| !settled_by_clause(index))
            .map(|&index| (1_u64 << groups[index].support.len) - count_forbidden(&known[index]))
            .sum();
        probing
            .lease
            .extend(probes.saturating_mul(probing.allowance));
        let mut progressed = false;
        let cancellation = probing.cancellation;
        probing.with_budget(peak, |storage, budget| {
            for &index in &pending {
                if settled_by_clause(index) {
                    tables[index].forbidden = Words::new(*known[index].storage());
                    refuted[index] = count_forbidden(&known[index]);
                    progressed = true;
                    continue;
                }
                let found = probe.table(
                    probes::Target {
                        group: groups[index],
                        known: &known[index],
                        forbidden: &mut tables[index].forbidden,
                    },
                    parity.as_ref(),
                    scratch,
                    cancellation,
                    storage,
                    budget,
                )?;
                statistics.probes += found.run;
                progressed |= found.refuted > refuted[index];
                refuted[index] = found.refuted;
            }
            Ok(())
        })?;
        // The old systems and worklist have served their last probe. End their
        // lifetimes before loading the replacement, and retain the tables.
        probing.storage.release(pending)?;
        if let Some(previous) = parity.take() {
            probing.storage.release_record(previous.retained_bytes())?;
        }
        if let Some(previous) = cardinality.take() {
            probing.storage.release_record(previous.retained_bytes())?;
        }
        let loaded = load_round(theory, &tables, &admitted.wide, probing, statistics, peak)?;
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
    if statistics.rounds == 0 && !admitted.wide.is_empty() {
        // No group to probe: the wide clauses' bounds load alone.
        let loaded = load_round(theory, &tables, &admitted.wide, probing, statistics, peak)?;
        parity = loaded.parity;
        cardinality = loaded.cardinality;
        probing.storage.release(loaded.inexact)?;
    }
    statistics.refuted_patterns = refuted.iter().sum();
    probing.storage.release(pending)?;
    probing.storage.release(known)?;
    probing.storage.release(refuted)?;
    probing.storage.release(tables)?;
    Ok((parity, cardinality))
}

/// Load a round's systems from `tables` and the `wide` clauses' bounds, the
/// lease bounding work and the budget bytes.
fn load_round<Q: Quota + ?Sized>(
    theory: &Theory,
    tables: &[ProbedGroup],
    wide: &[&proposals::Wide],
    probing: &mut Probing<'_, '_, Q>,
    statistics: &mut FrozenPreparationStatistics,
    peak: &mut u128,
) -> Result<Loaded, BudgetError<LeaseRefusal<Q::Error>>> {
    probing.with_budget(peak, |storage, budget| {
        load(theory, tables, wide, statistics, storage, budget)
    })
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
/// echelon form, and the counting system of the tables' bounds and the wide
/// clauses'; with the indices of the tables whose hull is inexact.
fn load<Q: Quota + ?Sized>(
    theory: &Theory,
    tables: &[ProbedGroup],
    wide: &[&proposals::Wide],
    statistics: &mut FrozenPreparationStatistics,
    outer: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<Loaded, BudgetError<Q::Error>> {
    let mut inexact = outer.reserve_bytes(tables.len(), budget)?;
    if tables.is_empty() && wide.is_empty() {
        return Ok(Loaded {
            parity: None,
            cardinality: None,
            inexact,
        });
    }
    let parity = load_parity(theory, tables, &mut inexact, statistics, outer, budget)?;
    // Transfer only the surviving parity owner. Its builder can also have
    // held temporary empty evidence and vector headers on a row-free input.
    if let Some(system) = &parity {
        outer.admit_record(system.retained_bytes(), budget)?;
    }
    let mut storage = Storage::with_live_bytes(outer.bytes());
    let cardinality = load_counts(theory, tables, wide, statistics, &mut storage, budget)?;
    if let Some(system) = &cardinality {
        outer.admit_record(system.retained_bytes(), budget)?;
    }
    Ok(Loaded {
        parity,
        cardinality,
        inexact,
    })
}

/// Load the tables' implied equations into a parity system. The caller has
/// reserved one inexact-index slot per input table.
fn load_parity<Q: Quota + ?Sized>(
    theory: &Theory,
    tables: &[ProbedGroup],
    inexact: &mut Vec<usize>,
    statistics: &mut FrozenPreparationStatistics,
    outer: &Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<Option<ParitySystem>, BudgetError<Q::Error>> {
    let mut storage = Storage::with_live_bytes(outer.bytes());
    let (facts, mut recognized_equations, counts) =
        budget.in_phase(Phase::Recognition, |budget| {
            storage.admit_record(
                (size_of::<rows::Recognized>()
                    - size_of::<CheckedRootFacts>()
                    - size_of::<Vec<rows::Equation>>()
                    - size_of::<Vec<source::Clause>>()) as u128,
                budget,
            )?;
            let facts = CheckedRootFacts::none(theory, &mut storage, budget)?;
            let mut equations = storage.reserve(0, budget)?;
            let mut counts = rows::Counts::default();
            for (group, table) in tables.iter().enumerate() {
                let fixed = size_of::<hull::Hull>() as u128;
                storage.admit_record(fixed, budget)?;
                let proposed =
                    storage.with_record(hull::SCRATCH_BYTES as u128, budget, |_, budget| {
                        rows::propose(
                            table.support,
                            table.forbidden.storage(),
                            &mut counts,
                            budget,
                        )
                    })?;
                if !proposed.exact {
                    inexact.push(group);
                }
                for &local in proposed.equations() {
                    let words =
                        rows::materialize(theory, table.support, local, &mut storage, budget)?;
                    budget.charge(1)?; // the constant
                    let candidate = rows::Equation {
                        words,
                        constant: local.constant,
                    };
                    let valid = source::implied_by_table(
                        table.forbidden.storage(),
                        table.support.len,
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
    storage.admit_record(size_of::<Vec<source::Clause>>() as u128, budget)?;
    let recognized = rows::Recognized {
        facts,
        clauses: Vec::new(),
        equations: recognized_equations,
        counts,
    };
    statistics.parity = ParityStatistics::from_preparation(&recognized, &prepared, budget);
    Ok(Some(ParitySystem::from_rows(recognized, prepared)))
}

/// The counting system of the tables' bounds and the wide clauses', when
/// any was found: the bounds proposed, their families prepared and the
/// root's double count taken, as the outer preparation takes them.
fn load_counts<Q: Quota + ?Sized>(
    theory: &Theory,
    tables: &[ProbedGroup],
    wide: &[&proposals::Wide],
    statistics: &mut FrozenPreparationStatistics,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<Option<CardinalitySystem>, BudgetError<Q::Error>> {
    let mut counted = CardinalityStatistics::default();
    let start = budget.work();
    let bounds = budget.in_phase(Phase::Recognition, |budget| {
        counts::propose(theory, tables, wide, &mut counted, storage, budget)
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
    storage.admit_record(
        (size_of::<CardinalitySystem>()
            - size_of::<cardinality::Prepared>()
            - size_of::<Vec<super::cardinality_source::Bound>>()) as u128,
        budget,
    )?;
    let system = CardinalitySystem::from_bounds(theory.clone(), bounds, prepared);
    counted.retained_bytes = system.retained_bytes();
    statistics.cardinality = counted;
    Ok(Some(system))
}
