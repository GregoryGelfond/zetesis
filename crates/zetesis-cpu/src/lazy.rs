//! Bounded, world-isolated reduct rounds with an injected chunk evaluator.
//!
//! Every chunk reads the same immutable round snapshots. Only complete per-world
//! source coverage and successful evaluation of its final chunk permit a commit.
//! A new round rebuilds source relations from the union of derived atoms; seed
//! atoms remain separate. The union offers instances and never establishes truth
//! in an individual world.
//! One appendable catalog owns demanded atoms. Each source round borrows its
//! committed prefix through canonical local IDs while callbacks append only to
//! a disjoint tail. Identity publication does not establish consequence truth;
//! every offered identity is committed even when the last round adds no truth.

use std::fmt;

use zetesis_core::atom_interner::{
    AtomAppender, AtomInterner, Failure as InternFailure, Limits as InternLimits,
};
use zetesis_core::{Atom, AtomCatalog, Model, ModelError, Program, Seed, SeedView};

use crate::oracle::{Work, worlds};
use crate::{Control, Stop, source};

pub mod shared;

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
    /// Includes seed interning, checked identity comparisons/metadata, canonical
    /// row selection and committing newly demanded identities. Transport packing
    /// and backend-private evaluation retain their separate accounting contracts.
    pub max_source_work: u64,
    /// Maximum instances per injected evaluation.
    pub max_chunk_rules: usize,
    /// Maximum encoded instance words, excluding the offset table.
    pub max_chunk_words: usize,
    /// Fixed scratch allowance for one copied source instance, reserved
    /// separately from all catalog growth throughout every scan.
    pub max_instance_bytes: usize,
    /// Maximum admitted host storage envelope: actual committed/pending catalog,
    /// AVL/path and ordered-ID capacities, measured nested atom payload, requested
    /// packed truth/seed/delta/chunk/result storage, source membership/workspace
    /// and requested final model-selection slots. Catalog and mask growth
    /// include their named old/new overlap. Source rows borrow catalog atoms.
    /// Input program/seeds, Arc envelopes, allocator overhead, final selection
    /// Vec capacity beyond its requested slots, rounding outside the catalog and
    /// ordered-ID capacities, and backend-private transport are excluded. This
    /// bound is not RSS; a backend bounds its transport separately.
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
    /// Source and demanded-identity operation charges, including interrupted
    /// preparation, scans and committed-prefix transfer.
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
    /// Peak retained ordered source-ID capacity plus requested membership/frame
    /// payload, including join storage retained between source rounds. Interner
    /// capacity and failed temporary order preparation are bounded separately by
    /// `max_host_bytes`; this subtotal is not process RSS or total catalog storage.
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
    /// Consume the check and transfer its closure without copying atoms or
    /// their owned payload. The program association and verdict are discarded.
    /// Rejected checks also have a closure; this raw interpretation alone makes
    /// no membership claim. Its leastness still requires the evaluator contract.
    #[must_use]
    pub fn into_closure(self) -> Model {
        self.closure
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
    round_index: u64,
    words: usize,
    worlds: usize,
    catalog_atoms: usize,
    snapshots: &'a [u32],
    seeds: &'a [u32],
    offsets: &'a [u32],
    records: &'a [u32],
}

impl Chunk<'_> {
    /// Zero-based immutable-round index within this invoking batch only. It is
    /// not a globally unique identity: a new batch starts at zero. Snapshots
    /// remain unchanged within this round except for zero-filled width growth;
    /// seeds remain frozen throughout the batch. A cache must own its batch
    /// boundary and include the current layout when reusing either input.
    #[must_use]
    pub const fn round_index(&self) -> u64 {
        self.round_index
    }
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
        let result = &mut output[world * chunk.result_words()..][..chunk.result_words()];
        evaluate_world(chunk, world, result, &mut |_| Ok(()))?;
    }
    Ok(output)
}

#[derive(Clone, Copy)]
enum EvaluationStep {
    Instance,
    Antecedent,
}

