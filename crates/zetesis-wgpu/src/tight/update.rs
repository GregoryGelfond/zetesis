//! The update route's host side: the readers index, its intrinsic bounds, the
//! per-slot state's shape, the route's derived work, and the runtime guard
//! that declines the route while its updates cost more than dense evaluation.
//!
//! A slot that holds reuse authority and whose state was seeded by an earlier
//! dense or complete evaluation can bring that evaluation to a new candidate by
//! re-evaluating, in dependency-level order, only the atom nodes of changed
//! atoms and the readers of changed values. The readers index lists, for every
//! node, the connectives and numeric-row owners reading it, the heads of the
//! producers whose body it is, and the original root positions holding it; and
//! for every atom, its atom nodes. It is built once per resident theory, at the
//! first batch holding reuse authority, from the theory, the certificate's
//! producers and roots, and the retained owner column of the cold numeric-row
//! directory, and only for a schedule the route [`engages`].

use super::packing::{Graph, Packed};
use super::{TightSupport, poll};
use crate::{GpuError, GpuErrorKind};
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{NodeView, TightPlan};

/// Result-record routes, shared with `check.wgsl`: the complete route, dense
/// reuse, the update route, and dense reuse after a change count found the
/// candidate's change beyond the threshold.
pub(super) const ROUTE_COMPLETE: u32 = 0;
pub(super) const ROUTE_DENSE: u32 = 1;
pub(super) const ROUTE_UPDATE: u32 = 2;
pub(super) const ROUTE_BEYOND_THRESHOLD: u32 = 3;
/// A record's word 6 holds the route in its two low bits and, in this bit,
/// whether the slot's update state was seeded.
pub(super) const SEEDED: u32 = 4;
/// Reader kinds in an entry's two high bits; the low 30 bits are its index.
pub(super) const READER_NODE: u32 = 0;
pub(super) const READER_PRODUCER: u32 = 1 << 30;
pub(super) const READER_ROOT: u32 = 2 << 30;
/// One more than the largest node ID, atom or root position an entry names.
pub(super) const INDEX_LIMIT: u32 = 1 << 30;
/// The per-slot summary block: summarized, and the false-root and unsupported
/// counts. Whether a slot is seeded lives in its result record instead.
pub(super) const SUMMARY_WORDS: u32 = 3;
/// Words of the result record: epoch, slot, status, witness, work, marker,
/// route with seeded, nodes evaluated and an update's path. Writing it is part
/// of the update floor.
pub(super) const RECORD_WORDS: u64 = 9;
/// The most batches the runtime guard makes the route wait after a declining
/// judgment: its backoff doubles from 1 up to this.
pub(super) const GUARD_WAIT_LIMIT: u32 = 32;

/// Workgroup strides per dependency level that dense evaluation must take, on
/// average, before a theory builds the update route's readers index.
///
/// Dense evaluation of one candidate takes `I(W) = sum(ceil(width / W))`
/// strided iterations over the level widths at the dispatched workgroup size
/// `W`. An update keeps every level's synchronization, adds five per
/// candidate (eight when it summarizes), and at each level evaluates each
/// queued word's taken nodes, with their readers' queue bits, counts and root
/// sets, in one invocation through storage atomics. It pays only where `I(W)`
/// exceeds the level count several times. The factor covers the change
/// patterns it was measured on, their per-level chains included: on an Apple
/// M4 Pro GPU under Metal, scattered changes and clustered changes of up to
/// 64 atoms with one reader per input. The runtime guard ([`Guard`]) bounds,
/// per batch, the patterns beyond them, such as a change reaching many nodes
/// of one queued word. The factor still decides whether a theory builds the
/// index at all: it spares narrow theories the index, the state, the seeding
/// batches and every losing probe, and covers the update's extra
/// synchronization, which the guard cannot see. It is an execution choice and
/// changes no verdict. Declining keeps the dense routes, so a factor set high
/// forgoes gains, while one set low slows the schedules it engages.
pub(super) const ENGAGEMENT_STRIDES: u64 = 32;

/// Whether the update route engages for a resident schedule of `levels` levels
/// whose dense strided depth at its workgroup size is `depth`.
pub(super) const fn engages(depth: u64, levels: u32) -> bool {
    levels != 0 && depth >= ENGAGEMENT_STRIDES * levels as u64
}

