//! Regions of candidates over a formula theory, narrowed by its readings.
//!
//! A region (`zetesis_cpu::regions::Region`) holds some atoms in, cuts some
//! out and leaves the rest open; here it decides over the theory's atoms. A
//! formula has two readings under a region: it is *sure* when every seed of
//! the region satisfies it and *never* when no seed does, with a held atom
//! sure, a cut atom never, and the connectives combining the readings as
//! the closure route's definite and possible gates do (`FormulaBounds.read`).
//! The readings are what the knowledge below means: a node known to hold
//! is sure, and a node known never is never.
//!
//! One narrowing closes, to a fixed point, what every candidate of the
//! region must make of each node and each atom: known to hold, known to
//! fail, or open. The rules are sound for stable models. Every root is
//! known to hold and falsum to fail; a node learns from its operands as
//! the connective dictates, and teaches its operands what its own
//! knowledge leaves them, a conjunction known to hold both operands, a
//! disjunction known to hold with one operand failing the other, an
//! implication known to hold its consequent when its antecedent holds and
//! the failure of its antecedent when its consequent fails, and the duals
//! for a node known to fail; an atom node and its atom know the same,
//! so a node shared by several parents is known once for all of them.
//! A maximal tree of one connective, a clause or a body, is read as one
//! node over its operands with the n-ary rules (`FormulaChains`), so a
//! decision costs one step per occurrence of its atom.
//! This is unit propagation over a clause form of the theory, read on the
//! theory itself (`FormulaBounds.Known`, `known_sound`). In the producer
//! fragment of the support restriction, an atom none of whose producers
//! can support it, each having a body known to fail or another head known
//! to hold, is known to fail (`unsupported_cut`), and an atom known to
//! hold with exactly one producer able to support it forces that
//! producer's body (`sole_support_forces`). A node or atom known both to
//! hold and to fail refutes the region. The open atoms known are then
//! held or cut.
//!
//! Work is charged per node read, per root tested and per producer checked,
//! against `RegionLimits`, and control is polled once per narrowing. Metered
//! entry points instead acquire each permit from the caller's quota, which can
//! also poll control, and retain the admitted prefix on every returned failure.

use std::collections::BTreeSet;
use std::mem::size_of;

use zetesis_cpu::regions::{Narrowing, Region};
use zetesis_cpu::{Cancellation, Stop};

use crate::{Node, Theory};

/// The work ceiling of one narrowing, and of producer extraction. Every
/// propagation event reads at least one node, so the work bounds the
/// events too.
#[derive(Clone, Copy, Debug)]
pub struct RegionLimits {
    /// Charged node reads, root tests and producer checks.
    pub max_work: u64,
}
impl Default for RegionLimits {
    fn default() -> Self {
        Self {
            max_work: 100_000_000,
        }
    }
}

/// One rule-shaped root: its body node, or none for a fact, and its head
/// atoms; a choice head supports its atom whenever its body can hold.
#[derive(Clone, Debug)]
struct Producer {
    body: Option<usize>,
    heads: Vec<usize>,
    choice: bool,
}

/// What one narrowing reads: the theory; the producers of its support
/// fragment, when the support cut applies; and the frozen truth of every
/// node under which the theory is read as a candidate's reduct, when it
/// is. A knowledge is closed under one subject and reused under the same
/// ([`Knowledge`]).
#[derive(Clone, Copy)]
struct Subject<'a> {
    theory: &'a Theory,
    producers: Option<&'a Producers>,
    frozen: Option<&'a [bool]>,
}

/// The producers of a theory in the support fragment, by head atom.
#[derive(Clone, Debug)]
pub struct Producers {
    rules: Vec<Producer>,
    by_head: Vec<Vec<usize>>,
    /// The producers whose body is this node.
    by_body: Vec<Vec<usize>>,
}

/// The outcome of extracting a theory's producers.
#[derive(Clone, Debug)]
pub struct Extraction {
    /// The producers, or none when a root lies outside the fragment.
    pub producers: Option<Producers>,
    /// Charged node and root visits, the same for a declined extraction.
    pub work: u64,
}

/// Extract the producers when every root is an implication, or a bare head,
/// whose head is a disjunction of atoms with falsum, an atomic choice, or
/// falsum: the fragment `DisjunctiveSupport.Covered` names, with a fact's
/// head standing alone. No producers when a root lies outside it, in which
/// case the support rule does not apply.
///
/// # Errors
/// Returns the stop when extraction exceeds `limits.max_work` or control stops.
pub fn producers(
    theory: &Theory,
    limits: RegionLimits,
    cancellation: &Cancellation,
) -> Result<Extraction, Stop> {
    cancellation.poll()?;
    let mut work = Work::new(limits.max_work);
    let producers = extract(theory, &mut work)?;
    Ok(Extraction {
        producers,
        work: work.spent,
    })
}

