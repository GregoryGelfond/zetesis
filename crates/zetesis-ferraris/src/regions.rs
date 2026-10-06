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
use std::num::NonZeroUsize;

use zetesis_cpu::regions::{Narrowing, Region};
use zetesis_cpu::{Cancellation, Stop};

use crate::{Node, Theory};

mod adjacency;
mod counters;
use adjacency::Adjacency;
use counters::{Count, Counters, compact_fits};

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

/// A theory read for its candidates, with the producers of its support
/// fragment when the support cut applies: what original narrowing reads.
#[derive(Clone, Copy, Debug)]
pub struct OriginalSubject<'a> {
    theory: &'a Theory,
    producers: Option<&'a Producers>,
}

impl<'a> OriginalSubject<'a> {
    /// The candidates of `theory`, cut by `producers`' support when given.
    #[must_use]
    pub const fn new(theory: &'a Theory, producers: Option<&'a Producers>) -> Self {
        Self { theory, producers }
    }
}

impl<'a> From<OriginalSubject<'a>> for Subject<'a> {
    fn from(subject: OriginalSubject<'a>) -> Self {
        Self {
            theory: subject.theory,
            producers: subject.producers,
            frozen: None,
        }
    }
}

/// A theory read as a candidate's reduct: a node false in `truth`, the
/// candidate's truth of every node, reads as falsum (`FerrarisMask`). What
/// frozen narrowing reads.
#[derive(Clone, Copy, Debug)]
pub struct FrozenSubject<'a> {
    theory: &'a Theory,
    truth: &'a [bool],
}

impl<'a> FrozenSubject<'a> {
    /// The reduct of `theory` under the candidate whose node truth is `truth`.
    #[must_use]
    pub const fn new(theory: &'a Theory, truth: &'a [bool]) -> Self {
        Self { theory, truth }
    }
}

impl<'a> From<FrozenSubject<'a>> for Subject<'a> {
    fn from(subject: FrozenSubject<'a>) -> Self {
        Self {
            theory: subject.theory,
            producers: None,
            frozen: Some(subject.truth),
        }
    }
}

/// The producers of a theory in the support fragment, by head atom.
#[derive(Clone, Debug)]
pub struct Producers {
    rules: Vec<Producer>,
    by_head: Adjacency,
    /// The producers whose body is this node.
    by_body: Adjacency,
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
/// Returns the stop when extraction exceeds `limits.max_work`, control stops,
/// or compact-adjacency storage cannot be represented or reserved. Other
/// extraction storage retains its existing infallible allocation behavior.
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

fn extract(theory: &Theory, work: &mut Work<'_>) -> Result<Option<Producers>, Stop> {
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
        rules.push(Producer {
            body,
            heads,
            choice,
        });
    }
    let by_head = Adjacency::build(
        theory.atom_count(),
        rules
            .iter()
            .enumerate()
            .flat_map(|(index, producer)| producer.heads.iter().map(move |&atom| (atom, index))),
    )?;
    let by_body = Adjacency::build(
        nodes.len(),
        rules
            .iter()
            .enumerate()
            .filter_map(|(index, producer)| producer.body.map(|body| (body, index))),
    )?;
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

/// A caller-owned allowance for one narrowing's charged reads, granted in
/// batches. The narrowing asks for at most [`NARROWING_BATCH`] permits at a
/// time, spends them one per charged read, and returns the unspent rest when
/// it ends, whatever its outcome; so the permits it keeps are exactly the
/// reads it made.
pub trait NarrowingQuota {
    /// Grant between one and `wanted` permits, or refuse. A quota that polls
    /// cancellation or a deadline does so here, so control is observed at
    /// least once every [`NARROWING_BATCH`] charged reads. A refusal stops
    /// the narrowing before the read that asked; a grant of zero is a refusal.
    ///
    /// # Errors
    /// The stop that refuses the permits.
    fn reserve(&mut self, wanted: u64) -> Result<u64, Stop>;
    /// Take back permits granted to this narrowing and not spent.
    fn refund(&mut self, unspent: u64);
}

/// The most charged reads a narrowing asks its [`NarrowingQuota`] for at once,
/// and so the most reads between two consultations of it.
pub const NARROWING_BATCH: u64 = 256;

/// The narrowing's charged reads: a fixed ceiling, or permits granted by a
/// quota in batches and spent locally.
struct Work<'q> {
    spent: u64,
    ceiling: u64,
    available: u64,
    quota: Option<&'q mut dyn NarrowingQuota>,
}
impl Work<'static> {
    fn new(ceiling: u64) -> Self {
        Self {
            spent: 0,
            ceiling,
            available: 0,
            quota: None,
        }
    }
}
impl<'q> Work<'q> {
    fn reserved(quota: &'q mut dyn NarrowingQuota) -> Self {
        Self {
            spent: 0,
            ceiling: u64::MAX,
            available: 0,
            quota: Some(quota),
        }
    }
    #[inline]
    fn tick(&mut self) -> Result<(), Stop> {
        if self.spent >= self.ceiling {
            return Err(Stop::WorkLimit);
        }
        if let Some(quota) = &mut self.quota {
            if self.available == 0 {
                self.available = quota.reserve(NARROWING_BATCH)?;
                if self.available == 0 {
                    return Err(Stop::WorkLimit);
                }
            }
            self.available -= 1;
        }
        self.spent += 1;
        Ok(())
    }
    /// Return the unspent permits to the quota.
    fn settle(&mut self) {
        if let Some(quota) = &mut self.quota {
            quota.refund(std::mem::take(&mut self.available));
        }
    }
}