/// The device's large-change threshold: a slot whose candidate changed more
/// atoms than this takes dense reuse. An execution choice, starting at the CPU
/// update's rule; every route returns the same verdict.
pub(super) const fn threshold(atoms: u32) -> u32 {
    let eighth = atoms / 8;
    if eighth > 64 { eighth } else { 64 }
}

/// Whether `count` nodes, atoms or root positions all fit an entry's 30-bit
/// index: the largest, `count - 1`, is below [`INDEX_LIMIT`].
pub(super) const fn addressable(count: u32) -> bool {
    count <= INDEX_LIMIT
}

/// The narrowest packed width, 8, 16 or 32 bits, that holds `largest` enabled
/// producers of one head.
pub(super) const fn count_bits(largest: u32) -> u32 {
    if largest <= u8::MAX as u32 {
        8
    } else if largest <= u16::MAX as u32 {
        16
    } else {
        32
    }
}

fn capacity(detail: &str) -> GpuError {
    GpuError::new(GpuErrorKind::Capacity, detail)
}

fn vector<T>(length: usize) -> Result<Vec<T>, GpuError> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(length)
        .map_err(|_| GpuError::new(GpuErrorKind::Allocation, "tight index reservation failed"))?;
    Ok(output)
}

fn add(values: &[u64]) -> Option<u64> {
    values
        .iter()
        .try_fold(0_u64, |total, value| total.checked_add(*value))
}

/// Words of a set over `bits` positions.
const fn set_words(bits: u32) -> u32 {
    bits.div_ceil(32)
}

/// Words of `atoms` packed counts of `count_bits` bits each.
const fn count_words(atoms: u32, count_bits: u32) -> u32 {
    atoms.div_ceil(32 / count_bits)
}

/// A theory's update shape, decided once from its intrinsic bounds: the
/// count width, the per-slot state's words, and the readers index's bound.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Bounds {
    pub(super) count_bits: u32,
    pub(super) slot_words: u32,
    /// Readers entries at most `edges + producers + roots`.
    pub(super) entries: u32,
    /// Atom-node entries at most `nodes`.
    pub(super) atom_entries: u32,
    /// Producer-buffer words before the index: records and grouped offsets.
    pub(super) base: u32,
}

impl Bounds {
    /// Decide the intrinsic bounds: every node ID, atom and root position fits
    /// 30 bits; the producer buffer rebuilt with the index at its bound fits
    /// `u32` with the kernel's stride headroom and both of the device's buffer
    /// limits; and the per-slot state's word count fits `u32`, at the count
    /// width the cold batch's producer count decided. `None` is a refusal that
    /// holds for the resident theory, so a predictable overflow is refused
    /// before any device effect. Constant time; allocates nothing.
    pub(super) fn new(graph: &Graph, device: &wgpu::Limits) -> Option<Self> {
        if ![graph.nodes, graph.atoms, graph.roots]
            .into_iter()
            .all(addressable)
        {
            return None;
        }
        let base = u32::try_from(graph.producer_bytes / 4).ok()?;
        let entries = add(&[
            u64::from(graph.edges),
            u64::from(graph.producers),
            u64::from(graph.roots),
        ])?;
        let words = add(&[
            u64::from(base),
            u64::from(graph.nodes) + 1,
            entries,
            u64::from(graph.atoms) + 1,
            u64::from(graph.nodes),
        ])?;
        if !u32::try_from(words).is_ok_and(crate::kernel::fits_strided_loop)
            || words * 4 > device.max_buffer_size
            || words * 4 > device.max_storage_buffer_binding_size
        {
            return None;
        }
        let count_bits = count_bits(graph.largest_producers);
        Some(Self {
            count_bits,
            slot_words: slot_words(graph, count_bits)?,
            entries: u32::try_from(entries).ok()?,
            atom_entries: graph.nodes,
            base,
        })
    }

    /// The index's resident bytes at its bound, admitted before it is built.
    pub(super) fn index_bytes(&self, graph: &Graph) -> u64 {
        u64::from(table_words(graph, self.entries, self.atom_entries)) * 4
    }

