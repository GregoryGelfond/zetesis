//! Size-specialized implementations of original-region narrowing.
//!
//! Narrowing a region (`Region.narrow` and the `refuted` constructor of
//! `Search.lean`'s coverage tree) may refute it only when no accepted
//! candidate lies in it. The general narrowing reaches that verdict through
//! the closure over the theory's nodes, support and foundedness. Below a
//! measured size, the same operation has cheaper specialized
//! implementations, the way a sort hands its small runs to insertion sort:
//!
//! * [`Variant::Enumerated`] reads the region's `2^k` completions over its `k`
//!   open atoms in packed 64-lane words and evaluates, lane-parallel, the
//!   nodes above an open atom, the roots, and the support condition. A region
//!   none of whose completions passes holds no accepted candidate.
//! * [`Variant::Learned`] reads the same necessary conditions as clauses over
//!   atoms and nodes, and searches the region's completions with conflict
//!   learning, the region's decisions taken as assumptions. A refutation
//!   under those assumptions shows that no completion satisfies the
//!   conditions. Every learned clause is a consequence of the conditions
//!   alone, so it is kept for later regions of the same theory.
//!
//! The necessary conditions are exactly those the general narrowing already
//! reads (`LeafRefinement.lean`): every root holds classically, and every held
//! atom has a producer, not circular, whose body holds and whose other
//! ordinary heads fail, or an atomic choice whose body holds. Thresholds are
//! read exactly; an ordinal extremum cut is left unconstrained in the clausal
//! reading, which only weakens it, and declines enumeration.
//!
//! Neither variant decides an atom or splits a region: each either refutes the
//! region or leaves it as the general narrowing closed it, so the coverage
//! tree, its outputs and their order are those of the general walk with some
//! empty subtrees replaced by one refutation. The policy selects them by the
//! region's open-atom count; the measured policy remains the default.

use std::{mem::size_of, time::Instant};

use zetesis_algebra::{
    Cancellation, Failure, Meter, Metered, Quota, Refusal, Stop, region::Region,
};

use super::Producers;
use crate::{NodeView, Theory};

/// A remembered refutation core: the restriction guards it assumed, and per
/// decision word the held and cut atoms it used.
type Core = (Vec<u32>, Vec<(u32, u64, u64)>);

/// Which specialization a region of a given size takes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SmallPolicy {
    /// Enumerate regions with at most this many open atoms (0: never). At
    /// most 22 (`2^22` completions).
    pub enumerate_up_to: usize,
    /// Search regions with more open atoms than enumeration takes, and at
    /// most this many, with learning (0: never).
    pub learn_up_to: usize,
    /// Conflicts one learned search may spend before it leaves the region to
    /// the general narrowing. This local allowance does not replace the global
    /// work ceiling: encoding and every admitted search visit use that account.
    pub learn_conflicts: u64,
    /// Adapt the learned search's size from this start (0: fixed): a
    /// refutation raises it past the refuted region's size, a region left
    /// open lowers it, as cube-and-conquer adapts its cutoff.
    pub adapt_from: usize,
    /// Let the learned search decide every encoding variable, not only atoms
    /// (and free extremum cuts); node variables are defined by them.
    pub decide_all: bool,
    /// Failed-literal probes one region may spend (0: never): each open atom
    /// is tried both ways by propagation alone; both failing refutes.
    pub probe: usize,
    /// Refutation yield, in percent, below which the learned search stops
    /// reading regions (0: always read). The yield is a moving average of
    /// the learned path's verdicts; a region left open, a cached completion
    /// and a spent allowance count against it. One region in
    /// 32 skipped is still read, so the yield can recover.
    pub gate: usize,
    /// The learned search reads only regions with at least this many open
    /// atoms (0: any): below it the general narrowing's own subtree is small
    /// and cheaper than one search over the whole encoding.
    pub learn_from: usize,
    /// The learned search declines a theory whose at-most-one atoms are at
    /// least this percentage of its atoms (0: never).
    pub matching: usize,
    /// The learned search starts only after this many regions were offered
    /// to it, so a run the general narrowing settles quickly builds no
    /// encoding (0: from the first).
    pub learn_after: usize,
    /// The learned search declines a theory with more than this many formula
    /// nodes per atom (0: any): its clausal encoding would restate, at
    /// several times the size, arithmetic the algebra already reads natively.
    pub density: usize,
    /// Only one walker runs the learned search: the one its walk designates
    /// (the calling thread's walker while the walk is single-threaded, the
    /// first parallel worker once workers start), or, for a walker no walk
    /// designates, the one on the main thread. Parallel workers would each
    /// build and search their own encoding, multiplying its memory by the
    /// worker count and repeating its work.
    pub main_only: bool,
    /// Nanoseconds per formula node the walker must have walked since its
    /// first region offered to the learned search before it reads the shape
    /// and builds the clausal encoding (0: at once). Both cost about linear
    /// time in the theory's nodes, so a walk that ends sooner never pays
    /// for them, and one that runs on spends at most about as long again.
    pub amortize: usize,
    /// The learned search yields to idle parallel peers: it starts no
    /// search while one waits, and a running search ends as spent when one
    /// starts waiting, so the walker splits its region for them.
    pub yield_idle: bool,
}

/// The measured shape-and-encoding cost per formula node, in nanoseconds,
/// that [`SmallPolicy::measured`] amortizes.
const AMORTIZE_NANOS_PER_NODE: usize = 1500;

/// No enumeration reads more than `2^22` completions.
pub(crate) const ENUMERATION_CEILING: usize = 22;

/// The yield gate's unit: a yield of 1.
const YIELD_ONE: u32 = 1 << 10;

/// The yield gate still reads one region in this many it would skip.
const GATE_RETRY: u32 = 32;

/// Recent refutation cores a walker keeps.
const CORES: usize = 64;

impl SmallPolicy {
    /// The measured default: the learned search on regions of at least 256
    /// open atoms, 500 conflicts each, behind the yield gate (25%), outside
    /// matching-shaped (at-most-one atoms >= 50%) and formula-dense (more
    /// than 32 nodes per atom) theories, in the designated walker only.
    #[must_use]
    pub const fn measured() -> Self {
        Self {
            enumerate_up_to: 0,
            learn_up_to: usize::MAX,
            learn_conflicts: 500,
            adapt_from: 0,
            decide_all: false,
            probe: 0,
            gate: 25,
            learn_from: 256,
            matching: 50,
            learn_after: 0,
            density: 32,
            main_only: true,
            amortize: AMORTIZE_NANOS_PER_NODE,
            yield_idle: true,
        }
    }

    /// The policy named by `ZETESIS_SMALL_REGIONS`, e.g.
    /// `enumerate=10,learn=256,conflicts=4000`; [`Self::measured`] when
    /// unset, and off when `off`.
    #[must_use]
    pub fn from_environment() -> Self {
        let Ok(text) = std::env::var("ZETESIS_SMALL_REGIONS") else {
            return Self::measured();
        };
        Self::parse(&text)
    }

    /// The policy a `key=value,...` text names; keys left out are off, and
    /// `off` names no specialization.
    #[must_use]
    pub fn parse(text: &str) -> Self {
        let mut policy = Self {
            learn_conflicts: 4000,
            ..Self::default()
        };
        for part in text.split(',') {
            let Some((key, value)) = part.split_once('=') else {
                continue;
            };
            let value = value.trim();
            let parsed = if value == "inf" {
                usize::MAX
            } else {
                value.parse().unwrap_or(0)
            };
            match key.trim() {
                "enumerate" => policy.enumerate_up_to = parsed.min(ENUMERATION_CEILING),
                "learn" => policy.learn_up_to = parsed,
                "conflicts" => policy.learn_conflicts = parsed as u64,
                "adapt" => policy.adapt_from = parsed,
                "decide" => policy.decide_all = value == "all",
                "probe" => policy.probe = parsed,
                "gate" => policy.gate = parsed,
                "from" => policy.learn_from = parsed,
                "matching" => policy.matching = parsed,
                "after" => policy.learn_after = parsed,
                "density" => policy.density = parsed,
                "main" => policy.main_only = parsed > 0,
                "amortize" => policy.amortize = parsed,
                "idle" => policy.yield_idle = parsed > 0,
                _ => {}
            }
        }
        policy
    }

    /// The environment's policy, read once per process.
    pub(crate) fn environment() -> Self {
        static POLICY: std::sync::OnceLock<SmallPolicy> = std::sync::OnceLock::new();
        *POLICY.get_or_init(Self::from_environment)
    }

    pub(crate) const fn active(&self) -> bool {
        self.enumerate_up_to > 0 || self.learn_up_to > 0 || self.adapt_from > 0 || self.probe > 0
    }
}

/// The specialization that read a region.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variant {
    /// Packed enumeration of every completion.
    Enumerated,
    /// Clausal search with conflict learning.
    Learned,
    /// Failed-literal probing by propagation, without search.
    Probed,
}

/// What a specialized narrowing concluded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SmallNarrowing {
    /// No specialization is eligible under the policy or packed shape.
    Declined,
    /// Some necessary-condition completion passes, or the local search/probe
    /// allowance was spent: the region stays as general narrowing closed it.
    /// This does not certify an answer set; global refusal is returned as an error.
    Open(Variant),
    /// No completion of the region satisfies the necessary conditions.
    Refuted(Variant),
}

/// Diagnostic counts and time of one walker's specialized narrowings.
/// The `Metered::charged` receipt, rather than these counters, accounts work.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SmallStatistics {
    /// Regions read by enumeration.
    pub enumerated: u64,
    /// Of those, regions refuted.
    pub enumerated_refuted: u64,
    /// Completions enumerated (lanes, including padding of a partial word).
    pub lanes: u64,
    /// Regions refuted by a recent core, with no search.
    pub core_hits: u64,
    /// Regions read by the learned search.
    pub learned: u64,
    /// Of those, regions refuted.
    pub learned_refuted: u64,
    /// Learned searches that spent their conflict allowance.
    pub learned_exhausted: u64,
    /// Learned searches answered by a cached completion.
    pub cached: u64,
    /// Search decisions, conflicts and propagated literals.
    pub decisions: u64,
    /// Conflicts of the learned search.
    pub conflicts: u64,
    /// Literals the learned search propagated.
    pub propagations: u64,
    /// Clauses learned and kept.
    pub learned_clauses: u64,
    /// Sum of the widths of refutation cores (assumptions used).
    pub core_width: u64,
    /// Nanoseconds spent in enumeration and in the learned search.
    pub enumerated_nanos: u64,
    /// Nanoseconds spent in the learned search.
    pub learned_nanos: u64,
    /// Regions the yield gate kept from the learned search.
    pub gated: u64,
    /// Regions probed for failed literals.
    pub probed: u64,
    /// Probed regions refuted.
    pub probed_refuted: u64,
    /// Time spent probing.
    pub probed_nanos: u64,
    /// Clausal encodings built.
    pub encodings: u64,
    /// Atoms of the theory last read.
    pub atoms: u64,
    /// Of those, the atoms an at-most-one root reads.
    pub amo_atoms: u64,
    /// Formula nodes of the theory last read.
    pub nodes: u64,
    /// Regions the theory's shape (matching or density) kept from the
    /// learned search.
    pub shape_declined: u64,
    /// Regions offered before the walk had run long enough to amortize the
    /// shape and encoding.
    pub deferred: u64,
    /// Regions not searched, and searches ended, because a parallel peer
    /// was idle.
    pub yielded: u64,
}

/// A walker's specialized-narrowing state: its policy, the clausal reading of
/// the theory it last read, kept with its learned clauses, and enumeration
/// buffers. It belongs to the walker's scratch, never to a region.
#[derive(Debug, Default)]
pub(crate) struct SmallScratch {
    policy: Option<SmallPolicy>,
    shape: Option<Shape>,
    solver: Option<Solver>,
    lanes: Vec<u64>,
    stamp: Vec<u32>,
    epoch: u32,
    cone: Vec<usize>,
    constant: Vec<u8>,
    assumptions: Vec<u32>,
    /// Recent refutation cores over atoms, as (word, held bits, cut bits),
    /// most recently used first: a region deciding a core's literals holds no
    /// completion satisfying the necessary conditions.
    cores: Vec<Core>,
    /// Guards of the restrictions active at the last learned search.
    guards: Vec<u32>,
    /// The adapted learned-search size, when adapting.
    cutoff: usize,
    /// Moving average of learned-path refutations, out of [`YIELD_ONE`].
    yield_score: u32,
    /// Regions the yield gate skipped since its last read.
    skipped: u32,
    /// Regions offered to the learned search so far.
    offered: u64,
    /// Whether this walker runs the learned search under `main_only`: set by
    /// its walk, or read once from the thread's name.
    designated: Option<bool>,
    /// When this walker first offered a region to the learned search.
    first_offer: Option<Instant>,
    /// Idle peers of a parallel walker, when its walk shares the count.
    idle: Option<std::sync::Arc<std::sync::atomic::AtomicUsize>>,
    /// Completions recently found to pass, as held-atom words.
    models: Vec<Vec<u64>>,
    /// The counts so far.
    pub(crate) statistics: SmallStatistics,
}

impl Drop for SmallScratch {
    fn drop(&mut self) {
        let s = &self.statistics;
        if (s.enumerated > 0
            || s.learned > 0
            || s.cached > 0
            || s.shape_declined > 0
            || s.deferred > 0)
            && reports()
        {
            eprintln!("zetesis-small-regions {s:?}");
        }
    }
}

/// Whether this thread writes the diagnostics `ZETESIS_SMALL_STATS` asks
/// for. Only the main thread does: a parallel worker writing to stderr while
/// the caller holds it to report the run would never finish.
fn reports() -> bool {
    std::thread::current().name() == Some("main")
        && std::env::var_os("ZETESIS_SMALL_STATS").is_some()
}

/// A node's shape, read once per theory.
#[derive(Debug)]
struct Shape {
    theory: Theory,
    atoms: usize,
    /// Every node's kind, bit-packed: the kind in the top [`KIND_BITS`]
    /// bits, an atom node's atom or a threshold's index below them.
    kinds: Vec<u32>,
    /// Every node's operands, flattened in node order, and each node's
    /// first operand (one past the last node: the operand count).
    operands: Vec<u32>,
    offsets: Vec<u32>,
    /// Each threshold's bound and the start of its weights in `weights`,
    /// which holds threshold weights only, in node order.
    thresholds: Vec<(i128, u32)>,
    weights: Vec<u64>,
    /// Logical parents of each node.
    parents_offsets: Vec<u32>,
    parents: Vec<u32>,
    /// Nodes reading each atom.
    atom_nodes_offsets: Vec<u32>,
    atom_nodes: Vec<u32>,
    roots: Vec<usize>,
    /// Support rows: for each atom with a support condition, its producers as
    /// (body or none, other ordinary heads, choice).
    support: Option<Support>,
    has_cuts: bool,
    /// Atoms read by an at-most-one root: `a ∧ b → ⊥`, or a unit-weight
    /// threshold of two over atoms implying `⊥`.
    amo_atoms: usize,
}

/// Bits of a packed node kind that name the kind.
const KIND_BITS: u32 = 3;

/// The payload bits of a packed node kind.
const PAYLOAD: u32 = (1 << (32 - KIND_BITS)) - 1;