fn extract(theory: &Theory, work: &mut Work) -> Result<Option<Producers>, Stop> {
    let nodes = theory.nodes();
    let mut ordinary = Vec::with_capacity(nodes.len());
    for node in nodes {
        work.tick()?;
        ordinary.push(match *node {
            Node::Atom(_) | Node::False => true,
            Node::Or(left, right) => ordinary[left] && ordinary[right],
            _ => false,
        });
    }
    let mut rules = Vec::new();
    let mut by_head = vec![Vec::new(); theory.atom_count()];
    let mut by_body = vec![Vec::new(); nodes.len()];
    for &root in theory.roots() {
        work.tick()?;
        let (body, head) = match nodes[root] {
            Node::Implies(body, head) => (Some(body), head),
            _ => (None, root),
        };
        if matches!(nodes[head], Node::False) {
            continue;
        }
        let (heads, choice) = if ordinary[head] {
            let mut heads = BTreeSet::new();
            let mut stack = vec![head];
            while let Some(node) = stack.pop() {
                work.tick()?;
                match nodes[node] {
                    Node::Atom(atom) => {
                        heads.insert(atom);
                    }
                    Node::Or(left, right) => stack.extend([left, right]),
                    _ => {}
                }
            }
            (heads.into_iter().collect::<Vec<_>>(), false)
        } else if let Some(atom) = crate::atomic_choice::atom(theory, head) {
            (vec![atom], true)
        } else {
            return Ok(None);
        };
        let index = rules.len();
        for &atom in &heads {
            by_head[atom].push(index);
        }
        if let Some(body) = body {
            by_body[body].push(index);
        }
        rules.push(Producer {
            body,
            heads,
            choice,
        });
    }
    Ok(Some(Producers {
        rules,
        by_head,
        by_body,
    }))
}

/// The work one narrowing charged.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NarrowingStatistics {
    /// Charged node reads, root tests and producer checks.
    pub work: u64,
    /// Propagation events: nodes and atoms learned and their neighbours
    /// revisited, and atoms whose support was rechecked.
    pub propagations: u64,
    /// Atoms held by the narrowing.
    pub held: u64,
    /// Atoms cut by the narrowing.
    pub cut: u64,
}

/// One narrowing's verdict or original quota/control failure, with the
/// admitted work prefix on either outcome. A failed attempt leaves sound but
/// partially closed knowledge and region state; both must be abandoned.
#[derive(Debug)]
pub struct NarrowingAttempt<E = Stop> {
    /// Complete narrowing or the unchanged failure supplied by its quota.
    pub result: Result<Narrowing, E>,
    /// Charged reads and propagation progress, including a failed prefix.
    pub statistics: NarrowingStatistics,
}

struct Work<F = fn() -> Result<(), Stop>> {
    spent: u64,
    ceiling: u64,
    charge: F,
}
impl Work {
    fn new(ceiling: u64) -> Self {
        Self {
            spent: 0,
            ceiling,
            charge: || Ok(()),
        }
    }
}
impl<E: From<Stop>, F: FnMut() -> Result<(), E>> Work<F> {
    fn metered(charge: F) -> Self {
        Self {
            spent: 0,
            ceiling: u64::MAX,
            charge,
        }
    }
    fn tick(&mut self) -> Result<(), E> {
        if self.spent >= self.ceiling {
            return Err(Stop::WorkLimit.into());
        }
        (self.charge)()?;
        self.spent += 1;
        Ok(())
    }
}

/// The shape of a theory a narrowing walks: each maximal tree of one
/// connective read as one node, a *chain*, with its operands; the parents
/// of each node that is not inside a chain, an implication or a chain root
/// reading it as an operand; the nodes carrying each atom; and the atoms
/// among each node's operands. Built once per theory in one pass over its
/// nodes.
///
/// A node is absorbed into its parent's chain when it has that one parent,
/// the same connective, and is not a root of the theory; every other
/// conjunction or disjunction is the root of its own chain, of two operands
/// at least. An absorbed node has no knowledge of its own: the chain's
/// readings are the n-ary readings of its operands (`FormulaChains`), and
/// a node false under a frozen mask is either an operand, which fails, or
/// an absorbed node whose operands all fail (disjunction) or one of which
/// fails (conjunction), so the operands' masks already read it.
#[derive(Clone, Debug)]
pub struct Narrower {
    parents: Vec<Vec<usize>>,
    atom_nodes: Vec<Vec<usize>>,
    chains: Vec<Chain>,
    /// The chain a node is the root of.
    chain_of: Vec<Option<usize>>,
    /// Whether a node is inside a chain, with no knowledge of its own.
    absorbed: Vec<bool>,
    /// The atoms among a node's operands, for the split ranking.
    atom_operands: Vec<Vec<usize>>,
}