    /// Simultaneous host bytes of building the index: the repacked producer
    /// records, the rebuilt producer buffer with its index at its bound, and
    /// the per-node and per-atom counts and cursors.
    pub(super) fn build_bytes(&self, graph: &Graph) -> u64 {
        let counts = u64::from(graph.nodes) + 1 + u64::from(graph.atoms) + 1;
        let cursors = u64::from(graph.nodes) + u64::from(graph.atoms);
        let rebuilt =
            u64::from(self.base) + u64::from(table_words(graph, self.entries, self.atom_entries));
        (u64::from(self.base) + rebuilt + counts + cursors) * 4
    }
}

/// Words of the readers index: node offsets and entries, then atom offsets and
/// atom-node entries.
fn table_words(graph: &Graph, entries: u32, atom_entries: u32) -> u32 {
    // Bounds::new checked the bound's sum, which no actual table exceeds.
    graph.nodes + 1 + entries + graph.atoms + 1 + atom_entries
}

/// Per-slot state words: the summary block, the retained candidate, the
/// touched and unsupported atom sets, the false-root and queued sets, and the
/// packed support counts. `None` when they exceed `u32`.
pub(super) fn slot_words(graph: &Graph, count_bits: u32) -> Option<u32> {
    let words = add(&[
        u64::from(SUMMARY_WORDS),
        3 * u64::from(graph.words),
        u64::from(set_words(graph.roots)),
        u64::from(set_words(graph.nodes)),
        u64::from(count_words(graph.atoms, count_bits)),
    ])?;
    u32::try_from(words).ok()
}

/// Operations every update performs, whatever changed: the change count and
/// the presence pass over both candidates' words, the settling scan, the
/// queued-set scan of each level, and the record's words.
pub(super) fn floor(graph: &Graph, levels: u32) -> u64 {
    5 * u64::from(graph.words)
        + u64::from(set_words(graph.nodes))
        + u64::from(levels)
        + RECORD_WORDS
}

/// Dense reuse after the update route's change count, with the state's writes:
/// the dense charge, the count's `2 * words` and the retained candidate's
/// `words + 1`. Both the index's build gate and every allowed plan reserve it.
pub(super) fn dense_after_count(graph: &Graph, levels: u32) -> u64 {
    u64::from(graph.work)
        + u64::from(levels)
        + 2 * u64::from(graph.nodes)
        + u64::from(graph.edges)
        + 4
        + 3 * u64::from(graph.words)
        + 1
}

/// The update route's derived worst case per candidate: the change count, a
/// summary, presence of `min(threshold, atoms)` changed atoms with every
/// atom-node entry, every node evaluated once with all of its reads and every
/// readers entry visited, settling every atom, one verdict scan and the record.
pub(super) fn worst_case(
    graph: &Graph,
    levels: u32,
    count_bits: u32,
    entries: u32,
    atom_entries: u32,
) -> Option<u64> {
    let words = u64::from(graph.words);
    let atoms = u64::from(graph.atoms);
    let roots = set_words(graph.roots);
    add(&[
        2 * words,
        summary(graph, count_bits)?,
        atoms,
        2 * words,
        u64::from(threshold(graph.atoms)).min(atoms),
        u64::from(atom_entries),
        u64::from(set_words(graph.nodes)),
        u64::from(levels),
        u64::from(graph.nodes),
        u64::from(graph.edges),
        u64::from(graph.numeric_work),
        u64::from(entries),
        words,
        atoms,
        u64::from(roots).max(words),
        RECORD_WORDS,
    ])
}

/// A summary's fixed operations: clearing the counts, unsupported and
/// false-root sets, then visiting every producer, root and retained word. The
/// device adds one count read per present atom.
pub(super) fn summary(graph: &Graph, count_bits: u32) -> Option<u64> {
    add(&[
        u64::from(count_words(graph.atoms, count_bits)),
        2 * u64::from(graph.words),
        u64::from(set_words(graph.roots)),
        u64::from(graph.producers),
        u64::from(graph.roots),
    ])
}

