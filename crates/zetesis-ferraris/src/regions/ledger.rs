//! Learned rows: bounded, exact-original reuse of consequences of the theory.
//!
//! Every row says that every answer set of its owner's consequence class that
//! satisfies its signed atom conditions satisfies each of its signed outputs
//! (`LearnedRegions.RowOf`). Rows are independent of the region that supplied
//! them. Deleting a row loses pruning only. No rejected candidate, partial
//! closure, objective bound or frozen reduct supplies a certificate, and no row
//! enters a membership certificate.
//!
//! A ledger is a tier: one walk's or worker's rows. A [`LearningStore`] holds
//! the rows one solve's streams share, by class; tiers are seeded from it and
//! deliver to it at their walks' boundaries, never inside a narrowing. Rows
//! reach a ledger only from the narrower's own sources, today the founded
//! collector, or from the store those sources filled: no caller supplies,
//! edits or reads a row.

use std::sync::atomic::{AtomicU64, Ordering};

use super::founded::explanations::{self, Explanation, Input, Limits};
use crate::regions::{SupportExplanationStatistics, Theory, Work};
use zetesis_algebra::{Stop, words::Words};

mod row;

pub(in crate::regions) use row::OutputCursor;
use row::Row;

#[cfg(test)]
mod tests;

/// Up to this many rows the matching step scans every row; above it, matching
/// reads the occurrence index for the atoms decided since the last step.
const SCAN_ROWS: usize = u64::BITS as usize;

/// The largest tier [`LearningLimits::for_allowance`] derives.
const MAX_TIER_ROWS: usize = 4_096;

/// Allowances for learned rows: per tier (one walk's or worker's ledger), per
/// store, per source, and what and when a tier shares.
///
/// The defaults are the limits the founded source has always had: 64 rows,
/// 256 capture attempts, 131,072 capture reads over a tier's lifetime, 4,096
/// reads and 65,536 bytes per capture, and rows of at most 16 conditions and
/// 64 outputs. A tier shares rows of at most 8 conditions, or rows that have
/// hit twice, and delivers every 4,096 regions its walk visits.
/// [`LearningLimits::for_allowance`] raises the tier and store capacities
/// with the memory allowance and never lowers any limit below these.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LearningLimits {
    /// Rows one tier retains. A full tier evicts by decayed hits.
    pub tier_rows: usize,
    /// Rows the store retains per consequence class.
    pub store_rows: usize,
    /// The founded source's reserved share of every tier: eviction never takes
    /// a founded row while the tier holds this many or fewer of them.
    pub founded_floor: usize,
    /// Signed conditions one founded row may require.
    pub founded_conditions: usize,
    /// Atoms one founded row may exclude.
    pub founded_outputs: usize,
    /// Founded capture attempts over a tier's lifetime.
    pub founded_attempts: usize,
    /// Founded discovery, collection and comparison reads over a tier's life.
    pub founded_work: u64,
    /// Reads of one founded capture.
    pub founded_event_work: u64,
    /// Workspace bytes of one founded capture.
    pub founded_event_bytes: u64,
    /// Seeding and delivery reads over a tier's lifetime. Exhausting them
    /// stops the tier's sharing; its own rows still prune its walk.
    pub share_work: u64,
    /// The quality filter: a tier shares a row with at most this many
    /// conditions, or one that has hit at least `share_hits` times.
    pub share_conditions: usize,
    /// See `share_conditions`.
    pub share_hits: u64,
    /// The publication trigger: a walk delivers its tier each time it has
    /// visited this many regions since the tier last delivered, as well as at
    /// the walk's end. Zero never triggers.
    pub publish_regions: u64,
}

impl LearningLimits {
    /// The defaults: the founded source's limits as they have always been.
    pub const DEFAULT: Self = Self {
        tier_rows: 64,
        store_rows: 256,
        founded_floor: 64,
        founded_conditions: 16,
        founded_outputs: 64,
        founded_attempts: 256,
        founded_work: 131_072,
        founded_event_work: 4_096,
        founded_event_bytes: 65_536,
        share_work: 131_072,
        share_conditions: 8,
        share_hits: 2,
        publish_regions: 4_096,
    };