/// A maximal tree of one connective, read as one node over its operands.
#[derive(Clone, Debug)]
struct Chain {
    disjunction: bool,
    root: usize,
    /// The leaves, distinct nodes, in operand order.
    operands: Vec<usize>,
}

/// The chains of a theory: each conjunction or disjunction not absorbed
/// into its parent is a root, with the chain it roots and, per node,
/// whether the node is absorbed. Operands precede their parents, so a
/// node's chain is complete when its parent is reached: a same-connective
/// operand that is not a root of the theory and has this one parent joins
/// the parent's chain, its own dissolving into it.
fn chains(theory: &Theory) -> (Vec<Chain>, Vec<Option<usize>>, Vec<bool>) {
    let nodes = theory.nodes();
    let mut roots = vec![false; nodes.len()];
    for &root in theory.roots() {
        roots[root] = true;
    }
    let mut parent_count = vec![0usize; nodes.len()];
    for node in nodes {
        match *node {
            Node::Atom(_) | Node::False => {}
            Node::And(a, b) | Node::Or(a, b) | Node::Implies(a, b) => {
                parent_count[a] += 1;
                if b != a {
                    parent_count[b] += 1;
                }
            }
        }
    }
    let mut built: Vec<Chain> = Vec::new();
    let mut chain_of: Vec<Option<usize>> = vec![None; nodes.len()];
    let mut absorbed = vec![false; nodes.len()];
    for (index, node) in nodes.iter().enumerate() {
        let (disjunction, a, b) = match *node {
            Node::Or(a, b) => (true, a, b),
            Node::And(a, b) => (false, a, b),
            _ => continue,
        };
        let mut operands = Vec::new();
        // A node reached through both sides is one operand, seen once: seen
        // twice, an absorbed one would return as an opaque second operand,
        // its chain already dissolved, and the chain would wait for
        // knowledge it never gets.
        let sides: &[usize] = if b == a { &[a] } else { &[a, b] };
        for &operand in sides {
            let inner = if roots[operand] || parent_count[operand] != 1 {
                None
            } else {
                chain_of[operand].filter(|&k| built[k].disjunction == disjunction)
            };
            // The operands stay distinct: a node reached twice, through
            // both sides, is one operand, as it is one node of the DAG.
            if let Some(inner) = inner {
                chain_of[operand] = None;
                absorbed[operand] = true;
                for leaf in std::mem::take(&mut built[inner].operands) {
                    if !operands.contains(&leaf) {
                        operands.push(leaf);
                    }
                }
            } else if !operands.contains(&operand) {
                operands.push(operand);
            }
        }
        chain_of[index] = Some(built.len());
        built.push(Chain {
            disjunction,
            root: index,
            operands,
        });
    }
    // Dissolved chains keep their slot, emptied; renumber the live ones.
    let mut live = Vec::with_capacity(built.len());
    let mut renumbered = vec![None; built.len()];
    for (old, chain) in built.into_iter().enumerate() {
        if !absorbed[chain.root] {
            renumbered[old] = Some(live.len());
            live.push(chain);
        }
    }
    for entry in &mut chain_of {
        *entry = entry.and_then(|old| renumbered[old]);
    }
    (live, chain_of, absorbed)
}

/// What a narrowing knows about a region, carried from a region to its
/// children: the knowledge of a region holds in every region inside it
/// (`FormulaBounds.known_mono`), so a child's narrowing starts from its
/// parent's knowledge and learns only what the split decided. A fresh
/// value knows nothing and is seeded in full on first use.
///
/// A value is linear in the theory: a bit pair over the nodes and one over
/// the atoms, two counters per chain, a count per atom and the worklists.
/// A split that offers one child to another worker clones it, so the
/// clone is the split's cost. A knowledge belongs to the narrower that
/// made it and to the region it was closed for: narrowing a region with a
/// knowledge from another narrower, theory, producer set or frozen mask,
/// or from a region not enclosing it, is unsound, and nothing checks it.
/// The proposers keep the invariant by carrying each region's knowledge
/// from its parent and by narrowing with the narrower that made it.
#[derive(Clone, Debug)]
pub struct Knowledge {
    known: Known,
}

impl Knowledge {
    /// Header and owned vector capacities in bytes, including empty worklists'
    /// retained capacity. The shared theory, narrower and producer index are
    /// excluded, as are allocator bookkeeping and temporary clones.
    #[must_use]
    pub fn retained_bytes(&self) -> u128 {
        let known = &self.known;
        let booleans = [
            known.sure.capacity(),
            known.never.capacity(),
            known.atom_sure.capacity(),
            known.atom_never.capacity(),
        ];
        let indices = [
            known.sure_operands.capacity(),
            known.never_operands.capacity(),
            known.unknown.capacity(),
            known.learned.capacity(),
            known.heads.capacity(),
        ];
        size_of::<Self>() as u128
            + booleans.into_iter().map(|n| n as u128).sum::<u128>() * size_of::<bool>() as u128
            + indices.into_iter().map(|n| n as u128).sum::<u128>() * size_of::<usize>() as u128
            + known.nodes.capacity() as u128 * size_of::<(usize, bool)>() as u128
    }
}