/// One world's pure consequence relation, with an injected work boundary.
/// Output is private until every world and the source round have completed.
fn evaluate_world(
    chunk: &Chunk<'_>,
    world: usize,
    result: &mut [u32],
    step: &mut impl FnMut(EvaluationStep) -> Result<(), Stop>,
) -> Result<(), Stop> {
    let snapshot = &chunk.snapshots[world * chunk.words..][..chunk.words];
    let seed = &chunk.seeds[world * chunk.words..][..chunk.words];
    for offset in chunk.offsets {
        step(EvaluationStep::Instance)?;
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
                step(EvaluationStep::Antecedent)?;
                enabled &= contains(truth, *atom as usize) == required;
            }
            start += count;
        }
        if enabled {
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
    Ok(())
}

/// Compute complete per-world reduct closures by bounded immutable rounds.
/// `execute` must implement [`evaluate`]'s consequence relation exactly; shape
/// validation cannot establish gate truth or semantic correctness of an injected
/// backend. Rust/WGSL refinement is an explicit proof and qualification boundary.
///
/// No completed checks are returned on any incomplete scan or execution failure.
/// Atom IDs grow only when seeds or source instances demand them. All seed atoms
/// participate in the final projection check, even if no rule derives them.
/// Empty batches perform no source or evaluator work. Catalog/index, ordered-ID
/// and transport vector reservations return typed allocation failures. Borrowed
/// source-relation grouping, copied Instance payloads and shared ownership
/// envelopes retain their infallible allocation contracts.
/// Completed worlds share one final atom catalog and retain their own selected
/// positions. This is a logical payload bound, not a process-RSS guarantee.
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
    check_with_views(
        program,
        seeds.iter().map(Seed::view),
        limits,
        control,
        execute,
    )
}

/// Check borrowed candidate views with the same immutable-round protocol as
/// [`check_with`]. Cloning the iterator must preserve its length, order and
/// candidate identities. The views borrow their true atoms; this boundary
/// creates no owned seed or temporary view vector. Each distinct demanded atom
/// is copied into one catalog at most once; round selections own only IDs. The
/// final catalog transfers to shared result models without copying atom payloads.
///
/// # Errors
/// Returns the same failures and retained progress as [`check_with`]. Actual
/// occurrence counts inconsistent with the claimed size refuse as
/// [`Stop::InvalidProgram`] before an out-of-range world write or publication.
pub fn check_with_views<'seed, E>(
    program: &Program,
    seeds: impl ExactSizeIterator<Item = SeedView<'seed>> + Clone,
    limits: Limits,
    control: &Control,
    execute: impl FnMut(&Chunk<'_>) -> Result<Vec<u32>, E>,
) -> Result<Batch, Failure<E>> {
    check_with_source_views(
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
/// Identity preparation, mask construction/intersections and committing newly
/// demanded identities consume the batch's cumulative source work.
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
    execute: impl FnMut(&Chunk<'_>) -> Result<Vec<u32>, E>,
) -> Result<Batch, Failure<E>> {
    check_with_source_views(
        program,
        seeds.iter().map(Seed::view),
        limits,
        selection,
        control,
        execute,
    )
}

/// Check borrowed candidates under the selected source traversal. The iterator
/// contract and allocation boundary are those of [`check_with_views`]; source
/// work, complete-world coverage and failure progress are unchanged from
/// [`check_with_source`].
///
/// # Errors
/// Returns no complete checks after any source, resource or evaluator failure.
pub fn check_with_source_views<'seed, E>(
    program: &Program,
    seeds: impl ExactSizeIterator<Item = SeedView<'seed>> + Clone,
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
    catalog: AtomInterner,
    transport: Transport,
    world_workspace: Option<worlds::Workspace>,
}

/// Dense round truth and chunk storage. Atom identity lives in the interner;
/// source views can borrow its committed region while this state grows masks.
struct Transport {
    payload_bytes: usize,
    fixed_bytes: usize,
    source_mask_bytes: usize,
    words: usize,
    snapshots: Vec<u32>,
    seeds: Vec<u32>,
    pending: Vec<u32>,
    violated: Vec<bool>,
    offsets: Vec<u32>,
    records: Vec<u32>,
}

/// The injected evaluator and its batch-local progress share one control door.
struct Evaluation<'a, F> {
    control: &'a Control,
    progress: &'a mut Progress,
    execute: &'a mut F,
}