/// A node's kind packed with its atom or threshold index, or `None` when
/// the payload does not fit.
fn pack_kind(kind: Kind, payload: usize) -> Option<u32> {
    let tag: u32 = match kind {
        Kind::Atom(_) => 0,
        Kind::False => 1,
        Kind::Implies => 2,
        Kind::And => 3,
        Kind::Or => 4,
        Kind::Threshold => 5,
        Kind::Cut => 6,
    };
    let payload = u32::try_from(payload).ok().filter(|&p| p <= PAYLOAD)?;
    Some((tag << (32 - KIND_BITS)) | payload)
}

#[derive(Debug)]
struct Support {
    /// Producers available to each atom: offsets into `entries`.
    offsets: Vec<u32>,
    /// (body node or `u32::MAX`, start/end into `others`).
    entries: Vec<(u32, u32, u32)>,
    others: Vec<u32>,
    /// For each node, the atoms whose support reads it as a body; for each
    /// atom, the atoms whose support reads it as a competing head.
    body_heads_offsets: Vec<u32>,
    body_heads: Vec<u32>,
    rival_offsets: Vec<u32>,
    rivals: Vec<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Atom(u32),
    False,
    Implies,
    And,
    Or,
    Threshold,
    Cut,
}

/// Copy operand identities in source order, admitting each visited cell.
fn extend_operands<Q: Quota + ?Sized>(
    operands: &mut Vec<u32>,
    row: &[usize],
    work: &mut Meter<'_, Q>,
) -> Result<(), Failure<Q::Error>> {
    for &child in row {
        work.tick()?;
        try_push(
            operands,
            u32::try_from(child).map_err(|_| Stop::StorageLimit)?,
        )?;
    }
    Ok(())
}

/// Packed adjacency rows: each offset range selects its flat entries.
struct Adjacency {
    offsets: Vec<u32>,
    entries: Vec<u32>,
}

fn csr<Q: Quota + ?Sized>(
    rows: usize,
    edges: &[(u32, u32)],
    work: &mut Meter<'_, Q>,
) -> Result<Adjacency, Failure<Q::Error>> {
    let mut offsets = filled_row(rows.checked_add(1).ok_or(Stop::StorageLimit)?, 0_u32, work)?;
    for &(row, _) in edges {
        work.tick()?;
        offsets[row as usize + 1] = offsets[row as usize + 1]
            .checked_add(1)
            .ok_or(Stop::StorageLimit)?;
    }
    for row in 0..rows {
        work.tick()?;
        offsets[row + 1] = offsets[row + 1]
            .checked_add(offsets[row])
            .ok_or(Stop::StorageLimit)?;
    }
    let mut cursor = copy_row(&offsets, work)?;
    let mut entries = filled_row(edges.len(), 0_u32, work)?;
    for &(row, value) in edges {
        work.tick()?;
        entries[cursor[row as usize] as usize] = value;
        cursor[row as usize] += 1;
    }
    Ok(Adjacency { offsets, entries })
}

impl Support {
    fn retained_bytes(&self) -> u128 {
        bytes(&self.offsets)
            + bytes(&self.entries)
            + bytes(&self.others)
            + bytes(&self.body_heads_offsets)
            + bytes(&self.body_heads)
            + bytes(&self.rival_offsets)
            + bytes(&self.rivals)
    }

    /// Each atom's producer rows, from the non-circular producers.
    fn read<Q: Quota + ?Sized>(
        producers: &Producers,
        atoms: usize,
        count: usize,
        work: &mut Meter<'_, Q>,
    ) -> Result<Self, Failure<Q::Error>> {
        let mut rows: Vec<Vec<(u32, u32, u32)>> = Vec::new();
        for _ in 0..atoms {
            work.tick()?;
            try_push(&mut rows, Vec::new())?;
        }
        let mut others = Vec::new();
        let mut body_edges = Vec::new();
        let mut rival_edges = Vec::new();
        for (index, rule) in producers.rules.iter().enumerate() {
            work.tick()?;
            if producers.circular(index) {
                continue;
            }
            let mut heads = copy_row(&rule.heads, work)?;
            sort_dedup(&mut heads, work)?;
            for &head in &heads {
                work.tick()?;
                let start = idx(others.len());
                if !rule.choice {
                    for &other in &heads {
                        work.tick()?;
                        if other != head {
                            try_push(&mut others, idx(other))?;
                            try_push(&mut rival_edges, (idx(other), idx(head)))?;
                        }
                    }
                }
                let body = if let Some(body) = rule.body {
                    try_push(&mut body_edges, (idx(body), idx(head)))?;
                    idx(body)
                } else {
                    u32::MAX
                };
                try_push(&mut rows[head], (body, start, idx(others.len())))?;
            }
        }
        let mut offsets = Vec::new();
        let mut entries = Vec::new();
        try_push(&mut offsets, 0)?;
        for row in rows {
            work.tick()?;
            append_copy(&mut entries, &row, work)?;
            try_push(&mut offsets, idx(entries.len()))?;
        }
        sort_dedup(&mut body_edges, work)?;
        sort_dedup(&mut rival_edges, work)?;
        let Adjacency {
            offsets: body_heads_offsets,
            entries: body_heads,
        } = csr(count, &body_edges, work)?;
        let Adjacency {
            offsets: rival_offsets,
            entries: rivals,
        } = csr(atoms, &rival_edges, work)?;
        Ok(Self {
            offsets,
            entries,
            others,
            body_heads_offsets,
            body_heads,
            rival_offsets,
            rivals,
        })
    }
}

/// Each node's distinct parents, in node order, as offsets and a flat row,
/// counted from the operand rows `offsets`/`operands` of `count` nodes.
fn parents_of<Q: Quota + ?Sized>(
    count: usize,
    offsets: &[u32],
    operands: &[u32],
    work: &mut Meter<'_, Q>,
) -> Result<Adjacency, Failure<Q::Error>> {
    let mut parents_offsets =
        filled_row(count.checked_add(1).ok_or(Stop::StorageLimit)?, 0_u32, work)?;
    for &child in operands {
        work.tick()?;
        parents_offsets[child as usize + 1] = parents_offsets[child as usize + 1]
            .checked_add(1)
            .ok_or(Stop::StorageLimit)?;
    }
    for node in 0..count {
        work.tick()?;
        parents_offsets[node + 1] = parents_offsets[node + 1]
            .checked_add(parents_offsets[node])
            .ok_or(Stop::StorageLimit)?;
    }
    let mut cursor = copy_row(&parents_offsets, work)?;
    let mut parents = filled_row(operands.len(), 0_u32, work)?;
    for node in 0..count {
        work.tick()?;
        for &child in &operands[offsets[node] as usize..offsets[node + 1] as usize] {
            work.tick()?;
            let slot = &mut cursor[child as usize];
            // A node reading the same child twice is its parent once.
            let row_start = parents_offsets[child as usize] as usize;
            if *slot as usize > row_start && parents[*slot as usize - 1] == idx(node) {
                continue;
            }
            parents[*slot as usize] = idx(node);
            *slot += 1;
        }
    }
    // Close the gaps left by repeated operands.
    let mut write = 0_usize;
    let mut compact_offsets = Vec::new();
    try_push(&mut compact_offsets, 0_u32)?;
    for node in 0..count {
        work.tick()?;
        let (start, end) = (parents_offsets[node] as usize, cursor[node] as usize);
        for read in start..end {
            work.tick()?;
            parents[write + read - start] = parents[read];
        }
        write += end - start;
        try_push(&mut compact_offsets, idx(write))?;
    }
    parents.truncate(write);

    drop(cursor);
    Ok(Adjacency {
        offsets: compact_offsets,
        entries: parents,
    })
}

impl Shape {
    fn retained_bytes(&self) -> u128 {
        bytes(&self.kinds)
            + bytes(&self.operands)
            + bytes(&self.offsets)
            + bytes(&self.thresholds)
            + bytes(&self.weights)
            + bytes(&self.parents_offsets)
            + bytes(&self.parents)
            + bytes(&self.atom_nodes_offsets)
            + bytes(&self.atom_nodes)
            + bytes(&self.roots)
            + self.support.as_ref().map_or(0, Support::retained_bytes)
    }