#[cfg(test)]
#[path = "../tests/support/knowledge_storage.rs"]
mod knowledge_storage_tests;

impl Narrower {
    /// Index the theory's DAG for narrowing.
    #[must_use]
    pub fn new(theory: &Theory) -> Self {
        let nodes = theory.nodes();
        let (chains, chain_of, absorbed) = chains(theory);
        let mut parents = vec![Vec::new(); nodes.len()];
        let mut atom_operands = vec![Vec::new(); nodes.len()];
        let atom_of = |node: usize| match nodes[node] {
            Node::Atom(atom) => Some(atom),
            _ => None,
        };
        for chain in &chains {
            for &operand in &chain.operands {
                parents[operand].push(chain.root);
                if let Some(atom) = atom_of(operand) {
                    atom_operands[chain.root].push(atom);
                }
            }
        }
        for (index, node) in nodes.iter().enumerate() {
            if let Node::Implies(a, b) = *node {
                // An implication from a node to itself is one parent of it,
                // counted once here and once among the atom operands.
                let operands: &[usize] = if b == a { &[a] } else { &[a, b] };
                for &operand in operands {
                    parents[operand].push(index);
                    if let Some(atom) = atom_of(operand) {
                        atom_operands[index].push(atom);
                    }
                }
            }
        }
        let mut atom_nodes = vec![Vec::new(); theory.atom_count()];
        for (index, node) in nodes.iter().enumerate() {
            if let Node::Atom(atom) = *node {
                atom_nodes[atom].push(index);
            }
        }
        Self {
            parents,
            atom_nodes,
            chains,
            chain_of,
            absorbed,
            atom_operands,
        }
    }

    /// The work indexing charged: one visit per node.
    #[must_use]
    pub fn work(&self) -> u64 {
        self.parents.len() as u64
    }

    /// Knowledge of nothing, for the root of a tree over this theory.
    #[must_use]
    pub fn knowledge(&self) -> Knowledge {
        let mut unknown = vec![0usize; self.atom_nodes.len()];
        for (atom, nodes) in self.atom_nodes.iter().enumerate() {
            for &node in nodes {
                unknown[atom] += self.parents[node].len();
            }
        }
        Knowledge {
            known: Known::empty(self.parents.len(), self.chains.len(), unknown),
        }
    }

    /// Narrow the region to the fixed point of the closure's rules from
    /// what is already known about it, [`Self::knowledge`] for the root and
    /// the parent's knowledge for a child, learning only the decisions the
    /// knowledge has not seen, and leave the knowledge closed for the
    /// region's children.
    ///
    /// `knowledge` is this narrower's, closed for a region enclosing this
    /// one under the same `producers` and `frozen` mask, as [`Knowledge`]
    /// states; the narrowing cannot tell a foreign knowledge from its own.
    ///
    /// # Errors
    /// Returns the stop when the narrowing exceeds its work or propagation
    /// ceiling, or control stops it. The region then holds the decisions the
    /// closure had learned before the stop, and the knowledge is partly
    /// closed: both sound, neither to be reused; the proposers abandon the
    /// region.
    pub fn narrow_known(
        &self,
        theory: &Theory,
        producers: Option<&Producers>,
        region: &mut Region,
        knowledge: &mut Knowledge,
        limits: RegionLimits,
        cancellation: &Cancellation,
    ) -> Result<(Narrowing, NarrowingStatistics), Stop> {
        let subject = Subject {
            theory,
            producers,
            frozen: None,
        };
        let attempt = self.narrow_with(
            subject,
            region,
            knowledge,
            Work::new(limits.max_work),
            cancellation,
        );
        attempt
            .result
            .map(|narrowing| (narrowing, attempt.statistics))
    }

    /// Narrow original candidates with a caller-owned work quota. The quota
    /// is invoked before every charged node, parent, producer or open-atom read;
    /// a refused permit prevents that read. It owns the work ceiling and may
    /// also poll control. This operation polls `cancellation` before any mutation,
    /// including when no charged read is necessary. The receipt counts only
    /// successful permits and survives every returned failure.
    ///
    /// The ownership and ancestor-knowledge preconditions of
    /// [`Self::narrow_known`] still apply. Any failed attempt's region and
    /// knowledge must be abandoned. The quota's error is preserved; entry
    /// control failures and exhaustion of the representable `u64` work count
    /// use `E::from(Stop)`.
    pub fn narrow_known_metered<E: From<Stop>>(
        &self,
        theory: &Theory,
        producers: Option<&Producers>,
        region: &mut Region,
        knowledge: &mut Knowledge,
        cancellation: &Cancellation,
        charge: impl FnMut() -> Result<(), E>,
    ) -> NarrowingAttempt<E> {
        let subject = Subject {
            theory,
            producers,
            frozen: None,
        };
        self.narrow_with(
            subject,
            region,
            knowledge,
            Work::metered(charge),
            cancellation,
        )
    }