/// The update path's floor at `size` invocations: what each invocation does of
/// the update's uniform word scans, which no phase tallies, and the record.
/// The change count's and presence's passes over both candidates' words and
/// settling's scan are five strided loops over `words`; each level scans the
/// queued words its positions span, `ceil(end / 32) - floor(start / 32)`, with
/// one more operation; and invocation 0 writes the record's words. Linear in
/// the levels. `None` when the offsets are malformed or a sum exceeds `u32`.
pub(super) fn floor_path(words: u32, offsets: &[u32], size: u32) -> Option<u32> {
    let share = |count: u64| count.div_ceil(u64::from(size));
    let mut path = 5 * share(u64::from(words)) + RECORD_WORDS;
    for level in offsets.windows(2) {
        let (start, end) = (u64::from(level[0]), u64::from(level[1]));
        let spanned = end.div_ceil(32).checked_sub(start / 32)?;
        path = path.checked_add(share(spanned) + 1)?;
    }
    u32::try_from(path).ok()
}

/// Dense reuse's least cost of evaluating packed node `index`, by the guard's
/// cost table: an atom, the falsum or a constant 1; a pair or implication 3,
/// the node and both operands; a wide conjunction or disjunction 2, the node
/// and the one operand that may end its reduction; a numeric row's owner
/// `n + g`, the change-bit reads and guard clears of a row none of whose `n`
/// conditions changed, since evaluating its `g` guards costs at least
/// `2 + n + g`; and a row's follower 1, its ownership check. `None` for a
/// malformed row.
fn dense_cost(nodes: &[u32], node_count: u32, index: u32) -> Option<u64> {
    let start = index as usize * 4;
    let header = nodes.get(start..start + 3)?;
    Some(match header[0] {
        2..=4 => 3,
        5 | 6 => 2,
        7..=9 => {
            let tail = node_count as usize * 4;
            let row = tail + *nodes.get(tail + header[1] as usize)? as usize + 3;
            if *nodes.get(row - 3)? == index {
                u64::from(header[2]) + u64::from(*nodes.get(row - 2)?)
            } else {
                1
            }
        }
        _ => 1,
    })
}

/// The dense path at `size` invocations, by the guard's cost table: a lower
/// bound on dense reuse's busiest invocation's cost, summed over its phases.
/// Each level costs its bounds and its busiest invocation's nodes, where dense
/// reuse gives invocation `lane` the positions `start + lane + j * size`; then
/// support construction: atomic support clears the support words, then visits
/// the roots and producers in one phase, while grouped support visits the
/// roots and each support word with its group's producers in one phase; then
/// the unsupported scan, two words per candidate word, and the record. Where
/// dense reuse's cost depends on the candidate, each node takes its least
/// ([`dense_cost`]). One pass over the schedule's positions, the producer
/// groups and buffers of `size` sums: `O(nodes + producers + roots + words)`.
///
/// # Errors
/// Reports cancellation and a failed reservation; a malformed schedule is a
/// capacity failure, which packing never produces.
pub(super) fn dense_path(
    graph: &Graph,
    packed: &Packed,
    size: u32,
    support: TightSupport,
    cancellation: &Cancellation,
) -> Result<u64, GpuError> {
    let malformed = || capacity("tight schedule offsets exceed the packed schedule");
    let cost = |node: u32| dense_cost(&packed.nodes, graph.nodes, node).ok_or_else(malformed);
    let mut path = levels_path(graph, &packed.roots, size, cost, cancellation)?;
    let share = |count: u32| u64::from(count.div_ceil(size));
    path += u64::from(graph.levels);
    path += match support {
        TightSupport::Atomic => share(graph.words) + share(graph.roots) + share(graph.producers),
        TightSupport::Grouped => {
            // Invocation `lane` visits the roots `lane + j * size` and writes
            // the support words `lane + j * size`, each after visiting the
            // word's producers.
            let start = graph.producers as usize * 4;
            let groups = packed
                .producers
                .get(start..=start + graph.words as usize)
                .ok_or_else(malformed)?;
            let lanes = size as usize;
            let mut sums = vector::<u64>(lanes)?;
            sums.extend(
                (0..size).map(|lane| u64::from(graph.roots.saturating_sub(lane).div_ceil(size))),
            );
            for (word, bounds) in groups.windows(2).enumerate() {
                poll(cancellation)?;
                sums[word % lanes] += 1 + u64::from(bounds[1] - bounds[0]);
            }
            sums.iter().copied().max().unwrap_or(0)
        }
    };
    Ok(path + 2 * share(graph.words) + RECORD_WORDS)
}