    fn read<Q: Quota + ?Sized>(
        theory: &Theory,
        producers: Option<&Producers>,
        work: &mut Meter<'_, Q>,
    ) -> Result<Option<Self>, Failure<Q::Error>> {
        let result = (|| -> Result<Self, Failure<Q::Error>> {
            let view = theory.view();
            let count = view.len();
            let atoms = theory.atom_count();
            let mut kinds = Vec::new();
            let mut operands: Vec<u32> = Vec::new();
            let mut offsets = Vec::new();
            let mut thresholds = Vec::new();
            let mut weights = Vec::new();
            let mut has_cuts = false;
            let mut atom_edges = Vec::new();
            for node in 0..count {
                work.tick()?;
                try_push(
                    &mut offsets,
                    u32::try_from(operands.len()).map_err(|_| Stop::StorageLimit)?,
                )?;
                let packed = match view.node(node).map_err(|_| Stop::InvalidProgram)? {
                    NodeView::Atom(atom) => {
                        try_push(&mut atom_edges, (idx(atom), idx(node)))?;
                        pack_kind(Kind::Atom(0), atom).ok_or(Stop::StorageLimit)?
                    }
                    NodeView::False => pack_kind(Kind::False, 0).ok_or(Stop::StorageLimit)?,
                    NodeView::Implies(left, right) => {
                        extend_operands(&mut operands, &[left, right], work)?;
                        pack_kind(Kind::Implies, 0).ok_or(Stop::StorageLimit)?
                    }
                    NodeView::And(row) => {
                        extend_operands(&mut operands, row, work)?;
                        pack_kind(Kind::And, 0).ok_or(Stop::StorageLimit)?
                    }
                    NodeView::Or(row) => {
                        extend_operands(&mut operands, row, work)?;
                        pack_kind(Kind::Or, 0).ok_or(Stop::StorageLimit)?
                    }
                    NodeView::Threshold(threshold) => {
                        extend_operands(&mut operands, threshold.conditions, work)?;
                        let index = thresholds.len();
                        try_push(
                            &mut thresholds,
                            (
                                threshold.bound,
                                u32::try_from(weights.len()).map_err(|_| Stop::StorageLimit)?,
                            ),
                        )?;
                        append_copy(&mut weights, threshold.weights, work)?;
                        pack_kind(Kind::Threshold, index).ok_or(Stop::StorageLimit)?
                    }
                    NodeView::ExtremumCut(cut) => {
                        has_cuts = true;
                        extend_operands(&mut operands, cut.row.conditions, work)?;
                        pack_kind(Kind::Cut, 0).ok_or(Stop::StorageLimit)?
                    }
                };
                try_push(&mut kinds, packed)?;
            }
            try_push(
                &mut offsets,
                u32::try_from(operands.len()).map_err(|_| Stop::StorageLimit)?,
            )?;
            let Adjacency {
                offsets: parents_offsets,
                entries: parents,
            } = parents_of(count, &offsets, &operands, work)?;
            let Adjacency {
                offsets: atom_nodes_offsets,
                entries: atom_nodes,
            } = csr(atoms, &atom_edges, work)?;
            drop(atom_edges);
            let support = producers
                .map(|producers| Support::read(producers, atoms, count, work))
                .transpose()?;

            let mut shape = Self {
                theory: theory.clone(),
                atoms,
                kinds,
                operands,
                offsets,
                thresholds,
                weights,
                parents_offsets,
                parents,
                atom_nodes_offsets,
                atom_nodes,
                roots: copy_row(theory.roots(), work)?,
                support,
                has_cuts,
                amo_atoms: 0,
            };
            shape.amo_atoms = shape.count_amo_atoms(work)?;
            Ok(shape)
        })();
        match result {
            Ok(shape) => Ok(Some(shape)),
            // This shape's packed indices/CSR cannot represent the theory:
            // decline the optional specialization. Caller quota failures are
            // Failure::Refused, and allocation failures remain Allocation.
            Err(Failure::Stopped(Stop::StorageLimit)) => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// The node count.
    fn nodes(&self) -> usize {
        self.kinds.len()
    }

    /// A node's kind, unpacked.
    fn kind(&self, node: usize) -> Kind {
        let packed = self.kinds[node];
        match packed >> (32 - KIND_BITS) {
            0 => Kind::Atom(packed & PAYLOAD),
            1 => Kind::False,
            2 => Kind::Implies,
            3 => Kind::And,
            4 => Kind::Or,
            5 => Kind::Threshold,
            _ => Kind::Cut,
        }
    }

    /// A threshold node's bound.
    fn bound(&self, node: usize) -> i128 {
        self.thresholds[(self.kinds[node] & PAYLOAD) as usize].0
    }

    /// The atoms an at-most-one root reads. Exactly-one choices crossed
    /// with at-most-one constraints (pigeonhole, permutation and matching
    /// shapes) are where resolution needs exponentially many steps and
    /// counting does not.
    fn count_amo_atoms<Q: Quota + ?Sized>(
        &self,
        work: &mut Meter<'_, Q>,
    ) -> Result<usize, Failure<Q::Error>> {
        let mut marked = filled_row(self.atoms, false, work)?;
        for &root in &self.roots {
            work.tick()?;
            let row = self.row(root);
            if self.kind(root) != Kind::Implies
                || row.len() != 2
                || self.kind(row[1] as usize) != Kind::False
            {
                continue;
            }
            let body = row[0] as usize;
            let children = self.row(body);
            let unit = match self.kind(body) {
                Kind::And => children.len() == 2,
                Kind::Threshold if self.bound(body) == 2 => {
                    let mut unit = true;
                    for &w in self.row_weights(body) {
                        work.tick()?;
                        if w != 1 {
                            unit = false;
                            break;
                        }
                    }
                    unit
                }
                _ => false,
            };
            if !unit {
                continue;
            }
            let mut atoms_only = true;
            for &c in children {
                work.tick()?;
                if !matches!(self.kind(c as usize), Kind::Atom(_)) {
                    atoms_only = false;
                    break;
                }
            }
            if atoms_only {
                for &c in children {
                    work.tick()?;
                    if let Kind::Atom(atom) = self.kind(c as usize) {
                        marked[atom as usize] = true;
                    }
                }
            }
        }
        let mut count = 0;
        for marked in marked {
            work.tick()?;
            count += usize::from(marked);
        }
        Ok(count)
    }

    fn row(&self, node: usize) -> &[u32] {
        &self.operands[self.offsets[node] as usize..self.offsets[node + 1] as usize]
    }

    /// A threshold node's weights, one per operand.
    fn row_weights(&self, node: usize) -> &[u64] {
        let start = self.thresholds[(self.kinds[node] & PAYLOAD) as usize].1 as usize;
        &self.weights[start..start + self.row(node).len()]
    }
}

impl SmallScratch {
    /// Owned retained capacities, including this value's Box allocation.
    pub(crate) fn retained_bytes(&self) -> u128 {
        size_of::<Self>() as u128
            + bytes(&self.lanes)
            + bytes(&self.stamp)
            + bytes(&self.cone)
            + bytes(&self.constant)
            + bytes(&self.assumptions)
            + bytes(&self.guards)
            + bytes(&self.cores)
            + self
                .cores
                .iter()
                .map(|(guards, words)| bytes(guards) + bytes(words))
                .sum::<u128>()
            + bytes(&self.models)
            + self.models.iter().map(bytes).sum::<u128>()
            + self.shape.as_ref().map_or(0, Shape::retained_bytes)
            + self.solver.as_ref().map_or(0, Solver::retained_bytes)
    }

    /// The policy, read once.
    pub(crate) fn policy(&mut self) -> SmallPolicy {
        *self.policy.get_or_insert_with(SmallPolicy::environment)
    }

    pub(crate) fn active(&mut self) -> bool {
        self.policy().active()
    }

    /// Say whether this walker is the one that runs the learned search
    /// under `main_only`.
    pub(crate) fn designate(&mut self, designated: bool) {
        self.designated = Some(designated);
    }

    /// Share the walk's count of idle peers.
    pub(crate) fn yield_to(&mut self, idle: std::sync::Arc<std::sync::atomic::AtomicUsize>) {
        self.idle = Some(idle);
    }

    /// Whether a peer waits for work and the policy yields to it.
    fn peers_idle(&self, policy: &SmallPolicy) -> bool {
        policy.yield_idle
            && self
                .idle
                .as_ref()
                .is_some_and(|idle| idle.load(std::sync::atomic::Ordering::Relaxed) > 0)
    }

    /// Replace the policy (tests and measurements).
    pub(crate) fn set_policy(&mut self, policy: SmallPolicy) {
        self.policy = Some(policy);
    }

    fn shape<Q: Quota + ?Sized>(
        &mut self,
        theory: &Theory,
        producers: Option<&Producers>,
        work: &mut Meter<'_, Q>,
    ) -> Result<Option<&Shape>, Failure<Q::Error>> {
        let fresh = self
            .shape
            .as_ref()
            .is_none_or(|shape| !shape.theory.same_instance(theory));
        if fresh {
            self.shape = Shape::read(theory, producers, work)?;
            self.solver = None;
            self.cores.clear();
            self.guards.clear();
            self.models.clear();
            self.stamp.clear();
            self.constant.clear();
        }
        Ok(self.shape.as_ref())
    }

    /// Narrow `region` by the size-specialized implementation its open-atom
    /// count selects. The region must already be closed by the general
    /// narrowing of the same theory and producers.
    ///
    /// # Errors
    /// Typed work, storage, allocation or control refusal, retaining admitted work.
    pub(crate) fn narrow<Q: Quota + ?Sized>(
        &mut self,
        theory: &Theory,
        producers: Option<&Producers>,
        restrictions: &[&Theory],
        region: &Region,
        cancellation: &Cancellation,
        work: &mut Meter<'_, Q>,
    ) -> Metered<SmallNarrowing, Failure<Q::Error>> {
        let before = work.charged();
        let result: Result<SmallNarrowing, Failure<Q::Error>> = (|| {
            cancellation.poll()?;
            let result =
                self.narrow_inner(theory, producers, restrictions, region, cancellation, work)?;
            cancellation.poll()?;
            Ok(result)
        })();
        let result = match result {
            Err(error) if error.is_resource_refusal() => match cancellation.poll() {
                Err(stop) => Err(Failure::Stopped(stop)),
                Ok(()) => Err(error),
            },
            other => other,
        };
        if result.is_err() {
            // A permit can fail between related watch or threshold mutations.
            // Only a completed attempt may leave reusable solver state.
            self.solver = None;
            self.cores.clear();
            self.guards.clear();
            self.models.clear();
            // An interrupted wrap-to-zero clear must not expose old epoch-1
            // marks on the next attempt. Reinitialize both enum stamp arrays.
            self.stamp.clear();
            self.constant.clear();
            self.cone.clear();
            self.lanes.clear();
            self.epoch = 0;
        }
        work.refund();
        Metered {
            result,
            charged: work.charged() - before,
        }
    }

    fn narrow_inner<Q: Quota + ?Sized>(
        &mut self,
        theory: &Theory,
        producers: Option<&Producers>,
        restrictions: &[&Theory],
        region: &Region,
        cancellation: &Cancellation,
        work: &mut Meter<'_, Q>,
    ) -> Result<SmallNarrowing, Failure<Q::Error>> {
        work.tick()?;
        if region.len() != theory.atom_count()
            || producers.is_some_and(|producers| !producers.belongs_to(theory))
        {
            return Err(Stop::WrongProgram.into());
        }
        let policy = self.policy();
        if !policy.active() {
            return Ok(SmallNarrowing::Declined);
        }
        let mut decided = 0usize;
        let (held, cut) = region.decision_words();
        for (&held, &cut) in held.iter().zip(cut) {
            work.tick()?;
            decided += (held | cut).count_ones() as usize;
        }
        let open = region.len() - decided;
        if open == 0 {
            return Ok(SmallNarrowing::Declined);
        }
        if open <= policy.enumerate_up_to {
            if self.shape(theory, producers, work)?.is_none() {
                return Ok(SmallNarrowing::Declined);
            }
            let started = Instant::now();
            let verdict = self.enumerate(region, work);
            self.statistics.enumerated_nanos += saturate(started.elapsed().as_nanos());
            if let Some(verdict) = verdict? {
                return Ok(verdict);
            }
        }
        if policy.probe > 0 {
            if self.shape(theory, producers, work)?.is_none() {
                return Ok(SmallNarrowing::Declined);
            }
            self.guard(restrictions, work)?;
            if self.core_hit(region, work)? {
                self.statistics.core_hits += 1;
                return Ok(SmallNarrowing::Refuted(Variant::Probed));
            }
            let started = Instant::now();
            let verdict = self.probe(region, policy.probe, cancellation, work);
            self.statistics.probed_nanos += saturate(started.elapsed().as_nanos());
            let verdict = verdict?;
            if let SmallNarrowing::Refuted(_) = verdict {
                return Ok(verdict);
            }
        }
        let learn_up_to = if policy.adapt_from > 0 {
            if self.cutoff == 0 {
                self.cutoff = policy.adapt_from;
            }
            self.cutoff
        } else {
            policy.learn_up_to
        };
        if open <= learn_up_to && open >= policy.learn_from {
            if !self.learn_admits(theory, producers, &policy, work)? {
                return Ok(SmallNarrowing::Declined);
            }
            self.guard(restrictions, work)?;
            if self.cached(region, work)? {
                self.statistics.cached += 1;
                self.record_yield(false);
                return Ok(SmallNarrowing::Open(Variant::Learned));
            }
            if self.core_hit(region, work)? {
                self.statistics.core_hits += 1;
                self.record_yield(true);
                return Ok(SmallNarrowing::Refuted(Variant::Learned));
            }
            let started = Instant::now();
            let verdict = self.learn(region, policy.learn_conflicts, cancellation, work);
            if let Ok(verdict) = &verdict {
                self.record_yield(matches!(verdict, SmallNarrowing::Refuted(_)));
            }
            self.statistics.learned_nanos += saturate(started.elapsed().as_nanos());
            if policy.adapt_from > 0 {
                match verdict {
                    Ok(SmallNarrowing::Refuted(_)) => {
                        self.cutoff = self.cutoff.max(open.saturating_mul(3) / 2);
                    }
                    Ok(_) => {
                        self.cutoff = (self.cutoff / 10 * 7 + self.cutoff % 10 * 7 / 10).max(1);
                    }
                    Err(_) => {}
                }
            }
            return verdict;
        }
        Ok(SmallNarrowing::Declined)
    }

    /// Whether the learned search may read the next offered region: past
    /// the deferral, outside dense and matching-shaped theories, and through
    /// the yield gate.
    fn learn_admits<Q: Quota + ?Sized>(
        &mut self,
        theory: &Theory,
        producers: Option<&Producers>,
        policy: &SmallPolicy,
        work: &mut Meter<'_, Q>,
    ) -> Result<bool, Failure<Q::Error>> {
        if policy.main_only
            && !*self
                .designated
                .get_or_insert_with(|| std::thread::current().name() == Some("main"))
        {
            return Ok(false);
        }
        self.offered += 1;
        if self.offered <= u64::try_from(policy.learn_after).unwrap_or(u64::MAX) {
            return Ok(false);
        }
        // The density reads only the theory's sizes: a dense theory is
        // declined before its shape is read.
        let nodes = theory.view().len();
        if policy.density > 0 && nodes > policy.density.saturating_mul(theory.atom_count().max(1)) {
            self.statistics.shape_declined += 1;
            return Ok(false);
        }
        if self.peers_idle(policy) {
            self.statistics.yielded += 1;
            return Ok(false);
        }
        let first_offer = *self.first_offer.get_or_insert_with(Instant::now);
        if policy.amortize > 0
            && self
                .shape
                .as_ref()
                .is_none_or(|shape| !shape.theory.same_instance(theory))
        {
            let cost = u64::try_from(nodes)
                .unwrap_or(u64::MAX)
                .saturating_mul(policy.amortize as u64);
            if saturate(first_offer.elapsed().as_nanos()) < cost {
                self.statistics.deferred += 1;
                return Ok(false);
            }
        }
        let Some(shape) = self.shape(theory, producers, work)? else {
            return Ok(false);
        };
        let (atoms, amo) = (shape.atoms, shape.amo_atoms);
        self.statistics.atoms = u64::try_from(atoms).unwrap_or(u64::MAX);
        self.statistics.amo_atoms = u64::try_from(amo).unwrap_or(u64::MAX);
        self.statistics.nodes = u64::try_from(nodes).unwrap_or(u64::MAX);
        if policy.matching > 0 && amo.saturating_mul(100) >= policy.matching.saturating_mul(atoms) {
            self.statistics.shape_declined += 1;
            return Ok(false);
        }
        if !self.gate_admits(policy.gate) {
            self.statistics.gated += 1;
            return Ok(false);
        }
        Ok(true)
    }

    /// Whether the yield gate lets the learned search read this region.
    fn gate_admits(&mut self, gate: usize) -> bool {
        if gate == 0 {
            return true;
        }
        if self.yield_score == 0 && self.skipped == 0 && self.statistics.learned == 0 {
            self.yield_score = YIELD_ONE; // optimistic start
        }
        let threshold = u64::from(YIELD_ONE) * gate as u64;
        if u64::from(self.yield_score) * 100 >= threshold {
            return true;
        }
        self.skipped += 1;
        if self.skipped >= GATE_RETRY {
            self.skipped = 0;
            return true;
        }
        false
    }

    /// Fold one learned-path verdict into the yield (weight 1/16).
    fn record_yield(&mut self, refuted: bool) {
        self.yield_score -= self.yield_score / 16;
        if refuted {
            self.yield_score += YIELD_ONE / 16;
        }
    }

    /// The general narrowing refuted a region of `open` open atoms: under an
    /// adapting policy, regions of that size go to the learned search next,
    /// as cube-and-conquer lowers its cutoff when lookahead itself fails.
    pub(crate) fn note_refuted<Q: Quota + ?Sized>(
        &mut self,
        region: &Region,
        work: &mut Meter<'_, Q>,
    ) -> Result<(), Failure<Q::Error>> {
        let policy = self.policy();
        if policy.adapt_from == 0 {
            return Ok(());
        }
        let (held, cut) = region.decision_words();
        let mut decided = 0;
        for (&held, &cut) in held.iter().zip(cut) {
            work.tick()?;
            decided += (held | cut).count_ones() as usize;
        }
        let open = region.len() - decided;
        if self.cutoff == 0 {
            self.cutoff = policy.adapt_from;
        }
        self.cutoff = self.cutoff.max(open.saturating_add(open / 20));
        Ok(())
    }

    /// Encode the active restrictions (once each) and make their guards the
    /// first assumptions; a changed set forgets the cached completions.
    fn guard<Q: Quota + ?Sized>(
        &mut self,
        restrictions: &[&Theory],
        work: &mut Meter<'_, Q>,
    ) -> Result<(), Failure<Q::Error>> {
        let Some(shape) = self.shape.as_ref() else {
            return Ok(());
        };
        if self.solver.is_none() {
            let solver = Solver::encode(shape, self.policy.is_some_and(|p| p.decide_all), work)?;
            if reports() {
                let mut terms = 0usize;
                for pb in &solver.pbs {
                    work.tick()?;
                    terms = terms
                        .checked_add(pb.conditions.len())
                        .ok_or(Stop::StorageLimit)?;
                }
                eprintln!(
                    "zetesis-small-encoding vars={} clauses={} pbs={} pb_terms={} atoms={}",
                    solver.vars,
                    solver.clauses.len(),
                    solver.pbs.len(),
                    terms,
                    shape.atoms
                );
            }
            self.solver = Some(solver);
            self.statistics.encodings += 1;
        }
        let solver = self.solver.as_mut().expect("encoded above");
        let mut guards = Vec::new();
        for t in restrictions {
            work.tick()?;
            try_push(&mut guards, solver.guard_of(t, work)?)?;
        }
        let mut same = guards.len() == self.guards.len();
        if same {
            for (left, right) in guards.iter().zip(&self.guards) {
                work.tick()?;
                if left != right {
                    same = false;
                    break;
                }
            }
        }
        if !same {
            self.guards = guards;
            self.models.clear();
        }

        Ok(())
    }

    fn core_hit<Q: Quota + ?Sized>(
        &mut self,
        region: &Region,
        work: &mut Meter<'_, Q>,
    ) -> Result<bool, Failure<Q::Error>> {
        let (held, cut) = region.decision_words();
        let mut hit = None;
        'cores: for (index, (requires, core)) in self.cores.iter().enumerate() {
            work.tick()?;
            for required in requires {
                let mut present = false;
                for guard in &self.guards {
                    work.tick()?;
                    if guard == required {
                        present = true;
                        break;
                    }
                }
                if !present {
                    continue 'cores;
                }
            }
            for &(word_index, h, c) in core {
                work.tick()?;
                let word_index = word_index as usize;
                if held[word_index] & h != h || cut[word_index] & c != c {
                    continue 'cores;
                }
            }
            hit = Some(index);
            break;
        }
        if let Some(index) = hit {
            // At most CORES headers move; no literals are copied.
            let core = self.cores.remove(index);
            self.cores.try_reserve(1).map_err(|_| Stop::Allocation)?;
            self.cores.insert(0, core);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn remember_core<Q: Quota + ?Sized>(
        &mut self,
        work: &mut Meter<'_, Q>,
    ) -> Result<(), Failure<Q::Error>> {
        let Some(solver) = self.solver.as_ref() else {
            return Ok(());
        };
        let atoms = self.shape.as_ref().map_or(0, |shape| shape.atoms);
        let mut words: Vec<(u32, u64, u64)> = Vec::new();
        let mut requires = Vec::new();
        for &l in &solver.core {
            work.tick()?;
            let atom = var(l) as usize;
            if atom >= atoms {
                try_push(&mut requires, var(l))?;
                continue;
            }
            let (word_index, bit) = (idx(atom / 64), 1_u64 << (atom % 64));
            let mut at = None;
            for (index, &(w, _, _)) in words.iter().enumerate() {
                work.tick()?;
                if w == word_index {
                    at = Some(index);
                    break;
                }
            }
            let at = if let Some(at) = at {
                at
            } else {
                try_push(&mut words, (word_index, 0, 0))?;
                words.len() - 1
            };
            let entry = &mut words[at];
            if l & 1 == 0 {
                entry.1 |= bit;
            } else {
                entry.2 |= bit;
            }
        }
        if !words.is_empty() {
            if self.cores.len() >= CORES {
                self.cores.pop();
            }
            self.cores.try_reserve(1).map_err(|_| Stop::Allocation)?;
            self.cores.insert(0, (requires, words));
        }
        Ok(())
    }

    fn cached<Q: Quota + ?Sized>(
        &self,
        region: &Region,
        work: &mut Meter<'_, Q>,
    ) -> Result<bool, Failure<Q::Error>> {
        let (held, cut) = region.decision_words();
        'models: for model in &self.models {
            work.tick()?;
            for ((&held, &cut), &model) in held.iter().zip(cut).zip(model) {
                work.tick()?;
                if held & !model != 0 || cut & model != 0 {
                    continue 'models;
                }
            }
            return Ok(true);
        }
        Ok(false)
    }

    // ---------------------------------------------------------------- enumeration

    /// `None` declines (an ordinal cut above an open atom).
    fn enumerate<Q: Quota + ?Sized>(
        &mut self,
        region: &Region,
        work: &mut Meter<'_, Q>,
    ) -> Result<Option<SmallNarrowing>, Failure<Q::Error>> {
        let Some(shape) = self.shape.as_ref() else {
            return Ok(None);
        };
        let nodes = shape.nodes();
        if self.stamp.len() != nodes {
            self.stamp = filled_row(nodes, 0, work)?;
            self.constant = filled_row(nodes, 2, work)?;
            self.epoch = 0;
        }
        self.epoch = self.epoch.wrapping_add(1);
        if self.epoch == 0 {
            for stamp in &mut self.stamp {
                work.tick()?;
                *stamp = 0;
            }
            self.epoch = 1;
        }
        let open = open_atoms(region, work)?;
        let k = open.len();
        if k > ENUMERATION_CEILING {
            return Ok(None);
        }
        collect_cone(
            shape,
            &open,
            &mut self.stamp,
            self.epoch,
            &mut self.cone,
            work,
        )?;
        if shape.has_cuts {
            for &node in &self.cone {
                work.tick()?;
                if shape.kind(node) == Kind::Cut {
                    return Ok(None);
                }
            }
        }
        let words = if k >= 6 { 1_usize << (k - 6) } else { 1 };
        let valid_lanes = if k >= 6 {
            u64::MAX
        } else {
            (1_u64 << (1 << k)) - 1
        };
        self.lanes.clear();
        for _ in &self.cone {
            work.tick()?;
            try_push(&mut self.lanes, 0)?;
        }
        let mut position = std::collections::HashMap::new();
        for (slot, &node) in self.cone.iter().enumerate() {
            work.tick()?;
            position.try_reserve(1).map_err(|_| Stop::Allocation)?;
            position.insert(node, slot);
        }
        let mut open_index = std::collections::HashMap::new();
        for (index, &atom) in open.iter().enumerate() {
            work.tick()?;
            open_index.try_reserve(1).map_err(|_| Stop::Allocation)?;
            open_index.insert(atom, index);
        }
        let reader = LaneReader {
            shape,
            region,
            position,
            open_index,
        };
        let checked = checked_heads(shape, region, &open, &self.cone, work)?;
        self.statistics.enumerated += 1;
        self.statistics.lanes += (words as u64) * 64;
        let mut lanes = std::mem::take(&mut self.lanes);
        let mut memo = std::collections::HashMap::new();
        let mut passes = false;
        for word in 0..words {
            work.tick()?;
            for (slot, &node) in self.cone.iter().enumerate() {
                lanes[slot] = reader.node(node, word, &lanes, &mut memo, work)?;
            }
            let mut alive = valid_lanes;
            for &root in &shape.roots {
                alive &= reader.operand(root, &lanes, &mut memo, work)?;
                if alive == 0 {
                    break;
                }
            }
            if alive != 0 {
                alive = reader.supported(&checked, alive, word, &lanes, &mut memo, work)?;
            }
            if alive != 0 {
                passes = true;
                break;
            }
        }
        self.lanes = lanes;
        if passes {
            Ok(Some(SmallNarrowing::Open(Variant::Enumerated)))
        } else {
            self.statistics.enumerated_refuted += 1;
            Ok(Some(SmallNarrowing::Refuted(Variant::Enumerated)))
        }
    }

    // ---------------------------------------------------------------- learning

    fn ensure_solver<Q: Quota + ?Sized>(
        &mut self,
        work: &mut Meter<'_, Q>,
    ) -> Result<bool, Failure<Q::Error>> {
        let Some(shape) = self.shape.as_ref() else {
            return Ok(false);
        };
        if self.solver.is_none() {
            self.solver = Some(Solver::encode(
                shape,
                self.policy.is_some_and(|p| p.decide_all),
                work,
            )?);
            self.statistics.encodings += 1;
        }
        Ok(true)
    }

    /// Failed-literal probing of the region's open atoms, at most `budget`.
    fn probe<Q: Quota + ?Sized>(
        &mut self,
        region: &Region,
        budget: usize,
        cancellation: &Cancellation,
        work: &mut Meter<'_, Q>,
    ) -> Result<SmallNarrowing, Failure<Q::Error>> {
        if !self.ensure_solver(work)? {
            return Ok(SmallNarrowing::Declined);
        }
        let solver = self.solver.as_mut().expect("encoded above");
        self.assumptions.clear();
        for &g in &self.guards {
            work.tick()?;
            try_push(&mut self.assumptions, lit(g, false))?;
        }
        let (held, cut) = region.decision_words();
        for (word, (&held, &cut)) in held.iter().zip(cut).enumerate() {
            work.tick()?;
            let mut decided = held | cut;
            while decided != 0 {
                work.tick()?;
                let bit = decided.trailing_zeros() as usize;
                decided &= decided - 1;
                try_push(
                    &mut self.assumptions,
                    lit(idx(word * 64 + bit), held & (1_u64 << bit) == 0),
                )?;
            }
        }
        let mut candidates = Vec::new();
        for (word, (&held, &cut)) in held.iter().zip(cut).enumerate() {
            work.tick()?;
            let mut open = !(held | cut);
            while open != 0 {
                work.tick()?;
                let bit = open.trailing_zeros() as usize;
                open &= open - 1;
                let atom = word * 64 + bit;
                if atom < region.len() {
                    try_push(&mut candidates, idx(atom))?;
                }
            }
        }
        self.statistics.probed += 1;
        let before = solver.counts.propagations;
        let outcome = solver.probe(&self.assumptions, &candidates, budget, cancellation, work);
        self.statistics.propagations += solver.counts.propagations - before;
        Ok(match outcome? {
            Outcome::Unsatisfiable(core) => {
                self.statistics.probed_refuted += 1;
                self.statistics.core_width += core as u64;
                self.remember_core(work)?;
                SmallNarrowing::Refuted(Variant::Probed)
            }
            _ => SmallNarrowing::Open(Variant::Probed),
        })
    }

    fn learn<Q: Quota + ?Sized>(
        &mut self,
        region: &Region,
        conflicts: u64,
        cancellation: &Cancellation,
        work: &mut Meter<'_, Q>,
    ) -> Result<SmallNarrowing, Failure<Q::Error>> {
        let Some(shape) = self.shape.as_ref() else {
            return Ok(SmallNarrowing::Declined);
        };
        if self.solver.is_none() {
            let solver = Solver::encode(shape, self.policy.is_some_and(|p| p.decide_all), work)?;
            if reports() {
                let mut terms = 0usize;
                for pb in &solver.pbs {
                    work.tick()?;
                    terms = terms
                        .checked_add(pb.conditions.len())
                        .ok_or(Stop::StorageLimit)?;
                }
                eprintln!(
                    "zetesis-small-encoding vars={} clauses={} pbs={} pb_terms={} atoms={}",
                    solver.vars,
                    solver.clauses.len(),
                    solver.pbs.len(),
                    terms,
                    shape.atoms
                );
            }
            self.solver = Some(solver);
            self.statistics.encodings += 1;
        }
        let solver = self.solver.as_mut().expect("encoded above");
        self.assumptions.clear();
        for &g in &self.guards {
            work.tick()?;
            try_push(&mut self.assumptions, lit(g, false))?;
        }
        let (held, cut) = region.decision_words();
        for (word, (&held, &cut)) in held.iter().zip(cut).enumerate() {
            work.tick()?;
            let mut decided = held | cut;
            while decided != 0 {
                work.tick()?;
                let bit = decided.trailing_zeros() as usize;
                decided &= decided - 1;
                try_push(
                    &mut self.assumptions,
                    lit(idx(word * 64 + bit), held & (1_u64 << bit) == 0),
                )?;
            }
        }
        self.statistics.learned += 1;
        let before = solver.counts;
        let idle = self
            .idle
            .as_deref()
            .filter(|_| self.policy.is_some_and(|p| p.yield_idle));
        let outcome = solver.solve(&self.assumptions, conflicts, cancellation, work, idle);
        let after = solver.counts;
        self.statistics.decisions += after.decisions - before.decisions;
        self.statistics.conflicts += after.conflicts - before.conflicts;
        self.statistics.propagations += after.propagations - before.propagations;
        self.statistics.learned_clauses += after.learned - before.learned;
        Ok(match outcome? {
            Outcome::Unsatisfiable(core) => {
                self.statistics.learned_refuted += 1;
                self.statistics.core_width += core as u64;
                self.remember_core(work)?;
                SmallNarrowing::Refuted(Variant::Learned)
            }
            Outcome::Satisfiable => {
                let words = shape.atoms.div_ceil(64);
                let mut model = filled_row(words, 0_u64, work)?;
                for atom in 0..shape.atoms {
                    work.tick()?;
                    if solver.value(idx(atom)) == TRUE {
                        model[atom / 64] |= 1 << (atom % 64);
                    }
                }
                if self.models.len() >= 8 {
                    self.models.remove(0);
                }
                try_push(&mut self.models, model)?;
                SmallNarrowing::Open(Variant::Learned)
            }
            Outcome::Unknown => {
                self.statistics.learned_exhausted += 1;
                SmallNarrowing::Open(Variant::Learned)
            }
        })
    }
}

/// Enumerate open atom identities in packed decision order.
fn open_atoms<Q: Quota + ?Sized>(
    region: &Region,
    work: &mut Meter<'_, Q>,
) -> Result<Vec<usize>, Failure<Q::Error>> {
    let mut open = Vec::new();
    let (held, cut) = region.decision_words();
    for (word, (&held, &cut)) in held.iter().zip(cut).enumerate() {
        work.tick()?;
        let mut bits = !(held | cut);
        while bits != 0 {
            work.tick()?;
            let bit = bits.trailing_zeros() as usize;
            bits &= bits - 1;
            let atom = word * 64 + bit;
            if atom < region.len() {
                try_push(&mut open, atom)?;
            }
        }
    }
    Ok(open)
}

/// The cone of the open atoms: every node above one, in node order.
fn collect_cone<Q: Quota + ?Sized>(
    shape: &Shape,
    open: &[usize],
    stamp: &mut [u32],
    epoch: u32,
    cone: &mut Vec<usize>,
    work: &mut Meter<'_, Q>,
) -> Result<(), Failure<Q::Error>> {
    cone.clear();
    let mut stack = Vec::new();
    for &atom in open {
        work.tick()?;
        let (start, end) = (
            shape.atom_nodes_offsets[atom] as usize,
            shape.atom_nodes_offsets[atom + 1] as usize,
        );
        for &node in &shape.atom_nodes[start..end] {
            work.tick()?;
            try_push(&mut stack, node as usize)?;
        }
    }
    while let Some(node) = stack.pop() {
        work.tick()?;
        if stamp[node] == epoch {
            continue;
        }
        stamp[node] = epoch;
        try_push(cone, node)?;
        let (start, end) = (
            shape.parents_offsets[node] as usize,
            shape.parents_offsets[node + 1] as usize,
        );
        for &parent in &shape.parents[start..end] {
            work.tick()?;
            try_push(&mut stack, parent as usize)?;
        }
    }
    sort_values(cone, work)
}

/// Support rows a completion can change; cut atoms need no support.
fn checked_heads<Q: Quota + ?Sized>(
    shape: &Shape,
    region: &Region,
    open: &[usize],
    cone: &[usize],
    work: &mut Meter<'_, Q>,
) -> Result<Vec<usize>, Failure<Q::Error>> {
    let Some(support) = &shape.support else {
        return Ok(Vec::new());
    };
    let mut checked = copy_row(open, work)?;
    for &node in cone {
        work.tick()?;
        let (start, end) = (
            support.body_heads_offsets[node] as usize,
            support.body_heads_offsets[node + 1] as usize,
        );
        for &head in &support.body_heads[start..end] {
            work.tick()?;
            try_push(&mut checked, head as usize)?;
        }
    }
    for &atom in open {
        work.tick()?;
        let (start, end) = (
            support.rival_offsets[atom] as usize,
            support.rival_offsets[atom + 1] as usize,
        );
        for &head in &support.rivals[start..end] {
            work.tick()?;
            try_push(&mut checked, head as usize)?;
        }
    }
    sort_dedup(&mut checked, work)?;
    let mut written = 0;
    for read in 0..checked.len() {
        work.tick()?;
        if !region.is_cut(checked[read]) {
            checked[written] = checked[read];
            written += 1;
        }
    }
    checked.truncate(written);
    Ok(checked)
}

/// Reads 64 completions at a time: lane `j` of word `w` is completion
/// `64 w + j` of the open atoms.
struct LaneReader<'a> {
    shape: &'a Shape,
    region: &'a Region,
    /// Lane-buffer slot of each cone node.
    position: std::collections::HashMap<usize, usize>,
    /// Index of each open atom among the open atoms.
    open_index: std::collections::HashMap<usize, usize>,
}

impl LaneReader<'_> {
    fn atom<Q: Quota + ?Sized>(
        &self,
        atom: usize,
        batch_index: usize,
        work: &mut Meter<'_, Q>,
    ) -> Result<u64, Failure<Q::Error>> {
        work.tick()?;
        Ok(self.open_index.get(&atom).map_or_else(
            || {
                if self.region.is_held(atom) {
                    u64::MAX
                } else {
                    0
                }
            },
            |&j| lane_pattern(j, batch_index),
        ))
    }

