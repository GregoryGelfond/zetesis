//! Bounded, world-isolated reduct rounds with an injected chunk evaluator.
//!
//! Every chunk reads the same immutable round snapshots. Only complete per-world
//! source coverage and successful evaluation of its final chunk permit a commit.
//! A new round rebuilds source relations from the union of derived atoms; seed
//! atoms remain separate. The union offers instances and never establishes truth
//! in an individual world.

use std::collections::BTreeMap;
use std::fmt;

use zetesis_core::{Atom, Model, Program, Seed};

use crate::oracle::{Work, worlds};
use crate::{Control, Stop, source};

#[cfg(test)]
#[path = "../tests/support/workspace_lifetime.rs"]
mod workspace_tests;

/// Rule header words: head tag and three antecedent lengths.
pub const RECORD_HEADER_WORDS: usize = 4;
/// A head tag of zero denotes a constraint; atom IDs are encoded plus one.
pub const CONSTRAINT_HEAD: u32 = 0;

// Snapshots, frozen seeds, pending deltas, one result and offered-head validation.
const ROUND_MASK_VECTORS: usize = 5;
// Three old masks can coexist with three replacement masks during stride growth.
const GROWTH_MASK_VECTORS: usize = 6;

/// Which positive source bindings may be offered within each immutable round.
/// Every evaluator still checks per-world positives and all frozen gates.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SourceSelection {
    /// Exhaust the union relation, including cross-world combinations.
    #[default]
    Union,
    /// Omit positive prefixes that no current world can satisfy. This does not
    /// prune the program, candidate carrier or any future snapshot.
    Worlds,
}

/// Explicit batch, source, catalog and transport limits for lazy rounds.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Maximum submitted seed occurrences, including duplicates.
    pub max_candidates: usize,
    /// Maximum demanded catalog atoms; no complete carrier is enumerated.
    pub max_atoms: usize,
    /// Maximum completed or pending round scans.
    pub max_rounds: u64,
    /// Shared source work across the entire batch, never reset per chunk/round.
    pub max_source_work: u64,
    /// Maximum instances per injected evaluation.
    pub max_chunk_rules: usize,
    /// Maximum encoded instance words, excluding the offset table.
    pub max_chunk_words: usize,
    /// Fixed scratch allowance for one copied source instance, reserved
    /// separately from all catalog growth throughout every scan.
    pub max_instance_bytes: usize,
    /// Maximum requested owned payload bytes for catalog copies, snapshots,
    /// seeds, pending deltas, chunk packing, one result and optional source
    /// membership masks/frames/indices. Input program/seeds, allocator rounding,
    /// tree-node bookkeeping and backend-private transport are excluded; a
    /// backend must bound its transport separately.
    pub max_host_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_candidates: 1024,
            max_atoms: 4096,
            max_rounds: 4097,
            max_source_work: 10_000_000,
            max_chunk_rules: 256,
            max_chunk_words: 16_384,
            max_instance_bytes: 1024 * 1024,
            max_host_bytes: 128 * 1024 * 1024,
        }
    }
}

/// Exact progress, also retained after source or evaluator failure.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Progress {
    /// Rounds with complete per-world source coverage and evaluation.
    pub rounds: u64,
    /// Source operation charges, including an incomplete scan.
    pub source_work: u64,
    /// Filter-valid instances offered by source scans.
    pub instances: u64,
    /// Successfully evaluated nonempty chunks.
    pub chunks: u64,
    /// Distinct demanded atom identities, including seed and underived heads.
    pub catalog_atoms: usize,
    /// Source-work charges for membership probes and packed mask operations.
    /// This is a subset of `source_work`, not additional work to add to it.
    pub mask_words: u64,
    /// Matched positive prefixes with empty current-world membership.
    /// Unvisited extensions are not claimed as enumerated instances.
    pub pruned_prefixes: u64,
    /// Peak requested membership/frame/index payload, including join storage
    /// retained between source rounds.
    /// Symbolic atoms are accounted separately; this is not process RSS.
    pub peak_mask_bytes: usize,
}

impl Progress {
    fn record_source(&mut self, statistics: source::ScanStatistics) {
        self.source_work += statistics.work;
        self.instances += statistics.bindings;
        self.mask_words += statistics.mask_words;
        self.pruned_prefixes += statistics.pruned_prefixes;
        self.peak_mask_bytes = self.peak_mask_bytes.max(statistics.mask_bytes);
    }
}