/// Dense evaluation's busiest invocation's `cost`, summed over the dependency
/// levels of the packed schedule `roots` (root IDs, then the levels' offsets,
/// then the positions' node IDs) at `size` invocations: invocation `lane` takes
/// a level's positions `start + lane + j * size`. With unit cost it is the
/// strided depth. One pass over the positions with `size` sums.
///
/// # Errors
/// Reports cancellation, a failed reservation, and a malformed schedule or a
/// failing `cost`.
pub(super) fn levels_path(
    graph: &Graph,
    roots: &[u32],
    size: u32,
    mut cost: impl FnMut(u32) -> Result<u64, GpuError>,
    cancellation: &Cancellation,
) -> Result<u64, GpuError> {
    let malformed = || capacity("tight schedule offsets exceed the packed schedule");
    let start = graph.roots as usize;
    let order = start + graph.levels as usize + 1;
    let offsets = roots.get(start..order).ok_or_else(malformed)?;
    let lanes = size as usize;
    let mut sums = vector::<u64>(lanes)?;
    sums.resize(lanes, 0);
    let mut path = 0_u64;
    for level in offsets.windows(2) {
        let positions = roots
            .get(order + level[0] as usize..order + level[1] as usize)
            .ok_or_else(malformed)?;
        let used = positions.len().min(lanes);
        sums[..used].fill(0);
        for (offset, &node) in positions.iter().enumerate() {
            poll(cancellation)?;
            sums[offset % lanes] += cost(node)?;
        }
        path += sums[..used].iter().copied().max().unwrap_or(0);
    }
    Ok(path)
}

/// Whether the runtime guard declines the route after a batch whose slowest
/// update slot's path is `slowest`, against dense reuse's path `dense`: that
/// update costs more than dense reuse does at least. Equality keeps the route.
pub(super) const fn declines(slowest: u64, dense: u64) -> bool {
    slowest > dense
}

/// The runtime guard of a resident theory, which decides whether the batches
/// planned next may update. It compares critical paths by one cost table: the
/// slowest update slot's path, the costliest invocation of each phase after
/// the summary summed, and of the summary's phases too when the slot took
/// dense reuse beyond the threshold before it, against dense reuse's least
/// path for one candidate. It is in one of two states. Engaged, a batch with
/// update slots is judged, and one that loses declines the route for the next
/// `backoff` successfully decoded batches, the backoff doubling up to
/// [`GUARD_WAIT_LIMIT`]; it never shrinks while the theory stays resident.
/// Declined, every decoded batch counts the wait down and is not judged; at
/// zero the guard is engaged again. The batch planned next then seeds every
/// slot, since every record written while the guard declined says unseeded,
/// and the batch after it updates and is judged. A failed or cancelled batch
/// is never settled, so it is not judged and consumes no wait. The guard
/// changes which route evaluates a batch, never a verdict.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Guard {
    /// Successfully decoded batches left to run with the route not allowed;
    /// zero while engaged.
    waiting: u32,
    /// The wait the next declining judgment sets.
    backoff: u32,
}

impl Guard {
    /// An engaged guard with a backoff of one batch.
    pub(super) const fn new() -> Self {
        Self {
            waiting: 0,
            backoff: 1,
        }
    }

    /// Whether the batch planned next may set the route allowed: whether the
    /// guard is engaged.
    pub(super) const fn allows(self) -> bool {
        self.waiting == 0
    }

    /// Settle one successfully decoded batch, after its reuse authority is
    /// re-established. Its slowest update slot's path is `slowest`, or `None`
    /// without update slots, against dense reuse's path `dense`. Declined, the
    /// batch counts the wait down; engaged, a batch with update slots is
    /// judged.
    pub(super) fn settle(&mut self, slowest: Option<u64>, dense: u64) {
        if self.waiting != 0 {
            self.waiting -= 1;
        } else if slowest.is_some_and(|slowest| declines(slowest, dense)) {
            self.waiting = self.backoff;
            self.backoff = (self.backoff * 2).min(GUARD_WAIT_LIMIT);
        }
    }
}