/// A caller's per-read charge as a quota: one permit per call, and the
/// caller's own refusal kept to be returned unchanged.
struct PerRead<F, E> {
    charge: F,
    failure: Option<E>,
}
impl<E, F: FnMut() -> Result<(), E>> NarrowingQuota for PerRead<F, E> {
    fn reserve(&mut self, _wanted: u64) -> Result<u64, Stop> {
        match (self.charge)() {
            Ok(()) => Ok(1),
            Err(error) => {
                self.failure = Some(error);
                Err(Stop::WorkLimit)
            }
        }
    }
    fn refund(&mut self, unspent: u64) {
        debug_assert_eq!(unspent, 0, "a per-read permit is spent as it is granted");
    }
}

/// The shape of a theory a narrowing walks: each maximal tree of one
/// connective read as one node, a *chain*, with its operands; the parents
/// of each node that is not inside a chain, an implication or a chain root
/// reading it as an operand; the nodes carrying each atom; and the atoms
/// among each node's operands. Built once per theory from its nodes. The three
/// immutable incidence maps store ordered rows as offsets
/// into contiguous entry vectors, retaining every occurrence already chosen by
/// chain formation. Compaction adds only linear construction passes.
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
    parents: Adjacency,
    atom_nodes: Adjacency,
    chains: Vec<Chain>,
    /// The chain a node roots, stored one-based so absence needs no extra word.
    chain_of: Vec<Option<NonZeroUsize>>,
    /// Whether a node is inside a chain, with no knowledge of its own.
    absorbed: Vec<bool>,
    /// The atoms among a node's operands, for the split ranking.
    atom_operands: Adjacency,
}

/// A maximal tree of one connective, read as one node over its operands.
#[derive(Clone, Debug)]
struct Chain {
    disjunction: bool,
    root: usize,
    /// The leaves, distinct nodes, in operand order.
    operands: Vec<usize>,
}

/// A one-based chain link. Both construction maps point into allocated chain
/// vectors, whose positions cannot reach `usize::MAX`; checked encoding retains
/// that invariant without imposing a smaller theory-size admission limit.
fn encoded_chain(position: usize) -> NonZeroUsize {
    NonZeroUsize::new(position.checked_add(1).expect("allocated chain position"))
        .expect("positive chain link")
}

/// Recover the zero-based position used by chain operands and counters.
fn chain_position(link: NonZeroUsize) -> usize {
    link.get() - 1
}