    fn operand<Q: Quota + ?Sized>(
        &self,
        node: usize,
        lanes: &[u64],
        memo: &mut std::collections::HashMap<usize, bool>,
        work: &mut Meter<'_, Q>,
    ) -> Result<u64, Failure<Q::Error>> {
        work.tick()?;
        Ok(match self.position.get(&node) {
            Some(&slot) => lanes[slot],
            None => {
                if evaluate_constant(self.shape, self.region, node, memo, work)? {
                    u64::MAX
                } else {
                    0
                }
            }
        })
    }

    /// A cone node's lanes from its operands' (earlier in node order).
    fn node<Q: Quota + ?Sized>(
        &self,
        node: usize,
        batch_index: usize,
        lanes: &[u64],
        memo: &mut std::collections::HashMap<usize, bool>,
        work: &mut Meter<'_, Q>,
    ) -> Result<u64, Failure<Q::Error>> {
        work.tick()?;
        let shape = self.shape;
        Ok(match shape.kind(node) {
            Kind::Atom(atom) => self.atom(atom as usize, batch_index, work)?,
            Kind::False => 0,
            Kind::Implies => {
                let row = shape.row(node);
                !self.operand(row[0] as usize, lanes, memo, work)?
                    | self.operand(row[1] as usize, lanes, memo, work)?
            }
            Kind::And => {
                let mut value = u64::MAX;
                for &child in shape.row(node) {
                    value &= self.operand(child as usize, lanes, memo, work)?;
                }
                value
            }
            Kind::Or => {
                let mut value = 0;
                for &child in shape.row(node) {
                    value |= self.operand(child as usize, lanes, memo, work)?;
                }
                value
            }
            Kind::Threshold => {
                let mut values = Vec::new();
                for &child in shape.row(node) {
                    try_push(
                        &mut values,
                        self.operand(child as usize, lanes, memo, work)?,
                    )?;
                }
                threshold_lanes(&values, shape.row_weights(node), shape.bound(node), work)?
            }
            Kind::Cut => unreachable!("cones with cuts are declined"),
        })
    }