    /// Narrow a region of the theory's frozen reduct under a candidate from
    /// what is already known about it, as [`Self::narrow_known`] does for
    /// the candidate tree: a node false in `truth`, the candidate's truth of
    /// every node, reads as falsum (`FerrarisMask`), and the rest of the DAG
    /// is read unchanged. No support cut applies, since a model of the
    /// reduct need not be supported: this narrows the proper-subset query,
    /// not the candidate tree. `knowledge` is this narrower's, closed for an
    /// enclosing region of the same query under the same `truth`, with the
    /// precondition [`Knowledge`] states.
    ///
    /// # Errors
    /// As [`Self::narrow_known`].
    pub fn narrow_frozen_known(
        &self,
        theory: &Theory,
        truth: &[bool],
        region: &mut Region,
        knowledge: &mut Knowledge,
        limits: RegionLimits,
        cancellation: &Cancellation,
    ) -> Result<(Narrowing, NarrowingStatistics), Stop> {
        let subject = Subject {
            theory,
            producers: None,
            frozen: Some(truth),
        };
        let attempt = self.narrow_with(
            subject,
            region,
            knowledge,
            Work::new(limits.max_work),
            cancellation,
        );
        attempt
            .result
            .map(|narrowing| (narrowing, attempt.statistics))
    }

    /// Narrow a frozen reduct with the quota and failure receipt contract of
    /// [`Self::narrow_known_metered`]. The frozen mask and ancestor knowledge
    /// retain the preconditions of [`Self::narrow_frozen_known`].
    pub fn narrow_frozen_known_metered<E: From<Stop>>(
        &self,
        theory: &Theory,
        truth: &[bool],
        region: &mut Region,
        knowledge: &mut Knowledge,
        cancellation: &Cancellation,
        charge: impl FnMut() -> Result<(), E>,
    ) -> NarrowingAttempt<E> {
        let subject = Subject {
            theory,
            producers: None,
            frozen: Some(truth),
        };
        self.narrow_with(
            subject,
            region,
            knowledge,
            Work::metered(charge),
            cancellation,
        )
    }

    fn narrow_with<E: From<Stop>>(
        &self,
        subject: Subject<'_>,
        region: &mut Region,
        knowledge: &mut Knowledge,
        mut work: Work<impl FnMut() -> Result<(), E>>,
        cancellation: &Cancellation,
    ) -> NarrowingAttempt<E> {
        let mut statistics = NarrowingStatistics::default();
        let result = (|| {
            cancellation.poll().map_err(E::from)?;
            let known = &mut knowledge.known;
            if known.close(subject, self, region, &mut work, &mut statistics)?
                == Step::Contradiction
            {
                return Ok(Narrowing::Refuted);
            }
            // The atoms this closure learned decide the region; the region's own
            // decisions, the split's and those made here, are then all seen.
            let mut changed = false;
            for atom in known.learned.drain(..) {
                let was_open = region.is_open(atom);
                let decided = if known.atom_sure[atom] {
                    statistics.held += u64::from(was_open);
                    region.hold(atom)
                } else {
                    statistics.cut += u64::from(was_open);
                    region.cut(atom)
                };
                debug_assert!(decided, "a learned atom agrees with the region");
                changed |= was_open;
            }
            known.seen = region.decisions().len();
            if let Some(atom) = most_constrained(region, known, &mut work)? {
                region.prefer(atom);
            }
            Ok(Narrowing::Fixed { changed })
        })();
        statistics.work = work.spent;
        NarrowingAttempt { result, statistics }
    }
}

/// The open atom with the most parents still unknown, the one whose
/// decision the theory is most sensitive to; ties go to the lower atom.
/// This chooses the split, as the clause search branches on the variable
/// with the most unresolved occurrences. The counts are kept as parents
/// become known, so the ranking is one read per open atom.
fn most_constrained<E: From<Stop>>(
    region: &Region,
    known: &Known,
    work: &mut Work<impl FnMut() -> Result<(), E>>,
) -> Result<Option<usize>, E> {
    let mut best: Option<(usize, usize)> = None;
    for atom in region.open() {
        work.tick()?;
        let unknown = known.unknown[atom];
        if best.is_none_or(|(_, count)| unknown > count) {
            best = Some((atom, unknown));
        }
    }
    Ok(best.map(|(atom, _)| atom))
}