/// The chains of a theory: each conjunction or disjunction not absorbed
/// into its parent is a root, with the chain it roots and, per node,
/// whether the node is absorbed. Operands precede their parents, so a
/// node's chain is complete when its parent is reached: a same-connective
/// operand that is not a root of the theory and has this one parent joins
/// the parent's chain, its own dissolving into it.
fn chains(theory: &Theory) -> (Vec<Chain>, Vec<Option<NonZeroUsize>>, Vec<bool>) {
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
    let mut chain_of = vec![None; nodes.len()];
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
                chain_of[operand]
                    .map(chain_position)
                    .filter(|&k| built[k].disjunction == disjunction)
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
        chain_of[index] = Some(encoded_chain(built.len()));
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
            renumbered[old] = Some(encoded_chain(live.len()));
            live.push(chain);
        }
    }
    for entry in &mut chain_of {
        *entry = entry.and_then(|old| renumbered[chain_position(old)]);
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
/// the atoms, two counters per chain and a count per atom; the worklists of
/// a narrowing belong to the walker's [`NarrowingScratch`]. Its counters
/// have one width, chosen when the root value is made.
/// A split that offers one child to another worker clones it, so the
/// clone is the split's cost. A knowledge belongs to the narrower that
/// made it and to the region it was closed for: narrowing a region with a
/// knowledge from another narrower, theory, producer set or frozen mask,
/// or from a region not enclosing it, is unsound, and nothing checks it.
/// The proposers keep the invariant by carrying each region's knowledge
/// from its parent and by narrowing with the narrower that made it.
#[derive(Clone, Debug)]
pub struct Knowledge {
    width: Width,
}

/// The knowledge at the counter width chosen when it was created: the
/// closure runs on one concrete width, chosen once.
#[derive(Clone, Debug)]
enum Width {
    Compact(Known<u32>),
    Native(Known<usize>),
}

impl Knowledge {
    /// Header, owned flag, seen and counter arrays in bytes. The shared
    /// theory, narrower and producer index are excluded, as are the walker's
    /// scratch, allocator bookkeeping and temporary clones.
    #[must_use]
    pub fn retained_bytes(&self) -> u128 {
        size_of::<Self>() as u128
            + match &self.width {
                Width::Compact(known) => known.retained_bytes(),
                Width::Native(known) => known.retained_bytes(),
            }
    }
}

impl<C: Count> Known<C> {
    /// Payload and capacities owned by this closure state; the enclosing
    /// [`Knowledge`] header is counted by its owner.
    fn retained_bytes(&self) -> u128 {
        let known = self;
        let flag_words = [
            known.sure.len(),
            known.never.len(),
            known.atom_sure.len(),
            known.atom_never.len(),
        ];
        let counters = [&known.sure_operands, &known.never_operands, &known.unknown];
        flag_words.into_iter().map(|n| n as u128).sum::<u128>() * size_of::<u64>() as u128
            + counters
                .into_iter()
                .map(Counters::allocated_bytes)
                .sum::<u128>()
            + known.seen.len() as u128 * size_of::<u64>() as u128
    }
}

#[cfg(test)]
mod tests;

/// The ordered operand-to-parent incidences already chosen by chain formation.
/// Chains precede implications as in the original index; a repeated implication
/// operand is one incidence. Distinct atom nodes carrying the same atom remain
/// distinct occurrences when this stream is projected to atom operands.
fn dependencies<'a>(
    nodes: &'a [Node],
    chains: &'a [Chain],
) -> impl Iterator<Item = (usize, usize)> + Clone + 'a {
    chains
        .iter()
        .flat_map(|chain| {
            chain
                .operands
                .iter()
                .map(move |&operand| (operand, chain.root))
        })
        .chain(nodes.iter().enumerate().flat_map(|(index, node)| {
            let operands = match *node {
                Node::Implies(a, b) => [Some(a), (b != a).then_some(b)],
                _ => [None, None],
            };
            operands
                .into_iter()
                .flatten()
                .map(move |operand| (operand, index))
        }))
}

impl Narrower {
    /// Index the theory's DAG for narrowing. Use [`Self::try_new`] to receive
    /// compact-adjacency capacity and allocation refusals as [`Stop`].
    ///
    /// # Panics
    /// Panics if compact adjacency cannot be represented or reserved. Other
    /// index construction retains its existing infallible allocation behavior.
    #[must_use]
    pub fn new(theory: &Theory) -> Self {
        Self::try_new(theory).expect("region adjacency storage could not be reserved")
    }

