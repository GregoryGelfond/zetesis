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
    /// most [`ENUMERATION_CEILING`].
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
}

/// No enumeration reads more than `2^22` completions.
pub(crate) const ENUMERATION_CEILING: usize = 22;

/// Recent refutation cores a walker keeps.
const CORES: usize = 64;

impl SmallPolicy {
    /// The policy named by `ZETESIS_SMALL_REGIONS`, e.g.
    /// `enumerate=10,learn=256,conflicts=4000`; off when unset.
    #[must_use]
    pub fn from_environment() -> Self {
        let mut policy = Self {
            learn_conflicts: 4000,
            ..Self::default()
        };
        let Ok(text) = std::env::var("ZETESIS_SMALL_REGIONS") else {
            return policy;
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
                _ => {}
            }
        }
        policy
    }

    const fn active(&self) -> bool {
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
    /// Regions probed for failed literals.
    pub probed: u64,
    /// Probed regions refuted.
    pub probed_refuted: u64,
    /// Time spent probing.
    pub probed_nanos: u64,
    /// Clausal encodings built.
    pub encodings: u64,
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
    /// Completions recently found to pass, as held-atom words.
    models: Vec<Vec<u64>>,
    /// The counts so far.
    pub(crate) statistics: SmallStatistics,
}

impl Drop for SmallScratch {
    fn drop(&mut self) {
        let s = &self.statistics;
        if (s.enumerated > 0 || s.learned > 0 || s.cached > 0)
            && std::env::var_os("ZETESIS_SMALL_STATS").is_some()
        {
            eprintln!("zetesis-small-regions {s:?}");
        }
    }
}

/// A node's shape, read once per theory.
#[derive(Debug)]
struct Shape {
    theory: Theory,
    atoms: usize,
    /// Every node's operands, flattened, and its kind.
    kinds: Vec<Kind>,
    operands: Vec<usize>,
    spans: Vec<(u32, u32)>,
    weights: Vec<u64>,
    bounds: Vec<i128>,
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

impl Shape {
    fn read(theory: &Theory, producers: Option<&Producers>) -> Option<Self> {
        let view = theory.view();
        let count = view.len();
        let atoms = theory.atom_count();
        let mut kinds = Vec::with_capacity(count);
        let mut operands = Vec::new();
        let mut spans = Vec::with_capacity(count);
        let mut weights = Vec::new();
        let mut bounds = vec![0_i128; count];
        let mut has_cuts = false;
        let mut parent_edges = Vec::new();
        let mut atom_edges = Vec::new();
        for (node, bound) in bounds.iter_mut().enumerate() {
            let start = idx(operands.len());
            let kind = match view.node(node).ok()? {
                NodeView::Atom(atom) => {
                    atom_edges.push((idx(atom), idx(node)));
                    Kind::Atom(idx(atom))
                }
                NodeView::False => Kind::False,
                NodeView::Implies(left, right) => {
                    operands.extend([left, right]);
                    weights.extend([0, 0]);
                    Kind::Implies
                }
                NodeView::And(row) => {
                    operands.extend_from_slice(row);
                    weights.extend(std::iter::repeat_n(0, row.len()));
                    Kind::And
                }
                NodeView::Or(row) => {
                    operands.extend_from_slice(row);
                    weights.extend(std::iter::repeat_n(0, row.len()));
                    Kind::Or
                }
                NodeView::Threshold(threshold) => {
                    operands.extend_from_slice(threshold.conditions);
                    weights.extend_from_slice(threshold.weights);
                    *bound = threshold.bound;
                    Kind::Threshold
                }
                NodeView::ExtremumCut(cut) => {
                    has_cuts = true;
                    operands.extend_from_slice(cut.row.conditions);
                    weights.extend(std::iter::repeat_n(0, cut.row.conditions.len()));
                    Kind::Cut
                }
            };
            let end = idx(operands.len());
            for &child in &operands[start as usize..end as usize] {
                parent_edges.push((idx(child), idx(node)));
            }
            kinds.push(kind);
            spans.push((start, end));
        }
        parent_edges.sort_unstable();
        parent_edges.dedup();
        let (parents_offsets, parents) = csr(count, &parent_edges);
        let (atom_nodes_offsets, atom_nodes) = csr(atoms, &atom_edges);
        let support = producers.map(|producers| Support::read(producers, atoms, count));
        Some(Self {
            theory: theory.clone(),
            atoms,
            kinds,
            operands,
            spans,
            weights,
            bounds,
            parents_offsets,
            parents,
            atom_nodes_offsets,
            atom_nodes,
            roots: theory.roots().to_vec(),
            support,
            has_cuts,
        })
    }