    /// Lanes in which every checked held atom has an eligible producer.
    fn supported<Q: Quota + ?Sized>(
        &self,
        checked: &[usize],
        mut alive: u64,
        batch_index: usize,
        lanes: &[u64],
        memo: &mut std::collections::HashMap<usize, bool>,
        work: &mut Meter<'_, Q>,
    ) -> Result<u64, Failure<Q::Error>> {
        let Some(support) = &self.shape.support else {
            return Ok(alive);
        };
        for &atom in checked {
            let held = self.atom(atom, batch_index, work)?;
            if held & alive == 0 {
                continue;
            }
            let mut available = 0_u64;
            let (start, end) = (
                support.offsets[atom] as usize,
                support.offsets[atom + 1] as usize,
            );
            for &(body, start, end) in &support.entries[start..end] {
                work.tick()?;
                let mut term = if body == u32::MAX {
                    u64::MAX
                } else {
                    self.operand(body as usize, lanes, memo, work)?
                };
                for &other in &support.others[start as usize..end as usize] {
                    term &= !self.atom(other as usize, batch_index, work)?;
                }
                available |= term;
                if available & held == held {
                    break;
                }
            }
            alive &= !held | available;
            if alive == 0 {
                break;
            }
        }
        Ok(alive)
    }
}

/// A scalar threshold term in each of the 64 lanes is one charged read.
fn threshold_lanes<Q: Quota + ?Sized>(
    values: &[u64],
    weights: &[u64],
    bound: i128,
    work: &mut Meter<'_, Q>,
) -> Result<u64, Failure<Q::Error>> {
    let mut out = 0_u64;
    for lane in 0..64 {
        work.tick()?;
        let mut sum = 0_i128;
        for (&value, &weight) in values.iter().zip(weights) {
            work.tick()?;
            if value >> lane & 1 == 1 {
                sum += i128::from(weight);
            }
        }
        if sum >= bound {
            out |= 1 << lane;
        }
    }
    Ok(out)
}

/// The value of a node no open atom reaches, under the region's decisions.
fn evaluate_constant<Q: Quota + ?Sized>(
    shape: &Shape,
    region: &Region,
    root: usize,
    memo: &mut std::collections::HashMap<usize, bool>,
    work: &mut Meter<'_, Q>,
) -> Result<bool, Failure<Q::Error>> {
    work.tick()?;
    if let Some(&value) = memo.get(&root) {
        return Ok(value);
    }
    let values = memo;
    let mut stack = vec![(root, false)];
    while let Some((node, expanded)) = stack.pop() {
        work.tick()?;
        if values.contains_key(&node) {
            continue;
        }
        let row = shape.row(node);
        if !expanded && !matches!(shape.kind(node), Kind::Atom(_) | Kind::False) {
            try_push(&mut stack, (node, true))?;
            for &child in row {
                work.tick()?;
                let child = child as usize;
                if !values.contains_key(&child) {
                    try_push(&mut stack, (child, false))?;
                }
            }
            continue;
        }
        let get = |child: u32| values.get(&(child as usize)).copied().unwrap_or(false);
        let value = match shape.kind(node) {
            Kind::Atom(atom) => region.is_held(atom as usize),
            Kind::False => false,
            Kind::Implies => !get(row[0]) || get(row[1]),
            Kind::And => {
                let mut value = true;
                for &child in row {
                    work.tick()?;
                    if !get(child) {
                        value = false;
                        break;
                    }
                }
                value
            }
            Kind::Or => {
                let mut value = false;
                for &child in row {
                    work.tick()?;
                    if get(child) {
                        value = true;
                        break;
                    }
                }
                value
            }
            Kind::Threshold => {
                let mut sum = 0_i128;
                for (&child, &weight) in row.iter().zip(shape.row_weights(node)) {
                    work.tick()?;
                    if get(child) {
                        sum += i128::from(weight);
                    }
                }
                sum >= shape.bound(node)
            }
            // Preserve the existing necessary-condition relaxation.
            Kind::Cut => true,
        };
        values.try_reserve(1).map_err(|_| Stop::Allocation)?;
        values.insert(node, value);
    }
    Ok(values[&root])
}

/// Lane pattern of open atom `j` in completion word `word`: completion
/// `64 * word + lane` holds atom `j` exactly when bit `j` of it is set.
const fn lane_pattern(j: usize, word: usize) -> u64 {
    const LOW: [u64; 6] = [
        0xAAAA_AAAA_AAAA_AAAA,
        0xCCCC_CCCC_CCCC_CCCC,
        0xF0F0_F0F0_F0F0_F0F0,
        0xFF00_FF00_FF00_FF00,
        0xFFFF_0000_FFFF_0000,
        0xFFFF_FFFF_0000_0000,
    ];
    if j < 6 {
        LOW[j]
    } else if (word >> (j - 6)) & 1 == 1 {
        u64::MAX
    } else {
        0
    }
}

// -------------------------------------------------------------------- clausal search

const TRUE: u8 = 1;
const FALSE: u8 = 0;
const UNDEF: u8 = 2;