    /// The limits for a candidate-memory allowance of `bytes`: one tier row
    /// for every MiB of it, between the default's 64 rows and 4,096, and a
    /// store four times a tier. Every other limit keeps its default, so an
    /// allowance of 64 MiB or less keeps every default.
    #[must_use]
    pub const fn for_allowance(bytes: usize) -> Self {
        let defaults = Self::DEFAULT;
        let rows = bytes >> 20;
        let rows = if rows < defaults.tier_rows {
            defaults.tier_rows
        } else if rows > MAX_TIER_ROWS {
            MAX_TIER_ROWS
        } else {
            rows
        };
        Self {
            tier_rows: rows,
            store_rows: rows.saturating_mul(4),
            ..defaults
        }
    }
}

impl Default for LearningLimits {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// The owner of learned rows: the consequence class every row it holds
/// belongs to, and the allowances of its tiers.
///
/// Its one class today is the original theory instance: every row holds for
/// every answer set of the theory, as the founded source's rows do
/// (`LearnedRegions.founded_row`). Such rows hold under any candidate
/// restriction, objective bound or region filter, by
/// `LearnedRegions.row_of_narrower_class`, so a walk under a filter learns and
/// applies them as rows of the theory.
#[derive(Clone, Debug)]
pub struct LearningOwner {
    /// The carrier subject reads this theory ([`crate::OriginalSubject::carrier`]).
    pub(in crate::regions) theory: Theory,
    limits: LearningLimits,
}

impl LearningOwner {
    /// The owner of rows of `theory`'s answer sets, under `limits`.
    #[must_use]
    pub fn theory(theory: &Theory, limits: LearningLimits) -> Self {
        Self {
            theory: theory.clone(),
            limits,
        }
    }

    /// The allowances of this owner's tiers.
    #[must_use]
    pub const fn limits(&self) -> &LearningLimits {
        &self.limits
    }

    /// True when this owner's class is the answer sets of `theory`, the
    /// same instance.
    #[must_use]
    pub fn owns(&self, theory: &Theory) -> bool {
        self.same_class(theory)
    }

    fn same_class(&self, theory: &Theory) -> bool {
        self.theory.same_instance(theory)
    }
}

/// Identities of stores, so a tier's seeding mark names the store it read.
static STORES: AtomicU64 = AtomicU64::new(0);

/// The rows one solve's streams share, by consequence class.
///
/// Its public surface is creating an empty store and its counts: rows enter
/// only by a tier's delivery and leave only by seeding a tier, and no caller
/// supplies, edits or reads one. The store charges no work of its own; the
/// tier that seeds or delivers pays for every read.
#[derive(Debug)]
pub struct LearningStore {
    /// Distinct among the stores of a process.
    id: u64,
    classes: Vec<Ledger>,
    deliveries: u64,
    seeded: u64,
}

impl Default for LearningStore {
    fn default() -> Self {
        Self {
            id: STORES.fetch_add(1, Ordering::Relaxed),
            classes: Vec::new(),
            deliveries: 0,
            seeded: 0,
        }
    }
}

impl LearningStore {
    /// An empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Rows held, over every class.
    #[must_use]
    pub fn len(&self) -> usize {
        self.classes.iter().map(Ledger::len).sum()
    }

    /// True when the store holds no row.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Rows tiers delivered and the store retained, over its life.
    #[must_use]
    pub const fn deliveries(&self) -> u64 {
        self.deliveries
    }

    /// Rows seeding copied into tiers, over the store's life.
    #[must_use]
    pub const fn seeded(&self) -> u64 {
        self.seeded
    }

    fn class(&self, owner: &LearningOwner) -> Option<&Ledger> {
        self.classes.iter().find(|ledger| {
            ledger
                .owner
                .as_ref()
                .is_some_and(|o| o.same_class(&owner.theory))
        })
    }

    fn class_mut(&mut self, owner: &LearningOwner) -> Result<&mut Ledger, Stop> {
        let position = self.classes.iter().position(|ledger| {
            ledger
                .owner
                .as_ref()
                .is_some_and(|o| o.same_class(&owner.theory))
        });
        if let Some(position) = position {
            return Ok(&mut self.classes[position]);
        }
        self.classes
            .try_reserve_exact(1)
            .map_err(|_| Stop::Allocation)?;
        // A class holds up to the store's rows, all of them evictable.
        let mut limits = owner.limits;
        limits.tier_rows = limits.store_rows;
        limits.founded_floor = 0;
        let mut ledger = Ledger::default();
        ledger.prepare(&LearningOwner {
            theory: owner.theory.clone(),
            limits,
        });
        self.classes.push(ledger);
        let last = self.classes.len() - 1;
        Ok(&mut self.classes[last])
    }
}

/// A consequence of one certified row under the current signed atom masks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::regions) enum Deduction {
    None,
    /// Every condition holds: each output must hold.
    ForceOutputs,
    /// Exactly one condition is open and an output's negation is decided: the
    /// open condition must fail, so `atom` takes `value`.
    Require {
        atom: usize,
        value: bool,
    },
}