/// What every candidate of the region must make of each node and each
/// atom: known to hold, known to fail, or open. One bit pair over the
/// nodes, one over the atoms, and two counters over each chain, the
/// operands known to hold and the operands known to fail, closed under the
/// upward rules from a node's operands and the downward rules from a node's
/// own knowledge until nothing changes: unit propagation on the theory
/// itself, with a node shared by several parents known once for all of
/// them (`FormulaBounds.Known`, `known_sound`; the chain rules are
/// `FormulaChains`). The closure is driven by a worklist: a node that
/// learns something is revisited once, and only its parents, operands and
/// dependent producers are read, a chain learning from an operand by one
/// counter step.
#[derive(Clone, Debug)]
struct Known {
    sure: Vec<bool>,
    never: Vec<bool>,
    atom_sure: Vec<bool>,
    atom_never: Vec<bool>,
    /// Per chain, the operands known to hold.
    sure_operands: Vec<usize>,
    /// Per chain, the operands known to fail.
    never_operands: Vec<usize>,
    /// Per atom, the parents of its nodes not yet known: the split ranking.
    /// A parent is counted once here and taken off once when it is
    /// revisited, so the count never goes below zero.
    unknown: Vec<usize>,
    /// The atoms this closure decided, not yet told to the region.
    learned: Vec<usize>,
    /// Nodes that learned something, with what, and have not been revisited.
    nodes: Vec<(usize, bool)>,
    /// Atoms whose support must be rechecked; queued only when producers
    /// are known, since only they say what supports an atom.
    heads: Vec<usize>,
    /// How many of the region's decisions, in the order made, are known.
    seen: usize,
    /// The roots, falsum and every atom's support have been seeded once;
    /// later closures learn only decisions not yet known.
    seeded: bool,
}

/// What a step of the closure did.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Step {
    Unchanged,
    Changed,
    /// A node or atom became known both to hold and to fail.
    Contradiction,
}

impl Step {
    fn join(self, other: Self) -> Self {
        match (self, other) {
            (Self::Contradiction, _) | (_, Self::Contradiction) => Self::Contradiction,
            (Self::Changed, _) | (_, Self::Changed) => Self::Changed,
            (Self::Unchanged, Self::Unchanged) => Self::Unchanged,
        }
    }
}

fn learn(known: &mut [bool], opposite: &[bool], index: usize) -> Step {
    if known[index] {
        Step::Unchanged
    } else if opposite[index] {
        Step::Contradiction
    } else {
        known[index] = true;
        Step::Changed
    }
}

impl Known {
    fn empty(nodes: usize, chains: usize, unknown: Vec<usize>) -> Self {
        Self {
            sure: vec![false; nodes],
            never: vec![false; nodes],
            atom_sure: vec![false; unknown.len()],
            atom_never: vec![false; unknown.len()],
            sure_operands: vec![0; chains],
            never_operands: vec![0; chains],
            unknown,
            learned: Vec::new(),
            nodes: Vec::new(),
            heads: Vec::new(),
            seen: 0,
            seeded: false,
        }
    }

    /// A node learns to hold; it is revisited if that is new.
    fn sure(&mut self, index: usize) -> Step {
        let step = learn(&mut self.sure, &self.never, index);
        if step == Step::Changed {
            self.nodes.push((index, true));
        }
        step
    }

    /// A node learns to fail; it is revisited if that is new.
    fn never(&mut self, index: usize) -> Step {
        let step = learn(&mut self.never, &self.sure, index);
        if step == Step::Changed {
            self.nodes.push((index, false));
        }
        step
    }

    /// An atom learns to hold or to fail, and every node carrying it
    /// learns the same. A newly held atom has its support rechecked when
    /// producers are known.
    fn atom(
        &mut self,
        index: &Narrower,
        producers: Option<&Producers>,
        atom: usize,
        value: bool,
    ) -> Step {
        let step = if value {
            learn(&mut self.atom_sure, &self.atom_never, atom)
        } else {
            learn(&mut self.atom_never, &self.atom_sure, atom)
        };
        if step != Step::Changed {
            return step;
        }
        self.learned.push(atom);
        let mut step = step;
        for &node in &index.atom_nodes[atom] {
            step = step.join(if value {
                self.sure(node)
            } else {
                self.never(node)
            });
        }
        if value && producers.is_some() {
            self.heads.push(atom);
        }
        step
    }