const fn lit(var: u32, negative: bool) -> u32 {
    var * 2 + negative as u32
}
const fn var(lit: u32) -> u32 {
    lit >> 1
}
const fn neg(lit: u32) -> u32 {
    lit ^ 1
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Counts {
    decisions: u64,
    conflicts: u64,
    propagations: u64,
    learned: u64,
}

enum Outcome {
    Satisfiable,
    /// Refuted using this many assumptions.
    Unsatisfiable(usize),
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Reason {
    None,
    Clause(u32),
    /// Forced by this threshold; the explanation is rebuilt from the trail
    /// prefix on demand.
    Pb(u32),
    /// Forced by a two-literal clause with this other, false, literal.
    Binary(u32),
}

/// A watch: the clause index in the high half, with [`BINARY`] set for a
/// two-literal clause, which is kept in the watches alone, and a blocking
/// literal of the clause in the low half (for a two-literal clause, its
/// other literal). A true blocker settles the clause without reading it.
type Watch = u64;

/// Marks a watch of a two-literal clause.
const BINARY: u32 = 1 << 31;

const fn watch(clause: u32, blocker: u32) -> Watch {
    ((clause as u64) << 32) | blocker as u64
}

fn watch_clause(entry: Watch) -> u32 {
    u32::try_from(entry >> 32).unwrap_or(u32::MAX)
}

fn watch_blocker(entry: Watch) -> u32 {
    u32::try_from(entry & u64::from(u32::MAX)).unwrap_or(u32::MAX)
}

/// A threshold constraint `out <-> sum(weights of true conditions) >= bound`.
#[derive(Debug)]
struct Pb {
    out: u32,
    conditions: Vec<u32>,
    weights: Vec<u64>,
    bound: i128,
    /// Weight of the true conditions under the current assignment.
    sure: i128,
    /// Weight of the unassigned conditions.
    open: i128,
    /// The largest single weight, to skip scans that cannot force.
    heaviest: i128,
}

/// Booleans packed one per bit.
#[derive(Debug, Default, Clone)]
struct Bits {
    words: Vec<u64>,
    len: usize,
}

impl Bits {
    fn filled<Q: Quota + ?Sized>(
        len: usize,
        value: bool,
        work: &mut Meter<'_, Q>,
    ) -> Result<Self, Failure<Q::Error>> {
        let mut bits = Self {
            words: filled_row(len.div_ceil(64), if value { u64::MAX } else { 0 }, work)?,
            len,
        };
        bits.clear_tail();
        Ok(bits)
    }

    fn clear_tail(&mut self) {
        if !self.len.is_multiple_of(64)
            && let Some(last) = self.words.last_mut()
        {
            *last &= (1 << (self.len % 64)) - 1;
        }
    }

    fn get(&self, index: usize) -> bool {
        self.words[index / 64] >> (index % 64) & 1 == 1
    }

    fn set(&mut self, index: usize, value: bool) {
        let bit = 1 << (index % 64);
        if value {
            self.words[index / 64] |= bit;
        } else {
            self.words[index / 64] &= !bit;
        }
    }

    fn push(&mut self, value: bool) -> Result<(), Stop> {
        if self.len.is_multiple_of(64) {
            try_push(&mut self.words, 0)?;
        }
        self.len += 1;
        self.set(self.len - 1, value);
        Ok(())
    }
}

/// A [`Reason`] packed in 32 bits: the variant in the top two bits, its
/// clause, threshold or literal below them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PackedReason(u32);

/// The payload bits of a [`PackedReason`].
const REASON_PAYLOAD: u32 = (1 << 30) - 1;

impl PackedReason {
    const NONE: Self = Self(0);

    fn pack(reason: Reason) -> Self {
        let (tag, payload) = match reason {
            Reason::None => (0, 0),
            Reason::Clause(index) => (1, index),
            Reason::Pb(index) => (2, index),
            Reason::Binary(other) => (3, other),
        };
        debug_assert!(payload <= REASON_PAYLOAD, "reason payload overflows");
        Self((tag << 30) | (payload & REASON_PAYLOAD))
    }

    const fn unpack(self) -> Reason {
        let payload = self.0 & REASON_PAYLOAD;
        match self.0 >> 30 {
            0 => Reason::None,
            1 => Reason::Clause(payload),
            2 => Reason::Pb(payload),
            _ => Reason::Binary(payload),
        }
    }
}

/// The thresholds a variable takes part in: as a condition (with its
/// weight) or as an output. Only variables of some threshold have one.
#[derive(Debug, Default)]
struct PbLists {
    occurs: Vec<u32>,
    terms: Vec<(u32, u64)>,
}

/// No [`PbLists`] for this variable.
const NO_PB: u32 = u32::MAX;

/// Conflict-learning search over the necessary conditions of one theory.
/// Clauses stored contiguously: one literal arena and each clause's span.
/// A clause per allocation costs more than its literals on large encodings.
#[derive(Debug, Default)]
struct Arena {
    lits: Vec<u32>,
    spans: Vec<(u32, u32)>,
}

impl Arena {
    fn len(&self) -> usize {
        self.spans.len()
    }

    fn push<Q: Quota + ?Sized>(
        &mut self,
        clause: impl AsRef<[u32]>,
        work: &mut Meter<'_, Q>,
    ) -> Result<u32, Failure<Q::Error>> {
        work.tick()?;
        let clause = clause.as_ref();
        let index = u32::try_from(self.spans.len()).map_err(|_| Stop::StorageLimit)?;
        if index > REASON_PAYLOAD {
            return Err(Stop::StorageLimit.into());
        }
        let start = u32::try_from(self.lits.len()).map_err(|_| Stop::StorageLimit)?;
        let len = u32::try_from(clause.len()).map_err(|_| Stop::StorageLimit)?;
        start.checked_add(len).ok_or(Stop::StorageLimit)?;
        self.spans.try_reserve(1).map_err(|_| Stop::Allocation)?;
        append_copy(&mut self.lits, clause, work)?;
        try_push(&mut self.spans, (start, len))?;
        Ok(index)
    }

    fn get(&self, index: usize) -> &[u32] {
        let (start, len) = self.spans[index];
        &self.lits[start as usize..(start + len) as usize]
    }

    fn get_mut(&mut self, index: usize) -> &mut [u32] {
        let (start, len) = self.spans[index];
        &mut self.lits[start as usize..(start + len) as usize]
    }
}

#[derive(Debug)]
struct Solver {
    vars: usize,
    clauses: Arena,
    /// Which clauses were learned (only those are ever dropped).
    is_learned: Bits,
    /// Learned clauses kept in the arena.
    learned_count: usize,
    watches: Vec<Vec<Watch>>,
    pbs: Vec<Pb>,
    /// Per variable, its slot in `pb_lists`, or [`NO_PB`].
    pb_slot: Vec<u32>,
    pb_lists: Vec<PbLists>,
    assigns: Vec<u8>,
    level: Vec<u32>,
    reason: Vec<PackedReason>,
    trail: Vec<u32>,
    trail_lim: Vec<usize>,
    head: usize,
    activity: Vec<f64>,
    increment: f64,
    phase: Bits,
    heap: Vec<u32>,
    heap_index: Vec<i32>,
    /// Trail position of each assigned variable.
    trail_pos: Vec<u32>,
    seen: Bits,
    /// Level-0 inconsistency: every call refutes.
    inconsistent: bool,
    counts: Counts,
    /// The assumptions the last refutation used.
    core: Vec<u32>,
    /// Restriction theories encoded, with their guard variables.
    guards: Vec<(Theory, u32)>,
    /// Variables the search decides; the rest are defined by them.
    decidable: Bits,
    decide_all: bool,
}

impl Solver {
    fn retained_bytes(&self) -> u128 {
        bytes(&self.clauses.lits)
            + bytes(&self.clauses.spans)
            + bytes(&self.is_learned.words)
            + bytes(&self.watches)
            + self.watches.iter().map(bytes).sum::<u128>()
            + bytes(&self.pbs)
            + self
                .pbs
                .iter()
                .map(|pb| bytes(&pb.conditions) + bytes(&pb.weights))
                .sum::<u128>()
            + bytes(&self.pb_slot)
            + bytes(&self.pb_lists)
            + self
                .pb_lists
                .iter()
                .map(|lists| bytes(&lists.occurs) + bytes(&lists.terms))
                .sum::<u128>()
            + bytes(&self.assigns)
            + bytes(&self.level)
            + bytes(&self.reason)
            + bytes(&self.trail)
            + bytes(&self.trail_lim)
            + bytes(&self.activity)
            + bytes(&self.phase.words)
            + bytes(&self.heap)
            + bytes(&self.heap_index)
            + bytes(&self.trail_pos)
            + bytes(&self.seen.words)
            + bytes(&self.core)
            + bytes(&self.guards)
            + bytes(&self.decidable.words)
    }

    fn encode<Q: Quota + ?Sized>(
        shape: &Shape,
        decide_all: bool,
        work: &mut Meter<'_, Q>,
    ) -> Result<Self, Failure<Q::Error>> {
        work.tick()?;
        let vars = shape.atoms;
        if vars > (REASON_PAYLOAD as usize / 2) + 1 {
            return Err(Stop::StorageLimit.into());
        }
        let mut solver = Self {
            vars,
            clauses: Arena::default(),
            is_learned: Bits::default(),
            learned_count: 0,
            watches: Vec::new(),
            pbs: Vec::new(),
            pb_slot: filled_row(vars, NO_PB, work)?,
            pb_lists: Vec::new(),
            assigns: filled_row(vars, UNDEF, work)?,
            level: filled_row(vars, 0, work)?,
            reason: filled_row(vars, PackedReason::NONE, work)?,
            trail: Vec::new(),
            trail_lim: Vec::new(),
            head: 0,
            activity: filled_row(vars, 0.0, work)?,
            increment: 1.0,
            phase: Bits::filled(vars, false, work)?,
            heap: Vec::new(),
            heap_index: filled_row(vars, -1, work)?,
            trail_pos: filled_row(vars, 0, work)?,
            seen: Bits::filled(vars, false, work)?,
            inconsistent: false,
            counts: Counts::default(),
            core: Vec::new(),
            guards: Vec::new(),
            decidable: Bits::filled(vars, true, work)?,
            decide_all,
        };
        for v in 0..idx(vars) {
            work.tick()?;
            try_push(&mut solver.watches, Vec::new())?;
            try_push(&mut solver.watches, Vec::new())?;
            solver.heap_insert(v, work)?;
        }
        solver.add_theory(shape, None, work)?;
        Ok(solver)
    }

    /// The thresholds of variable `v`, created on first use.
    fn pb_lists_mut(&mut self, v: usize) -> Result<&mut PbLists, Stop> {
        if self.pb_slot[v] == NO_PB {
            self.pb_slot[v] = idx(self.pb_lists.len());
            try_push(&mut self.pb_lists, PbLists::default())?;
        }
        Ok(&mut self.pb_lists[self.pb_slot[v] as usize])
    }

    /// A fresh variable, unassigned, in the decision heap.
    fn fresh<Q: Quota + ?Sized>(
        &mut self,
        decidable: bool,
        work: &mut Meter<'_, Q>,
    ) -> Result<u32, Failure<Q::Error>> {
        work.tick()?;
        if self.vars > REASON_PAYLOAD as usize / 2 {
            return Err(Stop::StorageLimit.into());
        }
        let v = idx(self.vars);
        self.decidable.push(decidable || self.decide_all)?;
        self.vars += 1;
        try_push(&mut self.watches, Vec::new())?;
        try_push(&mut self.watches, Vec::new())?;
        try_push(&mut self.pb_slot, NO_PB)?;
        try_push(&mut self.assigns, UNDEF)?;
        try_push(&mut self.level, 0)?;
        try_push(&mut self.reason, PackedReason::NONE)?;
        try_push(&mut self.activity, 0.0)?;
        self.phase.push(false)?;
        try_push(&mut self.heap_index, -1)?;
        try_push(&mut self.trail_pos, 0)?;
        self.seen.push(false)?;
        self.heap_insert(v, work)?;
        Ok(v)
    }

    /// The guard variable of a restriction theory, encoding it on first use:
    /// its node definitions are unguarded (they only name nodes), its roots
    /// hold when the guard does.
    fn guard_of<Q: Quota + ?Sized>(
        &mut self,
        theory: &Theory,
        work: &mut Meter<'_, Q>,
    ) -> Result<u32, Failure<Q::Error>> {
        work.tick()?;
        for (owner, guard) in &self.guards {
            work.tick()?;
            if owner.same_instance(theory) {
                return Ok(*guard);
            }
        }
        self.cancel_until(0, work)?;
        let guard = self.fresh(true, work)?;
        if let Some(shape) = Shape::read(theory, None, work)? {
            self.add_theory(&shape, Some(guard), work)?;
        }
        try_push(&mut self.guards, (theory.clone(), guard))?;
        Ok(guard)
    }

    /// Add one theory's definitions, roots (guarded by `guard` when given) and,
    /// unguarded original theories only, its support condition. Level 0.
    fn add_theory<Q: Quota + ?Sized>(
        &mut self,
        shape: &Shape,
        guard: Option<u32>,
        work: &mut Meter<'_, Q>,
    ) -> Result<(), Failure<Q::Error>> {
        work.tick()?;
        self.cancel_until(0, work)?;
        let atoms = shape.atoms;
        let nodes = shape.nodes();
        let node_var = self.variables_for(shape, work)?;
        let mut clauses = Arena::default();
        let mut pbs = Vec::new();
        for node in 0..nodes {
            work.tick()?;
            let v = node_var[node];
            let pos = lit(v, false);
            let mut row = Vec::new();
            for &c in shape.row(node) {
                work.tick()?;
                try_push(&mut row, node_var[c as usize])?;
            }
            match shape.kind(node) {
                Kind::Atom(_) | Kind::Cut => {}
                Kind::False => {
                    clauses.push([neg(pos)], work)?;
                }
                Kind::Implies => {
                    let (x, y) = (lit(row[0], false), lit(row[1], false));
                    clauses.push(vec![neg(pos), neg(x), y], work)?;
                    clauses.push(vec![pos, x], work)?;
                    clauses.push(vec![pos, neg(y)], work)?;
                }
                Kind::And => {
                    let mut long = vec![pos];
                    for &c in &row {
                        work.tick()?;
                        clauses.push(vec![neg(pos), lit(c, false)], work)?;
                        try_push(&mut long, lit(c, true))?;
                    }
                    clauses.push(long, work)?;
                }
                Kind::Or => {
                    let mut long = vec![neg(pos)];
                    for &c in &row {
                        work.tick()?;
                        clauses.push(vec![pos, lit(c, true)], work)?;
                        try_push(&mut long, lit(c, false))?;
                    }
                    clauses.push(long, work)?;
                }
                Kind::Threshold => try_push(
                    &mut pbs,
                    Pb {
                        out: v,
                        conditions: copy_row(&row, work)?,
                        weights: copy_row(shape.row_weights(node), work)?,
                        bound: shape.bound(node),
                        sure: 0,
                        open: 0,
                        heaviest: 0,
                    },
                )?,
            }
        }
        for &root in &shape.roots {
            work.tick()?;
            let mut clause = vec![lit(node_var[root], false)];
            if let Some(guard) = guard {
                try_push(&mut clause, lit(guard, true))?;
            }
            clauses.push(clause, work)?;
        }
        if let (Some(support), None) = (&shape.support, guard) {
            self.support_clauses(support, atoms, &node_var, &mut clauses, work)?;
        }
        let first_pb = self.pbs.len();
        for (offset, pb) in pbs.into_iter().enumerate() {
            work.tick()?;
            self.add_pb(idx(first_pb + offset), pb, work)?;
        }
        self.add_original_clauses(clauses, work)?;
        if self.inconsistent {
            return Ok(());
        }
        // Thresholds with nothing assigned still need an initial check.
        for index in first_pb..self.pbs.len() {
            work.tick()?;
            if self.pb_check(idx(index), work)?.is_some() {
                self.inconsistent = true;
                return Ok(());
            }
        }
        self.head = 0;
        if self.propagate(work)?.is_some() {
            self.inconsistent = true;
        }

        Ok(())
    }

    /// Give every non-atom node its own variable, retaining atom aliases.
    fn variables_for<Q: Quota + ?Sized>(
        &mut self,
        shape: &Shape,
        work: &mut Meter<'_, Q>,
    ) -> Result<Vec<u32>, Failure<Q::Error>> {
        // Atom nodes alias their atom; other nodes get their own variable.
        let mut node_var = Vec::new();
        for node in 0..shape.nodes() {
            work.tick()?;
            try_push(
                &mut node_var,
                match shape.kind(node) {
                    Kind::Atom(atom) => atom,
                    Kind::Cut => self.fresh(true, work)?,
                    _ => self.fresh(false, work)?,
                },
            )?;
        }
        Ok(node_var)
    }

    /// Normalize and install the prepared original clauses in arena order.
    fn add_original_clauses<Q: Quota + ?Sized>(
        &mut self,
        clauses: Arena,
        work: &mut Meter<'_, Q>,
    ) -> Result<(), Failure<Q::Error>> {
        let mut clause = Vec::new();
        for index in 0..clauses.len() {
            work.tick()?;
            clause.clear();
            append_copy(&mut clause, clauses.get(index), work)?;
            sort_dedup(&mut clause, work)?;
            let mut tautology = false;
            for w in clause.windows(2) {
                work.tick()?;
                if w[0] == neg(w[1]) {
                    tautology = true;
                    break;
                }
            }
            if tautology {
                continue;
            }
            self.add_original(&clause, work)?;
        }
        drop(clauses);
        Ok(())
    }

    /// The support condition of every atom: held only with a producer whose
    /// body holds and whose other ordinary heads fail.
    fn support_clauses<Q: Quota + ?Sized>(
        &mut self,
        support: &Support,
        atoms: usize,
        node_var: &[u32],
        clauses: &mut Arena,
        work: &mut Meter<'_, Q>,
    ) -> Result<(), Failure<Q::Error>> {
        work.tick()?;
        for atom in 0..atoms {
            work.tick()?;
            let (s, e) = (
                support.offsets[atom] as usize,
                support.offsets[atom + 1] as usize,
            );
            let mut clause = vec![lit(idx(atom), true)];
            let mut always = false;
            for &(body, start, end) in &support.entries[s..e] {
                work.tick()?;
                let others = &support.others[start as usize..end as usize];
                if others.is_empty() {
                    if body == u32::MAX {
                        always = true;
                        break;
                    }
                    try_push(&mut clause, lit(node_var[body as usize], false))?;
                } else {
                    // s <-> body & !others
                    let sp = lit(self.fresh(false, work)?, false);
                    let mut long = vec![sp];
                    if body != u32::MAX {
                        let b = lit(node_var[body as usize], false);
                        clauses.push(vec![neg(sp), b], work)?;
                        try_push(&mut long, neg(b))?;
                    }
                    for &other in others {
                        work.tick()?;
                        clauses.push(vec![neg(sp), lit(other, true)], work)?;
                        try_push(&mut long, lit(other, false))?;
                    }
                    clauses.push(long, work)?;
                    try_push(&mut clause, sp)?;
                }
            }
            if !always {
                clauses.push(clause, work)?;
            }
        }

        Ok(())
    }

    /// Install threshold `index`, counting what is already assigned.
    fn add_pb<Q: Quota + ?Sized>(
        &mut self,
        index: u32,
        mut pb: Pb,
        work: &mut Meter<'_, Q>,
    ) -> Result<(), Failure<Q::Error>> {
        work.tick()?;
        if index > REASON_PAYLOAD {
            return Err(Stop::StorageLimit.into());
        }
        try_push(&mut self.pb_lists_mut(pb.out as usize)?.occurs, index)?;
        let mut terms = Vec::new();
        for (&condition, &weight) in pb.conditions.iter().zip(&pb.weights) {
            work.tick()?;
            try_push(&mut terms, (condition, weight))?;
        }
        sort_values(&mut terms, work)?;
        let mut merged: Vec<(u32, u64)> = Vec::new();
        for (c, w) in terms {
            work.tick()?;
            match merged.last_mut() {
                Some(last) if last.0 == c => {
                    last.1 = last.1.checked_add(w).ok_or(Stop::StorageLimit)?;
                }
                _ => try_push(&mut merged, (c, w))?,
            }
        }
        pb.sure = 0;
        pb.open = 0;
        pb.heaviest = 0;
        for &(c, weight) in &merged {
            work.tick()?;
            let w = i128::from(weight);
            pb.heaviest = pb.heaviest.max(w);
            match self.assigns[c as usize] {
                TRUE => pb.sure += w,
                UNDEF => pb.open += w,
                _ => {}
            }
            try_push(&mut self.pb_lists_mut(c as usize)?.terms, (index, weight))?;
        }
        let mut seen_conditions = copy_row(&pb.conditions, work)?;
        sort_dedup(&mut seen_conditions, work)?;
        for &c in &seen_conditions {
            work.tick()?;
            try_push(&mut self.pb_lists_mut(c as usize)?.occurs, index)?;
        }
        try_push(&mut self.pbs, pb)?;

        Ok(())
    }

    fn value(&self, v: u32) -> u8 {
        self.assigns[v as usize]
    }

    fn lit_value(&self, l: u32) -> u8 {
        let a = self.assigns[var(l) as usize];
        if a == UNDEF {
            UNDEF
        } else {
            a ^ u8::from(l & 1 == 1)
        }
    }

    fn add_original<Q: Quota + ?Sized>(
        &mut self,
        clause: &[u32],
        work: &mut Meter<'_, Q>,
    ) -> Result<(), Failure<Q::Error>> {
        work.tick()?;
        if self.inconsistent {
            return Ok(());
        }
        match clause.len() {
            0 => self.inconsistent = true,
            1 => match self.lit_value(clause[0]) {
                FALSE => self.inconsistent = true,
                UNDEF => self.assign(clause[0], Reason::None, work)?,
                _ => {}
            },
            _ => {
                self.attach(clause, false, work)?;
            }
        }

        Ok(())
    }

    /// Store a clause of at least two literals and watch its first two; a
    /// two-literal clause lives in the watches alone. The reason that
    /// asserts its first literal is returned.
    fn attach<Q: Quota + ?Sized>(
        &mut self,
        clause: &[u32],
        learned: bool,
        work: &mut Meter<'_, Q>,
    ) -> Result<Reason, Failure<Q::Error>> {
        work.tick()?;
        if clause.len() == 2 {
            try_push(
                &mut self.watches[neg(clause[0]) as usize],
                watch(BINARY, clause[1]),
            )?;
            try_push(
                &mut self.watches[neg(clause[1]) as usize],
                watch(BINARY, clause[0]),
            )?;
            return Ok(Reason::Binary(clause[1]));
        }
        let index = self.clauses.push(clause, work)?;
        try_push(
            &mut self.watches[neg(clause[0]) as usize],
            watch(index, clause[1]),
        )?;
        try_push(
            &mut self.watches[neg(clause[1]) as usize],
            watch(index, clause[0]),
        )?;
        self.is_learned.push(learned)?;
        if learned {
            self.learned_count += 1;
        }
        Ok(Reason::Clause(index))
    }

    fn assign<Q: Quota + ?Sized>(
        &mut self,
        l: u32,
        reason: Reason,
        work: &mut Meter<'_, Q>,
    ) -> Result<(), Failure<Q::Error>> {
        work.tick()?;
        let v = var(l) as usize;
        self.assigns[v] = TRUE ^ u8::from(l & 1 == 1);
        self.level[v] = idx(self.trail_lim.len());
        self.reason[v] = PackedReason::pack(reason);
        self.trail_pos[v] = idx(self.trail.len());
        try_push(&mut self.trail, l)?;
        let positive = l & 1 == 0;
        let slot = self.pb_slot[v];
        let terms: &[(u32, u64)] = if slot == NO_PB {
            &[]
        } else {
            &self.pb_lists[slot as usize].terms
        };
        for &(index, w) in terms {
            work.tick()?;
            let pb = &mut self.pbs[index as usize];
            pb.open -= i128::from(w);
            if positive {
                pb.sure += i128::from(w);
            }
        }

        Ok(())
    }

    fn decision_level(&self) -> usize {
        self.trail_lim.len()
    }

    /// Unit propagation over clauses and thresholds; a conflict returns its
    /// clause (as literals, all false).
    fn propagate<Q: Quota + ?Sized>(
        &mut self,
        work: &mut Meter<'_, Q>,
    ) -> Result<Option<Vec<u32>>, Failure<Q::Error>> {
        work.tick()?;
        while self.head < self.trail.len() {
            work.tick()?;
            let p = self.trail[self.head];
            self.head += 1;
            self.counts.propagations += 1;
            // Clauses watching !p: those that contain neg(p)... we watch
            // neg(first) under index of the literal that falsifies it.
            let false_lit = neg(p);
            let mut watchers = std::mem::take(&mut self.watches[p as usize]);
            let mut i = 0;
            let mut conflict = None;
            while i < watchers.len() {
                work.tick()?;
                let entry = watchers[i];
                let blocker = watch_blocker(entry);
                let held = self.assigns[var(blocker) as usize];
                if held != UNDEF && (held ^ u8::from(blocker & 1 == 1)) == TRUE {
                    i += 1;
                    continue;
                }
                let ci = watch_clause(entry);
                if ci == BINARY {
                    if held == UNDEF {
                        self.assign(blocker, Reason::Binary(false_lit), work)?;
                        i += 1;
                    } else {
                        conflict = Some(vec![blocker, false_lit]);
                        break;
                    }
                    continue;
                }
                let ci = ci as usize;
                let clause = self.clauses.get_mut(ci);
                if clause[0] == false_lit {
                    clause.swap(0, 1);
                }
                let first = clause[0];
                let a = self.assigns[var(first) as usize];
                if a != UNDEF && (a ^ u8::from(first & 1 == 1)) == TRUE {
                    watchers[i] = watch(idx(ci), first);
                    i += 1;
                    continue;
                }
                let mut moved = false;
                for k in 2..clause.len() {
                    work.tick()?;
                    let l = clause[k];
                    let a = self.assigns[var(l) as usize];
                    if a == UNDEF || (a ^ u8::from(l & 1 == 1)) == TRUE {
                        clause.swap(1, k);
                        let other = clause[1];
                        try_push(
                            &mut self.watches[neg(other) as usize],
                            watch(idx(ci), first),
                        )?;
                        watchers.swap_remove(i);
                        moved = true;
                        break;
                    }
                }
                if moved {
                    continue;
                }
                if a == UNDEF {
                    self.assign(first, Reason::Clause(idx(ci)), work)?;
                    i += 1;
                } else {
                    conflict = Some(copy_row(self.clauses.get(ci), work)?);
                    break;
                }
            }
            let existing = std::mem::take(&mut self.watches[p as usize]);
            append_copy(&mut watchers, &existing, work)?;
            self.watches[p as usize] = watchers;
            if conflict.is_some() {
                self.head = self.trail.len();
                return Ok(conflict);
            }
            let slot = self.pb_slot[var(p) as usize];
            let occurs = if slot == NO_PB {
                Vec::new()
            } else {
                std::mem::take(&mut self.pb_lists[slot as usize].occurs)
            };
            let mut pb_conflict = None;
            for &index in &occurs {
                work.tick()?;
                if let Some(clause) = self.pb_check(index, work)? {
                    pb_conflict = Some(clause);
                    break;
                }
            }
            if slot != NO_PB {
                self.pb_lists[slot as usize].occurs = occurs;
            }
            if pb_conflict.is_some() {
                self.head = self.trail.len();
                return Ok(pb_conflict);
            }
        }
        Ok(None)
    }

    /// Propagate one threshold: assign what it forces, with explanations, or
    /// return the conflict clause.
    fn pb_check<Q: Quota + ?Sized>(
        &mut self,
        index: u32,
        work: &mut Meter<'_, Q>,
    ) -> Result<Option<Vec<u32>>, Failure<Q::Error>> {
        work.tick()?;
        let pb = &self.pbs[index as usize];
        let (sure, open) = (pb.sure, pb.open);
        let out = self.assigns[pb.out as usize];
        let bound = pb.bound;
        if sure < bound && bound <= sure + open {
            let quiet = match out {
                TRUE => sure + open - pb.heaviest >= bound,
                FALSE => sure + pb.heaviest < bound,
                _ => true,
            };
            if quiet {
                return Ok(None);
            }
        }
        let conditions = |solver: &Self,
                          truth: u8,
                          work: &mut Meter<'_, Q>|
         -> Result<Vec<u32>, Failure<Q::Error>> {
            let pb = &solver.pbs[index as usize];
            let mut row = Vec::new();
            for (&c, &w) in pb.conditions.iter().zip(&pb.weights) {
                work.tick()?;
                if w > 0 && solver.assigns[c as usize] == truth {
                    try_push(&mut row, lit(c, truth == TRUE))?;
                }
            }
            Ok(row)
        };
        let out_lit = lit(pb.out, false);
        if sure >= bound {
            // out must hold: (out | !true conditions)
            match out {
                TRUE => return Ok(None),
                UNDEF => {
                    self.assign(out_lit, Reason::Pb(index), work)?;
                    return Ok(None);
                }
                _ => {
                    let mut e = vec![out_lit];
                    append_copy(&mut e, &conditions(self, TRUE, work)?, work)?;
                    return Ok(Some(dedup(e, work)?));
                }
            }
        }
        if sure + open < bound {
            match out {
                FALSE => return Ok(None),
                UNDEF => {
                    self.assign(neg(out_lit), Reason::Pb(index), work)?;
                    return Ok(None);
                }
                _ => {
                    let mut e = vec![neg(out_lit)];
                    append_copy(&mut e, &conditions(self, FALSE, work)?, work)?;
                    return Ok(Some(dedup(e, work)?));
                }
            }
        }
        // sure < bound <= sure + open: out decides conditions.
        let pb = &self.pbs[index as usize];
        let mut forced: Vec<(u32, bool)> = Vec::new();
        if out == TRUE {
            for (&c, &w) in pb.conditions.iter().zip(&pb.weights) {
                work.tick()?;
                if w > 0 && self.assigns[c as usize] == UNDEF && sure + open - i128::from(w) < bound
                {
                    try_push(&mut forced, (c, true))?;
                }
            }
        } else if out == FALSE {
            for (&c, &w) in pb.conditions.iter().zip(&pb.weights) {
                work.tick()?;
                if w > 0 && self.assigns[c as usize] == UNDEF && sure + i128::from(w) >= bound {
                    try_push(&mut forced, (c, false))?;
                }
            }
        }
        if forced.is_empty() {
            return Ok(None);
        }
        for (c, value) in forced {
            work.tick()?;
            if self.assigns[c as usize] != UNDEF {
                continue; // duplicate position already forced
            }
            self.assign(lit(c, !value), Reason::Pb(index), work)?;
        }
        Ok(None)
    }

    /// The reason clause of `v` (implied literal first, the rest false).
    fn reason_lits<Q: Quota + ?Sized>(
        &self,
        v: usize,
        work: &mut Meter<'_, Q>,
    ) -> Result<Vec<u32>, Failure<Q::Error>> {
        work.tick()?;
        Ok(match self.reason[v].unpack() {
            Reason::Clause(ci) => copy_row(self.clauses.get(ci as usize), work)?,
            Reason::Binary(other) => vec![lit(idx(v), self.assigns[v] == FALSE), other],
            Reason::None => Vec::new(),
            Reason::Pb(index) => self.pb_reason(index, v, work)?,
        })
    }

    /// A threshold's explanation for `v`, from the conditions assigned before
    /// it: exactly the assignment under which the threshold forced it.
    fn pb_reason<Q: Quota + ?Sized>(
        &self,
        index: u32,
        v: usize,
        work: &mut Meter<'_, Q>,
    ) -> Result<Vec<u32>, Failure<Q::Error>> {
        work.tick()?;
        let pb = &self.pbs[index as usize];
        let before = self.trail_pos[v];
        let earlier =
            |c: u32| self.assigns[c as usize] != UNDEF && self.trail_pos[c as usize] < before;
        let implied = lit(idx(v), self.assigns[v] == FALSE);
        let mut e = vec![implied];
        let out_lit = lit(pb.out, false);
        // Which side is used: true conditions (a lower bound reached) or
        // false conditions (the bound unreachable).
        let use_true = if v == pb.out as usize {
            self.assigns[v] == TRUE
        } else {
            try_push(
                &mut e,
                if self.assigns[pb.out as usize] == TRUE {
                    neg(out_lit)
                } else {
                    out_lit
                },
            )?;
            self.assigns[pb.out as usize] == FALSE
        };
        for (&c, &w) in pb.conditions.iter().zip(&pb.weights) {
            work.tick()?;
            if w == 0 || c as usize == v || !earlier(c) {
                continue;
            }
            match (use_true, self.assigns[c as usize]) {
                (true, TRUE) => try_push(&mut e, lit(c, true))?,
                (false, FALSE) => try_push(&mut e, lit(c, false))?,
                _ => {}
            }
        }
        dedup_first(&e, work)
    }

    fn bump<Q: Quota + ?Sized>(
        &mut self,
        v: usize,
        work: &mut Meter<'_, Q>,
    ) -> Result<(), Failure<Q::Error>> {
        work.tick()?;
        self.activity[v] += self.increment;
        if self.activity[v] > 1e100 {
            for a in &mut self.activity {
                work.tick()?;
                *a *= 1e-100;
            }
            self.increment *= 1e-100;
        }
        if self.heap_index[v] >= 0 {
            self.heap_up(unslot(self.heap_index[v]), work)?;
        }

        Ok(())
    }

    /// First-UIP analysis; returns the learned clause (asserting literal
    /// first) and the backjump level.
    fn analyze<Q: Quota + ?Sized>(
        &mut self,
        conflict: Vec<u32>,
        work: &mut Meter<'_, Q>,
    ) -> Result<(Vec<u32>, usize), Failure<Q::Error>> {
        work.tick()?;
        let mut learned = vec![0_u32];
        let mut pending = 0;
        let mut index = self.trail.len();
        let mut current: Vec<u32> = conflict;
        let mut p: Option<u32> = None;
        let level = idx(self.decision_level());
        loop {
            work.tick()?;
            for &q in &current {
                work.tick()?;
                if Some(q) == p {
                    continue;
                }
                let v = var(q) as usize;
                if !self.seen.get(v) && self.level[v] > 0 {
                    self.seen.set(v, true);
                    self.bump(v, work)?;
                    if self.level[v] >= level {
                        pending += 1;
                    } else {
                        try_push(&mut learned, q)?;
                    }
                }
            }
            loop {
                work.tick()?;
                index -= 1;
                if self.seen.get(var(self.trail[index]) as usize) {
                    break;
                }
            }
            let next = self.trail[index];
            let v = var(next) as usize;
            self.seen.set(v, false);
            pending -= 1;
            if pending == 0 {
                learned[0] = neg(next);
                break;
            }
            p = Some(next);
            current = self.reason_lits(v, work)?;
        }
        for &q in &learned[1..] {
            work.tick()?;
            self.seen.set(var(q) as usize, false);
        }
        let mut back = 0;
        if learned.len() > 1 {
            let mut best = 1;
            for k in 2..learned.len() {
                work.tick()?;
                if self.level[var(learned[k]) as usize] > self.level[var(learned[best]) as usize] {
                    best = k;
                }
            }
            learned.swap(1, best);
            back = self.level[var(learned[1]) as usize] as usize;
        }
        self.increment *= 1.0 / 0.95;
        Ok((learned, back))
    }

    fn cancel_until<Q: Quota + ?Sized>(
        &mut self,
        level: usize,
        work: &mut Meter<'_, Q>,
    ) -> Result<(), Failure<Q::Error>> {
        work.tick()?;
        if self.decision_level() <= level {
            return Ok(());
        }
        let start = self.trail_lim[level];
        for k in (start..self.trail.len()).rev() {
            work.tick()?;
            let l = self.trail[k];
            let v = var(l) as usize;
            self.phase.set(v, l & 1 == 0);
            let slot = self.pb_slot[v];
            let terms: &[(u32, u64)] = if slot == NO_PB {
                &[]
            } else {
                &self.pb_lists[slot as usize].terms
            };
            for &(index, w) in terms {
                work.tick()?;
                let pb = &mut self.pbs[index as usize];
                pb.open += i128::from(w);
                if l & 1 == 0 {
                    pb.sure -= i128::from(w);
                }
            }
            self.assigns[v] = UNDEF;
            self.reason[v] = PackedReason::NONE;
            if self.heap_index[v] < 0 {
                self.heap_insert(idx(v), work)?;
            }
        }
        self.trail.truncate(start);
        self.trail_lim.truncate(level);
        self.head = start;

        Ok(())
    }

    /// Search under `assumptions`. `Unsatisfiable(n)` refutes using `n` of
    /// them.
    fn solve<Q: Quota + ?Sized>(
        &mut self,
        assumptions: &[u32],
        allowance: u64,
        cancellation: &Cancellation,
        work: &mut Meter<'_, Q>,
        idle: Option<&std::sync::atomic::AtomicUsize>,
    ) -> Result<Outcome, Failure<Q::Error>> {
        self.core.clear();
        if self.inconsistent {
            return Ok(Outcome::Unsatisfiable(0));
        }
        self.cancel_until(0, work)?;
        let start_conflicts = self.counts.conflicts;
        let mut restart_at = 100_u64;
        let mut since_restart = 0_u64;
        // All assumptions share decision level 1, so a backjump never
        // re-propagates them one level at a time.
        let mut assumed = false;
        loop {
            work.tick()?;
            if let Some(conflict) = self.propagate(work)? {
                self.counts.conflicts += 1;
                since_restart += 1;
                match self.decision_level() {
                    0 => {
                        self.inconsistent = true;
                        return Ok(Outcome::Unsatisfiable(0));
                    }
                    1 => {
                        // Under the assumptions alone: they are refuted.
                        let mut core = std::mem::take(&mut self.core);
                        self.conflict_core(&conflict, &mut core, work)?;
                        self.core = core;
                        self.cancel_until(0, work)?;
                        return Ok(Outcome::Unsatisfiable(self.core.len()));
                    }
                    _ => {}
                }
                let (learned, back) = self.analyze(conflict, work)?;
                if learned.len() == 1 || back == 0 {
                    self.cancel_until(0, work)?;
                    assumed = false;
                } else {
                    self.cancel_until(back, work)?;
                }
                if learned.len() == 1 {
                    self.assign(learned[0], Reason::None, work)?;
                } else {
                    let reason = self.attach(&learned, true, work)?;
                    self.counts.learned += 1;
                    self.assign(learned[0], reason, work)?;
                }
                if self.counts.conflicts.trailing_zeros() >= 8 {
                    cancellation.poll()?;
                }
                if self.counts.conflicts - start_conflicts >= allowance
                    || idle.is_some_and(|idle| idle.load(std::sync::atomic::Ordering::Relaxed) > 0)
                {
                    self.cancel_until(0, work)?;
                    return Ok(Outcome::Unknown);
                }
                continue;
            }
            if since_restart >= restart_at && self.decision_level() > 1 {
                since_restart = 0;
                restart_at = restart_at + restart_at / 2;
                self.cancel_until(1, work)?;
                if self.learned_count > 20_000 + width(self.counts.conflicts) / 4 {
                    self.reduce(work)?;
                }
                continue;
            }
            if !assumed {
                assumed = true;
                if let Some(refuted) = self.assume(assumptions, work)? {
                    return Ok(refuted);
                }
                continue;
            }
            let Some(v) = self.pick(work)? else {
                // Leave the model readable through `value`.
                return Ok(Outcome::Satisfiable);
            };
            self.counts.decisions += 1;
            let decision = lit(v, !self.phase.get(v as usize));
            try_push(&mut self.trail_lim, self.trail.len())?;
            self.assign(decision, Reason::None, work)?;
        }
    }

    /// Assume `assumptions` at level 1, then try each of at most `budget`
    /// unassigned `candidates` both ways by propagation alone. A side that
    /// fails is learned as the clause forcing the other (a consequence: its
    /// core implies the other side); both failing refutes the assumptions.
    fn probe<Q: Quota + ?Sized>(
        &mut self,
        assumptions: &[u32],
        candidates: &[u32],
        budget: usize,
        cancellation: &Cancellation,
        work: &mut Meter<'_, Q>,
    ) -> Result<Outcome, Failure<Q::Error>> {
        self.core.clear();
        if self.inconsistent {
            return Ok(Outcome::Unsatisfiable(0));
        }
        self.cancel_until(0, work)?;
        if self.propagate(work)?.is_some() {
            self.inconsistent = true;
            return Ok(Outcome::Unsatisfiable(0));
        }
        if let Some(refuted) = self.assume(assumptions, work)? {
            return Ok(refuted);
        }
        let mut spent = 0;
        let mut cores: [Vec<u32>; 2] = [Vec::new(), Vec::new()];
        for &v in candidates {
            work.tick()?;
            if spent >= budget {
                break;
            }
            if self.assigns[v as usize] != UNDEF {
                continue;
            }
            spent += 1;
            if spent % 64 == 0 {
                cancellation.poll()?;
            }
            let mut failed = [false; 2];
            for (side, &l) in [lit(v, false), lit(v, true)].iter().enumerate() {
                try_push(&mut self.trail_lim, self.trail.len())?;
                self.assign(l, Reason::None, work)?;
                if let Some(conflict) = self.propagate(work)? {
                    failed[side] = true;
                    let mut core = std::mem::take(&mut cores[side]);
                    self.conflict_core(&conflict, &mut core, work)?;
                    let mut written = 0;
                    for read in 0..core.len() {
                        work.tick()?;
                        if core[read] != l {
                            core[written] = core[read];
                            written += 1;
                        }
                    }
                    core.truncate(written);
                    cores[side] = core;
                }
                self.cancel_until(1, work)?;
            }
            match failed {
                [true, true] => {
                    let mut core = std::mem::take(&mut cores[0]);
                    append_copy(&mut core, &cores[1], work)?;
                    sort_dedup(&mut core, work)?;
                    self.core = core;
                    self.cancel_until(0, work)?;
                    return Ok(Outcome::Unsatisfiable(self.core.len()));
                }
                [true, false] | [false, true] => {
                    let side = usize::from(failed[0]);
                    let other = [lit(v, false), lit(v, true)][side];
                    let core = &cores[1 - side];
                    if core.is_empty() {
                        continue; // a level-0 consequence; leave it to search
                    }
                    let mut clause = vec![other];
                    for &k in core {
                        work.tick()?;
                        try_push(&mut clause, neg(k))?;
                    }
                    let reason = self.attach(&clause, true, work)?;
                    self.counts.learned += 1;
                    self.assign(other, reason, work)?;
                    if let Some(conflict) = self.propagate(work)? {
                        return self.refuted_by(&conflict, work);
                    }
                }
                [false, false] => {}
            }
        }
        self.cancel_until(0, work)?;
        Ok(Outcome::Unknown)
    }

    /// Open level 1 and assume, propagating as we go; a refutation of the
    /// assumptions alone is returned with its core.
    fn assume<Q: Quota + ?Sized>(
        &mut self,
        assumptions: &[u32],
        work: &mut Meter<'_, Q>,
    ) -> Result<Option<Outcome>, Failure<Q::Error>> {
        work.tick()?;
        try_push(&mut self.trail_lim, self.trail.len())?;
        for &a in assumptions {
            work.tick()?;
            match self.lit_value(a) {
                TRUE => {}
                FALSE => {
                    let mut core = std::mem::take(&mut self.core);
                    self.analyze_final(neg(a), &mut core, work)?;
                    self.core = core;
                    self.cancel_until(0, work)?;
                    return Ok(Some(Outcome::Unsatisfiable(self.core.len())));
                }
                _ => {
                    self.assign(a, Reason::None, work)?;
                    if let Some(conflict) = self.propagate(work)? {
                        return Ok(Some(self.refuted_by(&conflict, work)?));
                    }
                }
            }
        }
        Ok(None)
    }

    /// Refute the assumptions by a conflict under them, keeping its core.
    fn refuted_by<Q: Quota + ?Sized>(
        &mut self,
        conflict: &[u32],
        work: &mut Meter<'_, Q>,
    ) -> Result<Outcome, Failure<Q::Error>> {
        work.tick()?;
        let mut core = std::mem::take(&mut self.core);
        self.conflict_core(conflict, &mut core, work)?;
        self.core = core;
        self.cancel_until(0, work)?;
        Ok(Outcome::Unsatisfiable(self.core.len()))
    }

    /// The assumptions behind a conflict found at level 1, written to `core`.
    fn conflict_core<Q: Quota + ?Sized>(
        &mut self,
        conflict: &[u32],
        core: &mut Vec<u32>,
        work: &mut Meter<'_, Q>,
    ) -> Result<(), Failure<Q::Error>> {
        work.tick()?;
        core.clear();
        for &q in conflict {
            work.tick()?;
            let u = var(q) as usize;
            if self.level[u] > 0 {
                self.seen.set(u, true);
            }
        }
        let start = self.trail_lim[0];
        for k in (start..self.trail.len()).rev() {
            work.tick()?;
            let l = self.trail[k];
            let v = var(l) as usize;
            if !self.seen.get(v) {
                continue;
            }
            if self.reason[v] == PackedReason::NONE {
                try_push(core, l)?;
            } else {
                for &q in &self.reason_lits(v, work)?[1..] {
                    work.tick()?;
                    let u = var(q) as usize;
                    if self.level[u] > 0 {
                        self.seen.set(u, true);
                    }
                }
            }
            self.seen.set(v, false);
        }

        Ok(())
    }

    /// The assumptions that imply `p` (an assumption's negation now true),
    /// `p`'s own assumption included, written to `core`.
    fn analyze_final<Q: Quota + ?Sized>(
        &mut self,
        p: u32,
        core: &mut Vec<u32>,
        work: &mut Meter<'_, Q>,
    ) -> Result<(), Failure<Q::Error>> {
        work.tick()?;
        core.clear();
        try_push(core, neg(p))?;
        if self.decision_level() == 0 {
            return Ok(());
        }
        self.seen.set(var(p) as usize, true);
        let start = self.trail_lim[0];
        for k in (start..self.trail.len()).rev() {
            work.tick()?;
            let l = self.trail[k];
            let v = var(l) as usize;
            if !self.seen.get(v) {
                continue;
            }
            if self.reason[v] == PackedReason::NONE {
                if v != var(p) as usize {
                    try_push(core, l)?;
                }
            } else {
                let reason = self.reason_lits(v, work)?;
                for &q in &reason[1..] {
                    work.tick()?;
                    let u = var(q) as usize;
                    if self.level[u] > 0 {
                        self.seen.set(u, true);
                    }
                }
            }
            self.seen.set(v, false);
        }
        self.seen.set(var(p) as usize, false);

        Ok(())
    }

    fn reduce<Q: Quota + ?Sized>(
        &mut self,
        work: &mut Meter<'_, Q>,
    ) -> Result<(), Failure<Q::Error>> {
        work.tick()?;
        // Drop the older half of learned clauses not currently a reason.
        let mut locked = std::collections::HashSet::new();
        for &l in &self.trail {
            work.tick()?;
            if let Reason::Clause(ci) = self.reason[var(l) as usize].unpack() {
                locked.try_reserve(1).map_err(|_| Stop::Allocation)?;
                locked.insert(ci);
            }
        }
        let learned = self.learned_count;
        let mut keep = filled_row(self.clauses.len(), true, work)?;
        let mut dropped = 0;
        for (ci, kept) in keep.iter_mut().enumerate() {
            work.tick()?;
            if dropped >= learned / 2 {
                break;
            }
            if self.is_learned.get(ci)
                && self.clauses.get(ci).len() > 2
                && !locked.contains(&idx(ci))
            {
                *kept = false;
                dropped += 1;
            }
        }
        let mut remap = filled_row(self.clauses.len(), u32::MAX, work)?;
        let old = std::mem::take(&mut self.clauses);
        let mut kept = Arena {
            lits: Vec::new(),
            spans: Vec::new(),
        };
        let mut kept_learned = Bits::default();
        for (ci, &keep) in keep.iter().enumerate() {
            work.tick()?;
            if keep {
                remap[ci] = kept.push(old.get(ci), work)?;
                kept_learned.push(self.is_learned.get(ci))?;
            }
        }
        self.clauses = kept;
        self.is_learned = kept_learned;
        self.learned_count -= dropped;
        for watchers in &mut self.watches {
            work.tick()?;
            let mut written = 0;
            for read in 0..watchers.len() {
                work.tick()?;
                let w = watchers[read];
                let ci = watch_clause(w);
                let keep = if ci == BINARY {
                    true
                } else {
                    remap[ci as usize] != u32::MAX
                };
                if keep {
                    watchers[written] = if ci == BINARY {
                        w
                    } else {
                        watch(remap[ci as usize], watch_blocker(w))
                    };
                    written += 1;
                }
            }
            watchers.truncate(written);
        }
        for v in 0..self.vars {
            work.tick()?;
            if let Reason::Clause(ci) = self.reason[v].unpack() {
                self.reason[v] = PackedReason::pack(Reason::Clause(remap[ci as usize]));
            }
        }

        Ok(())
    }

    fn pick<Q: Quota + ?Sized>(
        &mut self,
        work: &mut Meter<'_, Q>,
    ) -> Result<Option<u32>, Failure<Q::Error>> {
        work.tick()?;
        while let Some(v) = self.heap_pop(work)? {
            work.tick()?;
            if self.assigns[v as usize] == UNDEF {
                return Ok(Some(v));
            }
        }
        // Defined variables are assigned once their definitions' inputs are;
        // any left over (none expected) is decided directly.
        for v in 0..idx(self.vars) {
            // One visit reads one assignment cell in this fallback scan.
            work.tick()?;
            if self.assigns[v as usize] == UNDEF {
                return Ok(Some(v));
            }
        }
        Ok(None)
    }

    fn heap_less(&self, a: u32, b: u32) -> bool {
        self.activity[a as usize] > self.activity[b as usize]
    }

    fn heap_insert<Q: Quota + ?Sized>(
        &mut self,
        v: u32,
        work: &mut Meter<'_, Q>,
    ) -> Result<(), Failure<Q::Error>> {
        work.tick()?;
        if !self.decidable.get(v as usize) {
            return Ok(());
        }
        self.heap_index[v as usize] = slot(self.heap.len());
        try_push(&mut self.heap, v)?;
        self.heap_up(self.heap.len() - 1, work)?;

        Ok(())
    }

    fn heap_up<Q: Quota + ?Sized>(
        &mut self,
        mut i: usize,
        work: &mut Meter<'_, Q>,
    ) -> Result<(), Failure<Q::Error>> {
        work.tick()?;
        let v = self.heap[i];
        while i > 0 {
            work.tick()?;
            let parent = (i - 1) / 2;
            if !self.heap_less(v, self.heap[parent]) {
                break;
            }
            self.heap[i] = self.heap[parent];
            self.heap_index[self.heap[i] as usize] = slot(i);
            i = parent;
        }
        self.heap[i] = v;
        self.heap_index[v as usize] = slot(i);

        Ok(())
    }

    fn heap_pop<Q: Quota + ?Sized>(
        &mut self,
        work: &mut Meter<'_, Q>,
    ) -> Result<Option<u32>, Failure<Q::Error>> {
        work.tick()?;
        let Some(&top) = self.heap.first() else {
            return Ok(None);
        };
        let last = self.heap.pop().expect("nonempty");
        self.heap_index[top as usize] = -1;
        if !self.heap.is_empty() {
            self.heap[0] = last;
            self.heap_index[last as usize] = 0;
            let mut i = 0;
            loop {
                work.tick()?;
                let (l, r) = (2 * i + 1, 2 * i + 2);
                let mut best = i;
                if l < self.heap.len() && self.heap_less(self.heap[l], self.heap[best]) {
                    best = l;
                }
                if r < self.heap.len() && self.heap_less(self.heap[r], self.heap[best]) {
                    best = r;
                }
                if best == i {
                    break;
                }
                self.heap.swap(i, best);
                self.heap_index[self.heap[i] as usize] = slot(i);
                self.heap_index[self.heap[best] as usize] = slot(best);
                i = best;
            }
        }
        Ok(Some(top))
    }
}

fn try_push<T>(values: &mut Vec<T>, value: T) -> Result<(), Stop> {
    values.try_reserve(1).map_err(|_| Stop::Allocation)?;
    values.push(value);
    Ok(())
}

/// Heap payload only; Vec headers are in their containing value/allocation.
fn bytes<T>(values: &Vec<T>) -> u128 {
    values.capacity() as u128 * size_of::<T>() as u128
}

fn append_copy<T: Copy, Q: Quota + ?Sized>(
    out: &mut Vec<T>,
    row: &[T],
    work: &mut Meter<'_, Q>,
) -> Result<(), Failure<Q::Error>> {
    for &value in row {
        work.tick()?;
        try_push(out, value)?;
    }
    Ok(())
}

fn copy_row<T: Copy, Q: Quota + ?Sized>(
    row: &[T],
    work: &mut Meter<'_, Q>,
) -> Result<Vec<T>, Failure<Q::Error>> {
    let mut out = Vec::new();
    append_copy(&mut out, row, work)?;
    Ok(out)
}

fn filled_row<T: Copy, Q: Quota + ?Sized>(
    len: usize,
    value: T,
    work: &mut Meter<'_, Q>,
) -> Result<Vec<T>, Failure<Q::Error>> {
    let mut out = Vec::new();
    for _ in 0..len {
        work.tick()?;
        try_push(&mut out, value)?;
    }
    Ok(out)
}

/// Sort by value through the shared fallible comparison/write primitive.
fn sort_values<T: Ord + Copy, Q: Quota + ?Sized>(
    values: &mut [T],
    work: &mut Meter<'_, Q>,
) -> Result<(), Failure<Q::Error>> {
    let mut order = Vec::new();
    let mut scratch = Vec::new();
    let mut source = Vec::new();
    for (i, &value) in values.iter().enumerate() {
        work.tick()?;
        try_push(&mut order, i)?;
        try_push(&mut scratch, 0)?;
        try_push(&mut source, value)?;
    }
    zetesis_core::checked_sort::sort(
        &mut order,
        &mut scratch,
        |cells, left, right, before| {
            before()?;
            Ok(values[cells[left]].cmp(&values[cells[right]]))
        },
        &mut || work.tick(),
    )?;
    for (value, &index) in values.iter_mut().zip(&order) {
        work.tick()?;
        *value = source[index];
    }
    Ok(())
}

fn sort_dedup<T: Ord + Copy, Q: Quota + ?Sized>(
    values: &mut Vec<T>,
    work: &mut Meter<'_, Q>,
) -> Result<(), Failure<Q::Error>> {
    sort_values(values, work)?;
    let mut written = 0;
    for read in 0..values.len() {
        work.tick()?;
        if written == 0 || values[read] != values[written - 1] {
            values[written] = values[read];
            written += 1;
        }
    }
    values.truncate(written);
    Ok(())
}

fn dedup<Q: Quota + ?Sized>(
    mut clause: Vec<u32>,
    work: &mut Meter<'_, Q>,
) -> Result<Vec<u32>, Failure<Q::Error>> {
    sort_dedup(&mut clause, work)?;
    Ok(clause)
}

/// Keep the first literal first, deduplicate the rest.
fn dedup_first<Q: Quota + ?Sized>(
    clause: &[u32],
    work: &mut Meter<'_, Q>,
) -> Result<Vec<u32>, Failure<Q::Error>> {
    work.tick()?;
    let first = clause[0];
    let mut rest = Vec::new();
    for &l in &clause[1..] {
        work.tick()?;
        if l != first {
            try_push(&mut rest, l)?;
        }
    }
    sort_dedup(&mut rest, work)?;
    let mut out = Vec::new();
    try_push(&mut out, first)?;
    for l in rest {
        work.tick()?;
        try_push(&mut out, l)?;
    }
    Ok(out)
}

/// An encoding index as stored: variables, clauses and nodes fit in `u32`.
fn idx(n: usize) -> u32 {
    u32::try_from(n).expect("encoding indices fit in u32")
}

/// A heap position as stored (`-1` marks absence).
fn slot(n: usize) -> i32 {
    i32::try_from(n).expect("heap positions fit in i32")
}

/// A stored heap position, known present.
fn unslot(n: i32) -> usize {
    usize::try_from(n).expect("present heap positions are nonnegative")
}

/// A count or duration in `u64`, saturating.
fn saturate(n: u128) -> u64 {
    u64::try_from(n).unwrap_or(u64::MAX)
}

/// A `u64` count as a size, saturating.
fn width(n: u64) -> usize {
    usize::try_from(n).unwrap_or(usize::MAX)
}

#[cfg(test)]
mod tests;