/// Sorted, duplicate-free signed conditions and signed outputs of one row.
#[derive(Debug, PartialEq, Eq)]
pub(in crate::regions) struct Literals {
    pub(in crate::regions) conditions: Vec<(usize, bool)>,
    pub(in crate::regions) outputs: Vec<(usize, bool)>,
}

impl Literals {
    /// Owned buffers; the caller accounts for its containing row/header.
    pub(in crate::regions) fn retained_bytes(&self) -> u128 {
        (self.conditions.capacity() + self.outputs.capacity()) as u128
            * size_of::<(usize, bool)>() as u128
    }

    /// The founded collector's unsupported atoms as absent outputs under its
    /// conditions (`LearnedRegions.founded_row`).
    fn from_founded(explanation: Explanation, peak: &mut u128) -> Result<Self, Stop> {
        let mut outputs = Vec::new();
        outputs
            .try_reserve_exact(explanation.unsupported.len())
            .map_err(|_| Stop::Allocation)?;
        *peak = (*peak).max(
            explanation.retained_bytes()
                + outputs.capacity() as u128 * size_of::<(usize, bool)>() as u128,
        );
        outputs.extend(explanation.unsupported.iter().map(|&atom| (atom, false)));
        Ok(Self {
            conditions: explanation.conditions,
            outputs,
        })
    }
}

/// Which source learned a row. The founded floor protects founded rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Source {
    Founded,
}

/// A retained row with what eviction and sharing read about it.
#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq, Eq))]
struct Entry {
    row: Row,
    source: Source,
    /// Refutations and deductions, halved each epoch.
    hits: u64,
    /// Hits since the tier last delivered this row.
    unshared: u64,
    /// Retention order: older rows have smaller values.
    born: u64,
    /// True once delivered, until the row changes.
    delivered: bool,
}

/// A row offered to retention, with what the ledger keeps about it.
struct Proposed {
    literals: Literals,
    source: Source,
    /// The hit count the row starts with.
    hits: u64,
    /// The most outputs a union with an existing row may give it.
    maximum_outputs: usize,
}

/// What comparing a proposed row with the retained rows established.
enum Compared {
    /// The retained row at this position gives the proposed row.
    Subsumed(usize),
    /// An allowance ran out before every comparison finished.
    Declined,
    /// The rows, one bit per position, the proposed row replaces: identical
    /// conditions whose outputs it now includes, or rows it dominates.
    Replaces(Words<u64>),
}

/// Where a retention left a proposed row.
enum Retained {
    /// Added, or replaced dominated rows, at this position.
    Added(usize),
    /// An existing row at this position already gives it.
    Subsumed(usize),
    /// Declined: an allowance ran out, or the tier is full of protected rows.
    Declined,
}

#[derive(Debug)]
pub(in crate::regions) struct Ledger {
    owner: Option<LearningOwner>,
    rows: Vec<Entry>,
    born: u64,
    /// Rows retained since the last halving of the hit counts.
    epoch: usize,
    attempts: usize,
    remaining: u64,
    sharing: u64,
    peak_bytes: u128,
    /// (atom, row position) for every atom of every row, sorted. Complete
    /// unless `stale`; the first indexed step after a change rebuilds it.
    postings: Vec<(usize, usize)>,
    /// The rows changed since the postings were built.
    stale: bool,
    /// The next matching step tests every row: a narrowing began, whose
    /// knowledge earlier steps never read, or the rows changed.
    full: bool,
    /// Matching steps that read the index, for tests of the route.
    #[cfg(test)]
    indexed_steps: u64,
    /// Founded rows held, which the founded floor protects.
    founded: usize,
    /// Rows evicted over the tier's life.
    evicted: u64,
    /// The store this tier last read completely, and that store's retention
    /// count for the tier's class then: seeding reads only rows retained
    /// since, which carry larger retention orders.
    seeded_from: Option<(u64, u64)>,
}