    /// Close the knowledge from the region's decisions, falsum and the
    /// roots. Each event on the worklist follows a new bit, or is one of
    /// the initial seeds, so the events are bounded by the bits.
    fn close<E: From<Stop>>(
        &mut self,
        subject: Subject<'_>,
        index: &Narrower,
        region: &Region,
        work: &mut Work<impl FnMut() -> Result<(), E>>,
        statistics: &mut NarrowingStatistics,
    ) -> Result<Step, E> {
        let Subject {
            theory,
            producers,
            frozen,
        } = subject;
        let nodes = theory.nodes();
        let mut step = Step::Unchanged;
        if !self.seeded {
            for (node, kind) in nodes.iter().enumerate() {
                if index.absorbed[node] {
                    continue;
                }
                let falsum =
                    matches!(kind, Node::False) || frozen.is_some_and(|truth| !truth[node]);
                if falsum {
                    step = step.join(self.never(node));
                }
            }
            for &root in theory.roots() {
                step = step.join(self.sure(root));
            }
            if producers.is_some() {
                self.heads.extend(0..theory.atom_count());
            }
            self.seeded = true;
        }
        for &atom in &region.decisions()[self.seen..] {
            let value = region.decision(atom).expect("a decided atom is decided");
            step = step.join(self.atom(index, producers, atom, value));
        }
        self.seen = region.decisions().len();
        if step == Step::Contradiction {
            return Ok(step);
        }
        loop {
            let step = if let Some((node, value)) = self.nodes.pop() {
                statistics.propagations += 1;
                self.revisit(subject, index, node, value, work)?
            } else if let Some(atom) = self.heads.pop() {
                statistics.propagations += 1;
                let producers =
                    producers.expect("a support recheck is queued only when producers are known");
                self.recheck(index, producers, atom, work)?
            } else {
                return Ok(Step::Changed);
            };
            if step == Step::Contradiction {
                return Ok(step);
            }
        }
    }

    /// A node that learned something teaches its operands, and lets each
    /// parent learn from it: a chain by one counter step, an implication
    /// from both its operands and, when already known, by teaching them
    /// again, since what it leaves them may have narrowed. A node false
    /// under a frozen mask is falsum in the reduct, a constant with no
    /// operands: it teaches nothing and learns nothing from them, and a
    /// parent under the mask likewise.
    fn revisit<E: From<Stop>>(
        &mut self,
        subject: Subject<'_>,
        index: &Narrower,
        node: usize,
        value: bool,
        work: &mut Work<impl FnMut() -> Result<(), E>>,
    ) -> Result<Step, E> {
        work.tick()?;
        let Subject {
            theory,
            producers,
            frozen,
        } = subject;
        let nodes = theory.nodes();
        let masked = |node: usize| frozen.is_some_and(|truth| !truth[node]);
        for &atom in &index.atom_operands[node] {
            self.unknown[atom] = self.unknown[atom]
                .checked_sub(1)
                .expect("a parent is counted before it is revisited");
        }
        let mut step = Step::Unchanged;
        if !masked(node) {
            step = step.join(self.teach_operands(nodes, index, producers, node));
        }
        for &parent in &index.parents[node] {
            work.tick()?;
            if masked(parent) {
                continue;
            }
            step = step.join(if let Some(chain) = index.chain_of[parent] {
                self.operand_changed(index, chain, value)
            } else {
                let up = self.learn_from_operands(nodes, parent);
                if self.sure[parent] || self.never[parent] {
                    up.join(self.teach_operands(nodes, index, producers, parent))
                } else {
                    up
                }
            });
        }
        Ok(step)
    }

    /// A chain learns from an operand that became known: one more operand
    /// holds or fails. A disjunction holds with one, fails with all, and a
    /// disjunction known to hold with all but one failing forces that one;
    /// a conjunction dually (`disj_chain_sure`, `disj_chain_never`,
    /// `disj_chain_unit` and the conjunction laws).
    fn operand_changed(&mut self, index: &Narrower, chain: usize, value: bool) -> Step {
        let Chain {
            disjunction,
            root,
            ref operands,
        } = index.chains[chain];
        let total = operands.len();
        if value {
            self.sure_operands[chain] += 1;
        } else {
            self.never_operands[chain] += 1;
        }
        let (sure, never) = (self.sure_operands[chain], self.never_operands[chain]);
        match (disjunction, value) {
            (true, true) => self.sure(root),
            (true, false) if never == total => self.never(root),
            (true, false) if self.sure[root] && never + 1 == total => self.unit(index, chain),
            (false, false) => self.never(root),
            (false, true) if sure == total => self.sure(root),
            (false, true) if self.never[root] && sure + 1 == total => self.unit(index, chain),
            _ => Step::Unchanged,
        }
    }

    /// The one operand of a chain not yet known learns what the chain's
    /// own knowledge leaves it: to hold, in a disjunction known to hold
    /// whose others fail; to fail, in a conjunction known to fail whose
    /// others hold. The operands are scanned once for it; a node's bit is
    /// set when it learns and counted when it is revisited, so the scan
    /// may find none, every operand being known with one count pending,
    /// and then the pending step decides the chain.
    fn unit(&mut self, index: &Narrower, chain: usize) -> Step {
        let Chain {
            disjunction,
            ref operands,
            ..
        } = index.chains[chain];
        let open = operands.iter().copied().find(|&operand| {
            if disjunction {
                !self.never[operand]
            } else {
                !self.sure[operand]
            }
        });
        match open {
            Some(operand) if disjunction => self.sure(operand),
            Some(operand) => self.never(operand),
            None => Step::Unchanged,
        }
    }

