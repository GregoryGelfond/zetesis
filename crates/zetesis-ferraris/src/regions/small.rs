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
//! empty subtrees replaced by one refutation. Both are off unless the policy
//! names a size; the metric is the region's open atoms.

use std::time::Instant;

use zetesis_algebra::{Cancellation, Stop, region::Region};

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
    /// the general narrowing.
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
    /// Refinements of the learned search and of how it steers the walk.
    pub refinements: Refinements,
    /// Under steering, a search that spends its allowance probes the root
    /// only while the learned search refutes fewer than this percentage of
    /// the regions it reads (its yield): a search that mostly refutes is
    /// proving, not looking for a completion. 100 always probes.
    pub steer_yield: usize,
    /// Under [`Refinements::LOCAL`], the longest restriction-free clause
    /// kept, whole, once its search ends.
    pub local_keep: usize,
}

/// Refinements of the learned search, bit-packed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Refinements(u8);

impl Refinements {
    /// Minimize each learned clause by dropping the literals its other
    /// literals imply through reasons (recursive minimization).
    pub const MINIMIZE: Self = Self(1);
    /// Restart the learned search on the Luby sequence (unit 64 conflicts)
    /// rather than on a geometric schedule.
    pub const LUBY: Self = Self(2);
    /// Let a completion the learned search finds steer the walk: a search
    /// that spends its allowance is followed by one under the restriction
    /// guards alone, the walk defers regions until it reaches one holding
    /// the completion found, and splits there on an atom it holds. Orders
    /// the walk only; refutes and decides nothing.
    pub const STEER: Self = Self(4);
    /// Under steering, probe the root once before the walk's first learned
    /// search, even while peers wait: in a parallel walk their regions are
    /// what the completion found orders.
    pub const EAGER: Self = Self(8);
    /// Encode compactly: a negation is its operand's literal negated, and a
    /// root implication is one clause over its premise's and conclusion's
    /// literals, reading through a conjunctive premise or a disjunctive
    /// conclusion that nothing else reads. Same models over the atoms, fewer
    /// auxiliary variables to propagate.
    pub const COMPACT: Self = Self(16);
    /// Keep learned clauses by their literal block distance (the decision
    /// levels they span): those spanning two levels or fewer stay, and a
    /// smaller database is halved more often, dropping the widest spans
    /// first rather than the oldest clauses.
    pub const GLUE: Self = Self(32);
    /// Learn under the restriction without its literals: a clause learned
    /// from a conflict that reads restriction literals is propagated
    /// without them for the rest of the search, which keeps those literals
    /// aside to explain any refutation it takes part in, and is put back
    /// whole (or forgotten, past `local_keep` literals) when the search
    /// ends. Shorter clauses to propagate; the same refutations.
    pub const LOCAL: Self = Self(64);
    /// Count thresholds over the same weighted conditions once: such a
    /// family (an aggregate compared with many bounds, as an assignment
    /// aggregate's values are) keeps one true and one open weight, updated
    /// once per assignment, and checks its members by bound.
    pub const FAMILY: Self = Self(128);
    /// Every refinement.
    pub const ALL: Self = Self(255);
    /// The measured default: minimized clauses and Luby restarts; steering
    /// is asked for with `steer=1`.
    pub const MEASURED: Self = Self(Self::MINIMIZE.0 | Self::LUBY.0);

    /// Whether every refinement of `other` is on.
    #[must_use]
    pub const fn has(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// These refinements with `other` on or off.
    #[must_use]
    pub const fn with(self, other: Self, on: bool) -> Self {
        if on {
            Self(self.0 | other.0)
        } else {
            Self(self.0 & !other.0)
        }
    }
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
    /// than 32 nodes per atom) theories, in the main thread's walker only,
    /// with minimized learned clauses and Luby restarts.
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
            refinements: Refinements::MEASURED,
            steer_yield: 100,
            local_keep: usize::MAX,
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
            steer_yield: 100,
            local_keep: usize::MAX,
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
                "minimize" => {
                    policy.refinements = policy.refinements.with(Refinements::MINIMIZE, parsed > 0);
                }
                "luby" => {
                    policy.refinements = policy.refinements.with(Refinements::LUBY, parsed > 0);
                }
                "steer" => {
                    policy.refinements = policy.refinements.with(Refinements::STEER, parsed > 0);
                }
                "glue" => {
                    policy.refinements = policy.refinements.with(Refinements::GLUE, parsed > 0);
                }
                "compact" => {
                    policy.refinements = policy.refinements.with(Refinements::COMPACT, parsed > 0);
                }
                "eager" => {
                    policy.refinements = policy.refinements.with(Refinements::EAGER, parsed > 0);
                }
                "steer_yield" => policy.steer_yield = parsed,
                "local" => {
                    policy.refinements = policy.refinements.with(Refinements::LOCAL, parsed > 0);
                }
                "local_keep" => policy.local_keep = parsed,
                "family" => {
                    policy.refinements = policy.refinements.with(Refinements::FAMILY, parsed > 0);
                }
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
    /// The region is outside every specialization's size, or its shape is.
    Declined,
    /// Some completion passes, or the search spent its allowance: the region
    /// stays as the general narrowing closed it.
    Open(Variant),
    /// No completion of the region satisfies the necessary conditions.
    Refuted(Variant),
}

/// Counts and time of the specialized narrowings one walker made.
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
    /// Searches under the restriction guards alone, after a learned search
    /// spent its allowance, and those that found a completion.
    pub root_probes: u64,
    /// See [`Self::root_probes`].
    pub root_completions: u64,
    /// Regions the walk was asked to visit after the others pending.
    pub deferrals: u64,
    /// Guided splits whose child lost the completion, stopping steering.
    pub steer_failures: u64,
    /// Guided walks that reached their completion's leaf, ending steering.
    pub steer_successes: u64,
    /// Completions adopted from a peer's root probe.
    pub adopted: u64,
}