    fn row(&self, node: usize) -> &[usize] {
        let (start, end) = self.spans[node];
        &self.operands[start as usize..end as usize]
    }

    fn row_weights(&self, node: usize) -> &[u64] {
        let (start, end) = self.spans[node];
        &self.weights[start as usize..end as usize]
    }
}

impl SmallScratch {
    /// The policy, read once.
    pub(crate) fn policy(&mut self) -> SmallPolicy {
        *self
            .policy
            .get_or_insert_with(SmallPolicy::from_environment)
    }

    pub(crate) fn active(&mut self) -> bool {
        self.policy().active()
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
        let policy = self.policy();
        if !policy.active() {
            return Ok(SmallNarrowing::Declined);
        }
        let open = region.open().count();
        if open == 0 {
            return Ok(SmallNarrowing::Declined);
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
        if open <= learn_up_to {
            if self.shape(theory, producers).is_none() {
                return Ok(SmallNarrowing::Declined);
            }
            self.guard(restrictions);
            if self.cached(region) {
                self.statistics.cached += 1;
                return Ok(SmallNarrowing::Open(Variant::Learned));
            }
            if self.core_hit(region) {
                self.statistics.core_hits += 1;
                return Ok(SmallNarrowing::Refuted(Variant::Learned));
            }
            let started = Instant::now();
            let verdict = self.learn(region, policy.learn_conflicts, cancellation);
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

    /// The general narrowing refuted a region of `open` open atoms: under an
    /// adapting policy, regions of that size go to the learned search next,
    /// as cube-and-conquer lowers its cutoff when lookahead itself fails.
    pub(crate) fn note_refuted(&mut self, region: &Region) {
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
            let solver = Solver::encode(shape, self.policy.is_some_and(|p| p.decide_all));
            if std::env::var_os("ZETESIS_SMALL_STATS").is_some() {
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

    fn cached(&self, region: &Region) -> bool {
        let (held, cut) = region.decision_words();
        self.models.iter().any(|model| {
            held.iter()
                .zip(cut)
                .zip(model)
                .all(|((&held, &cut), &model)| held & !model == 0 && cut & model == 0)
        })
    }

    // ---------------------------------------------------------------- enumeration

    /// `None` declines (an ordinal cut above an open atom).
    fn enumerate(&mut self, region: &Region) -> Option<SmallNarrowing> {
        let shape = self.shape.as_ref()?;
        let nodes = shape.kinds.len();
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
        if shape.has_cuts && self.cone.iter().any(|&n| shape.kinds[n] == Kind::Cut) {
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
            self.solver = Some(Solver::encode(
                shape,
                self.policy.is_some_and(|p| p.decide_all),
            ));
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
            let solver = Solver::encode(shape, self.policy.is_some_and(|p| p.decide_all));
            if std::env::var_os("ZETESIS_SMALL_STATS").is_some() {
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
        let outcome = solver.solve(&self.assumptions, conflicts, cancellation)?;
        let after = solver.counts;
        self.statistics.decisions += after.decisions - before.decisions;
        self.statistics.conflicts += after.conflicts - before.conflicts;
        self.statistics.propagations += after.propagations - before.propagations;
        self.statistics.learned_clauses += after.learned - before.learned;
        Ok(match outcome {
            Outcome::Unsatisfiable(core) => {
                self.statistics.learned_refuted += 1;
                self.statistics.core_width += core as u64;
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
        match shape.kinds[node] {
            Kind::Atom(atom) => self.atom(atom as usize, word),
            Kind::False => 0,
            Kind::Implies => {
                let row = shape.row(node);
                !self.operand(row[0], lanes, memo) | self.operand(row[1], lanes, memo)
            }
            Kind::And => shape
                .row(node)
                .iter()
                .fold(u64::MAX, |acc, &c| acc & self.operand(c, lanes, memo)),
            Kind::Or => shape
                .row(node)
                .iter()
                .fold(0, |acc, &c| acc | self.operand(c, lanes, memo)),
            Kind::Threshold => {
                let values: Vec<u64> = shape
                    .row(node)
                    .iter()
                    .map(|&c| self.operand(c, lanes, memo))
                    .collect();
                threshold_lanes(&values, shape.row_weights(node), shape.bounds[node])
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
        if !expanded && !matches!(shape.kinds[node], Kind::Atom(_) | Kind::False) {
            stack.push((node, true));
            for &child in row {
                if !values.contains_key(&child) {
                    stack.push((child, false));
                }
            }
            continue;
        }
        let get = |child: usize| values.get(&child).copied().unwrap_or(false);
        let value = match shape.kinds[node] {
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
                sum >= shape.bounds[node]
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

/// Conflict-learning search over the necessary conditions of one theory.
#[derive(Debug)]
struct Solver {
    vars: usize,
    clauses: Vec<Vec<u32>>,
    /// Which clauses were learned (only those are ever dropped).
    is_learned: Vec<bool>,
    watches: Vec<Vec<u32>>,
    pbs: Vec<Pb>,
    pb_occurs: Vec<Vec<u32>>,
    /// Per variable, the thresholds it is a condition of, with its weight.
    pb_terms: Vec<Vec<(u32, u64)>>,
    assigns: Vec<u8>,
    level: Vec<u32>,
    reason: Vec<Reason>,
    trail: Vec<u32>,
    trail_lim: Vec<usize>,
    head: usize,
    activity: Vec<f64>,
    increment: f64,
    phase: Vec<bool>,
    heap: Vec<u32>,
    heap_index: Vec<i32>,
    /// Trail position of each assigned variable.
    trail_pos: Vec<u32>,
    seen: Vec<bool>,
    /// Level-0 inconsistency: every call refutes.
    inconsistent: bool,
    counts: Counts,
    /// The assumptions the last refutation used.
    core: Vec<u32>,
    /// Restriction theories encoded, with their guard variables.
    guards: Vec<(Theory, u32)>,
    /// Variables the search decides; the rest are defined by them.
    decidable: Vec<bool>,
    decide_all: bool,
}

impl Solver {
    fn encode(shape: &Shape, decide_all: bool) -> Self {
        let vars = shape.atoms;
        let mut solver = Self {
            vars,
            clauses: Vec::new(),
            is_learned: Vec::new(),
            watches: vec![Vec::new(); vars * 2],
            pbs: Vec::new(),
            pb_occurs: vec![Vec::new(); vars],
            pb_terms: vec![Vec::new(); vars],
            assigns: vec![UNDEF; vars],
            level: vec![0; vars],
            reason: vec![Reason::None; vars],
            trail: Vec::with_capacity(vars),
            trail_lim: Vec::new(),
            head: 0,
            activity: vec![0.0; vars],
            increment: 1.0,
            phase: vec![false; vars],
            heap: Vec::new(),
            heap_index: vec![-1; vars],
            trail_pos: vec![0; vars],
            seen: vec![false; vars],
            inconsistent: false,
            counts: Counts::default(),
            core: Vec::new(),
            guards: Vec::new(),
            decidable: vec![true; vars],
            decide_all,
        };
        for v in 0..idx(vars) {
            solver.heap_insert(v);
        }
        solver.add_theory(shape, None);
        solver
    }

    /// A fresh variable, unassigned, in the decision heap.
    fn fresh(&mut self, decidable: bool) -> u32 {
        let v = idx(self.vars);
        self.decidable.push(decidable || self.decide_all);
        self.vars += 1;
        self.watches.push(Vec::new());
        self.watches.push(Vec::new());
        self.pb_occurs.push(Vec::new());
        self.pb_terms.push(Vec::new());
        self.assigns.push(UNDEF);
        self.level.push(0);
        self.reason.push(Reason::None);
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
        let atoms = shape.atoms;
        let nodes = shape.kinds.len();
        // Atom nodes alias their atom; other nodes get their own variable.
        let node_var: Vec<u32> = shape
            .kinds
            .iter()
            .map(|kind| match *kind {
                Kind::Atom(atom) => atom,
                Kind::Cut => self.fresh(true),
                _ => self.fresh(false),
            })
            .collect();
        let mut clauses: Vec<Vec<u32>> = Vec::new();
        let mut pbs = Vec::new();
        for node in 0..nodes {
            let v = node_var[node];
            let pos = lit(v, false);
            let row: Vec<u32> = shape.row(node).iter().map(|&c| node_var[c]).collect();
            match shape.kinds[node] {
                Kind::Atom(_) | Kind::Cut => {}
                Kind::False => clauses.push(vec![neg(pos)]),
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
                    bound: shape.bounds[node],
                    sure: 0,
                    open: 0,
                    heaviest: 0,
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
            self.support_clauses(support, atoms, &node_var, &mut clauses);
        }
        let first_pb = self.pbs.len();
        for (offset, pb) in pbs.into_iter().enumerate() {
            self.add_pb(idx(first_pb + offset), pb);
        }
        for mut clause in clauses {
            clause.sort_unstable();
            clause.dedup();
            if clause.windows(2).any(|w| w[0] == neg(w[1])) {
                continue; // tautology
            }
            self.add_original(clause);
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

    /// The support condition of every atom: held only with a producer whose
    /// body holds and whose other ordinary heads fail.
    fn support_clauses(
        &mut self,
        support: &Support,
        atoms: usize,
        node_var: &[u32],
        clauses: &mut Vec<Vec<u32>>,
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
                    clause.push(lit(node_var[body as usize], false));
                } else {
                    // s <-> body & !others
                    let sp = lit(self.fresh(false), false);
                    let mut long = vec![sp];
                    if body != u32::MAX {
                        let b = lit(node_var[body as usize], false);
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
        self.pb_occurs[pb.out as usize].push(index);
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
            self.pb_terms[c as usize].push((index, weight));
        }
        let mut seen_conditions: Vec<u32> = pb.conditions.clone();
        seen_conditions.sort_unstable();
        seen_conditions.dedup();
        for &c in &seen_conditions {
            self.pb_occurs[c as usize].push(index);
        }
        self.pbs.push(pb);
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

    fn add_original(&mut self, clause: Vec<u32>) {
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
                let index = idx(self.clauses.len());
                self.watches[neg(clause[0]) as usize].push(index);
                self.watches[neg(clause[1]) as usize].push(index);
                self.clauses.push(clause);
                self.is_learned.push(false);
            }
        }
    }

    fn assign(&mut self, l: u32, reason: Reason) {
        let v = var(l) as usize;
        self.assigns[v] = TRUE ^ u8::from(l & 1 == 1);
        self.level[v] = idx(self.trail_lim.len());
        self.reason[v] = reason;
        self.trail_pos[v] = idx(self.trail.len());
        self.trail.push(l);
        let positive = l & 1 == 0;
        for &(index, w) in &self.pb_terms[v] {
            let pb = &mut self.pbs[index as usize];
            pb.open -= i128::from(w);
            if positive {
                pb.sure += i128::from(w);
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
                let ci = watchers[i] as usize;
                let clause = &mut self.clauses[ci];
                if clause[0] == false_lit {
                    clause.swap(0, 1);
                }
                let first = clause[0];
                if self.assigns[var(first) as usize] != UNDEF
                    && (self.assigns[var(first) as usize] ^ u8::from(first & 1 == 1)) == TRUE
                {
                    i += 1;
                    continue;
                }
                let mut moved = false;
                for k in 2..clause.len() {
                    let l = clause[k];
                    let a = self.assigns[var(l) as usize];
                    if a == UNDEF || (a ^ u8::from(l & 1 == 1)) == TRUE {
                        clause.swap(1, k);
                        self.watches[neg(clause[1]) as usize].push(idx(ci));
                        watchers.swap_remove(i);
                        moved = true;
                        break;
                    }
                }
                if moved {
                    continue;
                }
                let a = self.assigns[var(first) as usize];
                if a == UNDEF {
                    let first = clause[0];
                    self.assign(first, Reason::Clause(idx(ci)));
                    i += 1;
                } else {
                    conflict = Some(self.clauses[ci].clone());
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
            let occurs = std::mem::take(&mut self.pb_occurs[var(p) as usize]);
            let mut pb_conflict = None;
            for &index in &occurs {
                if let Some(clause) = self.pb_check(index) {
                    pb_conflict = Some(clause);
                    break;
                }
            }
            self.pb_occurs[var(p) as usize] = occurs;
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

    /// The reason clause of `v` (implied literal first, the rest false).
    fn reason_lits(&self, v: usize) -> Vec<u32> {
        match self.reason[v] {
            Reason::Clause(ci) => self.clauses[ci as usize].clone(),
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
                if !self.seen[v] && self.level[v] > 0 {
                    self.seen[v] = true;
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
                if self.seen[var(self.trail[index]) as usize] {
                    break;
                }
            }
            let next = self.trail[index];
            let v = var(next) as usize;
            self.seen[v] = false;
            pending -= 1;
            if pending == 0 {
                learned[0] = neg(next);
                break;
            }
            p = Some(next);
            current = self.reason_lits(v);
        }
        for &q in &learned[1..] {
            self.seen[var(q) as usize] = false;
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

    fn cancel_until(&mut self, level: usize) {
        if self.decision_level() <= level {
            return;
        }
        let start = self.trail_lim[level];
        for k in (start..self.trail.len()).rev() {
            let l = self.trail[k];
            let v = var(l) as usize;
            self.phase[v] = l & 1 == 0;
            for &(index, w) in &self.pb_terms[v] {
                let pb = &mut self.pbs[index as usize];
                pb.open += i128::from(w);
                if l & 1 == 0 {
                    pb.sure -= i128::from(w);
                }
            }
            self.assigns[v] = UNDEF;
            self.reason[v] = Reason::None;
            if self.heap_index[v] < 0 {
                self.heap_insert(idx(v));
            }
        }
        self.trail.truncate(start);
        self.trail_lim.truncate(level);
        self.head = start;
    }

    /// Search under `assumptions`. `Unsatisfiable(n)` refutes using `n` of
    /// them.
    fn solve(
        &mut self,
        assumptions: &[u32],
        allowance: u64,
        cancellation: &Cancellation,
    ) -> Result<Outcome, Stop> {
        self.core.clear();
        if self.inconsistent {
            return Ok(Outcome::Unsatisfiable(0));
        }
        self.cancel_until(0);
        let start_conflicts = self.counts.conflicts;
        let mut restart_at = 100_u64;
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
                if learned.len() == 1 {
                    self.assign(learned[0], Reason::None);
                } else {
                    let index = idx(self.clauses.len());
                    self.watches[neg(learned[0]) as usize].push(index);
                    self.watches[neg(learned[1]) as usize].push(index);
                    let asserting = learned[0];
                    self.clauses.push(learned);
                    self.is_learned.push(true);
                    self.counts.learned += 1;
                    self.assign(asserting, Reason::Clause(index));
                }
                if self.counts.conflicts.trailing_zeros() >= 8 {
                    cancellation.poll()?;
                }
                if self.counts.conflicts - start_conflicts >= allowance {
                    self.cancel_until(0);
                    return Ok(Outcome::Unknown);
                }
                continue;
            }
            if since_restart >= restart_at && self.decision_level() > 1 {
                since_restart = 0;
                restart_at = restart_at + restart_at / 2;
                self.cancel_until(1);
                if self.is_learned.iter().filter(|&&l| l).count()
                    > 20_000 + width(self.counts.conflicts) / 4
                {
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
            let decision = lit(v, !self.phase[v as usize]);
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
                    let index = idx(self.clauses.len());
                    self.watches[neg(clause[0]) as usize].push(index);
                    self.watches[neg(clause[1]) as usize].push(index);
                    self.clauses.push(clause);
                    self.is_learned.push(true);
                    self.counts.learned += 1;
                    self.assign(other, Reason::Clause(index));
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
                self.seen[u] = true;
            }
        }
        let start = self.trail_lim[0];
        for k in (start..self.trail.len()).rev() {
            let l = self.trail[k];
            let v = var(l) as usize;
            if !self.seen[v] {
                continue;
            }
            if self.reason[v] == Reason::None {
                core.push(l);
            } else {
                for &q in &self.reason_lits(v)[1..] {
                    let u = var(q) as usize;
                    if self.level[u] > 0 {
                        self.seen[u] = true;
                    }
                }
            }
            self.seen[v] = false;
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
        self.seen[var(p) as usize] = true;
        let start = self.trail_lim[0];
        for k in (start..self.trail.len()).rev() {
            let l = self.trail[k];
            let v = var(l) as usize;
            if !self.seen[v] {
                continue;
            }
            if self.reason[v] == Reason::None {
                if v != var(p) as usize {
                    core.push(l);
                }
            } else {
                let reason = self.reason_lits(v);
                for &q in &reason[1..] {
                    let u = var(q) as usize;
                    if self.level[u] > 0 {
                        self.seen[u] = true;
                    }
                }
            }
            self.seen[v] = false;
        }
        self.seen[var(p) as usize] = false;
    }

    fn reduce(&mut self) {
        // Drop the older half of learned clauses not currently a reason.
        let locked: std::collections::HashSet<u32> = self
            .trail
            .iter()
            .filter_map(|&l| match self.reason[var(l) as usize] {
                Reason::Clause(ci) => Some(ci),
                _ => None,
            })
            .collect();
        let learned = self.is_learned.iter().filter(|&&l| l).count();
        let mut keep = vec![true; self.clauses.len()];
        let mut dropped = 0;
        for (ci, kept) in keep.iter_mut().enumerate() {
            if dropped >= learned / 2 {
                break;
            }
            if self.is_learned[ci] && self.clauses[ci].len() > 2 && !locked.contains(&idx(ci)) {
                *kept = false;
                dropped += 1;
            }
        }
        let mut remap = vec![u32::MAX; self.clauses.len()];
        let mut kept = Vec::with_capacity(self.clauses.len() - dropped);
        let mut kept_learned = Vec::with_capacity(self.clauses.len() - dropped);
        for (ci, clause) in std::mem::take(&mut self.clauses).into_iter().enumerate() {
            if keep[ci] {
                remap[ci] = idx(kept.len());
                kept.push(clause);
                kept_learned.push(self.is_learned[ci]);
            }
        }
        self.clauses = kept;
        self.is_learned = kept_learned;
        for watchers in &mut self.watches {
            watchers.retain_mut(|ci| {
                let mapped = remap[*ci as usize];
                *ci = mapped;
                mapped != u32::MAX
            });
        }
        for v in 0..self.vars {
            if let Reason::Clause(ci) = self.reason[v] {
                self.reason[v] = Reason::Clause(remap[ci as usize]);
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
        if !self.decidable[v as usize] {
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