enum SourceSnapshot<'a> {
    Union(worlds::Rows<'a>),
    Worlds(worlds::Snapshot<'a>),
}

impl SourceSnapshot<'_> {
    fn bytes(&self) -> usize {
        match self {
            Self::Union(rows) => rows.bytes(),
            Self::Worlds(snapshot) => snapshot.bytes(),
        }
    }

    fn into_workspace(self) -> Option<worlds::Workspace> {
        match self {
            Self::Union(_) => None,
            Self::Worlds(snapshot) => Some(snapshot.into_workspace()),
        }
    }

    fn scan<E>(
        &mut self,
        program: &Program,
        limits: source::ScanLimits,
        work: &mut Work<'_>,
        consume: impl FnMut(source::Instance, &mut Work<'_>) -> Result<(), E>,
    ) -> Result<source::ScanStatistics, source::ScanFailure<E>> {
        match self {
            Self::Union(rows) => source::scan_rows(program, rows.iter(), limits, work, consume),
            Self::Worlds(snapshot) => source::scan_worlds(program, snapshot, limits, work, consume),
        }
    }
}

fn prepare_snapshot<'a>(
    catalog: &'a mut AtomInterner,
    transport: &Transport,
    workspace: &mut Option<worlds::Workspace>,
    program: &Program,
    selection: SourceSelection,
    limits: Limits,
    work: &mut Work<'_>,
) -> Result<(SourceSnapshot<'a>, AtomAppender<'a>), Stop> {
    if let Some(retained) = workspace.as_ref() {
        retained.record_retained(work);
    }
    let bounds = transport.catalog_limits(limits, catalog.len(), transport.payload_bytes)?;
    let ordered = catalog
        .ordered_ids_with(bounds, || work.tick())
        .map_err(|error| intern_stop(&error))?;
    let available = transport.source_bytes(limits, catalog.storage_bytes(), catalog.len())?;
    let (committed, appender) = catalog.split();
    let rows = worlds::Rows::select(
        committed.as_slice(),
        ordered,
        &transport.snapshots,
        transport.words,
        transport.violated.len(),
        available,
        work,
    )?;
    let snapshot = match selection {
        SourceSelection::Union => SourceSnapshot::Union(rows),
        SourceSelection::Worlds => {
            let retained = workspace.take().map_or_else(
                || {
                    worlds::Workspace::new(
                        program,
                        transport.violated.len(),
                        available.saturating_sub(rows.bytes()),
                        work,
                    )
                },
                Ok,
            )?;
            SourceSnapshot::Worlds(retained.snapshot(
                rows,
                &transport.snapshots,
                transport.words,
                available,
                work,
            )?)
        }
    };
    Ok((snapshot, appender))
}

// Clone equivalence remains the caller's iterator contract. Check every actual
// length before indexed world writes so a bad ExactSizeIterator cannot overrun
// or silently omit an admitted occurrence.
fn validate_seeds<'seed>(
    program: &Program,
    seeds: impl Iterator<Item = SeedView<'seed>>,
    expected: usize,
) -> Result<(), Stop> {
    let mut count = 0;
    for seed in seeds {
        if !seed.program().same_instance(program) {
            return Err(Stop::WrongProgram);
        }
        if count >= expected {
            return Err(Stop::InvalidProgram);
        }
        count += 1;
    }
    if count != expected {
        return Err(Stop::InvalidProgram);
    }
    Ok(())
}

fn run<'seed, E>(
    program: &Program,
    seeds: impl ExactSizeIterator<Item = SeedView<'seed>> + Clone,
    limits: Limits,
    selection: SourceSelection,
    control: &Control,
    progress: &mut Progress,
    execute: &mut impl FnMut(&Chunk<'_>) -> Result<Vec<u32>, E>,
) -> Result<Vec<Check>, Cause<E>> {
    control.poll()?;
    let candidates = seeds.len();
    validate_seeds(program, seeds.clone(), candidates)?;
    if candidates == 0 {
        return Ok(Vec::new());
    }
    let mut state = State::new(candidates, limits)?;
    state.initialize(seeds.clone(), candidates, limits, control, progress)?;
    loop {
        progress.catalog_atoms = state.catalog.len();
        if progress.rounds >= limits.max_rounds {
            return Err(Stop::WorkLimit.into());
        }
        control.poll()?;
        state.transport.pending.fill(0);
        let scanned = state.scan(program, selection, limits, control, progress, execute);
        let source_statistics = match scanned {
            Ok(statistics) => statistics,
            Err(failure) => {
                progress.record_source(failure.statistics);
                progress.catalog_atoms = state.catalog.len();
                return Err(match failure.cause {
                    source::ScanCause::Source(stop) => Cause::Source(stop),
                    source::ScanCause::Consumer(cause) => cause,
                });
            }
        };
        progress.record_source(source_statistics);
        progress.catalog_atoms = state.catalog.len();
        state.transport.flush(
            state.catalog.len(),
            &mut Evaluation {
                control,
                progress,
                execute,
            },
        )?;
        // Identity may grow even when no consequence does. Freeze every offered
        // identity before testing convergence, including a final zero-delta round.
        state.commit(limits, control, progress)?;
        control.poll()?;
        progress.rounds += 1;
        let mut grew = false;
        for (old, delta) in state
            .transport
            .snapshots
            .iter_mut()
            .zip(&state.transport.pending)
        {
            grew |= delta & !*old != 0;
            *old |= delta;
        }
        if !grew {
            break;
        }
    }
    progress.catalog_atoms = state.catalog.len();
    state
        .conclusions(program, seeds, limits, control, progress)
        .map_err(Cause::Source)
}