impl Default for Ledger {
    /// An unowned tier with the default allowances, which its owner's
    /// preparation replaces.
    fn default() -> Self {
        let limits = LearningLimits::default();
        Self {
            owner: None,
            rows: Vec::new(),
            born: 0,
            epoch: 0,
            attempts: 0,
            remaining: limits.founded_work,
            sharing: limits.share_work,
            peak_bytes: 0,
            postings: Vec::new(),
            stale: true,
            full: true,
            #[cfg(test)]
            indexed_steps: 0,
            founded: 0,
            evicted: 0,
            seeded_from: None,
        }
    }
}

impl Ledger {
    /// Only an owner's carrier subject, seeding and delivery call this.
    /// Restriction, bound and frozen visits suspend use rather than
    /// discarding the owner's rows; an owner of a different class clears
    /// them, and an owner of the same class supplies its limits.
    pub(in crate::regions) fn prepare(&mut self, owner: &LearningOwner) {
        let Some(current) = &mut self.owner else {
            self.adopt(owner);
            return;
        };
        if !current.same_class(&owner.theory) {
            self.adopt(owner);
        } else if current.limits != owner.limits {
            current.limits = owner.limits;
        }
    }

    /// Clear the tier for a new class.
    fn adopt(&mut self, owner: &LearningOwner) {
        self.rows.clear();
        self.postings.clear();
        self.founded = 0;
        self.evicted = 0;
        self.seeded_from = None;
        self.owner = Some(owner.clone());
        self.attempts = 0;
        self.epoch = 0;
        self.remaining = owner.limits.founded_work;
        self.sharing = owner.limits.share_work;
        self.stale = true;
        self.full = true;
        self.peak_bytes = self.retained_bytes();
    }

    /// A narrowing begins: its first matching step tests every row, since
    /// the knowledge it starts from was not read by this tier's last step.
    pub(in crate::regions) fn begin_narrowing(&mut self) {
        self.full = true;
    }

    pub(in crate::regions) fn retained_bytes(&self) -> u128 {
        self.rows.capacity() as u128 * size_of::<Entry>() as u128
            + self
                .rows
                .iter()
                .map(|entry| entry.row.retained_bytes())
                .sum::<u128>()
            + self.postings.capacity() as u128 * size_of::<(usize, usize)>() as u128
    }

    pub(in crate::regions) fn peak_bytes(&self) -> u128 {
        self.peak_bytes.max(self.retained_bytes())
    }

    pub(in crate::regions) fn len(&self) -> usize {
        self.rows.len()
    }

    pub(in crate::regions) const fn evicted(&self) -> u64 {
        self.evicted
    }

    #[cfg(test)]
    pub(in crate::regions) fn output_count(&self, row: usize) -> usize {
        self.rows[row].row.output_count()
    }

    /// A row's outputs when every one is an absence, as founded rows are.
    #[cfg(test)]
    pub(in crate::regions) fn absent_outputs(&self, row: usize) -> Vec<usize> {
        self.rows[row]
            .row
            .outputs()
            .map(|(atom, present)| {
                assert!(!present, "a founded row's outputs are absences");
                atom
            })
            .collect()
    }

    /// Retain a founded row under the capture allowance, as capture does
    /// after collection; true when the row was added.
    #[cfg(test)]
    pub(in crate::regions) fn retain_founded(
        &mut self,
        literals: Literals,
        work: &mut Work<'_>,
        peak: &mut u128,
    ) -> Result<bool, Stop> {
        let mut remaining = self.remaining;
        let result = self.retain(
            Proposed {
                literals,
                source: Source::Founded,
                hits: 0,
                maximum_outputs: self.limits().founded_outputs,
            },
            &mut remaining,
            work,
            peak,
        );
        self.remaining = remaining;
        Ok(matches!(result?, Retained::Added(_)))
    }

    fn limits(&self) -> LearningLimits {
        self.owner
            .as_ref()
            .map_or_else(LearningLimits::default, |owner| owner.limits)
    }

    /// Whether atom decisions must be recorded for the occurrence index.
    pub(in crate::regions) fn records_decisions(&self) -> bool {
        self.rows.len() > SCAN_ROWS
    }