/// A walker's specialized-narrowing state: its policy, the clausal reading of
/// the theory it last read, kept with its learned clauses, and enumeration
/// buffers. It belongs to the walker's scratch, never to a region.
#[derive(Debug, Default)]
pub(crate) struct SmallScratch {
    /// The completion a root probe published to every walker of the walk.
    shared: Option<std::sync::Arc<SharedCompletion>>,
    /// The shared completion's generation this walker last adopted.
    adopted: u64,
    /// Whether this walker's one eager root probe ran.
    eager_probed: bool,
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
    /// A completion found by a root probe outside the current region: the
    /// walk defers regions until it reaches one holding it.
    jump: bool,
    /// Deferrals the current jump may still ask for.
    jump_left: u32,
    /// The completion the last guided split headed for: the next region
    /// the walk narrows is that split's held child, which holds it unless
    /// the general narrowing excluded it, a proof it is no answer set.
    following: Option<Vec<u64>>,
    /// Steering is over for this theory: a guided split's child lost its
    /// completion (the theory's completions are not answer sets), or a
    /// guided walk reached its completion's leaf (an answer was found).
    distrusted: bool,
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

/// A completion one walker's root probe found, shared with its peers so
/// each defers and splits toward it. Orders walks only.
#[derive(Debug, Default)]
pub struct SharedCompletion {
    /// Bumped on every publication; zero means none yet.
    generation: std::sync::atomic::AtomicU64,
    /// The latest completion, as held-atom words.
    model: std::sync::Mutex<Vec<u64>>,
}

impl SharedCompletion {
    fn publish(&self, model: &[u64]) {
        let mut slot = self
            .model
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        slot.clear();
        slot.extend_from_slice(model);
        self.generation
            .fetch_add(1, std::sync::atomic::Ordering::Release);
    }
}

/// Whether `region` holds the completion `model` (held-atom words): every
/// held atom of the region is held in it and no cut atom is.
fn holds(region: &Region, model: &[u64]) -> bool {
    let (held, cut) = region.decision_words();
    held.iter()
        .zip(cut)
        .zip(model)
        .all(|((&held, &cut), &model)| held & !model == 0 && cut & model == 0)
}

/// The `i`-th term (from 0) of the Luby sequence 1, 1, 2, 1, 1, 2, 4, ...
fn luby(mut i: u64) -> u64 {
    let mut size = 1_u64;
    let mut seq = 0_u32;
    while size < i + 1 {
        seq += 1;
        size = 2 * size + 1;
    }
    while size - 1 != i {
        size = (size - 1) >> 1;
        seq -= 1;
        i %= size;
    }
    1_u64 << seq
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

fn csr(rows: usize, edges: &[(u32, u32)]) -> (Vec<u32>, Vec<u32>) {
    let mut offsets = vec![0_u32; rows + 1];
    for &(row, _) in edges {
        offsets[row as usize + 1] += 1;
    }
    for row in 0..rows {
        offsets[row + 1] += offsets[row];
    }
    let mut cursor = offsets.clone();
    let mut entries = vec![0_u32; edges.len()];
    for &(row, value) in edges {
        entries[cursor[row as usize] as usize] = value;
        cursor[row as usize] += 1;
    }
    (offsets, entries)
}

impl Support {
    /// Each atom's producer rows, from the non-circular producers.
    fn read(producers: &Producers, atoms: usize, count: usize) -> Self {
        let mut rows: Vec<Vec<(u32, u32, u32)>> = vec![Vec::new(); atoms];
        let mut others = Vec::new();
        let mut body_edges = Vec::new();
        let mut rival_edges = Vec::new();
        for (index, rule) in producers.rules.iter().enumerate() {
            if producers.circular(index) {
                continue;
            }
            let mut heads = rule.heads.clone();
            heads.sort_unstable();
            heads.dedup();
            for &head in &heads {
                let start = idx(others.len());
                if !rule.choice {
                    for &other in &heads {
                        if other != head {
                            others.push(idx(other));
                            rival_edges.push((idx(other), idx(head)));
                        }
                    }
                }
                let body = rule.body.map_or(u32::MAX, |body| {
                    body_edges.push((idx(body), idx(head)));
                    idx(body)
                });
                rows[head].push((body, start, idx(others.len())));
            }
        }
        let mut offsets = Vec::with_capacity(atoms + 1);
        let mut entries = Vec::new();
        offsets.push(0);
        for row in rows {
            entries.extend(row);
            offsets.push(idx(entries.len()));
        }
        body_edges.sort_unstable();
        body_edges.dedup();
        rival_edges.sort_unstable();
        rival_edges.dedup();
        let (body_heads_offsets, body_heads) = csr(count, &body_edges);
        let (rival_offsets, rivals) = csr(atoms, &rival_edges);
        Self {
            offsets,
            entries,
            others,
            body_heads_offsets,
            body_heads,
            rival_offsets,
            rivals,
        }
    }
}

/// Each node's distinct parents, in node order, as offsets and a flat row,
/// counted from the operand rows `offsets`/`operands` of `count` nodes.
fn parents_of(count: usize, offsets: &[u32], operands: &[u32]) -> (Vec<u32>, Vec<u32>) {
    let mut parents_offsets = vec![0_u32; count + 1];
    for &child in operands {
        parents_offsets[child as usize + 1] += 1;
    }
    for node in 0..count {
        parents_offsets[node + 1] += parents_offsets[node];
    }
    let mut cursor = parents_offsets.clone();
    let mut parents = vec![0_u32; operands.len()];
    for node in 0..count {
        for &child in &operands[offsets[node] as usize..offsets[node + 1] as usize] {
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
    let mut compact_offsets = Vec::with_capacity(count + 1);
    compact_offsets.push(0_u32);
    for node in 0..count {
        let (start, end) = (parents_offsets[node] as usize, cursor[node] as usize);
        parents.copy_within(start..end, write);
        write += end - start;
        compact_offsets.push(idx(write));
    }
    parents.truncate(write);
    parents.shrink_to_fit();
    drop(cursor);
    (compact_offsets, parents)
}

impl Shape {
    fn read(theory: &Theory, producers: Option<&Producers>) -> Option<Self> {
        let view = theory.view();
        let count = view.len();
        let atoms = theory.atom_count();
        let mut kinds = Vec::with_capacity(count);
        let mut operands: Vec<u32> = Vec::new();
        let mut offsets = Vec::with_capacity(count + 1);
        let mut thresholds = Vec::new();
        let mut weights = Vec::new();
        let mut has_cuts = false;
        let mut atom_edges = Vec::new();
        let extend = |operands: &mut Vec<u32>, row: &[usize]| -> Option<()> {
            for &child in row {
                operands.push(u32::try_from(child).ok()?);
            }
            Some(())
        };
        for node in 0..count {
            offsets.push(u32::try_from(operands.len()).ok()?);
            let packed = match view.node(node).ok()? {
                NodeView::Atom(atom) => {
                    atom_edges.push((idx(atom), idx(node)));
                    pack_kind(Kind::Atom(0), atom)?
                }
                NodeView::False => pack_kind(Kind::False, 0)?,
                NodeView::Implies(left, right) => {
                    extend(&mut operands, &[left, right])?;
                    pack_kind(Kind::Implies, 0)?
                }
                NodeView::And(row) => {
                    extend(&mut operands, row)?;
                    pack_kind(Kind::And, 0)?
                }
                NodeView::Or(row) => {
                    extend(&mut operands, row)?;
                    pack_kind(Kind::Or, 0)?
                }
                NodeView::Threshold(threshold) => {
                    extend(&mut operands, threshold.conditions)?;
                    let index = thresholds.len();
                    thresholds.push((threshold.bound, u32::try_from(weights.len()).ok()?));
                    weights.extend_from_slice(threshold.weights);
                    pack_kind(Kind::Threshold, index)?
                }
                NodeView::ExtremumCut(cut) => {
                    has_cuts = true;
                    extend(&mut operands, cut.row.conditions)?;
                    pack_kind(Kind::Cut, 0)?
                }
            };
            kinds.push(packed);
        }
        offsets.push(u32::try_from(operands.len()).ok()?);
        let (parents_offsets, parents) = parents_of(count, &offsets, &operands);
        let (atom_nodes_offsets, atom_nodes) = csr(atoms, &atom_edges);
        drop(atom_edges);
        let support = producers.map(|producers| Support::read(producers, atoms, count));
        operands.shrink_to_fit();
        weights.shrink_to_fit();
        thresholds.shrink_to_fit();
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
            roots: theory.roots().to_vec(),
            support,
            has_cuts,
            amo_atoms: 0,
        };
        shape.amo_atoms = shape.count_amo_atoms();
        Some(shape)
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
    fn count_amo_atoms(&self) -> usize {
        let mut marked = vec![false; self.atoms];
        for &root in &self.roots {
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
                Kind::Threshold => {
                    self.bound(body) == 2 && self.row_weights(body).iter().all(|&w| w == 1)
                }
                _ => false,
            };
            if !unit {
                continue;
            }
            if children
                .iter()
                .all(|&c| matches!(self.kind(c as usize), Kind::Atom(_)))
            {
                for &c in children {
                    if let Kind::Atom(atom) = self.kind(c as usize) {
                        marked[atom as usize] = true;
                    }
                }
            }
        }
        marked.iter().filter(|&&m| m).count()
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

    /// Share the walk's completion slot, read and written by every walker.
    pub(crate) fn share_completions(&mut self, shared: std::sync::Arc<SharedCompletion>) {
        self.shared = Some(shared);
    }

    /// Take up a completion a peer published since this walker last looked:
    /// remember it and start a jump toward it.
    fn adopt(&mut self) {
        let Some(shared) = &self.shared else {
            return;
        };
        let generation = shared.generation.load(std::sync::atomic::Ordering::Acquire);
        if generation == self.adopted {
            return;
        }
        self.adopted = generation;
        let model = shared
            .model
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        if self.models.last() == Some(&model) {
            return;
        }
        if self.models.len() >= 8 {
            self.models.remove(0);
        }
        self.models.push(model);
        self.statistics.adopted += 1;
        self.jump = true;
        self.jump_left = 1 << 12;
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

    fn shape(&mut self, theory: &Theory, producers: Option<&Producers>) -> Option<&Shape> {
        let fresh = self
            .shape
            .as_ref()
            .is_none_or(|shape| !shape.theory.same_instance(theory));
        if fresh {
            self.shape = Shape::read(theory, producers);
            self.solver = None;
            self.models.clear();
            self.stamp.clear();
            self.constant.clear();
            self.eager_probed = false;
        }
        self.shape.as_ref()
    }

    /// Narrow `region` by the size-specialized implementation its open-atom
    /// count selects. The region must already be closed by the general
    /// narrowing of the same theory and producers.
    ///
    /// # Errors
    /// Cancellation observed during a learned search.
    pub(crate) fn narrow(
        &mut self,
        theory: &Theory,
        producers: Option<&Producers>,
        restrictions: &[&Theory],
        region: &Region,
        cancellation: &Cancellation,
    ) -> Result<SmallNarrowing, Stop> {
        let verdict = self.narrow_inner(theory, producers, restrictions, region, cancellation);
        if let Ok(SmallNarrowing::Refuted(_)) = verdict {
            self.observe(region, true);
        }
        verdict
    }

    fn narrow_inner(
        &mut self,
        theory: &Theory,
        producers: Option<&Producers>,
        restrictions: &[&Theory],
        region: &Region,
        cancellation: &Cancellation,
    ) -> Result<SmallNarrowing, Stop> {
        let policy = self.policy();
        if !policy.active() {
            return Ok(SmallNarrowing::Declined);
        }
        let open = region.open().count();
        if open == 0 {
            return Ok(SmallNarrowing::Declined);
        }
        if self.steering()
            && policy.refinements.has(Refinements::EAGER)
            && !self.eager_probed
            && self.probe_admits(theory, producers, &policy)
        {
            self.eager_probed = true;
            self.guard(restrictions);
            self.probe_root(policy.learn_conflicts, cancellation)?;
        }
        if open <= policy.enumerate_up_to {
            if self.shape(theory, producers).is_none() {
                return Ok(SmallNarrowing::Declined);
            }
            let started = Instant::now();
            let verdict = self.enumerate(region);
            self.statistics.enumerated_nanos += saturate(started.elapsed().as_nanos());
            if let Some(verdict) = verdict {
                return Ok(verdict);
            }
        }
        if policy.probe > 0 {
            if self.shape(theory, producers).is_none() {
                return Ok(SmallNarrowing::Declined);
            }
            self.guard(restrictions);
            if self.core_hit(region) {
                self.statistics.core_hits += 1;
                return Ok(SmallNarrowing::Refuted(Variant::Probed));
            }
            let started = Instant::now();
            let verdict = self.probe(region, policy.probe, cancellation)?;
            self.statistics.probed_nanos += saturate(started.elapsed().as_nanos());
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
            if !self.learn_admits(theory, producers, &policy) {
                return Ok(SmallNarrowing::Declined);
            }
            self.guard(restrictions);
            if self.cached(region) {
                self.statistics.cached += 1;
                self.record_yield(false);
                return Ok(SmallNarrowing::Open(Variant::Learned));
            }
            if self.core_hit(region) {
                self.statistics.core_hits += 1;
                self.record_yield(true);
                return Ok(SmallNarrowing::Refuted(Variant::Learned));
            }
            let started = Instant::now();
            let exhausted = self.statistics.learned_exhausted;
            let verdict = self.learn(region, policy.learn_conflicts, cancellation);
            if self.steering()
                && self.statistics.learned_exhausted > exhausted
                && u64::from(self.yield_score) * 100
                    < u64::from(YIELD_ONE) * policy.steer_yield as u64
                && !self.jump
                && !self.cached(region)
            {
                self.probe_root(policy.learn_conflicts, cancellation)?;
            }
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
    fn learn_admits(
        &mut self,
        theory: &Theory,
        producers: Option<&Producers>,
        policy: &SmallPolicy,
    ) -> bool {
        if policy.main_only
            && !*self
                .designated
                .get_or_insert_with(|| std::thread::current().name() == Some("main"))
        {
            return false;
        }
        self.offered += 1;
        if self.offered <= u64::try_from(policy.learn_after).unwrap_or(u64::MAX) {
            return false;
        }
        // The density reads only the theory's sizes: a dense theory is
        // declined before its shape is read.
        let nodes = theory.view().len();
        if policy.density > 0 && nodes > policy.density.saturating_mul(theory.atom_count().max(1)) {
            self.statistics.shape_declined += 1;
            return false;
        }
        if self.peers_idle(policy) {
            self.statistics.yielded += 1;
            return false;
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
                return false;
            }
        }
        let Some(shape) = self.shape(theory, producers) else {
            return false;
        };
        let (atoms, amo) = (shape.atoms, shape.amo_atoms);
        self.statistics.atoms = u64::try_from(atoms).unwrap_or(u64::MAX);
        self.statistics.amo_atoms = u64::try_from(amo).unwrap_or(u64::MAX);
        self.statistics.nodes = u64::try_from(nodes).unwrap_or(u64::MAX);
        if policy.matching > 0 && amo.saturating_mul(100) >= policy.matching.saturating_mul(atoms) {
            self.statistics.shape_declined += 1;
            return false;
        }
        if !self.gate_admits(policy.gate) {
            self.statistics.gated += 1;
            return false;
        }
        true
    }

    /// Whether the walker may run its one eager root probe now: the walker
    /// the learned search belongs to, outside dense and matching-shaped
    /// theories, past the encoding's amortization. Idle peers do not hold
    /// it back: the completion it finds is what they will walk toward.
    fn probe_admits(
        &mut self,
        theory: &Theory,
        producers: Option<&Producers>,
        policy: &SmallPolicy,
    ) -> bool {
        if policy.main_only
            && !*self
                .designated
                .get_or_insert_with(|| std::thread::current().name() == Some("main"))
        {
            return false;
        }
        let nodes = theory.view().len();
        if policy.density > 0 && nodes > policy.density.saturating_mul(theory.atom_count().max(1)) {
            return false;
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
                return false;
            }
        }
        let Some(shape) = self.shape(theory, producers) else {
            return false;
        };
        policy.matching == 0
            || shape.amo_atoms.saturating_mul(100) < policy.matching.saturating_mul(shape.atoms)
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
    pub(crate) fn note_refuted(&mut self, region: &Region) {
        self.observe(region, true);
        let policy = self.policy();
        if policy.adapt_from == 0 {
            return;
        }
        let open = region.open().count();
        if self.cutoff == 0 {
            self.cutoff = policy.adapt_from;
        }
        self.cutoff = self.cutoff.max(open + open / 20);
    }

    /// Encode the active restrictions (once each) and make their guards the
    /// first assumptions; a changed set forgets the cached completions.
    fn guard(&mut self, restrictions: &[&Theory]) {
        let Some(shape) = self.shape.as_ref() else {
            return;
        };
        if self.solver.is_none() {
            let solver = Solver::encode(shape, self.policy.unwrap_or_default());
            if reports() {
                eprintln!(
                    "zetesis-small-encoding vars={} clauses={} pbs={} pb_terms={} atoms={}",
                    solver.vars,
                    solver.clauses.len(),
                    solver.pbs.len(),
                    solver
                        .pbs
                        .iter()
                        .map(|pb| pb.conditions.len())
                        .sum::<usize>(),
                    shape.atoms
                );
            }
            self.solver = Some(solver);
            self.statistics.encodings += 1;
        }
        let solver = self.solver.as_mut().expect("encoded above");
        let guards: Vec<u32> = restrictions.iter().map(|t| solver.guard_of(t)).collect();
        if guards != self.guards {
            self.guards = guards;
            self.models.clear();
        }
    }

    fn core_hit(&mut self, region: &Region) -> bool {
        let (held, cut) = region.decision_words();
        let guards = &self.guards;
        let hit = self.cores.iter().position(|(requires, core)| {
            requires.iter().all(|g| guards.contains(g))
                && core.iter().all(|&(word, h, c)| {
                    let word = word as usize;
                    held[word] & h == h && cut[word] & c == c
                })
        });
        if let Some(index) = hit {
            let core = self.cores.remove(index);
            self.cores.insert(0, core);
            true
        } else {
            false
        }
    }

    fn remember_core(&mut self) {
        let Some(solver) = self.solver.as_ref() else {
            return;
        };
        let atoms = self.shape.as_ref().map_or(0, |shape| shape.atoms);
        let mut words: Vec<(u32, u64, u64)> = Vec::new();
        let mut requires = Vec::new();
        for &l in &solver.core {
            let atom = var(l) as usize;
            if atom >= atoms {
                requires.push(var(l));
                continue;
            }
            let (word, bit) = (idx(atom / 64), 1_u64 << (atom % 64));
            let at = if let Some(at) = words.iter().position(|(w, _, _)| *w == word) {
                at
            } else {
                words.push((word, 0, 0));
                words.len() - 1
            };
            let entry = &mut words[at];
            if l & 1 == 0 {
                entry.1 |= bit;
            } else {
                entry.2 |= bit;
            }
        }
        if words.is_empty() {
            return;
        }
        if self.cores.len() >= CORES {
            self.cores.pop();
        }
        self.cores.insert(0, (requires, words));
    }

    /// An open atom of `region` that the most recent remembered completion
    /// lying in it holds: splitting there, held child first (the walk's
    /// order), walks toward that completion. Only orders a split.
    pub(crate) fn guide(&mut self, region: &Region) -> Option<usize> {
        if !self.steering() {
            return None;
        }
        self.adopt();
        let model = self
            .models
            .iter()
            .rev()
            .find(|model| holds(region, model))?;
        let atom = region
            .open()
            .filter(|&atom| {
                model
                    .get(atom / 64)
                    .is_some_and(|w| w & (1 << (atom % 64)) != 0)
            })
            .last()?;
        self.following = Some(model.clone());
        Some(atom)
    }

    /// Whether completions may steer the walk: the policy asks for it and
    /// steering is not over (see `distrusted`).
    fn steering(&self) -> bool {
        !self.distrusted
            && self
                .policy
                .is_some_and(|p| p.refinements.has(Refinements::STEER))
    }

    /// Read the region narrowed after a guided split: refuted, or not
    /// holding the completion followed, the completion is no answer set
    /// (narrowing and refutation are sound for answer sets); holding it with
    /// nothing open, the guided walk arrived. Either way steering ends.
    fn observe(&mut self, region: &Region, refuted: bool) {
        let Some(model) = self.following.take() else {
            return;
        };
        if refuted || !holds(region, &model) {
            self.distrusted = true;
            self.jump = false;
            self.statistics.steer_failures += 1;
        } else if region.open().next().is_none() {
            // The walk reached the completion's leaf, which the walk checks
            // as an answer now. Steering exists to reach a first answer
            // sooner; the rest of the walk keeps its own order.
            self.distrusted = true;
            self.jump = false;
            self.statistics.steer_successes += 1;
        }
    }

    /// Search the conditions under the restriction guards alone, not the
    /// region's decisions, for a completion. Refutes nothing.
    fn probe_root(&mut self, allowance: u64, cancellation: &Cancellation) -> Result<(), Stop> {
        let Some(shape) = self.shape.as_ref() else {
            return Ok(());
        };
        let Some(solver) = self.solver.as_mut() else {
            return Ok(());
        };
        self.assumptions.clear();
        self.assumptions
            .extend(self.guards.iter().map(|&g| lit(g, false)));
        self.statistics.root_probes += 1;
        let before = solver.counts;
        let started = Instant::now();
        let outcome = solver.solve(&self.assumptions, allowance, cancellation, None)?;
        self.statistics.learned_nanos += saturate(started.elapsed().as_nanos());
        let after = solver.counts;
        self.statistics.conflicts += after.conflicts - before.conflicts;
        self.statistics.propagations += after.propagations - before.propagations;
        self.statistics.learned_clauses += after.learned - before.learned;
        if let Outcome::Satisfiable = outcome {
            let words = shape.atoms.div_ceil(64);
            let mut model = vec![0_u64; words];
            for atom in 0..shape.atoms {
                if solver.value(idx(atom)) == TRUE {
                    model[atom / 64] |= 1 << (atom % 64);
                }
            }
            if self.models.len() >= 8 {
                self.models.remove(0);
            }
            if let Some(shared) = &self.shared {
                shared.publish(&model);
                self.adopted = shared.generation.load(std::sync::atomic::Ordering::Acquire);
            }
            self.models.push(model);
            self.statistics.root_completions += 1;
            self.jump = true;
            self.jump_left = 1 << 12;
        }
        solver.cancel_until(0);
        Ok(())
    }

    /// Whether the walk should defer `region`: a jump is on and the
    /// completion it follows lies outside the region. Reaching a region
    /// that holds it, or spending the jump's deferrals, ends the jump.
    pub(crate) fn defers(&mut self, region: &Region) -> bool {
        self.observe(region, false);
        if self.steering() {
            self.adopt();
        }
        if !self.jump {
            return false;
        }
        let Some(model) = self.models.last() else {
            self.jump = false;
            return false;
        };
        if holds(region, model) || self.jump_left == 0 {
            self.jump = false;
            return false;
        }
        self.jump_left -= 1;
        self.statistics.deferrals += 1;
        true
    }

    fn cached(&self, region: &Region) -> bool {
        self.models.iter().any(|model| holds(region, model))
    }

    // ---------------------------------------------------------------- enumeration

    /// `None` declines (an ordinal cut above an open atom).
    fn enumerate(&mut self, region: &Region) -> Option<SmallNarrowing> {
        let shape = self.shape.as_ref()?;
        let nodes = shape.nodes();
        if self.stamp.len() != nodes {
            self.stamp = vec![0; nodes];
            self.constant = vec![2; nodes];
            self.epoch = 0;
        }
        self.epoch = self.epoch.wrapping_add(1);
        if self.epoch == 0 {
            self.stamp.fill(0);
            self.epoch = 1;
        }
        let open: Vec<usize> = region.open().collect();
        let k = open.len();
        collect_cone(shape, &open, &mut self.stamp, self.epoch, &mut self.cone);
        if shape.has_cuts && self.cone.iter().any(|&n| shape.kind(n) == Kind::Cut) {
            return None;
        }
        let words = if k >= 6 { 1_usize << (k - 6) } else { 1 };
        let valid_lanes = if k >= 6 {
            u64::MAX
        } else {
            (1_u64 << (1 << k)) - 1
        };
        self.lanes.clear();
        self.lanes.resize(self.cone.len(), 0);
        let reader = LaneReader {
            shape,
            region,
            position: self
                .cone
                .iter()
                .enumerate()
                .map(|(slot, &node)| (node, slot))
                .collect(),
            open_index: open.iter().enumerate().map(|(i, &a)| (a, i)).collect(),
        };
        let checked = checked_heads(shape, region, &open, &self.cone);
        self.statistics.enumerated += 1;
        self.statistics.lanes += (words as u64) * 64;
        let mut lanes = std::mem::take(&mut self.lanes);
        // Constant values of nodes outside the cone, memoized under the
        // region's decisions.
        let mut memo: std::collections::HashMap<usize, bool> = std::collections::HashMap::new();
        let mut passes = false;
        for word in 0..words {
            for (slot, &node) in self.cone.iter().enumerate() {
                lanes[slot] = reader.node(node, word, &lanes, &mut memo);
            }
            let mut alive = valid_lanes;
            for &root in &shape.roots {
                alive &= reader.operand(root, &lanes, &mut memo);
                if alive == 0 {
                    break;
                }
            }
            if alive != 0 {
                alive = reader.supported(&checked, alive, word, &lanes, &mut memo);
            }
            if alive != 0 {
                passes = true;
                break;
            }
        }
        self.lanes = lanes;
        if passes {
            Some(SmallNarrowing::Open(Variant::Enumerated))
        } else {
            self.statistics.enumerated_refuted += 1;
            Some(SmallNarrowing::Refuted(Variant::Enumerated))
        }
    }

    // ---------------------------------------------------------------- learning

    fn ensure_solver(&mut self) -> bool {
        let Some(shape) = self.shape.as_ref() else {
            return false;
        };
        if self.solver.is_none() {
            self.solver = Some(Solver::encode(shape, self.policy.unwrap_or_default()));
            self.statistics.encodings += 1;
        }
        true
    }

    /// Failed-literal probing of the region's open atoms, at most `budget`.
    fn probe(
        &mut self,
        region: &Region,
        budget: usize,
        cancellation: &Cancellation,
    ) -> Result<SmallNarrowing, Stop> {
        if !self.ensure_solver() {
            return Ok(SmallNarrowing::Declined);
        }
        let solver = self.solver.as_mut().expect("encoded above");
        self.assumptions.clear();
        self.assumptions
            .extend(self.guards.iter().map(|&g| lit(g, false)));
        for (atom, held) in region.decided() {
            self.assumptions.push(lit(idx(atom), !held));
        }
        let candidates: Vec<u32> = region.open().map(idx).collect();
        self.statistics.probed += 1;
        let before = solver.counts.propagations;
        let outcome = solver.probe(&self.assumptions, &candidates, budget, cancellation)?;
        self.statistics.propagations += solver.counts.propagations - before;
        Ok(match outcome {
            Outcome::Unsatisfiable(core) => {
                self.statistics.probed_refuted += 1;
                self.statistics.core_width += core as u64;
                self.remember_core();
                SmallNarrowing::Refuted(Variant::Probed)
            }
            _ => SmallNarrowing::Open(Variant::Probed),
        })
    }

    fn learn(
        &mut self,
        region: &Region,
        conflicts: u64,
        cancellation: &Cancellation,
    ) -> Result<SmallNarrowing, Stop> {
        let Some(shape) = self.shape.as_ref() else {
            return Ok(SmallNarrowing::Declined);
        };
        if self.solver.is_none() {
            let solver = Solver::encode(shape, self.policy.unwrap_or_default());
            if reports() {
                eprintln!(
                    "zetesis-small-encoding vars={} clauses={} pbs={} pb_terms={} atoms={}",
                    solver.vars,
                    solver.clauses.len(),
                    solver.pbs.len(),
                    solver
                        .pbs
                        .iter()
                        .map(|pb| pb.conditions.len())
                        .sum::<usize>(),
                    shape.atoms
                );
            }
            self.solver = Some(solver);
            self.statistics.encodings += 1;
        }
        let solver = self.solver.as_mut().expect("encoded above");
        self.assumptions.clear();
        self.assumptions
            .extend(self.guards.iter().map(|&g| lit(g, false)));
        for (atom, held) in region.decided() {
            self.assumptions.push(lit(idx(atom), !held));
        }
        self.statistics.learned += 1;
        let before = solver.counts;
        let idle = self
            .idle
            .as_deref()
            .filter(|_| self.policy.is_some_and(|p| p.yield_idle));
        let outcome = solver.solve(&self.assumptions, conflicts, cancellation, idle)?;
        let after = solver.counts;
        self.statistics.decisions += after.decisions - before.decisions;
        self.statistics.conflicts += after.conflicts - before.conflicts;
        self.statistics.propagations += after.propagations - before.propagations;
        self.statistics.learned_clauses += after.learned - before.learned;
        Ok(match outcome {
            Outcome::Unsatisfiable(core) => {
                self.statistics.learned_refuted += 1;
                self.statistics.core_width += core as u64;
                solver.keep_core();
                self.remember_core();
                SmallNarrowing::Refuted(Variant::Learned)
            }
            Outcome::Satisfiable => {
                let words = shape.atoms.div_ceil(64);
                let mut model = vec![0_u64; words];
                for atom in 0..shape.atoms {
                    if solver.value(idx(atom)) == TRUE {
                        model[atom / 64] |= 1 << (atom % 64);
                    }
                }
                if self.models.len() >= 8 {
                    self.models.remove(0);
                }
                self.models.push(model);
                SmallNarrowing::Open(Variant::Learned)
            }
            Outcome::Unknown => {
                self.statistics.learned_exhausted += 1;
                SmallNarrowing::Open(Variant::Learned)
            }
        })
    }
}

/// The cone of the open atoms: every node above one, in node order.
fn collect_cone(
    shape: &Shape,
    open: &[usize],
    stamp: &mut [u32],
    epoch: u32,
    cone: &mut Vec<usize>,
) {
    cone.clear();
    let mut stack: Vec<usize> = Vec::new();
    for &atom in open {
        let (start, end) = (
            shape.atom_nodes_offsets[atom] as usize,
            shape.atom_nodes_offsets[atom + 1] as usize,
        );
        stack.extend(
            shape.atom_nodes[start..end]
                .iter()
                .map(|&node| node as usize),
        );
    }
    while let Some(node) = stack.pop() {
        if stamp[node] == epoch {
            continue;
        }
        stamp[node] = epoch;
        cone.push(node);
        let (start, end) = (
            shape.parents_offsets[node] as usize,
            shape.parents_offsets[node + 1] as usize,
        );
        stack.extend(shape.parents[start..end].iter().map(|&p| p as usize));
    }
    cone.sort_unstable();
}

/// Support rows a completion can change: open atoms, heads whose body is in
/// the cone, and heads with an open rival; cut atoms need no support.
fn checked_heads(shape: &Shape, region: &Region, open: &[usize], cone: &[usize]) -> Vec<usize> {
    let Some(support) = &shape.support else {
        return Vec::new();
    };
    let mut checked: Vec<usize> = open.to_vec();
    for &node in cone {
        let (s, e) = (
            support.body_heads_offsets[node] as usize,
            support.body_heads_offsets[node + 1] as usize,
        );
        checked.extend(support.body_heads[s..e].iter().map(|&h| h as usize));
    }
    for &atom in open {
        let (s, e) = (
            support.rival_offsets[atom] as usize,
            support.rival_offsets[atom + 1] as usize,
        );
        checked.extend(support.rivals[s..e].iter().map(|&h| h as usize));
    }
    checked.sort_unstable();
    checked.dedup();
    checked.retain(|&atom| !region.is_cut(atom));
    checked
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
    fn atom(&self, atom: usize, word: usize) -> u64 {
        self.open_index.get(&atom).map_or_else(
            || {
                if self.region.is_held(atom) {
                    u64::MAX
                } else {
                    0
                }
            },
            |&j| lane_pattern(j, word),
        )
    }

    fn operand(
        &self,
        node: usize,
        lanes: &[u64],
        memo: &mut std::collections::HashMap<usize, bool>,
    ) -> u64 {
        match self.position.get(&node) {
            Some(&slot) => lanes[slot],
            None => {
                if evaluate_constant(self.shape, self.region, node, memo) {
                    u64::MAX
                } else {
                    0
                }
            }
        }
    }

    /// A cone node's lanes from its operands' (earlier in node order).
    fn node(
        &self,
        node: usize,
        word: usize,
        lanes: &[u64],
        memo: &mut std::collections::HashMap<usize, bool>,
    ) -> u64 {
        let shape = self.shape;
        match shape.kind(node) {
            Kind::Atom(atom) => self.atom(atom as usize, word),
            Kind::False => 0,
            Kind::Implies => {
                let row = shape.row(node);
                !self.operand(row[0] as usize, lanes, memo)
                    | self.operand(row[1] as usize, lanes, memo)
            }
            Kind::And => shape.row(node).iter().fold(u64::MAX, |acc, &c| {
                acc & self.operand(c as usize, lanes, memo)
            }),
            Kind::Or => shape
                .row(node)
                .iter()
                .fold(0, |acc, &c| acc | self.operand(c as usize, lanes, memo)),
            Kind::Threshold => {
                let values: Vec<u64> = shape
                    .row(node)
                    .iter()
                    .map(|&c| self.operand(c as usize, lanes, memo))
                    .collect();
                threshold_lanes(&values, shape.row_weights(node), shape.bound(node))
            }
            Kind::Cut => unreachable!("cones with cuts are declined"),
        }
    }

    /// The `alive` lanes in which every checked held atom has a producer
    /// whose body holds and whose rival heads fail.
    fn supported(
        &self,
        checked: &[usize],
        mut alive: u64,
        word: usize,
        lanes: &[u64],
        memo: &mut std::collections::HashMap<usize, bool>,
    ) -> u64 {
        let Some(support) = &self.shape.support else {
            return alive;
        };
        for &atom in checked {
            let held = self.atom(atom, word);
            if held & alive == 0 {
                continue;
            }
            let mut available = 0_u64;
            let (s, e) = (
                support.offsets[atom] as usize,
                support.offsets[atom + 1] as usize,
            );
            for &(body, start, end) in &support.entries[s..e] {
                let mut term = if body == u32::MAX {
                    u64::MAX
                } else {
                    self.operand(body as usize, lanes, memo)
                };
                for &other in &support.others[start as usize..end as usize] {
                    term &= !self.atom(other as usize, word);
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
        alive
    }
}

/// Lanes in which the weighted sum of the true operands reaches `bound`.
fn threshold_lanes(values: &[u64], weights: &[u64], bound: i128) -> u64 {
    let mut out = 0_u64;
    for lane in 0..64 {
        let sum: i128 = values
            .iter()
            .zip(weights)
            .filter(|(value, _)| *value >> lane & 1 == 1)
            .map(|(_, &weight)| i128::from(weight))
            .sum();
        if sum >= bound {
            out |= 1 << lane;
        }
    }
    out
}

/// The value of a node no open atom reaches, under the region's decisions.
fn evaluate_constant(
    shape: &Shape,
    region: &Region,
    root: usize,
    memo: &mut std::collections::HashMap<usize, bool>,
) -> bool {
    if let Some(&value) = memo.get(&root) {
        return value;
    }
    let values = memo;
    let mut stack = vec![(root, false)];
    while let Some((node, expanded)) = stack.pop() {
        if values.contains_key(&node) {
            continue;
        }
        let row = shape.row(node);
        if !expanded && !matches!(shape.kind(node), Kind::Atom(_) | Kind::False) {
            stack.push((node, true));
            for &child in row {
                let child = child as usize;
                if !values.contains_key(&child) {
                    stack.push((child, false));
                }
            }
            continue;
        }
        let get = |child: u32| values.get(&(child as usize)).copied().unwrap_or(false);
        let value = match shape.kind(node) {
            Kind::Atom(atom) => region.is_held(atom as usize),
            Kind::False => false,
            Kind::Implies => !get(row[0]) || get(row[1]),
            Kind::And => row.iter().all(|&c| get(c)),
            Kind::Or => row.iter().any(|&c| get(c)),
            Kind::Threshold => {
                let sum: i128 = row
                    .iter()
                    .zip(shape.row_weights(node))
                    .filter(|&(&c, _)| get(c))
                    .map(|(_, &w)| i128::from(w))
                    .sum();
                sum >= shape.bound(node)
            }
            // Not reached: a cut above no open atom is read as holding only
            // where enumeration declined; treat conservatively as true.
            Kind::Cut => true,
        };
        values.insert(node, value);
    }
    values[&root]
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

/// No literal assigned yet (compact encoding).
const NO_LIT: u32 = u32::MAX;

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
    /// The [`PbFamily`] counting it, or [`NO_FAMILY`] (it counts itself).
    family: u32,
}

/// Thresholds over the same weighted conditions, counted once.
#[derive(Debug)]
struct PbFamily {
    /// (bound, output, threshold index) of each member, by bound.
    members: Vec<(i128, u32, u32)>,
    sure: i128,
    open: i128,
    heaviest: i128,
}

/// A threshold that counts its own conditions.
const NO_FAMILY: u32 = u32::MAX;

/// Marks a [`PbFamily`] entry in a variable's threshold lists.
const FAMILY_ENTRY: u32 = 1 << 31;

/// The fewest thresholds over the same conditions counted as a family.
const FAMILY_MIN: usize = 4;

/// Booleans packed one per bit.
#[derive(Debug, Default, Clone)]
struct Bits {
    words: Vec<u64>,
    len: usize,
}

impl Bits {
    fn filled(len: usize, value: bool) -> Self {
        let mut bits = Self {
            words: vec![if value { u64::MAX } else { 0 }; len.div_ceil(64)],
            len,
        };
        bits.clear_tail();
        bits
    }

    fn with_capacity(capacity: usize) -> Self {
        Self {
            words: Vec::with_capacity(capacity.div_ceil(64)),
            len: 0,
        }
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

    fn push(&mut self, value: bool) {
        if self.len.is_multiple_of(64) {
            self.words.push(0);
        }
        self.len += 1;
        self.set(self.len - 1, value);
    }

    fn reserve(&mut self, additional: usize) {
        let words = (self.len + additional).div_ceil(64);
        self.words.reserve(words.saturating_sub(self.words.len()));
    }

    fn shrink_to_fit(&mut self) {
        self.words.shrink_to_fit();
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

    fn push(&mut self, clause: impl AsRef<[u32]>) -> u32 {
        let clause = clause.as_ref();
        let index = idx(self.spans.len());
        self.spans.push((idx(self.lits.len()), idx(clause.len())));
        self.lits.extend_from_slice(clause);
        index
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
    /// Each clause's literal block distance when learned (0 for originals).
    glue: Vec<u32>,
    /// Learned-clause reductions so far.
    reductions: u64,
    /// [`Refinements::LOCAL`]: the restriction literals set aside from each
    /// clause learned in the current search, by clause.
    aside: std::collections::HashMap<u32, Vec<u32>>,
    /// See [`SmallPolicy::local_keep`].
    local_keep: usize,
    watches: Vec<Vec<Watch>>,
    pbs: Vec<Pb>,
    families: Vec<PbFamily>,
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
    refinements: Refinements,
    /// Restarts so far, the Luby sequence's index.
    restarts: u64,
}

impl Solver {
    fn encode(shape: &Shape, policy: SmallPolicy) -> Self {
        let mut solver = Self::new(shape.atoms, policy);
        solver.add_theory(shape, None);
        solver.compact();
        solver
    }

    /// A solver over `vars` decidable variables and no constraints yet.
    fn new(vars: usize, policy: SmallPolicy) -> Self {
        let SmallPolicy {
            decide_all,
            refinements,
            local_keep,
            ..
        } = policy;
        let mut solver = Self {
            vars,
            clauses: Arena::default(),
            is_learned: Bits::default(),
            learned_count: 0,
            glue: Vec::new(),
            reductions: 0,
            aside: std::collections::HashMap::new(),
            local_keep,
            watches: vec![Vec::new(); vars * 2],
            pbs: Vec::new(),
            families: Vec::new(),
            pb_slot: vec![NO_PB; vars],
            pb_lists: Vec::new(),
            assigns: vec![UNDEF; vars],
            level: vec![0; vars],
            reason: vec![PackedReason::NONE; vars],
            trail: Vec::with_capacity(vars),
            trail_lim: Vec::new(),
            head: 0,
            activity: vec![0.0; vars],
            increment: 1.0,
            phase: Bits::filled(vars, false),
            heap: Vec::new(),
            heap_index: vec![-1; vars],
            trail_pos: vec![0; vars],
            seen: Bits::filled(vars, false),
            inconsistent: false,
            counts: Counts::default(),
            core: Vec::new(),
            guards: Vec::new(),
            decidable: Bits::filled(vars, true),
            decide_all,
            refinements,
            restarts: 0,
        };
        for v in 0..idx(vars) {
            solver.heap_insert(v);
        }
        solver
    }

    /// Room for `additional` more variables, so that encoding a theory's
    /// nodes does not grow every per-variable array by doubling.
    fn reserve_vars(&mut self, additional: usize) {
        self.decidable.reserve(additional);
        self.watches.reserve_exact(2 * additional);
        self.pb_slot.reserve_exact(additional);
        self.assigns.reserve_exact(additional);
        self.level.reserve_exact(additional);
        self.reason.reserve_exact(additional);
        self.activity.reserve_exact(additional);
        self.phase.reserve(additional);
        self.heap.reserve_exact(additional);
        self.heap_index.reserve_exact(additional);
        self.trail.reserve_exact(additional);
        self.trail_pos.reserve_exact(additional);
        self.seen.reserve(additional);
    }

    /// Release the slack left by building the encoding: every watch list
    /// and threshold list to its length.
    fn compact(&mut self) {
        for watchers in &mut self.watches {
            watchers.shrink_to_fit();
        }
        for lists in &mut self.pb_lists {
            lists.occurs.shrink_to_fit();
            lists.terms.shrink_to_fit();
        }
        self.pb_lists.shrink_to_fit();
        self.clauses.lits.shrink_to_fit();
        self.clauses.spans.shrink_to_fit();
        self.is_learned.shrink_to_fit();
    }

    /// The thresholds of variable `v`, created on first use.
    fn pb_lists_mut(&mut self, v: usize) -> &mut PbLists {
        if self.pb_slot[v] == NO_PB {
            self.pb_slot[v] = idx(self.pb_lists.len());
            self.pb_lists.push(PbLists::default());
        }
        &mut self.pb_lists[self.pb_slot[v] as usize]
    }

    /// A fresh variable, unassigned, in the decision heap.
    fn fresh(&mut self, decidable: bool) -> u32 {
        let v = idx(self.vars);
        self.decidable.push(decidable || self.decide_all);
        self.vars += 1;
        self.watches.push(Vec::new());
        self.watches.push(Vec::new());
        self.pb_slot.push(NO_PB);
        self.assigns.push(UNDEF);
        self.level.push(0);
        self.reason.push(PackedReason::NONE);
        self.activity.push(0.0);
        self.phase.push(false);
        self.heap_index.push(-1);
        self.trail_pos.push(0);
        self.seen.push(false);
        self.heap_insert(v);
        v
    }

    /// The guard variable of a restriction theory, encoding it on first use:
    /// its node definitions are unguarded (they only name nodes), its roots
    /// hold when the guard does.
    fn guard_of(&mut self, theory: &Theory) -> u32 {
        if let Some(&(_, guard)) = self.guards.iter().find(|(t, _)| t.same_instance(theory)) {
            return guard;
        }
        self.cancel_until(0);
        let guard = self.fresh(true);
        if let Some(shape) = Shape::read(theory, None) {
            self.add_theory(&shape, Some(guard));
        }
        self.guards.push((theory.clone(), guard));
        guard
    }

    /// Add one theory's definitions, roots (guarded by `guard` when given) and,
    /// unguarded original theories only, its support condition. Level 0.
    fn add_theory(&mut self, shape: &Shape, guard: Option<u32>) {
        self.cancel_until(0);
        let nodes = shape.nodes();
        self.reserve_vars(nodes);
        let mut clauses = Arena::default();
        let mut pbs = Vec::new();
        if self.refinements.has(Refinements::COMPACT) {
            self.compact_theory(shape, guard, &mut clauses, &mut pbs);
        } else {
            self.full_theory(shape, guard, &mut clauses, &mut pbs);
        }
        self.install(&clauses, pbs);
    }

    /// Install `clauses` and thresholds `pbs` at level 0 and propagate.
    fn install(&mut self, clauses: &Arena, mut pbs: Vec<Pb>) {
        let first_pb = self.pbs.len();
        let families = if self.refinements.has(Refinements::FAMILY) {
            self.add_families(first_pb, &mut pbs)
        } else {
            Vec::new()
        };
        for (offset, pb) in pbs.into_iter().enumerate() {
            self.add_pb(idx(first_pb + offset), pb);
        }
        for f in families {
            self.install_family(f);
        }
        let mut clause = Vec::new();
        for index in 0..clauses.len() {
            clause.clear();
            clause.extend_from_slice(clauses.get(index));
            clause.sort_unstable();
            clause.dedup();
            if clause.windows(2).any(|w| w[0] == neg(w[1])) {
                continue; // tautology
            }
            self.add_original(&clause);
        }
        if self.inconsistent {
            return;
        }
        // Thresholds with nothing assigned still need an initial check.
        for index in first_pb..self.pbs.len() {
            if self.pb_check(idx(index)).is_some() {
                self.inconsistent = true;
                return;
            }
        }
        self.head = 0;
        if self.propagate().is_some() {
            self.inconsistent = true;
        }
    }

    /// One variable per node, defined by its operands both ways.
    fn full_theory(
        &mut self,
        shape: &Shape,
        guard: Option<u32>,
        clauses: &mut Arena,
        pbs: &mut Vec<Pb>,
    ) {
        let nodes = shape.nodes();
        // Atom nodes alias their atom; other nodes get their own variable.
        let node_var: Vec<u32> = (0..nodes)
            .map(|node| match shape.kind(node) {
                Kind::Atom(atom) => atom,
                Kind::Cut => self.fresh(true),
                _ => self.fresh(false),
            })
            .collect();
        for node in 0..nodes {
            let v = node_var[node];
            let pos = lit(v, false);
            let row: Vec<u32> = shape
                .row(node)
                .iter()
                .map(|&c| node_var[c as usize])
                .collect();
            match shape.kind(node) {
                Kind::Atom(_) | Kind::Cut => {}
                Kind::False => {
                    clauses.push([neg(pos)]);
                }
                Kind::Implies => {
                    let (x, y) = (lit(row[0], false), lit(row[1], false));
                    clauses.push(vec![neg(pos), neg(x), y]);
                    clauses.push(vec![pos, x]);
                    clauses.push(vec![pos, neg(y)]);
                }
                Kind::And => {
                    let mut long = vec![pos];
                    for &c in &row {
                        clauses.push(vec![neg(pos), lit(c, false)]);
                        long.push(lit(c, true));
                    }
                    clauses.push(long);
                }
                Kind::Or => {
                    let mut long = vec![neg(pos)];
                    for &c in &row {
                        clauses.push(vec![pos, lit(c, true)]);
                        long.push(lit(c, false));
                    }
                    clauses.push(long);
                }
                Kind::Threshold => pbs.push(Pb {
                    out: v,
                    conditions: row.clone(),
                    weights: shape.row_weights(node).to_vec(),
                    bound: shape.bound(node),
                    sure: 0,
                    open: 0,
                    heaviest: 0,
                    family: NO_FAMILY,
                }),
            }
        }
        for &root in &shape.roots {
            let mut clause = vec![lit(node_var[root], false)];
            if let Some(guard) = guard {
                clause.push(lit(guard, true));
            }
            clauses.push(clause);
        }
        if let (Some(support), None) = (&shape.support, guard) {
            let lits: Vec<u32> = node_var.iter().map(|&v| lit(v, false)).collect();
            self.support_clauses(support, shape.atoms, &lits, clauses);
        }
    }

    /// [`Refinements::COMPACT`]: literals for the nodes something reads,
    /// negations folded and root implications written as single clauses.
    fn compact_theory(
        &mut self,
        shape: &Shape,
        guard: Option<u32>,
        clauses: &mut Arena,
        pbs: &mut Vec<Pb>,
    ) {
        let nodes = shape.nodes();
        let mut lits = vec![NO_LIT; nodes];
        let mut is_body = vec![false; nodes];
        if let (Some(support), None) = (&shape.support, guard) {
            for &(body, _, _) in &support.entries {
                if body != u32::MAX {
                    is_body[body as usize] = true;
                }
            }
        }
        // A premise conjunction or conclusion disjunction read only by its
        // root implication is read through.
        let inline = |node: usize, kind: Kind| {
            shape.kind(node) == kind
                && !is_body[node]
                && shape.parents_offsets[node + 1] - shape.parents_offsets[node] == 1
        };
        let mut negated: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();
        for &root in &shape.roots {
            let mut clause = Vec::new();
            if shape.kind(root) == Kind::Implies {
                let row = shape.row(root);
                let (x, y) = (row[0] as usize, row[1] as usize);
                if inline(x, Kind::And) {
                    for &c in shape.row(x) {
                        clause.push(neg(self.node_lit(
                            shape,
                            c as usize,
                            &mut lits,
                            &mut negated,
                            clauses,
                            pbs,
                        )));
                    }
                } else {
                    clause.push(neg(self.node_lit(
                        shape,
                        x,
                        &mut lits,
                        &mut negated,
                        clauses,
                        pbs,
                    )));
                }
                if shape.kind(y) == Kind::False {
                } else if inline(y, Kind::Or) {
                    for &c in shape.row(y) {
                        clause.push(self.node_lit(
                            shape,
                            c as usize,
                            &mut lits,
                            &mut negated,
                            clauses,
                            pbs,
                        ));
                    }
                } else {
                    clause.push(self.node_lit(shape, y, &mut lits, &mut negated, clauses, pbs));
                }
            } else {
                clause.push(self.node_lit(shape, root, &mut lits, &mut negated, clauses, pbs));
            }
            if let Some(guard) = guard {
                clause.push(lit(guard, true));
            }
            clauses.push(clause);
        }
        if let (Some(support), None) = (&shape.support, guard) {
            for &(body, _, _) in &support.entries {
                if body != u32::MAX {
                    self.node_lit(shape, body as usize, &mut lits, &mut negated, clauses, pbs);
                }
            }
            self.support_clauses(support, shape.atoms, &lits, clauses);
        }
    }

    /// Node `node`'s literal under [`Refinements::COMPACT`], defining it and
    /// the operands it reads on first use (an explicit stack, not recursion).
    fn node_lit(
        &mut self,
        shape: &Shape,
        node: usize,
        lits: &mut [u32],
        negated: &mut std::collections::HashMap<u32, u32>,
        clauses: &mut Arena,
        pbs: &mut Vec<Pb>,
    ) -> u32 {
        let mut stack = vec![(node, false)];
        while let Some((n, ready)) = stack.pop() {
            if lits[n] != NO_LIT {
                continue;
            }
            let row = shape.row(n);
            let negation =
                shape.kind(n) == Kind::Implies && shape.kind(row[1] as usize) == Kind::False;
            if !ready {
                let operands: &[u32] = match shape.kind(n) {
                    Kind::Atom(_) | Kind::Cut | Kind::False => &[],
                    _ if negation => &row[..1],
                    _ => row,
                };
                if operands.iter().any(|&c| lits[c as usize] == NO_LIT) {
                    stack.push((n, true));
                    stack.extend(operands.iter().map(|&c| (c as usize, false)));
                    continue;
                }
            }
            lits[n] = match shape.kind(n) {
                Kind::Atom(atom) => lit(atom, false),
                Kind::Cut => lit(self.fresh(true), false),
                Kind::False => {
                    let pos = lit(self.fresh(false), false);
                    clauses.push([neg(pos)]);
                    pos
                }
                Kind::Implies if negation => neg(lits[row[0] as usize]),
                Kind::Implies => {
                    let pos = lit(self.fresh(false), false);
                    let (x, y) = (lits[row[0] as usize], lits[row[1] as usize]);
                    clauses.push(vec![neg(pos), neg(x), y]);
                    clauses.push(vec![pos, x]);
                    clauses.push(vec![pos, neg(y)]);
                    pos
                }
                Kind::And => {
                    let pos = lit(self.fresh(false), false);
                    let mut long = vec![pos];
                    for &c in row {
                        let c = lits[c as usize];
                        clauses.push(vec![neg(pos), c]);
                        long.push(neg(c));
                    }
                    clauses.push(long);
                    pos
                }
                Kind::Or => {
                    let pos = lit(self.fresh(false), false);
                    let mut long = vec![neg(pos)];
                    for &c in row {
                        let c = lits[c as usize];
                        clauses.push(vec![pos, neg(c)]);
                        long.push(c);
                    }
                    clauses.push(long);
                    pos
                }
                Kind::Threshold => {
                    // Threshold conditions are variables: a negated operand
                    // gets one equal to it.
                    let conditions: Vec<u32> = row
                        .iter()
                        .map(|&c| {
                            let l = lits[c as usize];
                            if l & 1 == 0 {
                                return l >> 1;
                            }
                            *negated.entry(l).or_insert_with(|| {
                                let e = self.fresh(false);
                                clauses.push(vec![lit(e, true), l]);
                                clauses.push(vec![lit(e, false), neg(l)]);
                                e
                            })
                        })
                        .collect();
                    let v = self.fresh(false);
                    pbs.push(Pb {
                        out: v,
                        conditions,
                        weights: shape.row_weights(n).to_vec(),
                        bound: shape.bound(n),
                        sure: 0,
                        open: 0,
                        heaviest: 0,
                        family: NO_FAMILY,
                    });
                    lit(v, false)
                }
            };
        }
        lits[node]
    }

    /// The support condition of every atom: held only with a producer whose
    /// body holds and whose other ordinary heads fail.
    fn support_clauses(
        &mut self,
        support: &Support,
        atoms: usize,
        lits: &[u32],
        clauses: &mut Arena,
    ) {
        for atom in 0..atoms {
            let (s, e) = (
                support.offsets[atom] as usize,
                support.offsets[atom + 1] as usize,
            );
            let mut clause = vec![lit(idx(atom), true)];
            let mut always = false;
            for &(body, start, end) in &support.entries[s..e] {
                let others = &support.others[start as usize..end as usize];
                if others.is_empty() {
                    if body == u32::MAX {
                        always = true;
                        break;
                    }
                    clause.push(lits[body as usize]);
                } else {
                    // s <-> body & !others
                    let sp = lit(self.fresh(false), false);
                    let mut long = vec![sp];
                    if body != u32::MAX {
                        let b = lits[body as usize];
                        clauses.push(vec![neg(sp), b]);
                        long.push(neg(b));
                    }
                    for &other in others {
                        clauses.push(vec![neg(sp), lit(other, true)]);
                        long.push(lit(other, false));
                    }
                    clauses.push(long);
                    clause.push(sp);
                }
            }
            if !always {
                clauses.push(clause);
            }
        }
    }

    /// Install threshold `index`, counting what is already assigned.
    fn add_pb(&mut self, index: u32, mut pb: Pb) {
        self.pb_lists_mut(pb.out as usize).occurs.push(index);
        if pb.family != NO_FAMILY {
            // Its family counts and watches the conditions.
            self.pbs.push(pb);
            return;
        }
        let mut terms: Vec<(u32, u64)> = pb
            .conditions
            .iter()
            .copied()
            .zip(pb.weights.iter().copied())
            .collect();
        terms.sort_unstable();
        let mut merged: Vec<(u32, u64)> = Vec::with_capacity(terms.len());
        for (c, w) in terms {
            match merged.last_mut() {
                Some(last) if last.0 == c => last.1 += w,
                _ => merged.push((c, w)),
            }
        }
        pb.sure = 0;
        pb.open = 0;
        pb.heaviest = 0;
        for &(c, weight) in &merged {
            let w = i128::from(weight);
            pb.heaviest = pb.heaviest.max(w);
            match self.assigns[c as usize] {
                TRUE => pb.sure += w,
                UNDEF => pb.open += w,
                _ => {}
            }
            self.pb_lists_mut(c as usize).terms.push((index, weight));
        }
        let mut seen_conditions: Vec<u32> = pb.conditions.clone();
        seen_conditions.sort_unstable();
        seen_conditions.dedup();
        for &c in &seen_conditions {
            self.pb_lists_mut(c as usize).occurs.push(index);
        }
        self.pbs.push(pb);
    }

    /// [`Refinements::FAMILY`]: group `pbs` (thresholds `first_pb..`) by
    /// their weighted conditions, marking the members of each group of at
    /// least [`FAMILY_MIN`]; the families made, to install once the members
    /// are.
    fn add_families(&mut self, first_pb: usize, pbs: &mut [Pb]) -> Vec<u32> {
        let mut groups: std::collections::HashMap<(&[u32], &[u64]), Vec<usize>> =
            std::collections::HashMap::new();
        for (k, pb) in pbs.iter().enumerate() {
            groups
                .entry((pb.conditions.as_slice(), pb.weights.as_slice()))
                .or_default()
                .push(k);
        }
        let mut made: Vec<Vec<usize>> = groups
            .into_values()
            .filter(|members| members.len() >= FAMILY_MIN)
            .collect();
        made.sort_unstable();
        let mut families = Vec::new();
        for members in made {
            let f = idx(self.families.len());
            let mut listed: Vec<(i128, u32, u32)> = members
                .iter()
                .map(|&k| (pbs[k].bound, pbs[k].out, idx(first_pb + k)))
                .collect();
            listed.sort_unstable();
            for &k in &members {
                pbs[k].family = f;
            }
            self.families.push(PbFamily {
                members: listed,
                sure: 0,
                open: 0,
                heaviest: 0,
            });
            families.push(f);
        }
        families
    }

    /// Count family `f`'s conditions (those of its first member) and list
    /// it with them.
    fn install_family(&mut self, f: u32) {
        let first = self.families[f as usize].members[0].2 as usize;
        let mut terms: Vec<(u32, u64)> = self.pbs[first]
            .conditions
            .iter()
            .copied()
            .zip(self.pbs[first].weights.iter().copied())
            .collect();
        terms.sort_unstable();
        let mut merged: Vec<(u32, u64)> = Vec::with_capacity(terms.len());
        for (c, w) in terms {
            match merged.last_mut() {
                Some(last) if last.0 == c => last.1 += w,
                _ => merged.push((c, w)),
            }
        }
        let (mut sure, mut open, mut heaviest) = (0, 0, 0);
        let entry = FAMILY_ENTRY | f;
        for &(c, weight) in &merged {
            let w = i128::from(weight);
            heaviest = heaviest.max(w);
            match self.assigns[c as usize] {
                TRUE => sure += w,
                UNDEF => open += w,
                _ => {}
            }
            let lists = self.pb_lists_mut(c as usize);
            lists.terms.push((entry, weight));
            lists.occurs.push(entry);
        }
        let family = &mut self.families[f as usize];
        family.sure = sure;
        family.open = open;
        family.heaviest = heaviest;
    }

    /// Threshold `index`'s true weight, open weight and heaviest weight.
    fn counters(&self, index: u32) -> (i128, i128, i128) {
        let pb = &self.pbs[index as usize];
        if pb.family == NO_FAMILY {
            (pb.sure, pb.open, pb.heaviest)
        } else {
            let family = &self.families[pb.family as usize];
            (family.sure, family.open, family.heaviest)
        }
    }

    /// Check the members of family `f` that its counts can make act.
    fn family_check(&mut self, f: u32) -> Option<Vec<u32>> {
        let count = self.families[f as usize].members.len();
        for k in 0..count {
            let family = &self.families[f as usize];
            let (sure, open, heaviest) = (family.sure, family.open, family.heaviest);
            let (bound, out, index) = family.members[k];
            let quiet = match self.assigns[out as usize] {
                TRUE => sure >= bound || sure + open - heaviest >= bound,
                FALSE => sure + open < bound || sure + heaviest < bound,
                _ => sure < bound && bound <= sure + open,
            };
            if !quiet && let Some(conflict) = self.pb_check(index) {
                return Some(conflict);
            }
        }
        None
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

    fn add_original(&mut self, clause: &[u32]) {
        if self.inconsistent {
            return;
        }
        match clause.len() {
            0 => self.inconsistent = true,
            1 => match self.lit_value(clause[0]) {
                FALSE => self.inconsistent = true,
                UNDEF => self.assign(clause[0], Reason::None),
                _ => {}
            },
            _ => {
                self.attach(clause, false);
            }
        }
    }

    /// Store a clause of at least two literals and watch its first two; a
    /// two-literal clause lives in the watches alone. The reason that
    /// asserts its first literal is returned.
    fn attach(&mut self, clause: &[u32], learned: bool) -> Reason {
        if clause.len() == 2 {
            self.watches[neg(clause[0]) as usize].push(watch(BINARY, clause[1]));
            self.watches[neg(clause[1]) as usize].push(watch(BINARY, clause[0]));
            return Reason::Binary(clause[1]);
        }
        let index = self.clauses.push(clause);
        self.watches[neg(clause[0]) as usize].push(watch(index, clause[1]));
        self.watches[neg(clause[1]) as usize].push(watch(index, clause[0]));
        self.is_learned.push(learned);
        let glue = if learned {
            self.learned_count += 1;
            let mut levels: Vec<u32> = clause
                .iter()
                .map(|&l| self.level[var(l) as usize])
                .collect();
            levels.sort_unstable();
            levels.dedup();
            idx(levels.len())
        } else {
            0
        };
        self.glue.push(glue);
        Reason::Clause(index)
    }

    /// Record a learned clause after the backjump and assert its first
    /// literal.
    fn record(&mut self, learned: &[u32]) {
        let local = self.refinements.has(Refinements::LOCAL)
            && learned[1..]
                .iter()
                .any(|&l| self.level[var(l) as usize] == 1);
        if local {
            let mut short = vec![learned[0]];
            let mut aside = Vec::new();
            for &l in &learned[1..] {
                if self.level[var(l) as usize] > 1 {
                    short.push(l);
                } else {
                    aside.push(l);
                }
            }
            let reason = self.attach_local(&short, aside);
            self.counts.learned += 1;
            self.assign(short[0], reason);
        } else if learned.len() == 1 {
            self.assign(learned[0], Reason::None);
        } else {
            let reason = self.attach(learned, true);
            self.counts.learned += 1;
            self.assign(learned[0], reason);
        }
    }

    /// [`Refinements::LOCAL`]: attach `short` (asserting literal first, then
    /// the deepest), always as an indexed clause so that the restriction
    /// literals `aside` stay readable as part of its reason.
    fn attach_local(&mut self, short: &[u32], aside: Vec<u32>) -> Reason {
        let index = self.clauses.push(short);
        if short.len() > 1 {
            self.watches[neg(short[0]) as usize].push(watch(index, short[1]));
            self.watches[neg(short[1]) as usize].push(watch(index, short[0]));
        }
        self.is_learned.push(true);
        self.learned_count += 1;
        let mut levels: Vec<u32> = short.iter().map(|&l| self.level[var(l) as usize]).collect();
        levels.sort_unstable();
        levels.dedup();
        self.glue.push(idx(levels.len() + 1));
        self.aside.insert(index, aside);
        Reason::Clause(index)
    }

    /// [`Refinements::LOCAL`]: after a refutation, keep its core as a clause
    /// (the restriction literals it used cannot all hold), so what the
    /// search learned about the restriction outlives the clauses it set
    /// aside.
    fn keep_core(&mut self) {
        if !self.refinements.has(Refinements::LOCAL) || self.core.is_empty() {
            return;
        }
        self.cancel_until(0);
        let mut clause = Vec::with_capacity(self.core.len());
        for &l in &self.core {
            match self.lit_value(neg(l)) {
                TRUE => return,
                FALSE => {}
                _ => clause.push(neg(l)),
            }
        }
        clause.sort_unstable();
        clause.dedup();
        match clause.len() {
            0 => self.inconsistent = true,
            1 => {
                self.assign(clause[0], Reason::None);
                if self.propagate().is_some() {
                    self.inconsistent = true;
                }
            }
            width => {
                self.attach(&clause, true);
                // Levels are stale at level 0: rate it by its width.
                if width > 2
                    && let Some(glue) = self.glue.last_mut()
                {
                    *glue = idx(width);
                }
            }
        }
    }

    /// [`Refinements::LOCAL`], at level 0 once a search is over: put each
    /// clause learned without its restriction literals back whole, or
    /// forget it past `local_keep` literals.
    fn restore_locals(&mut self) {
        let aside = std::mem::take(&mut self.aside);
        let mut whole: Vec<Vec<u32>> = Vec::new();
        let mut keep = vec![true; self.clauses.len()];
        for (&ci, set_aside) in &aside {
            keep[ci as usize] = false;
            let mut clause: Vec<u32> = Vec::new();
            let mut satisfied = false;
            for &l in self.clauses.get(ci as usize).iter().chain(set_aside) {
                match self.lit_value(l) {
                    TRUE => satisfied = true,
                    FALSE => {} // false at level 0 for good
                    _ => clause.push(l),
                }
            }
            if !satisfied && clause.len() >= 2 && clause.len() <= self.local_keep {
                whole.push(clause);
            }
        }
        let dropped = aside.len();
        self.compact_clauses(&keep, dropped);
        for clause in whole {
            self.attach(&clause, true);
        }
    }

    fn assign(&mut self, l: u32, reason: Reason) {
        let v = var(l) as usize;
        self.assigns[v] = TRUE ^ u8::from(l & 1 == 1);
        self.level[v] = idx(self.trail_lim.len());
        self.reason[v] = PackedReason::pack(reason);
        self.trail_pos[v] = idx(self.trail.len());
        self.trail.push(l);
        let positive = l & 1 == 0;
        let slot = self.pb_slot[v];
        let terms: &[(u32, u64)] = if slot == NO_PB {
            &[]
        } else {
            &self.pb_lists[slot as usize].terms
        };
        for &(index, w) in terms {
            let (sure, open) = if index & FAMILY_ENTRY == 0 {
                let pb = &mut self.pbs[index as usize];
                (&mut pb.sure, &mut pb.open)
            } else {
                let family = &mut self.families[(index & !FAMILY_ENTRY) as usize];
                (&mut family.sure, &mut family.open)
            };
            *open -= i128::from(w);
            if positive {
                *sure += i128::from(w);
            }
        }
    }

    fn decision_level(&self) -> usize {
        self.trail_lim.len()
    }

    /// Unit propagation over clauses and thresholds; a conflict returns its
    /// clause (as literals, all false).
    fn propagate(&mut self) -> Option<Vec<u32>> {
        while self.head < self.trail.len() {
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
                        self.assign(blocker, Reason::Binary(false_lit));
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
                    let l = clause[k];
                    let a = self.assigns[var(l) as usize];
                    if a == UNDEF || (a ^ u8::from(l & 1 == 1)) == TRUE {
                        clause.swap(1, k);
                        let other = clause[1];
                        self.watches[neg(other) as usize].push(watch(idx(ci), first));
                        watchers.swap_remove(i);
                        moved = true;
                        break;
                    }
                }
                if moved {
                    continue;
                }
                if a == UNDEF {
                    self.assign(first, Reason::Clause(idx(ci)));
                    i += 1;
                } else {
                    conflict = Some(self.clause_lits(idx(ci)));
                    break;
                }
            }
            let existing = std::mem::take(&mut self.watches[p as usize]);
            watchers.extend(existing);
            self.watches[p as usize] = watchers;
            if conflict.is_some() {
                self.head = self.trail.len();
                return conflict;
            }
            let slot = self.pb_slot[var(p) as usize];
            let occurs = if slot == NO_PB {
                Vec::new()
            } else {
                std::mem::take(&mut self.pb_lists[slot as usize].occurs)
            };
            let mut pb_conflict = None;
            for &index in &occurs {
                let checked = if index & FAMILY_ENTRY == 0 {
                    self.pb_check(index)
                } else {
                    self.family_check(index & !FAMILY_ENTRY)
                };
                if let Some(clause) = checked {
                    pb_conflict = Some(clause);
                    break;
                }
            }
            if slot != NO_PB {
                self.pb_lists[slot as usize].occurs = occurs;
            }
            if pb_conflict.is_some() {
                self.head = self.trail.len();
                return pb_conflict;
            }
        }
        None
    }

    /// Propagate one threshold: assign what it forces, with explanations, or
    /// return the conflict clause.
    fn pb_check(&mut self, index: u32) -> Option<Vec<u32>> {
        let (sure, open, heaviest) = self.counters(index);
        let pb = &self.pbs[index as usize];
        let out = self.assigns[pb.out as usize];
        let bound = pb.bound;
        if sure < bound && bound <= sure + open {
            let quiet = match out {
                TRUE => sure + open - heaviest >= bound,
                FALSE => sure + heaviest < bound,
                _ => true,
            };
            if quiet {
                return None;
            }
        }
        let true_conditions = |solver: &Self| -> Vec<u32> {
            let pb = &solver.pbs[index as usize];
            pb.conditions
                .iter()
                .zip(&pb.weights)
                .filter(|&(&c, &w)| w > 0 && solver.assigns[c as usize] == TRUE)
                .map(|(&c, _)| lit(c, true))
                .collect()
        };
        let false_conditions = |solver: &Self| -> Vec<u32> {
            let pb = &solver.pbs[index as usize];
            pb.conditions
                .iter()
                .zip(&pb.weights)
                .filter(|&(&c, &w)| w > 0 && solver.assigns[c as usize] == FALSE)
                .map(|(&c, _)| lit(c, false))
                .collect()
        };
        let out_lit = lit(pb.out, false);
        if sure >= bound {
            // out must hold: (out | !true conditions)
            match out {
                TRUE => return None,
                UNDEF => {
                    self.assign(out_lit, Reason::Pb(index));
                    return None;
                }
                _ => {
                    let mut e = vec![out_lit];
                    e.extend(true_conditions(self));
                    return Some(dedup(e));
                }
            }
        }
        if sure + open < bound {
            match out {
                FALSE => return None,
                UNDEF => {
                    self.assign(neg(out_lit), Reason::Pb(index));
                    return None;
                }
                _ => {
                    let mut e = vec![neg(out_lit)];
                    e.extend(false_conditions(self));
                    return Some(dedup(e));
                }
            }
        }
        // sure < bound <= sure + open: out decides conditions.
        let pb = &self.pbs[index as usize];
        let mut forced: Vec<(u32, bool)> = Vec::new();
        if out == TRUE {
            for (&c, &w) in pb.conditions.iter().zip(&pb.weights) {
                if w > 0 && self.assigns[c as usize] == UNDEF && sure + open - i128::from(w) < bound
                {
                    forced.push((c, true));
                }
            }
        } else if out == FALSE {
            for (&c, &w) in pb.conditions.iter().zip(&pb.weights) {
                if w > 0 && self.assigns[c as usize] == UNDEF && sure + i128::from(w) >= bound {
                    forced.push((c, false));
                }
            }
        }
        if forced.is_empty() {
            return None;
        }
        for (c, value) in forced {
            if self.assigns[c as usize] != UNDEF {
                continue; // duplicate position already forced
            }
            self.assign(lit(c, !value), Reason::Pb(index));
        }
        None
    }

    /// Clause `ci` whole: with the restriction literals set aside from it.
    fn clause_lits(&self, ci: u32) -> Vec<u32> {
        let mut lits = self.clauses.get(ci as usize).to_vec();
        if let Some(aside) = self.aside.get(&ci) {
            lits.extend_from_slice(aside);
        }
        lits
    }

    /// The reason clause of `v` (implied literal first, the rest false).
    fn reason_lits(&self, v: usize) -> Vec<u32> {
        match self.reason[v].unpack() {
            Reason::Clause(ci) => self.clause_lits(ci),
            Reason::Binary(other) => vec![lit(idx(v), self.assigns[v] == FALSE), other],
            Reason::None => Vec::new(),
            Reason::Pb(index) => self.pb_reason(index, v),
        }
    }

    /// A threshold's explanation for `v`, from the conditions assigned before
    /// it: exactly the assignment under which the threshold forced it.
    fn pb_reason(&self, index: u32, v: usize) -> Vec<u32> {
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
            e.push(if self.assigns[pb.out as usize] == TRUE {
                neg(out_lit)
            } else {
                out_lit
            });
            self.assigns[pb.out as usize] == FALSE
        };
        for (&c, &w) in pb.conditions.iter().zip(&pb.weights) {
            if w == 0 || c as usize == v || !earlier(c) {
                continue;
            }
            match (use_true, self.assigns[c as usize]) {
                (true, TRUE) => e.push(lit(c, true)),
                (false, FALSE) => e.push(lit(c, false)),
                _ => {}
            }
        }
        dedup_first(&e)
    }

    fn bump(&mut self, v: usize) {
        self.activity[v] += self.increment;
        if self.activity[v] > 1e100 {
            for a in &mut self.activity {
                *a *= 1e-100;
            }
            self.increment *= 1e-100;
        }
        if self.heap_index[v] >= 0 {
            self.heap_up(unslot(self.heap_index[v]));
        }
    }

    /// First-UIP analysis; returns the learned clause (asserting literal
    /// first) and the backjump level.
    fn analyze(&mut self, conflict: Vec<u32>) -> (Vec<u32>, usize) {
        let mut learned = vec![0_u32];
        let mut pending = 0;
        let mut index = self.trail.len();
        let mut current: Vec<u32> = conflict;
        let mut p: Option<u32> = None;
        let level = idx(self.decision_level());
        loop {
            for &q in &current {
                if Some(q) == p {
                    continue;
                }
                let v = var(q) as usize;
                if !self.seen.get(v) && self.level[v] > 0 {
                    self.seen.set(v, true);
                    self.bump(v);
                    if self.level[v] >= level {
                        pending += 1;
                    } else {
                        learned.push(q);
                    }
                }
            }
            loop {
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
            current = self.reason_lits(v);
        }
        if self.refinements.has(Refinements::MINIMIZE) && learned.len() > 2 {
            let mut levels = 0_u32;
            for &q in &learned[1..] {
                levels |= 1 << (self.level[var(q) as usize] & 31);
            }
            let mut clear: Vec<usize> = learned[1..].iter().map(|&q| var(q) as usize).collect();
            let mut kept = 1;
            for i in 1..learned.len() {
                let q = learned[i];
                let v = var(q) as usize;
                if matches!(self.reason[v].unpack(), Reason::None)
                    || !self.redundant(v, levels, &mut clear)
                {
                    learned[kept] = q;
                    kept += 1;
                }
            }
            learned.truncate(kept);
            for v in clear {
                self.seen.set(v, false);
            }
        } else {
            for &q in &learned[1..] {
                self.seen.set(var(q) as usize, false);
            }
        }
        let mut back = 0;
        if learned.len() > 1 {
            let mut best = 1;
            for k in 2..learned.len() {
                if self.level[var(learned[k]) as usize] > self.level[var(learned[best]) as usize] {
                    best = k;
                }
            }
            learned.swap(1, best);
            back = self.level[var(learned[1]) as usize] as usize;
        }
        self.increment *= 1.0 / 0.95;
        (learned, back)
    }

    /// Whether `v`, seen in a learned clause, is implied by the clause's
    /// other literals through reasons alone (recursive minimization).
    fn redundant(&mut self, v: usize, levels: u32, clear: &mut Vec<usize>) -> bool {
        let top = clear.len();
        let mut stack = vec![v];
        while let Some(x) = stack.pop() {
            for r in self.reason_lits(x) {
                let u = var(r) as usize;
                if u == x || self.seen.get(u) || self.level[u] == 0 {
                    continue;
                }
                if !matches!(self.reason[u].unpack(), Reason::None)
                    && levels & (1 << (self.level[u] & 31)) != 0
                {
                    self.seen.set(u, true);
                    stack.push(u);
                    clear.push(u);
                } else {
                    for &w in &clear[top..] {
                        self.seen.set(w, false);
                    }
                    clear.truncate(top);
                    return false;
                }
            }
        }
        true
    }

    fn cancel_until(&mut self, level: usize) {
        if self.decision_level() <= level {
            return;
        }
        let start = self.trail_lim[level];
        for k in (start..self.trail.len()).rev() {
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
                let (sure, open) = if index & FAMILY_ENTRY == 0 {
                    let pb = &mut self.pbs[index as usize];
                    (&mut pb.sure, &mut pb.open)
                } else {
                    let family = &mut self.families[(index & !FAMILY_ENTRY) as usize];
                    (&mut family.sure, &mut family.open)
                };
                *open += i128::from(w);
                if l & 1 == 0 {
                    *sure -= i128::from(w);
                }
            }
            self.assigns[v] = UNDEF;
            self.reason[v] = PackedReason::NONE;
            if self.heap_index[v] < 0 {
                self.heap_insert(idx(v));
            }
        }
        self.trail.truncate(start);
        self.trail_lim.truncate(level);
        self.head = start;
        if level == 0 && !self.aside.is_empty() {
            self.restore_locals();
        }
    }

    /// Search under `assumptions`. `Unsatisfiable(n)` refutes using `n` of
    /// them.
    fn solve(
        &mut self,
        assumptions: &[u32],
        allowance: u64,
        cancellation: &Cancellation,
        idle: Option<&std::sync::atomic::AtomicUsize>,
    ) -> Result<Outcome, Stop> {
        self.core.clear();
        if self.inconsistent {
            return Ok(Outcome::Unsatisfiable(0));
        }
        self.cancel_until(0);
        let start_conflicts = self.counts.conflicts;
        let mut restart_at = if self.refinements.has(Refinements::LUBY) {
            64 * luby(self.restarts)
        } else {
            100_u64
        };
        let mut since_restart = 0_u64;
        // All assumptions share decision level 1, so a backjump never
        // re-propagates them one level at a time.
        let mut assumed = false;
        loop {
            if let Some(conflict) = self.propagate() {
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
                        self.conflict_core(&conflict, &mut core);
                        self.core = core;
                        self.cancel_until(0);
                        return Ok(Outcome::Unsatisfiable(self.core.len()));
                    }
                    _ => {}
                }
                let (learned, back) = self.analyze(conflict);
                if learned.len() == 1 || back == 0 {
                    self.cancel_until(0);
                    assumed = false;
                } else {
                    self.cancel_until(back);
                }
                self.record(&learned);
                if self.counts.conflicts.trailing_zeros() >= 8 {
                    cancellation.poll()?;
                }
                if self.counts.conflicts - start_conflicts >= allowance
                    || idle.is_some_and(|idle| idle.load(std::sync::atomic::Ordering::Relaxed) > 0)
                {
                    self.cancel_until(0);
                    return Ok(Outcome::Unknown);
                }
                continue;
            }
            if since_restart >= restart_at && self.decision_level() > 1 {
                since_restart = 0;
                if self.refinements.has(Refinements::LUBY) {
                    self.restarts += 1;
                    restart_at = 64 * luby(self.restarts);
                } else {
                    restart_at = restart_at + restart_at / 2;
                }
                self.cancel_until(1);
                let limit = if self.refinements.has(Refinements::GLUE) {
                    4_000 + 500 * width(self.reductions)
                } else {
                    20_000 + width(self.counts.conflicts) / 4
                };
                if self.learned_count > limit {
                    self.reduce();
                }
                continue;
            }
            if !assumed {
                assumed = true;
                if let Some(refuted) = self.assume(assumptions) {
                    return Ok(refuted);
                }
                continue;
            }
            let Some(v) = self.pick() else {
                // Leave the model readable through `value`.
                return Ok(Outcome::Satisfiable);
            };
            self.counts.decisions += 1;
            let decision = lit(v, !self.phase.get(v as usize));
            self.trail_lim.push(self.trail.len());
            self.assign(decision, Reason::None);
        }
    }

    /// Assume `assumptions` at level 1, then try each of at most `budget`
    /// unassigned `candidates` both ways by propagation alone. A side that
    /// fails is learned as the clause forcing the other (a consequence: its
    /// core implies the other side); both failing refutes the assumptions.
    fn probe(
        &mut self,
        assumptions: &[u32],
        candidates: &[u32],
        budget: usize,
        cancellation: &Cancellation,
    ) -> Result<Outcome, Stop> {
        self.core.clear();
        if self.inconsistent {
            return Ok(Outcome::Unsatisfiable(0));
        }
        self.cancel_until(0);
        if self.propagate().is_some() {
            self.inconsistent = true;
            return Ok(Outcome::Unsatisfiable(0));
        }
        if let Some(refuted) = self.assume(assumptions) {
            return Ok(refuted);
        }
        let mut spent = 0;
        let mut cores: [Vec<u32>; 2] = [Vec::new(), Vec::new()];
        for &v in candidates {
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
                self.trail_lim.push(self.trail.len());
                self.assign(l, Reason::None);
                if let Some(conflict) = self.propagate() {
                    failed[side] = true;
                    let mut core = std::mem::take(&mut cores[side]);
                    self.conflict_core(&conflict, &mut core);
                    core.retain(|&k| k != l);
                    cores[side] = core;
                }
                self.cancel_until(1);
            }
            match failed {
                [true, true] => {
                    let mut core = std::mem::take(&mut cores[0]);
                    core.extend(cores[1].iter().copied());
                    core.sort_unstable();
                    core.dedup();
                    self.core = core;
                    self.cancel_until(0);
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
                    clause.extend(core.iter().map(|&k| neg(k)));
                    let reason = self.attach(&clause, true);
                    self.counts.learned += 1;
                    self.assign(other, reason);
                    if let Some(conflict) = self.propagate() {
                        return Ok(self.refuted_by(&conflict));
                    }
                }
                [false, false] => {}
            }
        }
        self.cancel_until(0);
        Ok(Outcome::Unknown)
    }

    /// Open level 1 and assume, propagating as we go; a refutation of the
    /// assumptions alone is returned with its core.
    fn assume(&mut self, assumptions: &[u32]) -> Option<Outcome> {
        self.trail_lim.push(self.trail.len());
        for &a in assumptions {
            match self.lit_value(a) {
                TRUE => {}
                FALSE => {
                    let mut core = std::mem::take(&mut self.core);
                    self.analyze_final(neg(a), &mut core);
                    self.core = core;
                    self.cancel_until(0);
                    return Some(Outcome::Unsatisfiable(self.core.len()));
                }
                _ => {
                    self.assign(a, Reason::None);
                    if let Some(conflict) = self.propagate() {
                        return Some(self.refuted_by(&conflict));
                    }
                }
            }
        }
        None
    }

    /// Refute the assumptions by a conflict under them, keeping its core.
    fn refuted_by(&mut self, conflict: &[u32]) -> Outcome {
        let mut core = std::mem::take(&mut self.core);
        self.conflict_core(conflict, &mut core);
        self.core = core;
        self.cancel_until(0);
        Outcome::Unsatisfiable(self.core.len())
    }

    /// The assumptions behind a conflict found at level 1, written to `core`.
    fn conflict_core(&mut self, conflict: &[u32], core: &mut Vec<u32>) {
        core.clear();
        for &q in conflict {
            let u = var(q) as usize;
            if self.level[u] > 0 {
                self.seen.set(u, true);
            }
        }
        let start = self.trail_lim[0];
        for k in (start..self.trail.len()).rev() {
            let l = self.trail[k];
            let v = var(l) as usize;
            if !self.seen.get(v) {
                continue;
            }
            if self.reason[v] == PackedReason::NONE {
                core.push(l);
            } else {
                for &q in &self.reason_lits(v)[1..] {
                    let u = var(q) as usize;
                    if self.level[u] > 0 {
                        self.seen.set(u, true);
                    }
                }
            }
            self.seen.set(v, false);
        }
    }

    /// The assumptions that imply `p` (an assumption's negation now true),
    /// `p`'s own assumption included, written to `core`.
    fn analyze_final(&mut self, p: u32, core: &mut Vec<u32>) {
        core.clear();
        core.push(neg(p));
        if self.decision_level() == 0 {
            return;
        }
        self.seen.set(var(p) as usize, true);
        let start = self.trail_lim[0];
        for k in (start..self.trail.len()).rev() {
            let l = self.trail[k];
            let v = var(l) as usize;
            if !self.seen.get(v) {
                continue;
            }
            if self.reason[v] == PackedReason::NONE {
                // An assumption; `p` itself when the opposite one was made.
                core.push(l);
            } else {
                let reason = self.reason_lits(v);
                for &q in &reason[1..] {
                    let u = var(q) as usize;
                    if self.level[u] > 0 {
                        self.seen.set(u, true);
                    }
                }
            }
            self.seen.set(v, false);
        }
        self.seen.set(var(p) as usize, false);
    }

    fn reduce(&mut self) {
        // Drop the older half of learned clauses not currently a reason.
        let locked: std::collections::HashSet<u32> = self
            .trail
            .iter()
            .filter_map(|&l| match self.reason[var(l) as usize].unpack() {
                Reason::Clause(ci) => Some(ci),
                _ => None,
            })
            .collect();
        let learned = self.learned_count;
        let mut keep = vec![true; self.clauses.len()];
        let mut dropped = 0;
        self.reductions += 1;
        if self.refinements.has(Refinements::GLUE) {
            // Widest spans first, older first among equals; spans of two
            // levels or fewer are kept.
            let mut candidates: Vec<usize> = (0..self.clauses.len())
                .filter(|&ci| {
                    self.is_learned.get(ci) && self.glue[ci] > 2 && !locked.contains(&idx(ci))
                })
                .collect();
            candidates.sort_by_key(|&ci| (std::cmp::Reverse(self.glue[ci]), ci));
            for ci in candidates.into_iter().take(learned / 2) {
                keep[ci] = false;
                dropped += 1;
            }
        } else {
            for (ci, kept) in keep.iter_mut().enumerate() {
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
        }
        self.compact_clauses(&keep, dropped);
    }

    /// Keep the clauses `keep` marks (`dropped` learned ones are not),
    /// renumbering watches, reasons and set-aside literals.
    fn compact_clauses(&mut self, keep: &[bool], dropped: usize) {
        let mut remap = vec![u32::MAX; self.clauses.len()];
        let old = std::mem::take(&mut self.clauses);
        let mut kept = Arena {
            lits: Vec::with_capacity(old.lits.len()),
            spans: Vec::with_capacity(old.len() - dropped),
        };
        let mut kept_learned = Bits::with_capacity(old.len() - dropped);
        let mut kept_glue = Vec::with_capacity(old.len() - dropped);
        for (ci, &keep) in keep.iter().enumerate() {
            if keep {
                remap[ci] = kept.push(old.get(ci));
                kept_learned.push(self.is_learned.get(ci));
                kept_glue.push(self.glue[ci]);
            }
        }
        if !self.aside.is_empty() {
            self.aside = std::mem::take(&mut self.aside)
                .into_iter()
                .filter_map(|(ci, lits)| {
                    let mapped = remap[ci as usize];
                    (mapped != u32::MAX).then_some((mapped, lits))
                })
                .collect();
        }
        self.glue = kept_glue;
        self.clauses = kept;
        self.is_learned = kept_learned;
        self.learned_count -= dropped;
        for watchers in &mut self.watches {
            watchers.retain_mut(|w| {
                let ci = watch_clause(*w);
                if ci == BINARY {
                    return true;
                }
                let mapped = remap[ci as usize];
                *w = watch(mapped, watch_blocker(*w));
                mapped != u32::MAX
            });
        }
        for v in 0..self.vars {
            if let Reason::Clause(ci) = self.reason[v].unpack() {
                self.reason[v] = PackedReason::pack(Reason::Clause(remap[ci as usize]));
            }
        }
    }

    fn pick(&mut self) -> Option<u32> {
        while let Some(v) = self.heap_pop() {
            if self.assigns[v as usize] == UNDEF {
                return Some(v);
            }
        }
        // Defined variables are assigned once their definitions' inputs are;
        // any left over (none expected) is decided directly.
        (0..idx(self.vars)).find(|&v| self.assigns[v as usize] == UNDEF)
    }

    fn heap_less(&self, a: u32, b: u32) -> bool {
        self.activity[a as usize] > self.activity[b as usize]
    }

    fn heap_insert(&mut self, v: u32) {
        if !self.decidable.get(v as usize) {
            return;
        }
        self.heap_index[v as usize] = slot(self.heap.len());
        self.heap.push(v);
        self.heap_up(self.heap.len() - 1);
    }

    fn heap_up(&mut self, mut i: usize) {
        let v = self.heap[i];
        while i > 0 {
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
    }

    fn heap_pop(&mut self) -> Option<u32> {
        let top = *self.heap.first()?;
        let last = self.heap.pop().expect("nonempty");
        self.heap_index[top as usize] = -1;
        if !self.heap.is_empty() {
            self.heap[0] = last;
            self.heap_index[last as usize] = 0;
            let mut i = 0;
            loop {
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
        Some(top)
    }
}

fn dedup(mut clause: Vec<u32>) -> Vec<u32> {
    clause.sort_unstable();
    clause.dedup();
    clause
}

/// Keep the first literal first, deduplicate the rest.
fn dedup_first(clause: &[u32]) -> Vec<u32> {
    let first = clause[0];
    let mut rest: Vec<u32> = clause[1..]
        .iter()
        .copied()
        .filter(|&l| l != first)
        .collect();
    rest.sort_unstable();
    rest.dedup();
    let mut out = Vec::with_capacity(rest.len() + 1);
    out.push(first);
    out.extend(rest);
    out
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
mod solver_tests;