    /// Index the same DAG using checked compact adjacency construction.
    /// Each incidence map reads its immutable edge stream twice and uses linear
    /// row scans, preserving row order and duplicates. The existing logical
    /// index receipt remains one visit per theory node; construction passes and
    /// storage initialization are not additional charged propagation reads.
    ///
    /// # Errors
    /// Returns [`Stop::Allocation`] for compact-adjacency offset overflow or
    /// reservation failure. Chain construction and later knowledge allocation
    /// retain their existing infallible behavior; this is not universal OOM
    /// recovery. This constructor has no independent cancellation contract.
    pub fn try_new(theory: &Theory) -> Result<Self, Stop> {
        let nodes = theory.nodes();
        let (chains, chain_of, absorbed) = chains(theory);
        let parents = Adjacency::build(nodes.len(), dependencies(nodes, &chains))?;
        let atom_operands = Adjacency::build(
            nodes.len(),
            dependencies(nodes, &chains).filter_map(|(operand, parent)| match nodes[operand] {
                Node::Atom(atom) => Some((parent, atom)),
                _ => None,
            }),
        )?;
        let atom_nodes = Adjacency::build(
            theory.atom_count(),
            nodes
                .iter()
                .enumerate()
                .filter_map(|(index, node)| match *node {
                    Node::Atom(atom) => Some((atom, index)),
                    _ => None,
                }),
        )?;
        Ok(Self {
            parents,
            atom_nodes,
            chains,
            chain_of,
            absorbed,
            atom_operands,
        })
    }

    /// The work indexing charged: one visit per node.
    #[must_use]
    pub fn work(&self) -> u64 {
        self.parents.len() as u64
    }

    /// Knowledge of nothing, for the root of a tree over this theory.
    #[must_use]
    pub fn knowledge(&self) -> Knowledge {
        // Every chain operand and every parent counted for an atom is an
        // occurrence in this same incidence stream. Its total bounds all
        // three counter arrays; no theory-size or language cap is imposed.
        let incidences = self.parents.entry_count();
        Knowledge {
            width: if compact_fits(incidences) {
                Width::Compact(self.root_known())
            } else {
                Width::Native(self.root_known())
            },
        }
    }

