//! The update route's host side: the readers index, its intrinsic bounds, the
//! per-slot state's shape and the route's derived work.
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

use super::packing::Graph;
use super::poll;
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
/// Reader kinds in an entry's two high bits; the low 30 bits are its index.
pub(super) const READER_NODE: u32 = 0;
pub(super) const READER_PRODUCER: u32 = 1 << 30;
pub(super) const READER_ROOT: u32 = 2 << 30;
/// One more than the largest node ID, atom or root position an entry names.
pub(super) const INDEX_LIMIT: u32 = 1 << 30;
/// The per-slot summary block: summarized, and the false-root and unsupported
/// counts. Whether a slot is seeded lives in its result record instead.
pub(super) const SUMMARY_WORDS: u32 = 3;
/// The binding a transport without state supplies in the state's place.
pub(super) const PLACEHOLDER_BYTES: u64 = 16;
/// Words of the result record: epoch, slot, status, witness, work, marker,
/// route, nodes evaluated and seeded. Writing it is part of the update floor.
pub(super) const RECORD_WORDS: u64 = 9;

/// Workgroup strides per dependency level that dense evaluation must take, on
/// average, before the update route engages.
///
/// Dense evaluation of one candidate takes `I(W) = sum(ceil(width / W))`
/// strided iterations over the level widths at the dispatched workgroup size
/// `W`. An update keeps every level's synchronization, adds five per
/// candidate (eight when it summarizes), and at each level evaluates each
/// queued word's taken nodes, with their readers' queue bits, counts and root
/// sets, in one invocation through storage atomics. It pays only where `I(W)`
/// exceeds the level count several times; how many times depends on the
/// device, and grows with how many nodes of one queued word a change reaches,
/// which no structural rule can see. The factor is an execution choice,
/// calibrated by measurement on an Apple M4 Pro GPU under Metal, and changes no
/// verdict. Declining keeps the dense routes, so a factor set high forgoes
/// gains, while one set low slows the schedules it engages.
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