/// A resident theory's update admission, decided at most once per step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Stage {
    /// The theory evaluates original truth serially, which has no levels.
    Serial,
    /// Dense evaluation takes fewer than [`ENGAGEMENT_STRIDES`] strides per
    /// level at the resident's workgroup size, so the rule declines the route.
    Narrow,
    /// No batch holding reuse authority at its stateless count has decided the
    /// bounds yet.
    Pending,
    /// The bounds hold; the index awaits a batch that admits it.
    Bounded(Bounds),
    /// The readers index is resident.
    Built(Index),
    /// An intrinsic bound refused the index, while the theory stays resident.
    Refused,
}

impl Stage {
    /// The stage a theory starts in once its schedule is packed: a serial
    /// schedule has no levels to update by, and the engagement rule declines
    /// a narrow one.
    pub(super) const fn of(graph: &Graph) -> Self {
        if graph.levels == 0 {
            Self::Serial
        } else if engages(graph.strided_depth, graph.levels) {
            Self::Pending
        } else {
            Self::Narrow
        }
    }

    /// The theory's bounds at a batch holding reuse authority: decided once by
    /// `decide` and kept. A refusal is kept too, so no later batch decides the
    /// bounds again or builds. A serial or built stage has no bounds to decide.
    pub(super) fn bounds(&mut self, decide: impl FnOnce() -> Option<Bounds>) -> Option<Bounds> {
        match *self {
            Self::Bounded(bounds) => Some(bounds),
            Self::Pending => {
                let decided = decide();
                *self = decided.map_or(Self::Refused, Self::Bounded);
                decided
            }
            Self::Serial | Self::Narrow | Self::Built(_) | Self::Refused => None,
        }
    }
}

/// A built readers index, resident after the producer records.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Index {
    /// Word offset of the per-node offsets; entries follow them.
    pub(super) readers: u32,
    /// Word offset of the per-atom offsets; atom-node entries follow them.
    pub(super) atom_nodes: u32,
    pub(super) entries: u32,
    pub(super) atom_entries: u32,
    pub(super) count_bits: u32,
    pub(super) slot_words: u32,
}

/// The rebuilt producer buffer: `records` followed by the readers index.
pub(super) struct Built {
    pub(super) words: Vec<u32>,
    pub(super) index: Index,
}

/// Build the readers index after `records`, the packed producer records and
/// grouped offsets. Entries name, for every node, the connectives reading it,
/// the owner of every live numeric row holding it as a condition (one entry
/// per occurrence), the head of every producer whose body it is, and the root
/// positions holding it; and, for every atom, its atom nodes. One counting
/// pass and one filling pass, each `O(nodes + edges + producers + roots)`.
///
/// # Errors
/// Reports cancellation, a failed reservation, or a table beyond its bounds.
pub(super) fn build(
    graph: &Graph,
    certificate: &TightPlan,
    owners: &[(u32, u32)],
    bounds: Bounds,
    records: &[u32],
    cancellation: &Cancellation,
) -> Result<Built, GpuError> {
    let nodes = graph.nodes as usize;
    let atoms = graph.atoms as usize;
    let mut readers = vector::<u32>(nodes + 1)?;
    readers.resize(nodes + 1, 0);
    let mut atom_nodes = vector::<u32>(atoms + 1)?;
    atom_nodes.resize(atoms + 1, 0);
    // Counts occupy the next node's slot; prefix sums turn them into
    // half-open offsets ending at the entry count.
    visit(
        graph,
        certificate,
        owners,
        cancellation,
        |slot, _| match slot {
            Slot::Reader(node) => readers[node + 1] += 1,
            Slot::Atom(atom) => atom_nodes[atom + 1] += 1,
        },
    )?;
    prefix(&mut readers, cancellation)?;
    prefix(&mut atom_nodes, cancellation)?;
    let entries = readers[nodes];
    let atom_entries = atom_nodes[atoms];
    if entries > bounds.entries || atom_entries > bounds.atom_entries {
        return Err(capacity("tight readers index exceeds its admitted bound"));
    }
    let base = u32::try_from(records.len()).map_err(|_| capacity("tight producers exceed u32"))?;
    let reader_start = base + graph.nodes + 1;
    let atom_offsets = reader_start + entries;
    let total = atom_offsets as usize + atoms + 1 + atom_entries as usize;
    let mut words = vector::<u32>(total)?;
    words.extend_from_slice(records);
    words.extend_from_slice(&readers);
    words.resize(atom_offsets as usize, 0);
    words.extend_from_slice(&atom_nodes);
    words.resize(total, 0);
    let mut reader_cursors = vector::<u32>(nodes)?;
    reader_cursors.extend_from_slice(&readers[..nodes]);
    let mut atom_cursors = vector::<u32>(atoms)?;
    atom_cursors.extend_from_slice(&atom_nodes[..atoms]);
    drop(readers);
    drop(atom_nodes);
    let atom_start = atom_offsets as usize + atoms + 1;
    visit(graph, certificate, owners, cancellation, |slot, entry| {
        let (start, cursor) = match slot {
            Slot::Reader(node) => (reader_start as usize, &mut reader_cursors[node]),
            Slot::Atom(atom) => (atom_start, &mut atom_cursors[atom]),
        };
        words[start + *cursor as usize] = entry;
        *cursor += 1;
    })?;
    Ok(Built {
        words,
        index: Index {
            readers: base,
            atom_nodes: atom_offsets,
            entries,
            atom_entries,
            count_bits: bounds.count_bits,
            slot_words: bounds.slot_words,
        },
    })
}