/// An interrupted batch; none of its candidates has a completed check.
#[derive(Debug)]
pub struct Failure<E> {
    /// Typed interruption, including execution failures.
    pub cause: Cause<E>,
    /// Charged progress before failure; complete rounds are not complete checks.
    pub progress: Progress,
}

/// Source and backend causes do not represent logical rejection.
#[derive(Debug)]
pub enum Cause<E> {
    /// Cancellation, source/catalog/round limits or invalid input.
    Source(Stop),
    /// The evaluator failed; its partial output is discarded.
    Execution(E),
    /// Output shape, status, tail bits or derivable-head bounds were invalid.
    InvalidOutput,
}

impl<E> From<Stop> for Cause<E> {
    fn from(stop: Stop) -> Self {
        Self::Source(stop)
    }
}

/// Completed checks in the exact submitted seed order and batch work.
#[derive(Debug)]
pub struct Batch {
    /// Each check has its complete closure, constraint and seed verdict.
    pub checks: Vec<Check>,
    /// Shared work is not duplicated into per-world CPU work counters.
    pub progress: Progress,
}

/// One completed per-world consequence closure under the injected evaluator's
/// exactness contract. This value carries no scalar CPU operation statistics or
/// stable receipt conversion. Backend refinement remains a separate obligation.
#[derive(Debug)]
pub struct Check {
    program: Program,
    closure: Model,
    constraint_violated: bool,
    seed_mismatch: bool,
}

impl Check {
    /// Owning admitted program; constant-time borrow.
    #[must_use]
    pub const fn program(&self) -> &Program {
        &self.program
    }
    /// Complete least reduct closure if the evaluator fulfills its contract.
    #[must_use]
    pub const fn closure(&self) -> &Model {
        &self.closure
    }
    /// Whether an enabled constraint holds in the completed closure.
    #[must_use]
    pub const fn constraint_violated(&self) -> bool {
        self.constraint_violated
    }
    /// Whether the final gate projection differs from the supplied seed.
    #[must_use]
    pub const fn seed_mismatch(&self) -> bool {
        self.seed_mismatch
    }
    /// Stable membership under the injected evaluator's exactness contract.
    #[must_use]
    pub const fn accepted(&self) -> bool {
        !self.constraint_violated && !self.seed_mismatch
    }
}

impl<E: fmt::Display> fmt::Display for Cause<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(stop) => stop.fmt(f),
            Self::Execution(error) => write!(f, "lazy chunk execution failed: {error}"),
            Self::InvalidOutput => f.write_str("lazy chunk output violated its encoded contract"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for Cause<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source(stop) => Some(stop),
            Self::Execution(error) => Some(error),
            Self::InvalidOutput => None,
        }
    }
}

impl<E: fmt::Display> fmt::Display for Failure<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.cause.fmt(f)
    }
}

impl<E: std::error::Error + 'static> std::error::Error for Failure<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.cause)
    }
}

/// Immutable dense input to one consequence step. IDs remain stable throughout
/// the batch. Every round starts from complete snapshots and separate seeds.
///
/// Records contain `head + 1` (zero denotes a constraint), three antecedent
/// lengths, then positive, true-gate and false-gate IDs. The offset table indexes
/// record starts. All indices and lengths are validated before construction.
/// A backend evaluates each record against one world's snapshot and frozen seed,
/// emits only newly true heads, and sets its constraint word to zero or one.
pub struct Chunk<'a> {
    words: usize,
    worlds: usize,
    catalog_atoms: usize,
    snapshots: &'a [u32],
    seeds: &'a [u32],
    offsets: &'a [u32],
    records: &'a [u32],
}