    /// The rows a matching step tests: every row when the tier is small, at
    /// a narrowing's first step and after the rows changed; otherwise the
    /// rows that mention an atom decided since the last step. The first
    /// indexed step after a change rebuilds the index, at one read per
    /// posting. Positions are ascending and distinct.
    ///
    /// Within one narrowing knowledge only grows, and a row's deduction reads
    /// only its own atoms. A row none of whose atoms was decided since the
    /// last step therefore has the deduction that step already applied, so
    /// the index loses no deduction.
    pub(in crate::regions) fn candidates(
        &mut self,
        decided: &[usize],
        rows: &mut Vec<usize>,
        work: &mut Work<'_>,
    ) -> Result<(), Stop> {
        rows.clear();
        if self.rows.len() <= SCAN_ROWS || self.full {
            self.full = false;
            rows.try_reserve(self.rows.len())
                .map_err(|_| Stop::Allocation)?;
            rows.extend(0..self.rows.len());
            return Ok(());
        }
        if self.stale {
            self.rebuild_postings(work)?;
        }
        #[cfg(test)]
        {
            self.indexed_steps += 1;
        }
        for &atom in decided {
            work.tick()?;
            let start = self.postings.partition_point(|&(a, _)| a < atom);
            for &(a, position) in &self.postings[start..] {
                if a != atom {
                    break;
                }
                work.tick()?;
                rows.try_reserve(1).map_err(|_| Stop::Allocation)?;
                rows.push(position);
            }
        }
        rows.sort_unstable();
        rows.dedup();
        Ok(())
    }

    /// Record a refutation or deduction by a row.
    pub(in crate::regions) fn hit(&mut self, row: usize) {
        let entry = &mut self.rows[row];
        entry.hits = entry.hits.saturating_add(1);
        entry.unshared = entry.unshared.saturating_add(1);
    }

    pub(in crate::regions) fn next_publication_output(
        &self,
        row: usize,
        cursor: &mut OutputCursor,
        held: &[u64],
        cut: &[u64],
        work: &mut Work<'_>,
    ) -> Result<Option<(usize, bool)>, Stop> {
        self.rows[row]
            .row
            .next_publication_output(cursor, held, cut, work)
    }

    pub(in crate::regions) fn can_capture(&self) -> bool {
        let limits = self.limits();
        self.attempts < limits.founded_attempts
            && self.remaining != 0
            && (self.rows.len() < limits.tier_rows || self.can_evict())
    }

    /// Whether a full tier has a row eviction may take.
    fn can_evict(&self) -> bool {
        self.rows.len() > self.founded || self.founded > self.limits().founded_floor
    }

    /// Discovery, subsumption and conversion share the optional capture allowance.
    /// Its exhaustion declines retention; a real quota stop still propagates.
    pub(in crate::regions) fn capture_read(&mut self, work: &mut Work<'_>) -> Result<bool, Stop> {
        capture_read(&mut self.remaining, work)
    }

    /// A complete condition forces every output. If an output's negation is
    /// decided and exactly one condition is open, that condition must instead
    /// fail. An opposite premise or two open premises supply no deduction.
    /// This scans scalar entries or compact word groups and allocates no
    /// matching state.
    pub(in crate::regions) fn deduction(
        &self,
        row: usize,
        held: &[u64],
        cut: &[u64],
        work: &mut Work<'_>,
    ) -> Result<Deduction, Stop> {
        self.rows[row].row.deduction(held, cut, work)
    }

    pub(in crate::regions) fn capture(
        &mut self,
        input: Input<'_>,
        atom: usize,
        work: &mut Work<'_>,
        counts: &mut SupportExplanationStatistics,
    ) -> Result<(), Stop> {
        if !self.can_capture() {
            return Ok(());
        }
        let limits = self.limits();
        self.attempts += 1;
        counts.attempts += 1;
        let mut temporary_peak = 0;
        let before = work.spent();
        let result = explanations::collect(
            input,
            atom,
            Limits {
                atoms: limits.founded_outputs,
                literals: limits.founded_conditions,
                visits: self.remaining.min(limits.founded_event_work),
                bytes: u128::from(limits.founded_event_bytes),
            },
            work,
            &mut temporary_peak,
        );
        self.remaining = self.remaining.saturating_sub(work.spent() - before);
        self.peak_bytes = self.peak_bytes.max(self.retained_bytes() + temporary_peak);
        let Some(explanation) = result? else {
            counts.declined += 1;
            return Ok(());
        };
        let literals = Literals::from_founded(explanation, &mut temporary_peak)?;
        let retained = self.retained_bytes();
        let mut remaining = self.remaining;
        let result = self.retain(
            Proposed {
                literals,
                source: Source::Founded,
                hits: 0,
                maximum_outputs: limits.founded_outputs,
            },
            &mut remaining,
            work,
            &mut temporary_peak,
        );
        self.remaining = remaining;
        self.peak_bytes = self
            .peak_bytes
            .max(retained + temporary_peak)
            .max(self.retained_bytes());
        match result? {
            Retained::Added(_) => counts.certified += 1,
            Retained::Subsumed(_) | Retained::Declined => counts.declined += 1,
        }
        Ok(())
    }