impl State {
    fn new(candidates: usize, limits: Limits) -> Result<Self, Stop> {
        let state = Self {
            catalog: AtomInterner::new(),
            transport: Transport::new(candidates, limits)?,
            world_workspace: None,
        };
        if state.catalog.storage_bytes() > state.transport.catalog_limits(limits, 0, 0)?.max_bytes {
            return Err(Stop::Allocation);
        }
        Ok(state)
    }

    fn intern(&mut self, atom: &Atom, limits: Limits, work: &mut Work<'_>) -> Result<u32, Stop> {
        let (_, mut appender) = self.catalog.split();
        self.transport.intern(&mut appender, atom, limits, work)
    }

    fn initialize<'seed>(
        &mut self,
        seeds: impl Iterator<Item = SeedView<'seed>>,
        candidates: usize,
        limits: Limits,
        control: &Control,
        progress: &mut Progress,
    ) -> Result<(), Stop> {
        let mut work = Work::source(
            control,
            limits.max_source_work.saturating_sub(progress.source_work),
        );
        let result = (|| {
            let mut packed = 0;
            for (world, seed) in seeds.enumerate() {
                if world >= candidates {
                    return Err(Stop::InvalidProgram);
                }
                packed += 1;
                for atom in seed.atoms() {
                    let id = self.intern(atom, limits, &mut work)?;
                    let words = self.transport.words;
                    insert(
                        &mut self.transport.seeds[world * words..][..words],
                        id as usize,
                    );
                }
            }
            if packed != candidates {
                return Err(Stop::InvalidProgram);
            }
            Ok(())
        })();
        progress.record_source(work.source_statistics(0));
        progress.catalog_atoms = self.catalog.len();
        result?;
        self.commit(limits, control, progress)
    }

    fn commit(
        &mut self,
        limits: Limits,
        control: &Control,
        progress: &mut Progress,
    ) -> Result<(), Stop> {
        let mut work = Work::source(
            control,
            limits.max_source_work.saturating_sub(progress.source_work),
        );
        let result = self
            .transport
            .catalog_limits(limits, self.catalog.len(), self.transport.payload_bytes)
            .and_then(|bounds| {
                self.catalog
                    .commit_with(bounds, || work.tick())
                    .map_err(|error| intern_stop(&error))
            });
        progress.record_source(work.source_statistics(0));
        // Pending published identities count even if vector-transfer admission
        // fails; a failed commit returns no completed checks or round receipt.
        progress.catalog_atoms = self.catalog.len();
        result
    }

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
        let mut work = Work::source(control, scan_limits.max_work);
        let prepared = prepare_snapshot(
            &mut self.catalog,
            &self.transport,
            &mut self.world_workspace,
            program,
            selection,
            limits,
            &mut work,
        );
        let transport = &mut self.transport;
        let workspace = &mut self.world_workspace;
        let mut evaluation = Evaluation {
            control,
            progress,
            execute,
        };
        match prepared {
            Ok((mut snapshot, mut appender)) => {
                transport.source_mask_bytes = snapshot.bytes();
                let result = snapshot.scan(program, scan_limits, &mut work, |instance, work| {
                    transport.offer(&mut appender, &instance, limits, work, &mut evaluation)
                });
                // Round truth has dropped, but fixed join storage remains live
                // through flushing, catalog growth and the next snapshot.
                *workspace = snapshot.into_workspace();
                transport.source_mask_bytes =
                    workspace.as_ref().map_or(0, worlds::Workspace::bytes);
                result
            }
            Err(stop) => {
                // Preparation may stop before taking the retained workspace.
                transport.source_mask_bytes =
                    workspace.as_ref().map_or(0, worlds::Workspace::bytes);
                Err(source::ScanFailure {
                    cause: source::ScanCause::Source(stop),
                    statistics: work.source_statistics(0),
                })
            }
        }
    }

    fn conclusions<'seed>(
        self,
        program: &Program,
        seeds: impl ExactSizeIterator<Item = SeedView<'seed>>,
        limits: Limits,
        control: &Control,
        progress: &mut Progress,
    ) -> Result<Vec<Check>, Stop> {
        // Source rounds are complete. Transfer the sole dense Atom vector;
        // its integer lookup metadata can now drop without copying payloads.
        let bounds = self.transport.catalog_limits(
            limits,
            self.catalog.len(),
            self.transport.payload_bytes,
        )?;
        let mut work = Work::source(
            control,
            limits.max_source_work.saturating_sub(progress.source_work),
        );
        let atoms = self
            .catalog
            .into_atoms_with(bounds, || work.tick())
            .map_err(|error| intern_stop(&error));
        progress.record_source(work.source_statistics(0));
        let catalog = AtomCatalog::new(atoms?);
        let transport = self.transport;
        let mut checks = Vec::new();
        checks
            .try_reserve_exact(transport.violated.len())
            .map_err(|_| Stop::Allocation)?;
        for (world, seed) in seeds.enumerate() {
            control.poll()?;
            if world >= transport.violated.len() {
                return Err(Stop::InvalidProgram);
            }
            let words = &transport.snapshots[world * transport.words..][..transport.words];
            let closure = Model::from_positions(
                &catalog,
                (0..catalog.atoms().len()).filter(|id| contains(words, *id)),
            )
            .map_err(|error| match error {
                ModelError::Allocation => Stop::Allocation,
                ModelError::Position { .. } => Stop::InvalidProgram,
            })?;
            let mismatch = closure
                .atoms()
                .iter()
                .any(|atom| program.contains_gate_atom(atom) && !seed.contains(atom))
                || seed.atoms().any(|atom| !closure.contains(atom));
            checks.push(Check {
                program: program.clone(),
                closure,
                constraint_violated: transport.violated[world],
                seed_mismatch: mismatch,
            });
        }
        if checks.len() != transport.violated.len() {
            return Err(Stop::InvalidProgram);
        }
        control.poll()?;
        Ok(checks)
    }
}