impl Chunk<'_> {
    /// Current bitset width per world, grown only as catalog IDs are demanded.
    #[must_use]
    pub const fn words(&self) -> usize {
        self.words
    }
    /// Exact seed occurrences, including duplicates.
    #[must_use]
    pub const fn worlds(&self) -> usize {
        self.worlds
    }
    /// Words per returned world record: derived-head delta and violation flag.
    #[must_use]
    pub const fn result_words(&self) -> usize {
        self.words + 1
    }
    /// Constraint-violation offset within one returned world record.
    #[must_use]
    pub const fn violation_offset(&self) -> usize {
        self.words
    }
    /// Number of currently assigned stable atom IDs.
    #[must_use]
    pub const fn catalog_atoms(&self) -> usize {
        self.catalog_atoms
    }
    /// Immutable positive truth, world-major with [`Self::words`] stride.
    #[must_use]
    pub const fn snapshots(&self) -> &[u32] {
        self.snapshots
    }
    /// Immutable gate truth, with the same stride as snapshots.
    #[must_use]
    pub const fn seeds(&self) -> &[u32] {
        self.seeds
    }
    /// Start offsets in [`Self::records`], one for each offered instance.
    #[must_use]
    pub const fn offsets(&self) -> &[u32] {
        self.offsets
    }
    /// Validated linear rule encoding described by [`Chunk`].
    #[must_use]
    pub const fn records(&self) -> &[u32] {
        self.records
    }
}

/// Evaluate a chunk with exact scalar integer operations. The returned layout
/// has `words + 1` entries per world: head delta followed by constraint violation.
/// This is the portable reference for device transport tests; it never mutates
/// snapshots or seeds. Work is O(worlds * encoded antecedents), and returned
/// storage is O(worlds * words), bounded when the chunk was constructed.
///
/// # Errors
/// Returns allocation failure before output construction.
pub fn evaluate(chunk: &Chunk<'_>) -> Result<Vec<u32>, Stop> {
    let mut output = zeros(chunk.worlds * chunk.result_words())?;
    for world in 0..chunk.worlds {
        let snapshot = &chunk.snapshots[world * chunk.words..][..chunk.words];
        let seed = &chunk.seeds[world * chunk.words..][..chunk.words];
        for offset in chunk.offsets {
            let record = &chunk.records[*offset as usize..];
            let counts = [record[1] as usize, record[2] as usize, record[3] as usize];
            let mut start = RECORD_HEADER_WORDS;
            let mut enabled = true;
            for (count, truth, required) in [
                (counts[0], snapshot, true),
                (counts[1], seed, true),
                (counts[2], seed, false),
            ] {
                for atom in &record[start..start + count] {
                    enabled &= contains(truth, *atom as usize) == required;
                }
                start += count;
            }
            if enabled {
                let result = &mut output[world * chunk.result_words()..][..chunk.result_words()];
                if record[0] == CONSTRAINT_HEAD {
                    result[chunk.violation_offset()] = 1;
                } else {
                    let head = (record[0] - 1) as usize;
                    if !contains(snapshot, head) {
                        insert(result, head);
                    }
                }
            }
        }
    }
    Ok(output)
}