    /// Copy the store's rows of this tier's class into the tier by retention,
    /// so a row the tier holds or subsumes is not added twice. Each copied
    /// row starts at the store's hit count. A tier that read this store
    /// completely before reads only the rows retained there since, at one
    /// read for each row it passes over. Charges the tier's sharing
    /// allowance; its exhaustion ends seeding, never the run.
    pub(in crate::regions) fn seed(
        &mut self,
        store: &mut LearningStore,
        work: &mut Work<'_>,
    ) -> Result<(), Stop> {
        let Some(owner) = self.owner.clone() else {
            return Ok(());
        };
        let Some(class) = store.class(&owner) else {
            return Ok(());
        };
        let read = match self.seeded_from {
            Some((id, born)) if id == store.id => born,
            _ => 0,
        };
        let mark = (store.id, class.born);
        let mut transfer = Transfer {
            sharing: self.sharing,
            moved: 0,
            peak: 0,
        };
        let result = self.seed_rows(
            &class.rows,
            read,
            owner.limits.founded_outputs,
            &mut transfer,
            work,
        );
        // A stopped seeding keeps the rows it completed and pays for its
        // prefix, as a finished one does.
        self.sharing = transfer.sharing;
        self.peak_bytes = self.peak_bytes.max(self.retained_bytes() + transfer.peak);
        store.seeded += transfer.moved;
        if result? {
            self.seeded_from = Some(mark);
        }
        Ok(())
    }

    /// Retain each of `rows` retained after `read`, each starting at its
    /// store hit count; true when every row was read.
    fn seed_rows(
        &mut self,
        rows: &[Entry],
        read: u64,
        founded_outputs: usize,
        transfer: &mut Transfer,
        work: &mut Work<'_>,
    ) -> Result<bool, Stop> {
        for entry in rows {
            if entry.born <= read {
                work.tick()?;
                continue;
            }
            if !capture_read(&mut transfer.sharing, work)? {
                return Ok(false);
            }
            let Some(literals) =
                entry
                    .row
                    .to_literals(&mut transfer.sharing, work, &mut transfer.peak)?
            else {
                return Ok(false);
            };
            let maximum_outputs = literals.outputs.len().max(founded_outputs);
            match self.retain(
                Proposed {
                    literals,
                    source: entry.source,
                    hits: entry.hits,
                    maximum_outputs,
                },
                &mut transfer.sharing,
                work,
                &mut transfer.peak,
            )? {
                Retained::Added(position) => {
                    // A seeded row is the store's; the tier delivers only
                    // what it learns or what hits after seeding.
                    self.rows[position].delivered = true;
                    transfer.moved += 1;
                }
                Retained::Subsumed(_) => {}
                Retained::Declined => return Ok(false),
            }
        }
        Ok(true)
    }

    /// Deliver the rows retained or hit since the last delivery that pass the
    /// quality filter to the store, under this tier's class. A delivered
    /// row's hits since its last delivery add to the store's count for the
    /// row that keeps them. Each row examined costs one read; delivering one
    /// is charged to the tier's sharing allowance, whose exhaustion ends
    /// delivery, never the run.
    pub(in crate::regions) fn deliver(
        &mut self,
        store: &mut LearningStore,
        work: &mut Work<'_>,
    ) -> Result<(), Stop> {
        let Some(owner) = self.owner.clone() else {
            return Ok(());
        };
        let class = store.class_mut(&owner)?;
        let mut transfer = Transfer {
            sharing: self.sharing,
            moved: 0,
            peak: 0,
        };
        let result = deliver_rows(&mut self.rows, class, owner.limits, &mut transfer, work);
        // A stopped delivery keeps the rows it completed and pays for its
        // prefix, as a finished one does.
        self.sharing = transfer.sharing;
        self.peak_bytes = self.peak_bytes.max(self.retained_bytes() + transfer.peak);
        store.deliveries += transfer.moved;
        result
    }