    /// The root's closure state at one counter width.
    fn root_known<C: Count>(&self) -> Known<C> {
        let mut unknown = Counters::zeros(self.atom_nodes.len());
        for (atom, nodes) in self.atom_nodes.iter().enumerate() {
            for &node in nodes {
                unknown.add(atom, self.parents[node].len());
            }
        }
        Known::empty(self.parents.len(), self.chains.len(), unknown)
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
        subject: OriginalSubject<'_>,
        region: &mut Region,
        knowledge: &mut Knowledge,
        scratch: &mut NarrowingScratch,
        limits: RegionLimits,
        cancellation: &Cancellation,
    ) -> Result<(Narrowing, NarrowingStatistics), Stop> {
        let subject = Subject::from(subject);
        let attempt = self.narrow_with(
            subject,
            region,
            knowledge,
            scratch,
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
        subject: OriginalSubject<'_>,
        region: &mut Region,
        knowledge: &mut Knowledge,
        scratch: &mut NarrowingScratch,
        cancellation: &Cancellation,
        charge: impl FnMut() -> Result<(), E>,
    ) -> NarrowingAttempt<E> {
        let subject = Subject::from(subject);
        let mut quota = PerRead {
            charge,
            failure: None,
        };
        let attempt = self.narrow_with(
            subject,
            region,
            knowledge,
            scratch,
            Work::reserved(&mut quota),
            cancellation,
        );
        NarrowingAttempt {
            result: attempt
                .result
                .map_err(|stop| quota.failure.take().unwrap_or_else(|| E::from(stop))),
            statistics: attempt.statistics,
        }
    }

    /// Narrow a region of the theory's frozen reduct under a candidate from
    /// what is already known about it, as [`Self::narrow_known`] does for
    /// the candidate tree: a node false in the subject's truth, the
    /// candidate's truth of every node, reads as falsum (`FerrarisMask`), and
    /// the rest of the DAG is read unchanged. No support cut applies, since a model of the
    /// reduct need not be supported: this narrows the proper-subset query,
    /// not the candidate tree. `knowledge` is this narrower's, closed for an
    /// enclosing region of the same query under the same truth, with the
    /// precondition [`Knowledge`] states.
    ///
    /// # Errors
    /// As [`Self::narrow_known`].
    pub fn narrow_frozen_known(
        &self,
        subject: FrozenSubject<'_>,
        region: &mut Region,
        knowledge: &mut Knowledge,
        scratch: &mut NarrowingScratch,
        limits: RegionLimits,
        cancellation: &Cancellation,
    ) -> Result<(Narrowing, NarrowingStatistics), Stop> {
        let subject = Subject::from(subject);
        let attempt = self.narrow_with(
            subject,
            region,
            knowledge,
            scratch,
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
        subject: FrozenSubject<'_>,
        region: &mut Region,
        knowledge: &mut Knowledge,
        scratch: &mut NarrowingScratch,
        cancellation: &Cancellation,
        charge: impl FnMut() -> Result<(), E>,
    ) -> NarrowingAttempt<E> {
        let subject = Subject::from(subject);
        let mut quota = PerRead {
            charge,
            failure: None,
        };
        let attempt = self.narrow_with(
            subject,
            region,
            knowledge,
            scratch,
            Work::reserved(&mut quota),
            cancellation,
        );
        NarrowingAttempt {
            result: attempt
                .result
                .map_err(|stop| quota.failure.take().unwrap_or_else(|| E::from(stop))),
            statistics: attempt.statistics,
        }
    }

    /// Narrow original candidates with work granted in batches by `quota`
    /// (see [`NarrowingQuota`]): the same closure, decisions, receipt and
    /// refusal point as [`Self::narrow_known_metered`] with a per-read charge
    /// of the same allowance, consulting the quota at most once every
    /// [`NARROWING_BATCH`] charged reads and refunding the unspent permits on
    /// every outcome. This operation polls `cancellation` before any mutation.
    /// The preconditions of [`Self::narrow_known`] apply, and a failed
    /// attempt's region and knowledge must be abandoned.
    pub fn narrow_known_reserved(
        &self,
        subject: OriginalSubject<'_>,
        region: &mut Region,
        knowledge: &mut Knowledge,
        scratch: &mut NarrowingScratch,
        cancellation: &Cancellation,
        quota: &mut dyn NarrowingQuota,
    ) -> NarrowingAttempt {
        let subject = Subject::from(subject);
        self.narrow_with(
            subject,
            region,
            knowledge,
            scratch,
            Work::reserved(quota),
            cancellation,
        )
    }

    /// Narrow a frozen reduct with work granted in batches by `quota`, as
    /// [`Self::narrow_known_reserved`] does for original candidates. The
    /// preconditions of [`Self::narrow_frozen_known`] apply.
    pub fn narrow_frozen_known_reserved(
        &self,
        subject: FrozenSubject<'_>,
        region: &mut Region,
        knowledge: &mut Knowledge,
        scratch: &mut NarrowingScratch,
        cancellation: &Cancellation,
        quota: &mut dyn NarrowingQuota,
    ) -> NarrowingAttempt {
        let subject = Subject::from(subject);
        self.narrow_with(
            subject,
            region,
            knowledge,
            scratch,
            Work::reserved(quota),
            cancellation,
        )
    }

    fn narrow_with(
        &self,
        subject: Subject<'_>,
        region: &mut Region,
        knowledge: &mut Knowledge,
        scratch: &mut NarrowingScratch,
        mut work: Work<'_>,
        cancellation: &Cancellation,
    ) -> NarrowingAttempt {
        let mut statistics = NarrowingStatistics::default();
        let result = cancellation.poll().and_then(|()| {
            // One choice of width per narrowing; the closure runs on it.
            match &mut knowledge.width {
                Width::Compact(known) => self.narrow_known_width(
                    subject,
                    region,
                    known,
                    scratch,
                    &mut work,
                    &mut statistics,
                ),
                Width::Native(known) => self.narrow_known_width(
                    subject,
                    region,
                    known,
                    scratch,
                    &mut work,
                    &mut statistics,
                ),
            }
        });
        work.settle();
        statistics.work = work.spent;
        NarrowingAttempt { result, statistics }
    }

    /// The closure, the region's new decisions and the split choice, at the
    /// knowledge's counter width.
    fn narrow_known_width<C: Count>(
        &self,
        subject: Subject<'_>,
        region: &mut Region,
        known: &mut Known<C>,
        scratch: &mut NarrowingScratch,
        work: &mut Work<'_>,
        statistics: &mut NarrowingStatistics,
    ) -> Result<Narrowing, Stop> {
        // Whatever an earlier narrowing that refuted or stopped left here
        // belongs to another region; it is discarded before this one reads.
        scratch.prepare(subject.theory.atom_count());
        let mut closure = Closure {
            known,
            lists: scratch,
        };
        if closure.close(subject, self, region, work, statistics)? == Step::Contradiction {
            return Ok(Narrowing::Refuted);
        }
        // The atoms this closure learned decide the region; the region's own
        // decisions, the split's and those made here, are then all seen.
        let mut changed = false;
        for atom in scratch.learned.drain(..) {
            let was_open = region.is_open(atom);
            let decided = if bit(&known.atom_sure, atom) {
                statistics.held += u64::from(was_open);
                region.hold(atom)
            } else {
                statistics.cut += u64::from(was_open);
                region.cut(atom)
            };
            debug_assert!(decided, "a learned atom agrees with the region");
            changed |= was_open;
        }
        region.snapshot_decided(&mut known.seen);
        if let Some(atom) = most_constrained(region, known, work)? {
            region.prefer(atom);
        }
        Ok(Narrowing::Fixed { changed })
    }
}

/// The open atom with the most parents still unknown, the one whose
/// decision the theory is most sensitive to; ties go to the lower atom.
/// This chooses the split, as the clause search branches on the variable
/// with the most unresolved occurrences. The counts are kept as parents
/// become known, so the ranking is one read per open atom.
fn most_constrained<C: Count>(
    region: &Region,
    known: &Known<C>,
    work: &mut Work<'_>,
) -> Result<Option<usize>, Stop> {
    let mut best: Option<(usize, usize)> = None;
    for atom in region.open() {
        work.tick()?;
        let unknown = known.unknown.get(atom);
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
struct Known<C> {
    sure: Box<[u64]>,
    never: Box<[u64]>,
    atom_sure: Box<[u64]>,
    atom_never: Box<[u64]>,
    /// Per chain, the operands known to hold.
    sure_operands: Counters<C>,
    /// Per chain, the operands known to fail.
    never_operands: Counters<C>,
    /// Per atom, the parents of its nodes not yet known: the split ranking.
    /// A parent is counted once here and taken off once when it is
    /// revisited, so the count never goes below zero.
    unknown: Counters<C>,
    /// The region's decided-mask snapshot already told to this closure; new
    /// decisions are the region's decided atoms not set here.
    seen: Box<[u64]>,
    /// The roots, falsum and every atom's support have been seeded once;
    /// later closures learn only decisions not yet known.
    seeded: bool,
}

/// The worklists of a narrowing: what a closure has learned and must still
/// propagate. They belong to the walker, not to a region's knowledge: one
/// value serves every narrowing a walker makes, keeping the capacity its
/// worklists grew, and a knowledge copied at a split carries none. Its
/// contents mean nothing between narrowings; each narrowing empties it
/// first, so whatever a refuted, stopped or cancelled narrowing left is never
/// read by the next. Its capacity grows with the largest closure a walker
/// has made and is held until the value is dropped.
#[derive(Debug, Default)]
pub struct NarrowingScratch {
    /// The atoms this closure decided, not yet told to the region.
    learned: Vec<usize>,
    /// Nodes that learned something, with what, and have not been revisited.
    nodes: Vec<(usize, bool)>,
    /// Atoms whose support must be rechecked; queued only when producers
    /// are known, since only they say what supports an atom.
    heads: Vec<usize>,
    /// One bit per atom, set exactly while the atom is in `heads`, so an
    /// atom's recheck is queued once until it runs.
    pending: Vec<u64>,
}

impl NarrowingScratch {
    /// Discard every entry, keeping the capacity, for a narrowing over
    /// `atoms` atoms.
    fn prepare(&mut self, atoms: usize) {
        for &atom in &self.heads {
            self.pending[atom / 64] &= !(1u64 << (atom % 64));
        }
        self.learned.clear();
        self.nodes.clear();
        self.heads.clear();
        if self.pending.len() < flag_words(atoms) {
            self.pending.resize(flag_words(atoms), 0);
        }
    }

    /// Queue the atom's support recheck unless one is already pending. Its
    /// supporters only fall as knowledge grows, so the pending recheck,
    /// made with the later knowledge, derives what this one would have.
    fn queue_recheck(&mut self, atom: usize) {
        let (word, flag) = (atom / 64, 1u64 << (atom % 64));
        if self.pending[word] & flag == 0 {
            self.pending[word] |= flag;
            self.heads.push(atom);
        }
    }

    /// The next atom to recheck, no longer pending: a support change during
    /// or after its recheck queues it again.
    fn next_recheck(&mut self) -> Option<usize> {
        let atom = self.heads.pop()?;
        self.pending[atom / 64] &= !(1u64 << (atom % 64));
        Some(atom)
    }
}

/// One narrowing's closure: a region's knowledge and the walker's
/// worklists, borrowed together for the call.
struct Closure<'a, C> {
    known: &'a mut Known<C>,
    lists: &'a mut NarrowingScratch,
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

/// The number of 64-bit words a bitset over `n` flags needs.
const fn flag_words(n: usize) -> usize {
    n.div_ceil(64)
}

/// Whether flag `index` is set in a bitset.
#[inline]
fn bit(mask: &[u64], index: usize) -> bool {
    mask[index / 64] & (1u64 << (index % 64)) != 0
}

fn learn(known: &mut [u64], opposite: &[u64], index: usize) -> Step {
    let (word, flag) = (index / 64, 1u64 << (index % 64));
    if known[word] & flag != 0 {
        Step::Unchanged
    } else if opposite[word] & flag != 0 {
        Step::Contradiction
    } else {
        known[word] |= flag;
        Step::Changed
    }
}

impl<C: Count> Known<C> {
    fn empty(nodes: usize, chains: usize, unknown: Counters<C>) -> Self {
        let seen = vec![0u64; unknown.len().div_ceil(64)].into_boxed_slice();
        Self {
            sure: vec![0; flag_words(nodes)].into_boxed_slice(),
            never: vec![0; flag_words(nodes)].into_boxed_slice(),
            atom_sure: vec![0; flag_words(unknown.len())].into_boxed_slice(),
            atom_never: vec![0; flag_words(unknown.len())].into_boxed_slice(),
            sure_operands: Counters::zeros(chains),
            never_operands: Counters::zeros(chains),
            unknown,
            seen,
            seeded: false,
        }
    }
}

impl<C: Count> Closure<'_, C> {
    /// A node learns to hold; it is revisited if that is new.
    fn sure(&mut self, index: usize) -> Step {
        let step = learn(&mut self.known.sure, &self.known.never, index);
        if step == Step::Changed {
            self.lists.nodes.push((index, true));
        }
        step
    }

    /// A node learns to fail; it is revisited if that is new.
    fn never(&mut self, index: usize) -> Step {
        let step = learn(&mut self.known.never, &self.known.sure, index);
        if step == Step::Changed {
            self.lists.nodes.push((index, false));
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
            learn(&mut self.known.atom_sure, &self.known.atom_never, atom)
        } else {
            learn(&mut self.known.atom_never, &self.known.atom_sure, atom)
        };
        if step != Step::Changed {
            return step;
        }
        self.lists.learned.push(atom);
        let mut step = step;
        for &node in &index.atom_nodes[atom] {
            step = step.join(if value {
                self.sure(node)
            } else {
                self.never(node)
            });
        }
        if value && producers.is_some() {
            self.lists.queue_recheck(atom);
        }
        step
    }

    /// Close the knowledge from the region's decisions, falsum and the
    /// roots. Each event on the worklist follows a new bit, or is one of
    /// the initial seeds, so the events are bounded by the bits.
    fn close(
        &mut self,
        subject: Subject<'_>,
        index: &Narrower,
        region: &Region,
        work: &mut Work<'_>,
        statistics: &mut NarrowingStatistics,
    ) -> Result<Step, Stop> {
        let Subject {
            theory,
            producers,
            frozen,
        } = subject;
        let nodes = theory.nodes();
        let mut step = Step::Unchanged;
        if !self.known.seeded {
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
                for atom in 0..theory.atom_count() {
                    self.lists.queue_recheck(atom);
                }
            }
            self.known.seeded = true;
        }
        // Take the seen mask out so the new-decision iterator borrows the local
        // rather than `self`, leaving `self.atom` free to mutate the closure;
        // the swap moves a box pointer and copies nothing.
        let mut seen = std::mem::take(&mut self.known.seen);
        for (atom, value) in region.decided_since(&seen) {
            step = step.join(self.atom(index, producers, atom, value));
        }
        region.snapshot_decided(&mut seen);
        self.known.seen = seen;
        if step == Step::Contradiction {
            return Ok(step);
        }
        loop {
            let step = if let Some((node, value)) = self.lists.nodes.pop() {
                statistics.propagations += 1;
                self.revisit(subject, index, node, value, work)?
            } else if let Some(atom) = self.lists.next_recheck() {
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
    fn revisit(
        &mut self,
        subject: Subject<'_>,
        index: &Narrower,
        node: usize,
        value: bool,
        work: &mut Work<'_>,
    ) -> Result<Step, Stop> {
        work.tick()?;
        let Subject {
            theory,
            producers,
            frozen,
        } = subject;
        let nodes = theory.nodes();
        let masked = |node: usize| frozen.is_some_and(|truth| !truth[node]);
        for &atom in &index.atom_operands[node] {
            self.known.unknown.decrement(atom);
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
                self.operand_changed(index, chain_position(chain), value)
            } else {
                let up = self.learn_from_operands(nodes, parent);
                if bit(&self.known.sure, parent) || bit(&self.known.never, parent) {
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
            self.known.sure_operands.add(chain, 1);
        } else {
            self.known.never_operands.add(chain, 1);
        }
        let (sure, never) = (
            self.known.sure_operands.get(chain),
            self.known.never_operands.get(chain),
        );
        match (disjunction, value) {
            (true, true) => self.sure(root),
            (true, false) if never == total => self.never(root),
            (true, false) if bit(&self.known.sure, root) && never + 1 == total => {
                self.unit(index, chain)
            }
            (false, false) => self.never(root),
            (false, true) if sure == total => self.sure(root),
            (false, true) if bit(&self.known.never, root) && sure + 1 == total => {
                self.unit(index, chain)
            }
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
                !bit(&self.known.never, operand)
            } else {
                !bit(&self.known.sure, operand)
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
            step = step.join(self.teach_chain(index, chain_position(chain)));
        }
        if bit(&self.known.sure, node) {
            step = step.join(match nodes[node] {
                Node::Atom(atom) => {
                    // A held head blocks the other heads of its producers.
                    if let Some(producers) = producers {
                        for &producer in &producers.by_head[atom] {
                            for &head in &producers.rules[producer].heads {
                                if head != atom {
                                    self.lists.queue_recheck(head);
                                }
                            }
                        }
                    }
                    self.atom(index, producers, atom, true)
                }
                Node::False => Step::Contradiction,
                Node::And(..) | Node::Or(..) => Step::Unchanged,
                Node::Implies(a, b) => {
                    if bit(&self.known.sure, a) {
                        self.sure(b)
                    } else if bit(&self.known.never, b) {
                        self.never(a)
                    } else {
                        Step::Unchanged
                    }
                }
            });
        }
        if bit(&self.known.never, node) {
            step = step.join(match nodes[node] {
                Node::Atom(atom) => self.atom(index, producers, atom, false),
                Node::False | Node::And(..) | Node::Or(..) => Step::Unchanged,
                Node::Implies(a, b) => self.sure(a).join(self.never(b)),
            });
            if let Some(producers) = producers {
                for &producer in &producers.by_body[node] {
                    for &head in &producers.rules[producer].heads {
                        self.lists.queue_recheck(head);
                    }
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
        if bit(&self.known.sure, root) {
            if disjunction {
                if self.known.never_operands.get(chain) + 1 == total {
                    step = step.join(self.unit(index, chain));
                }
            } else {
                for &operand in operands {
                    step = step.join(self.sure(operand));
                }
            }
        }
        if bit(&self.known.never, root) {
            if disjunction {
                for &operand in operands {
                    step = step.join(self.never(operand));
                }
            } else if self.known.sure_operands.get(chain) + 1 == total {
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
                if bit(&self.known.never, a) || bit(&self.known.sure, b) {
                    up = up.join(self.sure(node));
                }
                if bit(&self.known.sure, a) && bit(&self.known.never, b) {
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
    fn recheck(
        &mut self,
        index: &Narrower,
        producers: &Producers,
        atom: usize,
        work: &mut Work<'_>,
    ) -> Result<Step, Stop> {
        if bit(&self.known.atom_never, atom) {
            return Ok(Step::Unchanged);
        }
        let mut supporters = 0;
        let mut sole = None;
        for &producer in &producers.by_head[atom] {
            work.tick()?;
            let producer = &producers.rules[producer];
            let body_impossible = producer
                .body
                .is_some_and(|body| bit(&self.known.never, body));
            let other_held = !producer.choice
                && producer
                    .heads
                    .iter()
                    .any(|&head| head != atom && bit(&self.known.atom_sure, head));
            if !body_impossible && !other_held {
                supporters += 1;
                sole = producer.body;
            }
        }
        Ok(match (supporters, sole) {
            (0, _) => self.atom(index, Some(producers), atom, false),
            (1, Some(body)) if bit(&self.known.atom_sure, atom) => self.sure(body),
            _ => Step::Unchanged,
        })
    }
}