/// Where an index entry belongs: the readers list of a node, or the atom-node
/// list of an atom.
#[derive(Clone, Copy)]
enum Slot {
    Reader(usize),
    Atom(usize),
}

/// Pass every readers entry and every atom-node entry to `sink`, in one fixed
/// order.
fn visit(
    graph: &Graph,
    certificate: &TightPlan,
    owners: &[(u32, u32)],
    cancellation: &Cancellation,
    mut sink: impl FnMut(Slot, u32),
) -> Result<(), GpuError> {
    // Bounds were checked before: every node ID, atom and root position fits
    // an entry's 30 bits, so these conversions cannot fail.
    let entry = |index: usize| {
        u32::try_from(index)
            .ok()
            .filter(|index| *index < INDEX_LIMIT)
            .ok_or_else(|| capacity("tight readers index exceeds 30 bits"))
    };
    let theory = &graph.theory;
    let view = theory.view();
    for index in 0..graph.nodes as usize {
        poll(cancellation)?;
        let entry = READER_NODE | entry(index)?;
        match view.node(index).expect("admitted node") {
            NodeView::And(children) | NodeView::Or(children) => {
                for &child in children {
                    sink(Slot::Reader(child), entry);
                }
            }
            NodeView::Implies(left, right) => {
                sink(Slot::Reader(left), entry);
                sink(Slot::Reader(right), entry);
            }
            NodeView::Atom(atom) => sink(Slot::Atom(atom), entry),
            NodeView::False | NodeView::Threshold(_) | NodeView::ExtremumCut(_) => {}
        }
    }
    let parts = theory.parts();
    for &(source, owner) in owners {
        poll(cancellation)?;
        let row = parts.rows()[source as usize];
        for &condition in &parts.conditions()[row.start..row.start + row.length] {
            sink(Slot::Reader(condition), READER_NODE | owner);
        }
    }
    for producer in certificate.producers() {
        poll(cancellation)?;
        if let Some(body) = producer.body() {
            sink(
                Slot::Reader(body),
                READER_PRODUCER | entry(producer.head())?,
            );
        }
    }
    for (position, &root) in theory.roots().iter().enumerate() {
        poll(cancellation)?;
        sink(Slot::Reader(root), READER_ROOT | entry(position)?);
    }
    Ok(())
}

fn prefix(counts: &mut [u32], cancellation: &Cancellation) -> Result<(), GpuError> {
    let mut total = 0_u32;
    for count in counts {
        poll(cancellation)?;
        total = total
            .checked_add(*count)
            .ok_or_else(|| capacity("tight readers index exceeds u32"))?;
        *count = total;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