    /// The row a full tier gives up for a new row: the fewest decayed hits,
    /// oldest first among ties, never a founded row while the tier holds no
    /// more founded rows than its floor.
    fn evictable(&self) -> Option<usize> {
        let protected = self.founded <= self.limits().founded_floor;
        self.rows
            .iter()
            .enumerate()
            .filter(|(_, entry)| !protected || entry.source != Source::Founded)
            .min_by_key(|(_, entry)| (entry.hits, entry.born))
            .map(|(position, _)| position)
    }

    /// Consolidate a completed row without changing any retained row until
    /// every comparison, allocation and publication visit has succeeded. Row
    /// positions are private to a matching pass; retention runs between
    /// passes. `remaining` is the allowance the caller charges: the founded
    /// capture's, or the tier's sharing allowance.
    fn retain(
        &mut self,
        proposed: Proposed,
        remaining: &mut u64,
        work: &mut Work<'_>,
        peak: &mut u128,
    ) -> Result<Retained, Stop> {
        let Proposed {
            mut literals,
            source,
            hits,
            maximum_outputs,
        } = proposed;
        let rows = self.rows.len();
        let replaced = match self.compare(&mut literals, maximum_outputs, remaining, work, peak)? {
            Compared::Replaces(replaced) => replaced,
            Compared::Subsumed(position) => return Ok(Retained::Subsumed(position)),
            Compared::Declined => return Ok(Retained::Declined),
        };
        let any = replaced.iter().any(|&word| word != 0);
        // A full tier with nothing to replace evicts before it adds.
        let victim = if !any && rows >= self.limits().tier_rows {
            let Some(victim) = self.evictable() else {
                return Ok(Retained::Declined);
            };
            Some(victim)
        } else {
            None
        };
        let Some(row) = Row::prepare(literals, remaining, work, peak)? else {
            return Ok(Retained::Declined);
        };
        // Pay for the bounded compaction before its first mutation. A stop
        // during this admission preserves all previous rows.
        if any || victim.is_some() {
            for _ in 0..rows {
                if !capture_read(remaining, work)? {
                    return Ok(Retained::Declined);
                }
            }
        }
        self.born += 1;
        let entry = Entry {
            row,
            source,
            hits,
            unshared: 0,
            born: self.born,
            delivered: false,
        };
        let position = self.place(entry, &replaced, victim)?;
        self.settle();
        Ok(Retained::Added(position))
    }

    /// Compare a proposed row with every retained row, changing neither: an
    /// existing row that subsumes it, or the rows it replaces once it holds
    /// the outputs of every row with identical conditions.
    fn compare(
        &self,
        literals: &mut Literals,
        maximum_outputs: usize,
        remaining: &mut u64,
        work: &mut Work<'_>,
        peak: &mut u128,
    ) -> Result<Compared, Stop> {
        let rows = self.rows.len();
        let mut replaced = Words::new(Vec::new());
        replaced
            .storage_mut()
            .try_reserve_exact(rows.div_ceil(64))
            .map_err(|_| Stop::Allocation)?;
        replaced.storage_mut().resize(rows.div_ceil(64), 0_u64);
        for position in 0..rows {
            if !capture_read(remaining, work)? {
                return Ok(Compared::Declined);
            }
            let row = &self.rows[position].row;
            let Some(subsumed) = row.subsumes(literals, remaining, work)? else {
                return Ok(Compared::Declined);
            };
            if subsumed {
                return Ok(Compared::Subsumed(position));
            }
            let Some(combined) =
                row.union_outputs(literals, maximum_outputs, remaining, work, peak)?
            else {
                return Ok(Compared::Declined);
            };
            if combined {
                replaced.insert(position);
            }
        }
        // A union can dominate a row examined before its outputs were added.
        // Compare the completed proposed row against every other row.
        for position in 0..rows {
            if replaced.contains(position) {
                continue;
            }
            if !capture_read(remaining, work)? {
                return Ok(Compared::Declined);
            }
            let Some(dominated) = self.rows[position]
                .row
                .subsumed_by(literals, remaining, work)?
            else {
                return Ok(Compared::Declined);
            };
            if dominated {
                replaced.insert(position);
            }
        }
        Ok(Compared::Replaces(replaced))
    }