    /// A known node teaches its operands what its knowledge leaves them,
    /// tells its atom, and, when it is a body that fails, has the
    /// producers' heads rechecked. A chain known to hold forces its one
    /// open operand (disjunction) or every operand (conjunction); known to
    /// fail, every operand (disjunction) or its one open operand
    /// (conjunction).
    fn teach_operands(
        &mut self,
        nodes: &[Node],
        index: &Narrower,
        producers: Option<&Producers>,
        node: usize,
    ) -> Step {
        let mut step = Step::Unchanged;
        if let Some(chain) = index.chain_of[node] {
            step = step.join(self.teach_chain(index, chain));
        }
        if self.sure[node] {
            step = step.join(match nodes[node] {
                Node::Atom(atom) => {
                    // A held head blocks the other heads of its producers.
                    if let Some(producers) = producers {
                        for &producer in &producers.by_head[atom] {
                            self.heads.extend(
                                producers.rules[producer]
                                    .heads
                                    .iter()
                                    .copied()
                                    .filter(|&head| head != atom),
                            );
                        }
                    }
                    self.atom(index, producers, atom, true)
                }
                Node::False => Step::Contradiction,
                Node::And(..) | Node::Or(..) => Step::Unchanged,
                Node::Implies(a, b) => {
                    if self.sure[a] {
                        self.sure(b)
                    } else if self.never[b] {
                        self.never(a)
                    } else {
                        Step::Unchanged
                    }
                }
            });
        }
        if self.never[node] {
            step = step.join(match nodes[node] {
                Node::Atom(atom) => self.atom(index, producers, atom, false),
                Node::False | Node::And(..) | Node::Or(..) => Step::Unchanged,
                Node::Implies(a, b) => self.sure(a).join(self.never(b)),
            });
            if let Some(producers) = producers {
                for &producer in &producers.by_body[node] {
                    self.heads
                        .extend(producers.rules[producer].heads.iter().copied());
                }
            }
        }
        step
    }

    /// What a chain's own knowledge leaves its operands.
    fn teach_chain(&mut self, index: &Narrower, chain: usize) -> Step {
        let Chain {
            disjunction,
            root,
            ref operands,
        } = index.chains[chain];
        let total = operands.len();
        let mut step = Step::Unchanged;
        if self.sure[root] {
            if disjunction {
                if self.never_operands[chain] + 1 == total {
                    step = step.join(self.unit(index, chain));
                }
            } else {
                for &operand in operands {
                    step = step.join(self.sure(operand));
                }
            }
        }
        if self.never[root] {
            if disjunction {
                for &operand in operands {
                    step = step.join(self.never(operand));
                }
            } else if self.sure_operands[chain] + 1 == total {
                step = step.join(self.unit(index, chain));
            }
        }
        step
    }

    /// An implication learns from its operands what the connective
    /// dictates; chains learn by their counters.
    fn learn_from_operands(&mut self, nodes: &[Node], node: usize) -> Step {
        match nodes[node] {
            Node::Implies(a, b) => {
                let mut up = Step::Unchanged;
                if self.never[a] || self.sure[b] {
                    up = up.join(self.sure(node));
                }
                if self.sure[a] && self.never[b] {
                    up = up.join(self.never(node));
                }
                up
            }
            Node::Atom(_) | Node::False | Node::And(..) | Node::Or(..) => Step::Unchanged,
        }
    }

    /// An atom none of its producers can support is known to fail, and an
    /// atom known to hold with exactly one producer able to support it
    /// forces that producer's body (`unsupported_cut`,
    /// `sole_support_forces`). A producer can support its atom when its
    /// body is not known to fail and, unless it is a choice, no other of
    /// its heads is known to hold.
    fn recheck<E: From<Stop>>(
        &mut self,
        index: &Narrower,
        producers: &Producers,
        atom: usize,
        work: &mut Work<impl FnMut() -> Result<(), E>>,
    ) -> Result<Step, E> {
        if self.atom_never[atom] {
            return Ok(Step::Unchanged);
        }
        let mut supporters = 0;
        let mut sole = None;
        for &producer in &producers.by_head[atom] {
            work.tick()?;
            let producer = &producers.rules[producer];
            let body_impossible = producer.body.is_some_and(|body| self.never[body]);
            let other_held = !producer.choice
                && producer
                    .heads
                    .iter()
                    .any(|&head| head != atom && self.atom_sure[head]);
            if !body_impossible && !other_held {
                supporters += 1;
                sole = producer.body;
            }
        }
        Ok(match (supporters, sole) {
            (0, _) => self.atom(index, Some(producers), atom, false),
            (1, Some(body)) if self.atom_sure[atom] => self.sure(body),
            _ => Step::Unchanged,
        })
    }
}