impl Transport {
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
            payload_bytes: 0,
            fixed_bytes,
            source_mask_bytes: 0,
            words,
            snapshots: zeros(bits)?,
            seeds: zeros(bits)?,
            pending: zeros(bits)?,
            violated: vec![false; candidates],
            offsets,
            records,
        })
    }

    fn catalog_limits(
        &self,
        limits: Limits,
        atoms: usize,
        payload: usize,
    ) -> Result<InternLimits, Stop> {
        let external = self
            .external_bytes(atoms, payload)?
            .checked_add(self.source_mask_bytes as u128)
            .ok_or(Stop::Allocation)?;
        let max_bytes = (limits.max_host_bytes as u128)
            .checked_sub(external)
            .ok_or(Stop::Allocation)?;
        Ok(InternLimits {
            max_atoms: limits.max_atoms,
            max_bytes,
        })
    }

    fn external_bytes(&self, atoms: usize, payload: usize) -> Result<u128, Stop> {
        // Completed worlds share the catalog. Bound requested selected positions
        // by one per catalog atom per world; Model Vec capacity slack is excluded
        // from this requested-payload allowance. No symbolic snapshot copy exists.
        let selected = atoms
            .checked_mul(self.violated.len())
            .and_then(|cells| cells.checked_mul(size_of::<usize>()))
            .ok_or(Stop::Allocation)?;
        Ok(self.fixed_bytes as u128 + payload as u128 + selected as u128)
    }

    fn source_bytes(
        &self,
        limits: Limits,
        catalog_bytes: u128,
        atoms: usize,
    ) -> Result<usize, Stop> {
        let used = self
            .external_bytes(atoms, self.payload_bytes)?
            .checked_add(catalog_bytes)
            .ok_or(Stop::Allocation)?;
        let remaining = (limits.max_host_bytes as u128)
            .checked_sub(used)
            .ok_or(Stop::Allocation)?;
        usize::try_from(remaining).map_err(|_| Stop::Allocation)
    }

    fn intern(
        &mut self,
        appender: &mut AtomAppender<'_>,
        atom: &Atom,
        limits: Limits,
        work: &mut Work<'_>,
    ) -> Result<u32, Stop> {
        let count = appender.len();
        let bounds = self.catalog_limits(limits, count, self.payload_bytes)?;
        let entry = appender
            .entry_atom_with(atom, bounds, || work.tick())
            .map_err(|error| intern_stop(&error))?;
        if let Some(id) = entry.position() {
            return u32::try_from(id).map_err(|_| Stop::CarrierLimit);
        }
        if count >= limits.max_atoms {
            return Err(Stop::CarrierLimit);
        }
        let bytes = atom_payload_bytes(atom)?;
        let total = self
            .payload_bytes
            .checked_add(bytes)
            .ok_or(Stop::Allocation)?;
        self.grow(count, total, entry.storage_bytes(), limits)?;
        let bounds = self.catalog_limits(limits, count + 1, total)?;
        let id = entry
            .insert_with(bounds, || work.tick())
            .map_err(|error| intern_stop(&error))?;
        self.payload_bytes = total;
        u32::try_from(id).map_err(|_| Stop::CarrierLimit)
    }

    fn grow(
        &mut self,
        atom: usize,
        payload_bytes: usize,
        catalog_bytes: u128,
        limits: Limits,
    ) -> Result<(), Stop> {
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
            .ok_or(Stop::Allocation)?;
        let selected = (atom + 1)
            .checked_mul(candidates)
            .and_then(|cells| cells.checked_mul(size_of::<usize>()))
            .ok_or(Stop::Allocation)?;
        let peak = peak as u128
            + payload_bytes as u128
            + selected as u128
            + self.source_mask_bytes as u128
            + catalog_bytes;
        if peak > limits.max_host_bytes as u128 {
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
        appender: &mut AtomAppender<'_>,
        instance: &source::Instance,
        limits: Limits,
        work: &mut Work<'_>,
        evaluation: &mut Evaluation<'_, impl FnMut(&Chunk<'_>) -> Result<Vec<u32>, E>>,
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
            self.flush(appender.len(), evaluation)?;
        }
        let offset = u32::try_from(self.records.len()).map_err(|_| Stop::CarrierLimit)?;
        let head = instance
            .head()
            .map(|atom| self.intern(appender, atom, limits, work))
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
            let id = self.intern(appender, atom, limits, work)?;
            self.records.push(id);
        }
        self.offsets.push(offset);
        Ok(())
    }

    fn flush<E>(
        &mut self,
        catalog_atoms: usize,
        evaluation: &mut Evaluation<'_, impl FnMut(&Chunk<'_>) -> Result<Vec<u32>, E>>,
    ) -> Result<(), Cause<E>> {
        if self.offsets.is_empty() {
            return Ok(());
        }
        evaluation.control.poll()?;
        let worlds = self.violated.len();
        let chunk = Chunk {
            round_index: evaluation.progress.rounds,
            words: self.words,
            worlds,
            catalog_atoms,
            snapshots: &self.snapshots,
            seeds: &self.seeds,
            offsets: &self.offsets,
            records: &self.records,
        };
        let result = (evaluation.execute)(&chunk).map_err(Cause::Execution)?;
        evaluation.control.poll()?;
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
        evaluation.progress.chunks += 1;
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

fn intern_stop(error: &InternFailure<Stop>) -> Stop {
    match error {
        InternFailure::Stopped(stop) => *stop,
        InternFailure::Atoms { .. } => Stop::CarrierLimit,
        InternFailure::Bytes { .. } | InternFailure::Allocation(_) | InternFailure::Overflow => {
            Stop::Allocation
        }
    }
}

fn atom_payload_bytes(atom: &Atom) -> Result<usize, Stop> {
    // The interner separately accounts Atom vector cells and ID-only metadata.
    let mut bytes = atom.predicate().name().len();
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