    /// Publish a completed entry in the evicted row's position, in the first
    /// replaced row's position with the other replaced rows removed, or after
    /// every row.
    fn place(
        &mut self,
        entry: Entry,
        replaced: &Words<u64>,
        victim: Option<usize>,
    ) -> Result<usize, Stop> {
        if let Some(victim) = victim {
            self.rows[victim] = entry;
            self.evicted += 1;
            return Ok(victim);
        }
        let first = replaced.next_from(0);
        if let Some(first) = first {
            self.rows[first] = entry;
            let mut position = 0;
            self.rows.retain(|_| {
                let keep = position == first || !replaced.contains(position);
                position += 1;
                keep
            });
            return Ok(first);
        }
        self.rows
            .try_reserve_exact(1)
            .map_err(|_| Stop::Allocation)?;
        self.peak_bytes = self
            .peak_bytes
            .max(self.retained_bytes() + size_of::<Entry>() as u128);
        self.rows.push(entry);
        Ok(self.rows.len() - 1)
    }

    /// After a retention: recount the founded rows, halve every row's hits once
    /// the tier has retained as many rows as it may hold, and send the next
    /// matching step through every row, since positions changed.
    fn settle(&mut self) {
        self.founded = self
            .rows
            .iter()
            .filter(|entry| entry.source == Source::Founded)
            .count();
        self.epoch += 1;
        if self.epoch >= self.limits().tier_rows {
            self.epoch = 0;
            for entry in &mut self.rows {
                entry.hits /= 2;
            }
        }
        // The first indexed step after the full scan rebuilds the index.
        self.postings.clear();
        self.stale = true;
        self.full = true;
    }

    /// Rebuild the occurrence index, one read per posting. A stop leaves
    /// `stale` set, so a partial index is never read.
    fn rebuild_postings(&mut self, work: &mut Work<'_>) -> Result<(), Stop> {
        self.postings.clear();
        let count = self.rows.iter().fold(0_usize, |count, entry| {
            let (conditions, outputs) = entry.row.lengths();
            count.saturating_add(conditions).saturating_add(outputs)
        });
        self.postings
            .try_reserve_exact(count)
            .map_err(|_| Stop::Allocation)?;
        self.peak_bytes = self.peak_bytes.max(self.retained_bytes());
        for (position, entry) in self.rows.iter().enumerate() {
            for atom in entry.row.atoms() {
                work.tick()?;
                self.postings.push((atom, position));
            }
        }
        self.postings.sort_unstable();
        self.stale = false;
        Ok(())
    }
}

/// What one seeding or delivery has spent and moved so far: its sharing
/// allowance, the rows it added and its transient peak. The tier records it
/// whether the transfer finishes or stops.
struct Transfer {
    sharing: u64,
    moved: u64,
    peak: u128,
}

/// Deliver each of a tier's `rows` new or hit since its last delivery that
/// passes the quality filter into `class`, one read per row examined.
fn deliver_rows(
    rows: &mut [Entry],
    class: &mut Ledger,
    limits: LearningLimits,
    transfer: &mut Transfer,
    work: &mut Work<'_>,
) -> Result<(), Stop> {
    for entry in rows {
        work.tick()?;
        if entry.delivered && entry.unshared == 0 {
            continue;
        }
        let (conditions, _) = entry.row.lengths();
        if conditions > limits.share_conditions && entry.hits < limits.share_hits {
            continue;
        }
        if !capture_read(&mut transfer.sharing, work)? {
            return Ok(());
        }
        let Some(literals) =
            entry
                .row
                .to_literals(&mut transfer.sharing, work, &mut transfer.peak)?
        else {
            return Ok(());
        };
        let maximum_outputs = literals.outputs.len().max(limits.founded_outputs);
        let hits = entry.unshared;
        match class.retain(
            Proposed {
                literals,
                source: entry.source,
                hits,
                maximum_outputs,
            },
            &mut transfer.sharing,
            work,
            &mut transfer.peak,
        )? {
            Retained::Added(_) => transfer.moved += 1,
            Retained::Subsumed(position) => {
                let kept = &mut class.rows[position];
                kept.hits = kept.hits.saturating_add(hits);
            }
            Retained::Declined => return Ok(()),
        }
        entry.delivered = true;
        entry.unshared = 0;
    }
    Ok(())
}

/// One finite allowance covers discovery, comparison and conversion.
fn capture_read(remaining: &mut u64, work: &mut Work<'_>) -> Result<bool, Stop> {
    if *remaining == 0 {
        return Ok(false);
    }
    work.tick()?;
    *remaining -= 1;
    Ok(true)
}