/// Compute complete per-world reduct closures by bounded immutable rounds.
/// `execute` must implement [`evaluate`]'s consequence relation exactly; shape
/// validation cannot establish gate truth or semantic correctness of an injected
/// backend. Rust/WGSL refinement is an explicit proof and qualification boundary.
///
/// No completed checks are returned on any incomplete scan or execution failure.
/// Atom IDs grow only when seeds or source instances demand them. All seed atoms
/// participate in the final projection check, even if no rule derives them.
/// Empty batches perform no source or evaluator work. Catalog trees use bounded
/// but infallible standard-library allocations, as do existing Atom/Model copies;
/// vector reservations are fallible. This is not a process-RSS guarantee.
///
/// # Errors
/// Returns source, evaluator or malformed-output failure with retained progress.
pub fn check_with<E>(
    program: &Program,
    seeds: &[Seed],
    limits: Limits,
    control: &Control,
    execute: impl FnMut(&Chunk<'_>) -> Result<Vec<u32>, E>,
) -> Result<Batch, Failure<E>> {
    check_with_source(
        program,
        seeds,
        limits,
        SourceSelection::Union,
        control,
        execute,
    )
}

/// Check the same per-world reduct closures using an explicit source selection.
/// [`SourceSelection::Worlds`] retains complete coverage of each current world,
/// but deliberately does not exhaust every cross-world union combination.
///
/// Mask construction/intersections consume the batch's cumulative source work.
/// Join frames/indices are reused within the batch; membership is rebuilt from
/// each immutable round. Live storage is charged against `max_host_bytes` during
/// catalog growth and between rounds.
/// This optional optimization can change resource stopping points. The injected
/// evaluator's exactness contract is identical to [`check_with`].
///
/// # Errors
/// Returns charged progress and no completed checks after any source, resource,
/// evaluator or protocol failure, including failed mask construction.
pub fn check_with_source<E>(
    program: &Program,
    seeds: &[Seed],
    limits: Limits,
    selection: SourceSelection,
    control: &Control,
    mut execute: impl FnMut(&Chunk<'_>) -> Result<Vec<u32>, E>,
) -> Result<Batch, Failure<E>> {
    let mut progress = Progress::default();
    run(
        program,
        seeds,
        limits,
        selection,
        control,
        &mut progress,
        &mut execute,
    )
    .map(|checks| Batch { checks, progress })
    .map_err(|cause| Failure { cause, progress })
}

struct State {
    catalog: BTreeMap<Atom, u32>,
    atoms: Vec<Atom>,
    payload_bytes: usize,
    fixed_bytes: usize,
    source_mask_bytes: usize,
    world_workspace: Option<worlds::Workspace>,
    words: usize,
    snapshots: Vec<u32>,
    seeds: Vec<u32>,
    pending: Vec<u32>,
    violated: Vec<bool>,
    offsets: Vec<u32>,
    records: Vec<u32>,
}

fn run<E>(
    program: &Program,
    seeds: &[Seed],
    limits: Limits,
    selection: SourceSelection,
    control: &Control,
    progress: &mut Progress,
    execute: &mut impl FnMut(&Chunk<'_>) -> Result<Vec<u32>, E>,
) -> Result<Vec<Check>, Cause<E>> {
    control.poll()?;
    if seeds
        .iter()
        .any(|seed| !seed.program().same_instance(program))
    {
        return Err(Stop::WrongProgram.into());
    }
    if seeds.is_empty() {
        return Ok(Vec::new());
    }
    let mut state = State::new(seeds.len(), limits)?;
    for (world, seed) in seeds.iter().enumerate() {
        for atom in seed.atoms() {
            control.poll()?;
            let id = state.intern(atom, limits)?;
            progress.catalog_atoms = state.atoms.len();
            insert(
                &mut state.seeds[world * state.words..][..state.words],
                id as usize,
            );
        }
    }
    loop {
        progress.catalog_atoms = state.atoms.len();
        if progress.rounds >= limits.max_rounds {
            return Err(Stop::WorkLimit.into());
        }
        control.poll()?;
        state.pending.fill(0);
        let scanned = state.scan(program, selection, limits, control, progress, execute);
        let source_statistics = match scanned {
            Ok(statistics) => statistics,
            Err(failure) => {
                progress.record_source(failure.statistics);
                progress.catalog_atoms = state.atoms.len();
                return Err(match failure.cause {
                    source::ScanCause::Source(stop) => Cause::Source(stop),
                    source::ScanCause::Consumer(cause) => cause,
                });
            }
        };
        progress.record_source(source_statistics);
        state.flush(seeds.len(), control, progress, execute)?;
        control.poll()?;
        progress.rounds += 1;
        let mut grew = false;
        for (old, delta) in state.snapshots.iter_mut().zip(&state.pending) {
            grew |= delta & !*old != 0;
            *old |= delta;
        }
        if !grew {
            break;
        }
    }
    progress.catalog_atoms = state.atoms.len();
    state
        .conclusions(program, seeds, control)
        .map_err(Cause::Source)
}

impl State {
    fn scan<E>(
        &mut self,
        program: &Program,
        selection: SourceSelection,
        limits: Limits,
        control: &Control,
        progress: &mut Progress,
        execute: &mut impl FnMut(&Chunk<'_>) -> Result<Vec<u32>, E>,
    ) -> Result<source::ScanStatistics, source::ScanFailure<Cause<E>>> {
        let scan_limits = source::ScanLimits {
            max_work: limits.max_source_work.saturating_sub(progress.source_work),
            max_instance_atoms: limits
                .max_chunk_words
                .saturating_sub(RECORD_HEADER_WORDS - 1),
            max_instance_bytes: limits.max_instance_bytes,
        };
        let candidates = self.violated.len();
        if selection == SourceSelection::Union {
            let snapshot = Model::new(
                self.atoms
                    .iter()
                    .enumerate()
                    .filter(|(id, _)| {
                        (0..candidates).any(|world| {
                            contains(&self.snapshots[world * self.words..][..self.words], *id)
                        })
                    })
                    .map(|(_, atom)| atom.clone()),
            );
            return source::scan(program, &snapshot, scan_limits, control, |instance| {
                self.offer(&instance, candidates, limits, control, progress, execute)
            });
        }
        let mut work = Work::source(control, scan_limits.max_work);
        let available = limits
            .max_host_bytes
            .saturating_sub(self.fixed_bytes)
            .saturating_sub(self.payload_bytes);
        let workspace = self.world_workspace.take().map_or_else(
            || worlds::Workspace::new(program, candidates, available, &mut work),
            Ok,
        );
        let snapshot = workspace.and_then(|workspace| {
            workspace.snapshot(
                &self.catalog,
                &self.snapshots,
                self.words,
                available,
                &mut work,
            )
        });
        match snapshot {
            Ok(mut snapshot) => {
                self.source_mask_bytes = snapshot.bytes();
                let result = source::scan_worlds(
                    program,
                    &mut snapshot,
                    scan_limits,
                    &mut work,
                    |instance| {
                        self.offer(&instance, candidates, limits, control, progress, execute)
                    },
                );
                // Round truth has dropped, but fixed join storage remains live
                // through flushing, catalog growth and the next snapshot.
                let workspace = snapshot.into_workspace();
                self.source_mask_bytes = workspace.bytes();
                self.world_workspace = Some(workspace);
                result
            }
            Err(stop) => {
                // Failed preparation consumes and drops the workspace too.
                self.source_mask_bytes = 0;
                Err(source::ScanFailure {
                    cause: source::ScanCause::Source(stop),
                    statistics: work.source_statistics(0),
                })
            }
        }
    }

    fn conclusions(
        &self,
        program: &Program,
        seeds: &[Seed],
        control: &Control,
    ) -> Result<Vec<Check>, Stop> {
        let mut checks = Vec::new();
        checks
            .try_reserve_exact(seeds.len())
            .map_err(|_| Stop::Allocation)?;
        for (world, seed) in seeds.iter().enumerate() {
            control.poll()?;
            let words = &self.snapshots[world * self.words..][..self.words];
            let closure = Model::new(
                self.atoms
                    .iter()
                    .enumerate()
                    .filter(|(id, _)| contains(words, *id))
                    .map(|(_, atom)| atom.clone()),
            );
            let mismatch = closure
                .atoms()
                .iter()
                .any(|atom| program.contains_gate_atom(atom) && !seed.contains(atom))
                || seed.atoms().iter().any(|atom| !closure.contains(atom));
            checks.push(Check {
                program: program.clone(),
                closure,
                constraint_violated: self.violated[world],
                seed_mismatch: mismatch,
            });
        }
        control.poll()?;
        Ok(checks)
    }
    fn new(candidates: usize, limits: Limits) -> Result<Self, Stop> {
        if candidates > limits.max_candidates
            || limits.max_atoms == 0
            || limits.max_atoms >= u32::MAX as usize
            || limits.max_chunk_rules == 0
            || limits.max_chunk_words < RECORD_HEADER_WORDS
        {
            return Err(Stop::CarrierLimit);
        }
        let words = 1usize;
        let bits = words.checked_mul(candidates).ok_or(Stop::Allocation)?;
        let fixed_words = bits
            .checked_mul(ROUND_MASK_VECTORS)
            .and_then(|n| candidates.checked_mul(2).and_then(|m| n.checked_add(m)))
            .and_then(|n| n.checked_add(limits.max_chunk_words))
            .and_then(|n| n.checked_add(limits.max_chunk_rules))
            .ok_or(Stop::Allocation)?;
        let fixed_bytes = fixed_words
            .checked_mul(size_of::<u32>())
            .and_then(|bytes| {
                candidates
                    .checked_mul(size_of::<Check>())
                    .and_then(|checks| bytes.checked_add(checks))
            })
            .and_then(|bytes| bytes.checked_add(limits.max_instance_bytes))
            .ok_or(Stop::Allocation)?;
        if fixed_bytes > limits.max_host_bytes {
            return Err(Stop::Allocation);
        }
        let mut offsets = Vec::new();
        offsets
            .try_reserve_exact(limits.max_chunk_rules)
            .map_err(|_| Stop::Allocation)?;
        let mut records = Vec::new();
        records
            .try_reserve_exact(limits.max_chunk_words)
            .map_err(|_| Stop::Allocation)?;
        Ok(Self {
            catalog: BTreeMap::new(),
            atoms: Vec::new(),
            payload_bytes: 0,
            fixed_bytes,
            source_mask_bytes: 0,
            world_workspace: None,
            words,
            snapshots: zeros(bits)?,
            seeds: zeros(bits)?,
            pending: zeros(bits)?,
            violated: vec![false; candidates],
            offsets,
            records,
        })
    }

    fn intern(&mut self, atom: &Atom, limits: Limits) -> Result<u32, Stop> {
        if let Some(id) = self.catalog.get(atom) {
            return Ok(*id);
        }
        if self.atoms.len() >= limits.max_atoms {
            return Err(Stop::CarrierLimit);
        }
        let bytes = atom_bytes(atom)?;
        // Two catalog copies, one union copy and at most one copy in each
        // returned world. This reserves the worst retained symbolic payload.
        let worlds = self.violated.len();
        let bytes = bytes
            .checked_mul(worlds.checked_add(3).ok_or(Stop::Allocation)?)
            .ok_or(Stop::Allocation)?;
        let total = self
            .payload_bytes
            .checked_add(bytes)
            .ok_or(Stop::Allocation)?;
        self.grow(self.atoms.len(), total, limits)?;
        if self
            .fixed_bytes
            .checked_add(total)
            .and_then(|bytes| bytes.checked_add(self.source_mask_bytes))
            .ok_or(Stop::Allocation)?
            > limits.max_host_bytes
        {
            return Err(Stop::Allocation);
        }
        self.atoms.try_reserve(1).map_err(|_| Stop::Allocation)?;
        let id = u32::try_from(self.atoms.len()).map_err(|_| Stop::CarrierLimit)?;
        self.catalog.insert(atom.clone(), id);
        self.atoms.push(atom.clone());
        self.payload_bytes = total;
        Ok(id)
    }

    fn grow(&mut self, atom: usize, payload_bytes: usize, limits: Limits) -> Result<(), Stop> {
        let needed = atom / 32 + 1;
        if needed <= self.words {
            return Ok(());
        }
        let width = needed
            .checked_next_power_of_two()
            .ok_or(Stop::Allocation)?
            .min(limits.max_atoms.div_ceil(32));
        let candidates = self.violated.len();
        let current_bits = self.words.checked_mul(candidates).ok_or(Stop::Allocation)?;
        let next_bits = width.checked_mul(candidates).ok_or(Stop::Allocation)?;
        let current_bytes = current_bits
            .checked_mul(ROUND_MASK_VECTORS * size_of::<u32>())
            .ok_or(Stop::Allocation)?;
        let base = self
            .fixed_bytes
            .checked_sub(current_bytes)
            .ok_or(Stop::InvalidProgram)?;
        // All three new vectors coexist with their old counterparts. Since the
        // width grows, six new-width vectors bound that temporary ownership.
        let peak = next_bits
            .checked_mul(GROWTH_MASK_VECTORS * size_of::<u32>())
            .and_then(|n| n.checked_add(base))
            .and_then(|n| n.checked_add(payload_bytes))
            .and_then(|n| n.checked_add(self.source_mask_bytes))
            .ok_or(Stop::Allocation)?;
        if peak > limits.max_host_bytes {
            return Err(Stop::Allocation);
        }
        let mut snapshots = zeros(next_bits)?;
        let mut seeds = zeros(next_bits)?;
        let mut pending = zeros(next_bits)?;
        for world in 0..candidates {
            let old = world * self.words;
            let next = world * width;
            snapshots[next..next + self.words]
                .copy_from_slice(&self.snapshots[old..old + self.words]);
            seeds[next..next + self.words].copy_from_slice(&self.seeds[old..old + self.words]);
            pending[next..next + self.words].copy_from_slice(&self.pending[old..old + self.words]);
        }
        self.snapshots = snapshots;
        self.seeds = seeds;
        self.pending = pending;
        self.words = width;
        self.fixed_bytes = base + next_bits * ROUND_MASK_VECTORS * size_of::<u32>();
        Ok(())
    }

    fn offer<E>(
        &mut self,
        instance: &source::Instance,
        worlds: usize,
        limits: Limits,
        control: &Control,
        progress: &mut Progress,
        execute: &mut impl FnMut(&Chunk<'_>) -> Result<Vec<u32>, E>,
    ) -> Result<(), Cause<E>> {
        let count = [
            instance.positive().len(),
            instance.gate_true().len(),
            instance.gate_false().len(),
        ]
        .into_iter()
        .try_fold(RECORD_HEADER_WORDS, usize::checked_add)
        .ok_or(Stop::Allocation)?;
        if count > limits.max_chunk_words {
            return Err(Stop::CarrierLimit.into());
        }
        if self.offsets.len() == limits.max_chunk_rules
            || self.records.len() > limits.max_chunk_words - count
        {
            self.flush(worlds, control, progress, execute)?;
        }
        let offset = u32::try_from(self.records.len()).map_err(|_| Stop::CarrierLimit)?;
        let head = instance
            .head()
            .map(|atom| self.intern(atom, limits))
            .transpose()?
            .map_or(CONSTRAINT_HEAD, |id| id + 1);
        self.records.push(head);
        for atoms in [
            instance.positive(),
            instance.gate_true(),
            instance.gate_false(),
        ] {
            self.records
                .push(u32::try_from(atoms.len()).map_err(|_| Stop::CarrierLimit)?);
        }
        for atom in instance
            .positive()
            .iter()
            .chain(instance.gate_true())
            .chain(instance.gate_false())
        {
            control.poll()?;
            let id = self.intern(atom, limits)?;
            self.records.push(id);
        }
        self.offsets.push(offset);
        Ok(())
    }

    fn flush<E>(
        &mut self,
        worlds: usize,
        control: &Control,
        progress: &mut Progress,
        execute: &mut impl FnMut(&Chunk<'_>) -> Result<Vec<u32>, E>,
    ) -> Result<(), Cause<E>> {
        if self.offsets.is_empty() {
            return Ok(());
        }
        control.poll()?;
        let chunk = Chunk {
            words: self.words,
            worlds,
            catalog_atoms: self.atoms.len(),
            snapshots: &self.snapshots,
            seeds: &self.seeds,
            offsets: &self.offsets,
            records: &self.records,
        };
        let result = execute(&chunk).map_err(Cause::Execution)?;
        control.poll()?;
        if result.len() != worlds * (self.words + 1) {
            return Err(Cause::InvalidOutput);
        }
        let mut offered = zeros(self.words)?;
        for offset in &self.offsets {
            let head = self.records[*offset as usize];
            if head != CONSTRAINT_HEAD {
                insert(&mut offered, (head - 1) as usize);
            }
        }
        for (world, output) in result.chunks_exact(self.words + 1).enumerate() {
            if output[self.words] > 1 {
                return Err(Cause::InvalidOutput);
            }
            for (word, delta) in output[..self.words].iter().enumerate() {
                if delta & !offered[word] != 0
                    || delta & self.snapshots[world * self.words + word] != 0
                {
                    return Err(Cause::InvalidOutput);
                }
                self.pending[world * self.words + word] |= delta;
            }
            self.violated[world] |= output[self.words] == 1;
        }
        progress.chunks += 1;
        self.offsets.clear();
        self.records.clear();
        Ok(())
    }
}

fn zeros(length: usize) -> Result<Vec<u32>, Stop> {
    let mut words = Vec::new();
    words
        .try_reserve_exact(length)
        .map_err(|_| Stop::Allocation)?;
    words.resize(length, 0);
    Ok(words)
}

fn atom_bytes(atom: &Atom) -> Result<usize, Stop> {
    let mut bytes = size_of::<Atom>()
        .checked_add(atom.predicate().name().len())
        .ok_or(Stop::Allocation)?;
    for value in atom.values() {
        bytes = bytes
            .checked_add(size_of::<zetesis_core::Value>())
            .and_then(|n| n.checked_add(value.payload_bytes()))
            .ok_or(Stop::Allocation)?;
    }
    Ok(bytes)
}

fn contains(words: &[u32], atom: usize) -> bool {
    words[atom / 32] & (1 << (atom % 32)) != 0
}
fn insert(words: &mut [u32], atom: usize) {
    words[atom / 32] |= 1 << (atom % 32);
}
