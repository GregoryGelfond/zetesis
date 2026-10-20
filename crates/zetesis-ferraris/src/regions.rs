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
//! producer's body (`sole_support_forces`). Once ordinary propagation drains,
//! a possible-foundedness closure can cut positive cycles without an external
//! seed. It relaxes nonpositive bodies, retains every uncut disjunctive head,
//! and never treats held atoms as seeds (`founded`). A node or atom known both to
//! hold and to fail refutes the region. The open atoms known are then
//! held or cut.
//!
//! Work is charged per node read, root test, producer check and threshold row
//! or relation visit, and residual coefficient or Euclidean step,
//! against `RegionLimits`, and control is polled once per narrowing. Metered
//! entry points instead acquire each permit from the caller's quota, which can
//! also poll control, and retain the admitted prefix on every returned failure.

use std::collections::BTreeSet;
use std::mem::size_of;
use std::num::NonZeroUsize;
use std::sync::OnceLock;

use zetesis_cpu::regions::{Narrowing, Region};
use zetesis_cpu::{Cancellation, Stop};

use crate::{FormulaView, NodeView, Theory};

mod adjacency;
mod counters;
mod extrema;
mod founded;
mod generators;
pub use generators::{
    GeneratorOrder, GeneratorOrderAttempt, GeneratorOrderError, GeneratorOrderLimits,
};
mod masks;
mod preference;
mod support;
mod thresholds;
mod witnesses;
use adjacency::Adjacency;
use counters::{Count, Counters, compact_fits};
use masks::Masks;

/// The work ceiling of one narrowing, and of producer extraction. Every
/// propagation event reads at least one node or threshold row, so the work
/// bounds the events too.
#[derive(Clone, Copy, Debug)]
pub struct RegionLimits {
    /// Charged node reads, root tests, producer checks and threshold operations.
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
/// fragment, when the support cut applies; the frozen truth of every
/// node under which the theory is read as a candidate's reduct, when it
/// is; and the choice heads its split selection ranks first. A knowledge is
/// closed under one subject and reused under the same ([`Knowledge`]).
#[derive(Clone, Copy)]
struct Subject<'a> {
    theory: &'a Theory,
    producers: Option<&'a Producers>,
    frozen: Option<&'a [bool]>,
    choices: &'a ChoiceHeads,
    generator_order: Option<&'a GeneratorOrder>,
}

/// The marks of a subject that ranks no atom as a choice.
static NO_CHOICES: ChoiceHeads = ChoiceHeads { words: Vec::new() };

/// A theory read for its candidates, with the producers of its support
/// fragment when the support cut applies: what original narrowing reads.
#[derive(Clone, Copy, Debug)]
pub struct OriginalSubject<'a> {
    theory: &'a Theory,
    producers: Option<&'a Producers>,
    choices: &'a ChoiceHeads,
    generator_order: Option<&'a GeneratorOrder>,
}

impl<'a> OriginalSubject<'a> {
    /// The candidates of `theory`, cut by `producers`' support when given.
    /// A narrowing that selects its own split ranks no atom as a choice.
    #[must_use]
    pub const fn new(theory: &'a Theory, producers: Option<&'a Producers>) -> Self {
        Self {
            theory,
            producers,
            choices: &NO_CHOICES,
            generator_order: None,
        }
    }

    /// The same subject, whose own split selection ranks `choices` first, as
    /// [`Knowledge::preferred_atom_reserved`] describes.
    #[must_use]
    pub const fn with_choices(self, choices: &'a ChoiceHeads) -> Self {
        Self { choices, ..self }
    }

    /// The same original subject with a prepared generator preference. The
    /// order must belong to this exact theory; selection refuses an unrelated
    /// owner before reading its ranks. Propagation and reduct membership are
    /// unchanged. Frozen subjects never carry this candidate preference.
    #[must_use]
    pub const fn with_generator_order(self, order: Option<&'a GeneratorOrder>) -> Self {
        Self {
            generator_order: order,
            ..self
        }
    }

    /// Optional immutable generator precedence for this original theory.
    #[must_use]
    pub const fn generator_order(&self) -> Option<&'a GeneratorOrder> {
        self.generator_order
    }

    /// The theory read.
    #[must_use]
    pub const fn theory(&self) -> &'a Theory {
        self.theory
    }

    /// The choice heads a split ranks first; empty unless given.
    #[must_use]
    pub const fn choices(&self) -> &'a ChoiceHeads {
        self.choices
    }
}

impl<'a> From<OriginalSubject<'a>> for Subject<'a> {
    fn from(subject: OriginalSubject<'a>) -> Self {
        Self {
            theory: subject.theory,
            producers: subject.producers,
            frozen: None,
            choices: subject.choices,
            generator_order: subject.generator_order,
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

    /// The theory read.
    #[must_use]
    pub const fn theory(&self) -> &'a Theory {
        self.theory
    }

    /// The candidate's truth of every node.
    #[must_use]
    pub const fn truth(&self) -> &'a [bool] {
        self.truth
    }
}

impl<'a> From<FrozenSubject<'a>> for Subject<'a> {
    fn from(subject: FrozenSubject<'a>) -> Self {
        Self {
            theory: subject.theory,
            producers: None,
            frozen: Some(subject.truth),
            choices: &NO_CHOICES,
            generator_order: None,
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
    /// Some extracted producer has at least two distinct heads. When false, holding a
    /// head cannot invalidate any other head's support.
    has_competing_heads: bool,
    /// Some extracted producer is an atomic choice.
    has_choice_heads: bool,
    /// A once-completed cycle test over the same original positive incidences.
    /// Failed preparation leaves this unset; concurrent successful tests agree.
    founded_cycles: OnceLock<bool>,
}

/// The atoms that head an atomic choice root of a theory, `a ∨ ¬a` standing
/// alone or as an implication's consequent. A mark records that the theory
/// chooses the atom, not that no other rule derives it. The default marks no
/// atom.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChoiceHeads {
    /// One bit per atom of the theory's universe.
    words: Vec<u64>,
}

impl ChoiceHeads {
    /// Whether `atom` heads an atomic choice root; false for any atom outside
    /// the universe the marks were taken over.
    #[must_use]
    pub fn contains(&self, atom: usize) -> bool {
        self.words
            .get(atom / 64)
            .is_some_and(|word| word & (1u64 << (atom % 64)) != 0)
    }

    /// Choices dominate every degree score. Rank only open choices when any
    /// remain; otherwise rank every open atom. Both streams are ascending and
    /// borrow the existing packed masks, with no per-region selection storage.
    fn eligible<'a>(&'a self, region: &'a Region) -> impl Iterator<Item = usize> + 'a {
        let mut choices = region.open_in(&self.words).peekable();
        let any = choices.peek().is_some();
        choices.chain((!any).then(|| region.open()).into_iter().flatten())
    }
}

/// The outcome of extracting a theory's producers.
#[derive(Clone, Debug)]
pub struct Extraction {
    /// The producers, or none when a root lies outside the fragment.
    pub producers: Option<Producers>,
    /// The heads of the theory's atomic choice roots, whether or not every
    /// root lies in the fragment.
    pub choice_heads: ChoiceHeads,
    /// Charged node and root visits, the same for a declined extraction.
    pub work: u64,
}

/// Extract the producers when every root is an implication, or a bare head,
/// whose head is a disjunction of atoms with falsum, an atomic choice, or
/// falsum: the fragment `DisjunctiveSupport.Covered` names, with a fact's
/// head standing alone. No producers when a root lies outside it, in which
/// case the support rule does not apply. The heads of atomic choice roots are
/// marked for every theory, in one more charged visit per root.
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
    let choice_heads = choice_heads(theory, &mut work)?;
    let producers = extract(theory, &mut work)?;
    Ok(Extraction {
        producers,
        choice_heads,
        work: work.spent,
    })
}

/// Mark the head of every atomic choice root, one charged visit per root.
fn choice_heads(theory: &Theory, work: &mut Work<'_>) -> Result<ChoiceHeads, Stop> {
    let nodes = theory.view();
    let mut words = vec![0; flag_words(theory.atom_count())];
    for &root in theory.roots() {
        work.tick()?;
        let head = match nodes.node(root).map_err(|_| Stop::InvalidProgram)? {
            NodeView::Implies(_, head) => head,
            _ => root,
        };
        if let Some(atom) = crate::atomic_choice::atom(theory, head) {
            words[atom / 64] |= 1u64 << (atom % 64);
        }
    }
    Ok(ChoiceHeads { words })
}

fn ordinary_operands(
    operands: &[usize],
    ordinary: &[bool],
    work: &mut Work<'_>,
) -> Result<bool, Stop> {
    let mut all = true;
    for &child in operands {
        work.tick()?;
        all &= ordinary[child];
    }
    Ok(all)
}

fn extract(theory: &Theory, work: &mut Work<'_>) -> Result<Option<Producers>, Stop> {
    let nodes = theory.view();
    let mut ordinary = Vec::with_capacity(nodes.len());
    for index in 0..nodes.len() {
        work.tick()?;
        ordinary.push(match nodes.node(index).map_err(|_| Stop::InvalidProgram)? {
            NodeView::Atom(_) | NodeView::False => true,
            NodeView::Or(operands) => ordinary_operands(operands, &ordinary, work)?,
            _ => false,
        });
    }
    let mut rules = Vec::new();
    let mut has_competing_heads = false;
    let mut has_choice_heads = false;
    for &root in theory.roots() {
        work.tick()?;
        let (body, head) = match nodes.node(root).map_err(|_| Stop::InvalidProgram)? {
            NodeView::Implies(body, head) => (Some(body), head),
            _ => (None, root),
        };
        if matches!(
            nodes.node(head).map_err(|_| Stop::InvalidProgram)?,
            NodeView::False
        ) {
            continue;
        }
        let (heads, choice) = if ordinary[head] {
            let mut heads = BTreeSet::new();
            let mut stack = vec![head];
            while let Some(node) = stack.pop() {
                work.tick()?;
                match nodes.node(node).map_err(|_| Stop::InvalidProgram)? {
                    NodeView::Atom(atom) => {
                        heads.insert(atom);
                    }
                    NodeView::Or(operands) => {
                        for &child in operands {
                            work.tick()?;
                            stack.push(child);
                        }
                    }
                    _ => {}
                }
            }
            (heads.into_iter().collect::<Vec<_>>(), false)
        } else if let Some(atom) = crate::atomic_choice::atom(theory, head) {
            (vec![atom], true)
        } else {
            return Ok(None);
        };
        has_competing_heads |= heads.len() > 1;
        has_choice_heads |= choice;
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
        has_competing_heads,
        has_choice_heads,
        founded_cycles: OnceLock::new(),
    }))
}

/// Bounded reuse of conditional deductions from completed possible support.
/// All reads are also included in [`NarrowingStatistics::work`]. Counts describe
/// this narrowing, including an interrupted prefix, rather than retained state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SupportExplanationStatistics {
    /// Completed support passes for which an explanation was requested.
    pub attempts: u64,
    /// Distinct complete explanations retained by the worker.
    pub certified: u64,
    /// Matching explanations that changed or refuted a region.
    pub matched: u64,
    /// Previously open atoms excluded by matching explanations.
    pub cut: u64,
    /// Open conditions forced opposite when a certified output is held.
    pub forced: u64,
    /// Regions refuted by a held atom in a matching unsupported set.
    pub refuted: u64,
    /// Capture attempts that supplied no new bounded explanation.
    pub declined: u64,
    /// Capture, deduplication and matching reads, including refused prefixes.
    pub work: u64,
    /// Maximum single-worker ledger and capture workspace bytes observed.
    /// This is not total solver memory or a sum across workers.
    pub peak_bytes: u128,
}

/// The work one narrowing charged.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NarrowingStatistics {
    /// Charged node reads, root tests, producer checks and threshold operations.
    pub work: u64,
    /// Producer rechecks, included in `work`, including a refused prefix.
    pub support_work: u64,
    /// Possible-foundedness preparation, closure and publication, included in
    /// `work`. Reusing a completed closure charges no work here.
    pub founded_work: u64,
    /// Conditional support deductions, included in the total reading work.
    pub support_explanations: SupportExplanationStatistics,
    /// Propagation events: nodes and atoms learned and their neighbours
    /// revisited, atoms whose support was rechecked, and threshold rows whose
    /// residual divisibility or changed requirements were checked.
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

/// One split-selection result and its charged reads, including a refused
/// prefix. A selection changes neither the region nor its closed knowledge.
#[derive(Debug)]
pub struct PreferenceAttempt {
    /// The open atom to split on, ranked by the producing selector:
    /// [`Knowledge::preferred_atom_reserved`] or
    /// [`Knowledge::preferred_atom_with_secondary_reserved`], or
    /// [`GeneratorOrder::preferred_atom_reserved`]. No atom is selected when
    /// the region is decided.
    pub result: Result<Option<usize>, Stop>,
    /// Charged ranking reads, including a refused prefix. The producing selector
    /// reads component ranks or unknown-parent counts once per eligible atom;
    /// secondary counts are read only to distinguish primary ties, at most once
    /// per eligible atom.
    pub work: u64,
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
/// and so the most reads between two consultations of it: the shared metered
/// polling interval, [`zetesis_cpu::POLL_INTERVAL`].
pub const NARROWING_BATCH: u64 = zetesis_cpu::POLL_INTERVAL;

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
    fn refund(&mut self) {
        if let Some(quota) = &mut self.quota {
            quota.refund(std::mem::take(&mut self.available));
        }
    }
}

/// The shape of a theory a narrowing walks: each maximal tree of one
/// connective read as one node, a *chain*, with its operands; the parents
/// of each node that is not inside a chain, an implication or a chain root
/// reading it as an operand; the nodes carrying each atom; and the atoms
/// among each node's operands. Built once per theory from its nodes. The three
/// immutable incidence maps store ordered rows as offsets
/// into contiguous entry vectors, retaining every occurrence already chosen by
/// chain formation. Chain formation and compaction use linear construction
/// passes, visiting each chain interior once when collecting its leaves.
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
    /// Largest chain length or initial per-atom parent-occurrence count.
    counter_bound: usize,
    thresholds: thresholds::Thresholds,
    extrema: extrema::Extrema,
    equal_extrema: extrema::EqualWitnesses,
    /// Roles in shared same-connective edges; empty when no such edge exists.
    witness_chains: Vec<u8>,
    /// Logical indexing visits: each native node and operand occurrence.
    work: u64,
}

/// A maximal tree of one connective, read as one node over its operands.
#[derive(Clone, Debug)]
struct Chain {
    disjunction: bool,
    root: usize,
    /// The leaves, distinct nodes, in operand order.
    operands: ChainOperands,
}

/// Ordinary chains own their leaves; ordinal cuts borrow a prepared row column.
/// This is an index view, never another formula or authoritative tuple store.
#[derive(Clone, Debug)]
enum ChainOperands {
    Owned(Vec<usize>),
    Ordinal { column: usize, length: usize },
}

impl Chain {
    fn leaves<'a>(&'a self, extrema: &'a extrema::Extrema) -> &'a [usize] {
        match &self.operands {
            ChainOperands::Owned(values) => values,
            ChainOperands::Ordinal { column, length } => extrema.leaves(*column, *length),
        }
    }

    fn ordinal(&self) -> bool {
        matches!(self.operands, ChainOperands::Ordinal { .. })
    }

    #[cfg(test)]
    fn ordinary_leaves(&self) -> &[usize] {
        match &self.operands {
            ChainOperands::Owned(leaves) => leaves,
            ChainOperands::Ordinal { .. } => panic!("ordinary-chain test fixture"),
        }
    }
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

/// A conjunction or disjunction and its ordered operand occurrences.
fn connective_operands(node: NodeView<'_>) -> Option<(bool, &[usize])> {
    match node {
        NodeView::Or(operands) => Some((true, operands)),
        NodeView::And(operands) => Some((false, operands)),
        NodeView::Atom(_)
        | NodeView::False
        | NodeView::Implies(_, _)
        | NodeView::Threshold(_)
        | NodeView::ExtremumCut(_) => None,
    }
}

/// Classify interiors by distinct parents, retaining asserted roots. Markers
/// coalesce repeated child occurrences in one parent without a quadratic scan.
fn absorbed_nodes(theory: &Theory) -> Vec<bool> {
    let nodes = theory.view();
    let mut roots = vec![false; nodes.len()];
    for &root in theory.roots() {
        roots[root] = true;
    }
    let mut parent_count = vec![0usize; nodes.len()];
    let mut last_parent = vec![None; nodes.len()];
    for index in 0..nodes.len() {
        let pair;
        let operands = match nodes.node(index).expect("admitted node") {
            NodeView::Atom(_) | NodeView::False => continue,
            NodeView::And(operands) | NodeView::Or(operands) => operands,
            NodeView::Threshold(row) => row.conditions,
            NodeView::ExtremumCut(cut) => {
                if cut.endpoint_truth().is_none() {
                    for (&child, &rank) in cut.row.conditions.iter().zip(cut.row.ranks) {
                        if cut.qualifies(rank) && last_parent[child] != Some(index) {
                            last_parent[child] = Some(index);
                            parent_count[child] += 1;
                        }
                    }
                }
                continue;
            }
            NodeView::Implies(a, b) => {
                pair = [a, b];
                &pair
            }
        };
        for &child in operands {
            if last_parent[child] != Some(index) {
                last_parent[child] = Some(index);
                parent_count[child] += 1;
            }
        }
    }
    let mut absorbed = vec![false; nodes.len()];
    for index in 0..nodes.len() {
        let Some((disjunction, operands)) =
            connective_operands(nodes.node(index).expect("admitted node"))
        else {
            continue;
        };
        for &operand in operands {
            absorbed[operand] = !roots[operand]
                && parent_count[operand] == 1
                && connective_operands(nodes.node(operand).expect("admitted operand"))
                    .is_some_and(|(inner, _)| inner == disjunction);
        }
    }
    absorbed
}

/// Collect each live chain's distinct leaves once, in left-to-right order.
/// Interiors have one parent, so the live chains partition them; no completed
/// prefix is copied into its parent. A node-indexed chain marker coalesces
/// repeated leaves without searching the already collected operand vector.
///
/// The admitted DAG's edges point backwards. Each pending interior is therefore
/// replaced by smaller indices, and each interior belongs to just one walk.
/// With N nodes, E edges and R asserted-root occurrences, classification and
/// collection take O(N + E + R) work, O(N + E) temporary and retained
/// storage. The explicit stack has at most E + 1 entries and is reused
/// between chains; no recursive call uses the formula depth. Construction
/// remains infallible, as before.
fn chains(theory: &Theory) -> (Vec<Chain>, Vec<Option<NonZeroUsize>>, Vec<bool>) {
    let nodes = theory.view();
    let absorbed = absorbed_nodes(theory);
    let mut chains = Vec::new();
    let mut chain_of = vec![None; nodes.len()];
    let mut seen = vec![None; nodes.len()];
    let mut pending = Vec::new();
    for root in 0..nodes.len() {
        let Some((disjunction, _)) = connective_operands(nodes.node(root).expect("admitted node"))
        else {
            continue;
        };
        if absorbed[root] {
            continue;
        }
        let link = encoded_chain(chains.len());
        chain_of[root] = Some(link);
        let mut operands = Vec::new();
        pending.push(root);
        while let Some(operand) = pending.pop() {
            if seen[operand] == Some(link) {
                continue;
            }
            seen[operand] = Some(link);
            if operand == root || absorbed[operand] {
                let (_, children) =
                    connective_operands(nodes.node(operand).expect("admitted node"))
                        .expect("chain interior connective");
                // Reverse pushing preserves the first left-to-right occurrence.
                pending.extend(children.iter().rev().copied());
            } else {
                operands.push(operand);
            }
        }
        chains.push(Chain {
            disjunction,
            root,
            operands: ChainOperands::Owned(operands),
        });
    }
    (chains, chain_of, absorbed)
}

/// What a narrowing knows about a region, carried from a region to its
/// children: the knowledge of a region holds in every region inside it
/// (`FormulaBounds.known_mono`), so a child's narrowing starts from its
/// parent's knowledge and learns only what the split decided. A fresh
/// value knows nothing and is seeded in full on first use.
///
/// A value is linear in the theory: a bit pair over the nodes and one over
/// the atoms, a counter and witness bit per chain, a count per atom and processed state per
/// shared threshold row; the worklists of
/// a narrowing belong to the walker's [`NarrowingScratch`]. Its counters
/// have one width, chosen when the root value is made.
/// A split that offers one child to another worker clones it, so the
/// clone is the split's cost. A knowledge belongs to the narrower that
/// made it and to the region it was closed for: narrowing a region with a
/// knowledge from another narrower, theory, producer set or frozen mask,
/// or from a region not enclosing it, is unsound, and nothing checks it.
/// The proposers keep the invariant by carrying each region's knowledge
/// from its parent and by narrowing with the narrower that made it.
/// A completed possible-foundedness pass is inherited with the knowledge;
/// any newly false projected producer-body node invalidates it. An atom with
/// no positive occurrence in that projection can only remove itself from the
/// upper closure. An absent or foreign projection invalidates on every false
/// atom or node. Unchanged relaxation inputs need no repeat pass; worker scratch can
/// identify an irrelevant node but never establish completion itself.
///
/// Support rechecks inherit two monotone producer-row positions per atom,
/// allocated only after a head with more than two producers is reached. Their
/// compact width is bounded by the complete producer count. Failed prefixes
/// stay unavailable in descendants, while surviving endpoints are revalidated;
/// this never replaces complete coverage for zero or sole support. Initial
/// allocation is fallible and charged by the narrowing; cloning includes its
/// owned capacity under the existing infallible clone boundary.
///
/// Cloning owns an independent copy. [`Clone::clone_from`] overwrites every
/// field, reusing equal-length storage at the same counter width. The six
/// masks occupy disjoint slices of one owned block; copying also replaces its
/// node/atom boundaries, even when the total block length is unchanged. The
/// destination's previous knowledge contributes no authority; the source's
/// narrowing preconditions still apply. Changed lengths or widths use fresh
/// storage, with the same infallible allocation boundary as [`Clone::clone`].
#[derive(Debug)]
pub struct Knowledge {
    width: Width,
}

impl Clone for Knowledge {
    fn clone(&self) -> Self {
        Self {
            width: self.width.clone(),
        }
    }

    fn clone_from(&mut self, source: &Self) {
        match (&mut self.width, &source.width) {
            (Width::Compact16(destination), Width::Compact16(source)) => {
                destination.clone_from(source);
            }
            (Width::Compact32(destination), Width::Compact32(source)) => {
                destination.clone_from(source);
            }
            (Width::Native(destination), Width::Native(source)) => destination.clone_from(source),
            (destination, source) => destination.clone_from(source),
        }
    }
}

/// The knowledge at the counter width chosen when it was created: the
/// closure runs on one concrete width, chosen once.
#[derive(Clone, Debug)]
enum Width {
    Compact16(Known<u16>),
    Compact32(Known<u32>),
    Native(Known<usize>),
}

impl Knowledge {
    /// Select the open atom to split on. An open atom marked in `choices`, the
    /// heads of the theory's atomic choice roots, ranks before every unmarked
    /// one: deciding the choices first leaves the atoms they derive to
    /// propagation and support. Among equally ranked atoms, the one with most
    /// unknown parents in this knowledge wins; ties, including all-zero
    /// scores, select the lowest atom. Empty `choices` rank by unknown parents
    /// alone. The region, knowledge and choices must use the same atom
    /// universe and atom identities; their correspondence is a caller
    /// precondition, not checked here. This preference only orders an
    /// exhaustive split; it establishes no truth and does not complete
    /// propagation.
    ///
    /// For `A` atoms and `E` eligible open atoms the scan takes `O(A / 64 + E)`
    /// time and constant additional space. Eligible atoms are the open choice
    /// heads when any remain, otherwise all open atoms. It charges one count
    /// read per eligible atom; packed mask traversal is not separately charged.
    /// It polls cancellation before reading and reserves/refunds the same batched
    /// permits as [`Narrower::narrow_known_reserved`]. Call after composed
    /// propagation has reached its joint fixed point, so the selected
    /// subject ranks the final region once.
    ///
    /// # Errors
    /// The receipt's result retains a cancellation/deadline stop or the
    /// quota's refusal; a zero grant is [`Stop::WorkLimit`]. Every failed
    /// attempt retains its charged prefix and changes neither argument.
    ///
    /// # Panics
    /// Panics if it reads an open atom outside this knowledge's universe,
    /// violating the caller's shape precondition.
    pub fn preferred_atom_reserved(
        &self,
        region: &Region,
        choices: &ChoiceHeads,
        cancellation: &Cancellation,
        quota: &mut dyn NarrowingQuota,
    ) -> PreferenceAttempt {
        let mut work = Work::reserved(quota);
        let result = cancellation
            .poll()
            .and_then(|()| self.preferred_atom(region, choices, &mut work));
        work.refund();
        PreferenceAttempt {
            result,
            work: work.spent,
        }
    }

    fn preferred_atom(
        &self,
        region: &Region,
        choices: &ChoiceHeads,
        work: &mut Work<'_>,
    ) -> Result<Option<usize>, Stop> {
        match &self.width {
            Width::Compact16(known) => preferred_split(region, known, choices, work),
            Width::Compact32(known) => preferred_split(region, known, choices, work),
            Width::Native(known) => preferred_split(region, known, choices, work),
        }
    }

    /// Header, owned flag, seen and counter arrays in bytes. The shared
    /// theory, narrower and producer index are excluded, as are the walker's
    /// scratch, allocator bookkeeping and temporary clones.
    #[must_use]
    pub fn retained_bytes(&self) -> u128 {
        size_of::<Self>() as u128
            + match &self.width {
                Width::Compact16(known) => known.retained_bytes(),
                Width::Compact32(known) => known.retained_bytes(),
                Width::Native(known) => known.retained_bytes(),
            }
    }
}

impl<C: Count> Known<C> {
    /// Payload and capacities owned by this closure state; the enclosing
    /// [`Knowledge`] header is counted by its owner.
    fn retained_bytes(&self) -> u128 {
        self.masks.words.len() as u128 * size_of::<u64>() as u128
            + self.thresholds.len() as u128 * size_of::<thresholds::State>() as u128
            + self.support_progress.allocated_bytes()
            + [&self.neutral_operands, &self.unknown]
                .into_iter()
                .map(Counters::allocated_bytes)
                .sum::<u128>()
    }
}

#[cfg(test)]
mod tests;

/// The ordered operand-to-parent incidences already chosen by chain formation.
/// Chains precede implications as in the original index; a repeated implication
/// operand is one incidence. Distinct atom nodes carrying the same atom remain
/// distinct occurrences when this stream is projected to atom operands.
fn dependencies<'a>(
    nodes: FormulaView<'a>,
    chains: &'a [Chain],
) -> impl Iterator<Item = (usize, usize)> + Clone + 'a {
    chains
        .iter()
        .filter_map(|chain| match &chain.operands {
            ChainOperands::Owned(operands) => Some((chain, operands)),
            ChainOperands::Ordinal { .. } => None,
        })
        .flat_map(|(chain, operands)| operands.iter().map(move |&operand| (operand, chain.root)))
        .chain((0..nodes.len()).flat_map(move |index| {
            let operands = match nodes.node(index).expect("admitted node") {
                NodeView::Implies(a, b) => [Some(a), (b != a).then_some(b)],
                _ => [None, None],
            };
            operands
                .into_iter()
                .flatten()
                .map(move |operand| (operand, index))
        }))
}

/// Bound each stored count, not the total size of the theory. A chain learns
/// each collected operand once. An atom's initial unknown count includes every
/// parent occurrence of every node carrying it; thereafter it only decreases.
/// Distinct nodes with the same atom therefore contribute separately, even
/// when they share a parent. The sum is bounded by the allocated parent table.
/// This visits chain lengths and atom-node rows once during index construction.
fn counter_bound(
    chains: &[Chain],
    parents: &Adjacency,
    atom_nodes: &Adjacency,
    thresholds: &thresholds::Thresholds,
    extrema: &extrema::Extrema,
) -> usize {
    let chain_bound = chains
        .iter()
        .map(|chain| chain.leaves(extrema).len())
        .max()
        .unwrap_or(0);
    atom_nodes.iter().fold(chain_bound, |bound, nodes| {
        let occurrences = nodes.iter().fold(0usize, |count, &node| {
            count
                .checked_add(parents[node].len())
                .and_then(|count| count.checked_add(thresholds.parent_count(node)))
                .and_then(|count| count.checked_add(extrema.parent_count(node)))
                .expect("atom parent occurrences fit their allocated table")
        });
        bound.max(occurrences)
    })
}

/// Existing one-theory operations choose immediately; composed closure
/// defers preference until every subject has stopped deciding atoms.
#[derive(Clone, Copy)]
enum SplitSelection {
    Choose,
    Defer,
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
    /// row scans, preserving row order and duplicates. Chain construction walks
    /// each final chain once with linear scratch; it never copies intermediate
    /// chain prefixes. The logical index receipt is one visit per native node and operand
    /// occurrence; construction passes and
    /// storage initialization are not additional charged propagation reads.
    /// Native rows also have an exact-prefix forest: sorting borrowed row IDs
    /// takes O(R log R * K) term comparisons in the worst case for R rows of at most
    /// K grouped terms; prefix discovery and checked incidence construction take
    /// O(E + R), where E is the number of grouped terms. This derived index keeps
    /// at most R - 1 links and does not change the logical index receipt.
    /// When unit rows exist, immutable anchor projections retain one entry per
    /// eligible row and matching Boolean chain, with sparse reverse chain
    /// lookup. Chain minima are read once; their operand order is unchanged.
    /// For N nodes, U unit rows, C chains, L chain leaves and A anchors, this
    /// adds O(N + R + U log U + L + (R + C + L) log(A + 1)) preparation work,
    /// including the per-chain anchored-cone summary, and O(U + C) storage,
    /// with fallible reservations. This anchor projection has no row/chain
    /// pairs or node-sized offsets. With no unit row it performs no allocation
    /// or chain scan. Runtime anchor lookups, posting visits and exact-prefix
    /// comparisons consume propagation work; no per-Boolean mutable state is added.
    ///
    /// A separate capacity projection retains conjunction/row intersections
    /// containing at least two distinct necessary unit contributions. It joins
    /// direct conjunction leaves and necessary aliases from original root
    /// implications with the existing row incidences. Its preparation work is
    /// O(R + C + D + B log(B + 1) + L log(B + 1) + J + sum(Pc log(Pc + 1))),
    /// where D counts root discovery visits (including repeated double-negation
    /// and target-incidence scans), B the collected aliases, J the projection's
    /// incidence visits, and Pc its entries before deduplication for chain c.
    /// Retained storage is O(R + I + T) for row offsets, I retained intersections
    /// and T distinct required conditions; a dense incidence join can produce
    /// a dense projection. Temporary alias and per-chain projection vectors can
    /// exceed the deduplicated payload. Reservations are fallible. There is no
    /// per-region capacity cache; its runtime visits consume propagation work.
    /// Original-root aliases are never used in frozen-reduct propagation.
    /// This constructor has no memory allowance. Callers can budget retained
    /// capacity publication using [`Self::capacity_index_bytes`]; temporary
    /// preparation peaks can precede that publication check.
    ///
    /// # Errors
    /// Returns [`Stop::Allocation`] for compact-adjacency offset overflow or
    /// reservation failure. Chain construction and later knowledge allocation
    /// retain their existing infallible behavior; this is not universal OOM
    /// recovery. An invalid operand view returns [`Stop::InvalidProgram`].
    /// This constructor has no independent cancellation contract.
    pub fn try_new(theory: &Theory) -> Result<Self, Stop> {
        let nodes = theory.view();
        let (mut chains, mut chain_of, absorbed) = chains(theory);
        let extrema = extrema::Extrema::prepare(theory)?;
        extrema.append_chains(&mut chains, &mut chain_of)?;
        let witness_chains = witnesses::nested(&chains, &chain_of, &extrema)?;
        let thresholds = thresholds::Thresholds::prepare(
            theory,
            &chains,
            &chain_of,
            &extrema,
            !witness_chains.is_empty(),
        )?;
        let parents = Adjacency::build(nodes.len(), dependencies(nodes, &chains))?;
        let atom_operands = Adjacency::try_build(
            nodes.len(),
            dependencies(nodes, &chains)
                .chain(thresholds.dependencies())
                .filter_map(|(operand, parent)| match nodes.node(operand) {
                    Ok(NodeView::Atom(atom)) => Some(Ok((parent, atom))),
                    Ok(_) => None,
                    Err(_) => Some(Err(Stop::InvalidProgram)),
                }),
        )?;
        let atom_nodes = Adjacency::try_build(
            theory.atom_count(),
            (0..nodes.len()).filter_map(|index| match nodes.node(index) {
                Ok(NodeView::Atom(atom)) => Some(Ok((atom, index))),
                Ok(_) => None,
                Err(_) => Some(Err(Stop::InvalidProgram)),
            }),
        )?;
        let counter_bound = counter_bound(&chains, &parents, &atom_nodes, &thresholds, &extrema);
        let mut result = Self {
            parents,
            atom_nodes,
            chains,
            chain_of,
            absorbed,
            atom_operands,
            counter_bound,
            thresholds,
            extrema,
            equal_extrema: extrema::EqualWitnesses::default(),
            witness_chains,
            work: u64::try_from(nodes.len() as u128 + theory.parts().occurrences() as u128)
                .map_err(|_| Stop::Allocation)?,
        };
        result.equal_extrema = extrema::EqualWitnesses::prepare(&result)?;
        Ok(result)
    }

    /// Additional retained native-extrema index capacity. Dedicated owners use
    /// their actual headers and vector capacities. For shared containers, native
    /// Chain slots plus unused capacity and the whole witness-role and anchor
    /// projection payloads are
    /// conservatively attributed to native cuts; live ordinary Chain slots are
    /// excluded. Zero-cut theories report zero. Temporary preparation and
    /// per-chain counters already in knowledge accounting are excluded.
    #[must_use]
    pub fn extremum_index_bytes(&self) -> u128 {
        if self.extrema.is_empty() {
            return 0;
        }
        let native_chains = self.extrema.chain_count();
        let shared = if native_chains == 0 {
            0
        } else {
            (native_chains as u128 + (self.chains.capacity() - self.chains.len()) as u128)
                * size_of::<Chain>() as u128
                + self.witness_chains.capacity() as u128 * size_of::<u8>() as u128
                + self.thresholds.connective_storage_bytes()
        };
        size_of::<extrema::Extrema>() as u128
            + size_of::<extrema::EqualWitnesses>() as u128
            + self.extrema.retained_bytes()
            + self.equal_extrema.retained_bytes()
            + shared
    }

    /// Additional retained cardinality-capacity projection storage. The shared
    /// immutable projection owns its actual vector capacities; no capacity
    /// state is cloned with regions. Temporary preparation storage is excluded.
    #[must_use]
    pub fn capacity_index_bytes(&self) -> u128 {
        self.thresholds.capacity_storage_bytes()
    }

    /// Direct Boolean parents followed by ordinal-cut parents. The ordinal
    /// incidences are streamed from compact row postings, never materialized.
    fn logical_parents(&self, node: usize) -> impl DoubleEndedIterator<Item = usize> + Clone + '_ {
        self.parents[node]
            .iter()
            .copied()
            .chain(self.extrema.parents(node))
    }

    /// The logical index receipt: one visit per node and operand occurrence.
    /// Construction passes, including native-row sorting and prefix discovery,
    /// are preparation costs rather than additional propagation reads.
    #[must_use]
    pub fn work(&self) -> u64 {
        self.work
    }

    /// Knowledge of nothing, for the root of a tree over this theory.
    #[must_use]
    pub fn knowledge(&self) -> Knowledge {
        Knowledge {
            width: if compact_fits::<u16>(self.counter_bound) {
                Width::Compact16(self.root_known())
            } else if compact_fits::<u32>(self.counter_bound) {
                Width::Compact32(self.root_known())
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
                unknown.add(
                    atom,
                    self.parents[node].len()
                        + self.thresholds.parent_count(node)
                        + self.extrema.parent_count(node),
                );
            }
        }
        let mut known = Known::empty(self.parents.len(), self.chains.len(), unknown);
        known.thresholds = self.thresholds.knowledge();
        known
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
            (subject, SplitSelection::Choose),
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
            (subject, SplitSelection::Choose),
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

    /// Narrow original candidates with work granted in batches by `quota`
    /// (see [`NarrowingQuota`]): the same closure and decisions as
    /// [`Self::narrow_known`], with every charged node, parent, producer or
    /// open-atom read spending one permit, so a refused grant prevents that
    /// read and the receipt counts the permits spent. The quota is consulted
    /// at most once every [`NARROWING_BATCH`] charged reads, so its control
    /// polls are at most that many reads apart; a quota that grants what
    /// remains refuses a work limit at the same read as per-read charging.
    /// Unspent permits are refunded on every outcome. This operation polls
    /// `cancellation` before any mutation.
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
            (subject, SplitSelection::Choose),
            region,
            knowledge,
            scratch,
            Work::reserved(quota),
            cancellation,
        )
    }

    /// Close original-candidate knowledge without selecting a split atom.
    /// This has the propagation, ownership and failure contracts of
    /// [`Self::narrow_known_reserved`], but charges no preference scan and
    /// leaves any existing preference unchanged. Compose several such
    /// closures to a joint fixed point, then select once with
    /// [`Knowledge::preferred_atom_reserved`].
    pub fn propagate_known_reserved(
        &self,
        subject: OriginalSubject<'_>,
        region: &mut Region,
        knowledge: &mut Knowledge,
        scratch: &mut NarrowingScratch,
        cancellation: &Cancellation,
        quota: &mut dyn NarrowingQuota,
    ) -> NarrowingAttempt {
        self.narrow_with(
            (Subject::from(subject), SplitSelection::Defer),
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
            (subject, SplitSelection::Choose),
            region,
            knowledge,
            scratch,
            Work::reserved(quota),
            cancellation,
        )
    }

    fn narrow_with(
        &self,
        (subject, selection): (Subject<'_>, SplitSelection),
        region: &mut Region,
        knowledge: &mut Knowledge,
        scratch: &mut NarrowingScratch,
        mut work: Work<'_>,
        cancellation: &Cancellation,
    ) -> NarrowingAttempt {
        let mut statistics = NarrowingStatistics::default();
        let result = cancellation.poll().and_then(|()| {
            // Select the counter width once for the closure.
            let narrowing = match &mut knowledge.width {
                Width::Compact16(known) => self.narrow_known_width(
                    subject,
                    region,
                    known,
                    scratch,
                    &mut work,
                    &mut statistics,
                ),
                Width::Compact32(known) => self.narrow_known_width(
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
            }?;
            if matches!(narrowing, Narrowing::Fixed { .. })
                && matches!(selection, SplitSelection::Choose)
            {
                let preferred = match subject.generator_order {
                    Some(order) => order.preferred(subject.theory, region, &mut work)?,
                    None => knowledge.preferred_atom(region, subject.choices, &mut work)?,
                };
                if let Some(atom) = preferred {
                    region.prefer(atom);
                }
            }
            Ok(narrowing)
        });
        work.refund();
        statistics.work = work.spent;
        statistics.support_explanations.peak_bytes = scratch.explanations.peak_bytes();
        NarrowingAttempt { result, statistics }
    }

    /// The closure and the region's new decisions, at the knowledge's
    /// counter width. Split selection is a separate read of the closed state.
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
        scratch.founded.end_local();
        if subject.producers.is_some() && subject.frozen.is_none() {
            // Only original support consumes this preparation. Candidate bounds
            // and frozen queries may share scratch without replacing its owner.
            scratch.founded.forget_foreign(subject.theory);
            scratch
                .founded
                .begin_local(subject.theory, known.founded_closed);
            scratch.explanations.prepare(subject.theory);
        }
        let mut closure = Closure {
            known: known.borrow(),
            lists: scratch,
        };
        if closure.close(subject, self, region, work, statistics)? == Step::Contradiction {
            return Ok(Narrowing::Refuted);
        }
        // The atoms this closure learned decide the region; the region's own
        // decisions, the split's and those made here, are then all seen.
        let mut changed = false;
        for atom in closure.lists.learned.drain(..) {
            let was_open = region.is_open(atom);
            let decided = if bit(closure.known.atom_sure, atom) {
                statistics.held += u64::from(was_open);
                region.hold(atom)
            } else {
                statistics.cut += u64::from(was_open);
                region.cut(atom)
            };
            debug_assert!(decided, "a learned atom agrees with the region");
            changed |= was_open;
        }
        region.snapshot_decided(closure.known.seen);
        Ok(Narrowing::Fixed { changed })
    }
}

/// The open atom to split on. Atoms marked in `choices` rank before every
/// unmarked one; among equally ranked atoms, the one with the most parents
/// still unknown, whose decision the theory is most sensitive to, as the
/// clause search branches on the variable with the most unresolved
/// occurrences; ties go to the lower atom. The counts are kept as parents
/// become known, so ranking reads each eligible atom once. Packed choice
/// filtering omits atoms that cannot win without changing the ordering.
fn preferred_split<C: Count>(
    region: &Region,
    known: &Known<C>,
    choices: &ChoiceHeads,
    work: &mut Work<'_>,
) -> Result<Option<usize>, Stop> {
    let mut best: Option<(usize, usize)> = None;
    for atom in choices.eligible(region) {
        work.tick()?;
        let rank = known.unknown.get(atom);
        if best.is_none_or(|(_, best)| rank > best) {
            best = Some((atom, rank));
        }
    }
    Ok(best.map(|(atom, _)| atom))
}

/// What every candidate of the region must make of each node and each
/// atom: known to hold, known to fail, or open. One bit pair over the
/// nodes, one over the atoms, and processed evidence for each chain, closed under the
/// upward rules from a node's operands and the downward rules from a node's
/// own knowledge until nothing changes: unit propagation on the theory
/// itself, with a node shared by several parents known once for all of
/// them (`FormulaBounds.Known`, `known_sound`; the chain rules are
/// `FormulaChains`). The closure is driven by a worklist: a node that
/// learns something is revisited once, and only its parents, operands and
/// dependent producers are read, a chain learning from an operand by one
/// counter or witness step. Until a witness settles the chain, a disjunction
/// counts failing operands and records whether a holding operand was processed;
/// a conjunction dually counts holding operands and records a failing witness.
/// Pending events contribute neither, and settled counters need no further update.
#[derive(Debug)]
struct Known<C> {
    masks: Masks,
    /// Per chain, processed neutral operands: false for OR, true for AND.
    /// The count preserves distinct operand-node occurrences, including nodes
    /// that denote the same atom. Repeated identical leaves are coalesced by
    /// chain formation. It is exact until an absorbing witness is processed;
    /// later operand events leave the now-irrelevant count unchanged.
    neutral_operands: Counters<C>,
    /// Per atom, the parents of its nodes not yet known: the split ranking.
    /// A parent is counted once here and taken off once when it is
    /// revisited, so the count never goes below zero.
    unknown: Counters<C>,
    /// Processed sums, required bounds and monotone cursors per shared row.
    thresholds: Box<[thresholds::State]>,
    /// The roots, falsum and every atom's support have been seeded once;
    /// later closures learn only decisions not yet known.
    seeded: bool,
    /// A successful possible-foundedness pass covered every uncut atom, and
    /// no projected body input has become false since. This has the same original
    /// subject and enclosing-region preconditions as every inherited mask.
    founded_closed: bool,
    /// Completed unavailable producer prefixes, inherited only by descendants.
    support_progress: support::Progress,
}

impl<C: Clone> Clone for Known<C> {
    fn clone(&self) -> Self {
        Self {
            masks: self.masks.clone(),
            neutral_operands: self.neutral_operands.clone(),
            unknown: self.unknown.clone(),
            thresholds: self.thresholds.clone(),
            seeded: self.seeded,
            founded_closed: self.founded_closed,
            support_progress: self.support_progress.clone(),
        }
    }

    fn clone_from(&mut self, source: &Self) {
        self.masks.clone_from(&source.masks);
        self.neutral_operands.clone_from(&source.neutral_operands);
        self.unknown.clone_from(&source.unknown);
        self.thresholds.clone_from(&source.thresholds);
        self.seeded = source.seeded;
        self.founded_closed = source.founded_closed;
        self.support_progress.clone_from(&source.support_progress);
    }
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
    /// Original-subject rows awaiting prefix/Boolean requirement transport.
    threshold_rows: Vec<usize>,
    /// One bit per row, set exactly while it is in `threshold_rows`.
    pending_thresholds: Vec<u64>,
    /// Original-subject arithmetic checks, deferred until mandatory propagation
    /// reaches a fixed point. These entries never change transport order.
    residual_rows: Vec<usize>,
    /// One bit per row, set exactly while it is in `residual_rows`.
    pending_residuals: Vec<u64>,
    /// Reversed suffix gcds for one drained residual row; never inherited.
    residual_suffix: Vec<i128>,
    /// Optional reachable-sum scratch; rebuilt from each drained row snapshot.
    possible_sums: Vec<u64>,
    /// Shared connective views and dirty worklists belong to the walker.
    witnesses: witnesses::Scratch,
    /// Possible founded atoms, computed only with complete original producers.
    founded: founded::Scratch,
    /// Exact-original conditional deductions; not cloned with region knowledge.
    explanations: founded::ledger::Ledger,
}

impl NarrowingScratch {
    /// Worklist and pending-mask capacity in bytes; the value's own header is
    /// counted by its owner.
    #[must_use]
    pub fn retained_bytes(&self) -> u128 {
        (self.learned.capacity() + self.heads.capacity()) as u128 * size_of::<usize>() as u128
            + self.nodes.capacity() as u128 * size_of::<(usize, bool)>() as u128
            + self.pending.capacity() as u128 * size_of::<u64>() as u128
            + (self.threshold_rows.capacity() + self.residual_rows.capacity()) as u128
                * size_of::<usize>() as u128
            + (self.pending_thresholds.capacity() + self.pending_residuals.capacity()) as u128
                * size_of::<u64>() as u128
            + self.residual_suffix.capacity() as u128 * size_of::<i128>() as u128
            + self.possible_sums.capacity() as u128 * size_of::<u64>() as u128
            + self.witnesses.retained_bytes()
            + self.founded.retained_bytes()
            + self.explanations.retained_bytes()
    }

    /// Discard every entry, keeping the capacity, for a narrowing over
    /// `atoms` atoms.
    fn prepare(&mut self, atoms: usize) {
        self.witnesses.clear();
        for &atom in &self.heads {
            self.pending[atom / 64] &= !(1u64 << (atom % 64));
        }
        for &row in &self.threshold_rows {
            self.pending_thresholds[row / 64] &= !(1u64 << (row % 64));
        }
        for &row in &self.residual_rows {
            self.pending_residuals[row / 64] &= !(1u64 << (row % 64));
        }
        self.learned.clear();
        self.nodes.clear();
        self.heads.clear();
        self.threshold_rows.clear();
        self.residual_rows.clear();
        self.residual_suffix.clear();
        self.possible_sums.clear();
        if self.pending.len() < flag_words(atoms) {
            self.pending.resize(flag_words(atoms), 0);
        }
    }

    /// Reserve each original-subject worklist only when its route is present.
    /// At most one entry per row is pending; enqueueing never allocates.
    /// Frozen subjects never request either additional worklist.
    fn prepare_thresholds(&mut self, rows: usize) -> Result<(), Stop> {
        Self::prepare_rows(&mut self.threshold_rows, &mut self.pending_thresholds, rows)
    }

    fn prepare_residuals(&mut self, rows: usize) -> Result<(), Stop> {
        Self::prepare_rows(&mut self.residual_rows, &mut self.pending_residuals, rows)
    }

    fn prepare_rows(
        entries: &mut Vec<usize>,
        pending: &mut Vec<u64>,
        rows: usize,
    ) -> Result<(), Stop> {
        entries
            .try_reserve_exact(rows.saturating_sub(entries.len()))
            .map_err(|_| Stop::Allocation)?;
        let words = flag_words(rows);
        if pending.len() < words {
            pending
                .try_reserve_exact(words - pending.len())
                .map_err(|_| Stop::Allocation)?;
            pending.resize(words, 0);
        }
        Ok(())
    }

    /// Requirement transport retains its own insertion order. Each producer
    /// then runs `threshold_row`, which separately marks eligible arithmetic.
    fn queue_threshold(&mut self, row: usize) {
        Self::queue_row(&mut self.threshold_rows, &mut self.pending_thresholds, row);
    }

    /// Condition changes request only arithmetic, not prefix or bridge walks.
    /// Mandatory waves coalesce into one check of their drained row state.
    /// Residual decisions return through those waves before a row is rechecked.
    fn queue_residual(&mut self, row: usize) {
        Self::queue_row(&mut self.residual_rows, &mut self.pending_residuals, row);
    }

    fn queue_row(entries: &mut Vec<usize>, pending: &mut [u64], row: usize) {
        let (word, flag) = (row / 64, 1u64 << (row % 64));
        if pending[word] & flag == 0 {
            pending[word] |= flag;
            entries.push(row);
        }
    }

    fn next_threshold(&mut self) -> Option<usize> {
        Self::next_row(&mut self.threshold_rows, &mut self.pending_thresholds)
    }

    fn next_residual(&mut self) -> Option<usize> {
        Self::next_row(&mut self.residual_rows, &mut self.pending_residuals)
    }

    fn next_row(entries: &mut Vec<usize>, pending: &mut [u64]) -> Option<usize> {
        let row = entries.pop()?;
        pending[row / 64] &= !(1u64 << (row % 64));
        Some(row)
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
    known: KnownMut<'a, C>,
    lists: &'a mut NarrowingScratch,
}

/// Disjoint borrows of one knowledge for a narrowing. Mask boundaries are
/// checked once here; hot bit accesses retain ordinary slice indexing.
struct KnownMut<'a, C> {
    sure: &'a mut [u64],
    never: &'a mut [u64],
    atom_sure: &'a mut [u64],
    atom_never: &'a mut [u64],
    /// Region decisions already incorporated into the closure.
    seen: &'a mut [u64],
    /// A processed absorbing operand: true for OR, false for AND.
    witnessed: &'a mut [u64],
    neutral_operands: &'a mut Counters<C>,
    unknown: &'a mut Counters<C>,
    thresholds: &'a mut [thresholds::State],
    seeded: &'a mut bool,
    founded_closed: &'a mut bool,
    support_progress: &'a mut support::Progress,
}

impl<C> Known<C> {
    fn borrow(&mut self) -> KnownMut<'_, C> {
        let [sure, never, atom_sure, atom_never, seen, witnessed] = self.masks.split();
        KnownMut {
            sure,
            never,
            atom_sure,
            atom_never,
            seen,
            witnessed,
            neutral_operands: &mut self.neutral_operands,
            unknown: &mut self.unknown,
            thresholds: &mut self.thresholds,
            seeded: &mut self.seeded,
            founded_closed: &mut self.founded_closed,
            support_progress: &mut self.support_progress,
        }
    }
}

/// What a step of the closure did. Contradiction is terminal: subsequent
/// propagation cannot restore the region, so its knowledge and pending events
/// are discarded instead of completing dependent work.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Step {
    Unchanged,
    Changed,
    /// A node or atom became known both to hold and to fail.
    Contradiction,
}

impl Step {
    /// Combine completed results. Arguments are evaluated eagerly, so a caller
    /// must return a contradiction before starting another operation.
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
        Self {
            masks: Masks::empty(nodes, unknown.len(), chains),
            neutral_operands: Counters::zeros(chains),
            unknown,
            thresholds: Box::default(),
            seeded: false,
            founded_closed: false,
            support_progress: support::Progress::default(),
        }
    }
}

impl<C: Count> Closure<'_, C> {
    /// A node learns to hold; it is revisited if that is new.
    fn sure(&mut self, index: usize) -> Step {
        let step = learn(self.known.sure, self.known.never, index);
        if step == Step::Changed {
            self.lists.nodes.push((index, true));
        }
        step
    }

    /// A node learns to fail; it is revisited if that is new.
    fn never(&mut self, index: usize) -> Step {
        let step = learn(self.known.never, self.known.sure, index);
        if step == Step::Changed {
            self.lists.founded.record_false(index);
            if *self.known.founded_closed && self.lists.founded.may_affect_body(index) {
                *self.known.founded_closed = false;
            }
            self.lists.nodes.push((index, false));
        }
        step
    }

    /// An atom learns to hold or to fail, and every unmasked node carrying
    /// it learns the same. Masked occurrences remain falsum independently
    /// of the tested interpretation. A newly held atom has its support
    /// rechecked when producers are known.
    fn atom(
        &mut self,
        index: &Narrower,
        producers: Option<&Producers>,
        frozen: Option<&[bool]>,
        atom: usize,
        value: bool,
    ) -> Step {
        let step = if value {
            learn(self.known.atom_sure, self.known.atom_never, atom)
        } else {
            learn(self.known.atom_never, self.known.atom_sure, atom)
        };
        if step != Step::Changed {
            return step;
        }
        if !value && !self.lists.founded.has_projection() {
            *self.known.founded_closed = false;
        }
        self.lists.learned.push(atom);
        let mut step = step;
        for &node in &index.atom_nodes[atom] {
            if !value {
                self.lists.founded.record_false(node);
            }
            // A false output-only atom removes only itself from the old upper
            // closure. Every included occurrence conservatively invalidates,
            // including an occurrence already blocked before this atom event.
            if !value && *self.known.founded_closed && self.lists.founded.may_affect_body(node) {
                *self.known.founded_closed = false;
            }
            if frozen.is_some_and(|truth| !truth[node]) {
                continue;
            }
            step = step.join(if value {
                self.sure(node)
            } else {
                self.never(node)
            });
            if step == Step::Contradiction {
                return step;
            }
        }
        if value && producers.is_some() {
            self.lists.queue_recheck(atom);
        }
        step
    }

    /// Close the knowledge from the region's decisions, falsum and the
    /// roots. Node events follow new bits or initial seeds. Original threshold
    /// row events follow strictly tighter requirements from the finite set of
    /// native and linked Boolean bounds, or processed condition events. A
    /// residual divisibility check runs only after all mandatory queues drain.
    /// Its forced conditions return through those same queues before another
    /// arithmetic check. Every new wave fixes a previously open condition,
    /// so residual forcing terminates; Boolean chains retain their counters.
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
            choices: _,
            generator_order: _,
        } = subject;
        if frozen.is_none() {
            if index.thresholds.has_relations() {
                self.lists.prepare_thresholds(index.thresholds.rows.len())?;
            }
            if index.thresholds.has_residuals() {
                self.lists.prepare_residuals(index.thresholds.rows.len())?;
            }
        }
        if !index.witness_chains.is_empty() && (frozen.is_none() || !index.extrema.is_empty()) {
            self.lists.witnesses.prepare_dirty(index.chains.len())?;
        }
        let nodes = theory.view();
        let mut step = Step::Unchanged;
        if !*self.known.seeded {
            for node in 0..nodes.len() {
                if index.absorbed[node] {
                    continue;
                }
                let falsum = matches!(
                    nodes.node(node).map_err(|_| Stop::InvalidProgram)?,
                    NodeView::False
                ) || frozen.is_some_and(|truth| !truth[node]);
                if falsum {
                    step = step.join(self.never(node));
                    if step == Step::Contradiction {
                        return Ok(step);
                    }
                }
            }
            step = step.join(self.seed_extrema(index, frozen, work)?);
            if step == Step::Contradiction {
                return Ok(step);
            }
            step = step.join(self.seed_thresholds(index, frozen, work)?);
            if step == Step::Contradiction {
                return Ok(step);
            }
            for &root in theory.roots() {
                step = step.join(self.sure(root));
                if step == Step::Contradiction {
                    return Ok(step);
                }
            }
            if producers.is_some() {
                for atom in 0..theory.atom_count() {
                    self.lists.queue_recheck(atom);
                }
            }
            *self.known.seeded = true;
        }
        // Move only the disjoint seen borrow so the iterator leaves
        // `self.atom` free to update the other masks. No words are copied.
        let seen = std::mem::take(&mut self.known.seen);
        for (atom, value) in region.decided_since(seen) {
            step = step.join(self.atom(index, producers, frozen, atom, value));
            if step == Step::Contradiction {
                break;
            }
        }
        if step != Step::Contradiction {
            region.snapshot_decided(seen);
        }
        // Restore the borrowed storage even when this knowledge is discarded;
        // only a completed incorporation may record all decisions as seen.
        self.known.seen = seen;
        if step == Step::Contradiction {
            return Ok(step);
        }
        self.close_events(subject, index, work, statistics)
    }

    /// Drain ordinary events before testing possible foundedness. New cuts
    /// reenter those same queues; contradiction stops either phase immediately.
    fn close_events(
        &mut self,
        subject: Subject<'_>,
        index: &Narrower,
        work: &mut Work<'_>,
        statistics: &mut NarrowingStatistics,
    ) -> Result<Step, Stop> {
        let Subject {
            theory,
            producers,
            frozen,
            choices: _,
            generator_order: _,
        } = subject;
        loop {
            let step = if let Some((node, value)) = self.lists.nodes.pop() {
                statistics.propagations += 1;
                self.revisit(subject, index, node, value, work)?
            } else if let Some(row) = self.lists.next_threshold() {
                debug_assert!(
                    frozen.is_none(),
                    "only original rows transport requirements"
                );
                statistics.propagations += 1;
                self.threshold_requirements(index, row, work)?
            } else if let Some(chain) = self.lists.witnesses.next_dirty() {
                statistics.propagations += 1;
                self.witness_changed(index, chain, frozen, work)?
            } else if let Some(atom) = self.lists.next_recheck() {
                statistics.propagations += 1;
                let producers =
                    producers.expect("a support recheck is queued only when producers are known");
                let before = work.spent;
                let result = self.recheck(index, producers, atom, work);
                statistics.support_work += work.spent - before;
                result?
            } else if let Some(row) = self.lists.next_residual() {
                debug_assert!(frozen.is_none(), "only original rows check residual sums");
                statistics.propagations += 1;
                let step = self.threshold_residual(index, row, work)?;
                if step == Step::Unchanged {
                    self.threshold_possible_sums(index, row, work)?
                } else {
                    step
                }
            } else if let Some(producers) = producers {
                debug_assert!(frozen.is_none(), "only original subjects have producers");
                let before = work.spent;
                let explanation_work = statistics.support_explanations.work;
                let result = self.founded(
                    theory,
                    index,
                    producers,
                    work,
                    &mut statistics.support_explanations,
                );
                statistics.founded_work +=
                    work.spent - before - (statistics.support_explanations.work - explanation_work);
                let founded = result?;
                if founded == Step::Unchanged {
                    return Ok(Step::Changed);
                }
                founded
            } else {
                return Ok(Step::Changed);
            };
            if step == Step::Contradiction {
                return Ok(step);
            }
        }
    }

    /// Every stable completion lies inside the possible founded closure. Cuts
    /// are published only after that closure completes, then ordinary events
    /// propagate them. Held atoms outside it refute the region. Candidate-only
    /// restrictions and frozen-reduct subjects supply no producer inventory.
    fn founded(
        &mut self,
        theory: &Theory,
        index: &Narrower,
        producers: &Producers,
        work: &mut Work<'_>,
        explanations: &mut SupportExplanationStatistics,
    ) -> Result<Step, Stop> {
        let before = work.spent;
        let matched = self.match_explanations(index, producers, work, explanations);
        explanations.work += work.spent - before;
        let matched = matched?;
        if matched != Step::Unchanged {
            return Ok(matched);
        }
        if *self.known.founded_closed
            || !self.lists.founded.needed(theory, index, producers, work)?
        {
            return Ok(Step::Unchanged);
        }
        self.lists.founded.complete(
            theory,
            index,
            producers,
            self.known.never,
            self.known.atom_never,
            work,
        )?;
        let before = work.spent;
        let captured = self.remember_unsupported(theory, index, producers, work, explanations);
        explanations.work += work.spent - before;
        captured?;
        let mut step = Step::Unchanged;
        for word in 0..self.lists.founded.publication_words() {
            work.tick()?;
            let mut missing = self
                .lists
                .founded
                .missing_in_word(word, self.known.atom_never[word]);
            while missing != 0 {
                work.tick()?;
                let atom = word * 64 + missing.trailing_zeros() as usize;
                missing &= missing - 1;
                step = step.join(self.atom(index, Some(producers), None, atom, false));
                if step == Step::Contradiction {
                    return Ok(step);
                }
            }
        }
        // A changed pass queued new false evidence; ordinary propagation must
        // finish and a new pass must account for it before closure is inherited.
        // Interrupted or contradictory prefixes never publish this authority.
        *self.known.founded_closed = step == Step::Unchanged;
        Ok(step)
    }

    fn remember_unsupported(
        &mut self,
        theory: &Theory,
        index: &Narrower,
        producers: &Producers,
        work: &mut Work<'_>,
        explanations: &mut SupportExplanationStatistics,
    ) -> Result<(), Stop> {
        if !self.lists.explanations.can_capture() {
            return Ok(());
        }
        // Prefer a direct contradiction. If none is present, capture one new
        // cut from the completed pass. Nothing is published during discovery:
        // this pass's outputs cannot become their own boundary premises.
        let mut start = None;
        for word in 0..self.lists.founded.publication_words() {
            if !self.lists.explanations.capture_read(work)? {
                // Discovery did not finish, so even the first open head must
                // wait: a later held head would have priority.
                return Ok(());
            }
            let missing = self
                .lists
                .founded
                .missing_in_word(word, self.known.atom_never[word]);
            let held_missing = missing & self.known.atom_sure[word];
            if held_missing != 0 {
                start = Some(word * 64 + held_missing.trailing_zeros() as usize);
                break;
            }
            if start.is_none() && missing != 0 {
                start = Some(word * 64 + missing.trailing_zeros() as usize);
            }
        }
        if let Some(atom) = start {
            self.lists.explanations.capture(
                founded::explanations::Input {
                    theory,
                    index,
                    producers,
                    completed: &self.lists.founded,
                    atom_sure: self.known.atom_sure,
                    atom_never: self.known.atom_never,
                },
                atom,
                work,
                explanations,
            )?;
        }
        Ok(())
    }

    /// Read conditional support as a filter over current signed atom masks.
    /// Consequences enter the existing event queues and preserve their order.
    fn match_explanations(
        &mut self,
        index: &Narrower,
        producers: &Producers,
        work: &mut Work<'_>,
        counts: &mut SupportExplanationStatistics,
    ) -> Result<Step, Stop> {
        let mut step = Step::Unchanged;
        for row in 0..self.lists.explanations.len() {
            match self.lists.explanations.deduction(
                row,
                self.known.atom_sure,
                self.known.atom_never,
                work,
            )? {
                founded::ledger::Deduction::None => continue,
                founded::ledger::Deduction::Require { atom, value } => {
                    let forced = self.atom(index, Some(producers), None, atom, value);
                    counts.forced += 1;
                    counts.matched += 1;
                    step = step.join(forced);
                    if step == Step::Contradiction {
                        return Ok(step);
                    }
                    continue;
                }
                founded::ledger::Deduction::ExcludeOutputs => {}
            }
            let mut changed = false;
            let mut outputs = founded::ledger::OutputCursor::default();
            while let Some(atom) = self.lists.explanations.next_publication_output(
                row,
                &mut outputs,
                self.known.atom_sure,
                self.known.atom_never,
                work,
            )? {
                if bit(self.known.atom_sure, atom) {
                    counts.matched += 1;
                    counts.refuted += 1;
                    return Ok(Step::Contradiction);
                }
                if !bit(self.known.atom_never, atom) {
                    step = step.join(self.atom(index, Some(producers), None, atom, false));
                    counts.cut += 1;
                    changed = true;
                }
            }
            counts.matched += u64::from(changed);
        }
        Ok(step)
    }

    /// A node that learned something teaches its operands, and lets each
    /// parent learn from it: a chain by a neutral count or absorbing witness,
    /// an implication from this event and its other operand. A known implication can then
    /// teach the remaining open operand. A node false
    /// under a frozen mask is falsum in the reduct, a constant with no
    /// operands: it teaches nothing and learns nothing from them, and a
    /// parent under the mask likewise. A newly false body wakes its heads
    /// here, once for its false-node event. All node events run before support
    /// rechecks, so repeated teaching of a known parent needs no body scan.
    /// Closed inherited knowledge already completed these wakeups; a refused
    /// closure is abandoned with its pending events. Once this event proves a
    /// contradiction, no remaining neighbour can restore the region; leave its
    /// unread tail uncharged and abandon the knowledge with the region.
    fn revisit(
        &mut self,
        subject: Subject<'_>,
        index: &Narrower,
        node: usize,
        value: bool,
        work: &mut Work<'_>,
    ) -> Result<Step, Stop> {
        work.tick()?;
        let Subject { theory, frozen, .. } = subject;
        let nodes = theory.view();
        let masked = |node: usize| frozen.is_some_and(|truth| !truth[node]);
        for &atom in &index.atom_operands[node] {
            self.known.unknown.decrement(atom);
        }
        if let Some(chain) = index.chain_of[node] {
            let chain = &index.chains[chain_position(chain)];
            if chain.ordinal() {
                for &operand in chain.leaves(&index.extrema) {
                    work.tick()?;
                    if let NodeView::Atom(atom) =
                        nodes.node(operand).map_err(|_| Stop::InvalidProgram)?
                    {
                        self.known.unknown.decrement(atom);
                    }
                }
            }
        }
        let mut step = Step::Unchanged;
        if !masked(node) {
            step = step.join(self.teach_operands(subject, index, node, work)?);
            if step == Step::Contradiction {
                return Ok(step);
            }
            if !value && let Some(producers) = subject.producers {
                for &producer in &producers.by_body[node] {
                    for &head in &producers.rules[producer].heads {
                        self.lists.queue_recheck(head);
                    }
                }
            }
        }
        step = step.join(self.threshold_condition(index, node, value, frozen, work)?);
        if step == Step::Contradiction {
            return Ok(step);
        }
        for parent in index.logical_parents(node) {
            work.tick()?;
            if masked(parent) {
                continue;
            }
            step = step.join(if let Some(chain) = index.chain_of[parent] {
                let chain = chain_position(chain);
                // A processed absorbing operand already settled this root.
                // Its neutral count and witness notices cannot teach anything
                // further. Keep this parent visit and every other event: node
                // truth, support wakeups and split ranking remain independent.
                if bit(self.known.witnessed, chain) {
                    continue;
                }
                let step = self.operand_changed(index, chain, value, work)?;
                // An absorbing operand supplies a witness and settles its root.
                // Neither relay nor an expanded unit obligation remains.
                if step != Step::Contradiction
                    && value != index.chains[chain].disjunction
                    && witnesses::participates(index, chain)
                    && self.needs_witness_notification(index, chain, frozen)
                {
                    self.lists.witnesses.queue_dirty(chain);
                }
                step
            } else {
                self.revisit_implication(nodes, parent, node, value)
            });
            if step == Step::Contradiction {
                return Ok(step);
            }
        }
        Ok(step)
    }

    /// A chain learns from an operand that became known: one more operand
    /// holds or fails. A disjunction holds with one, fails with all, and a
    /// disjunction known to hold with all but one failing forces that one;
    /// a conjunction dually (`disj_chain_sure`, `disj_chain_never`,
    /// `disj_chain_unit` and the conjunction laws).
    fn operand_changed(
        &mut self,
        index: &Narrower,
        chain: usize,
        value: bool,
        work: &mut Work<'_>,
    ) -> Result<Step, Stop> {
        let source = &index.chains[chain];
        let (disjunction, root) = (source.disjunction, source.root);
        let operands = source.leaves(&index.extrema);
        let total = operands.len();
        if value == disjunction {
            self.known.witnessed[chain / 64] |= 1u64 << (chain % 64);
            return Ok(if disjunction {
                self.sure(root)
            } else {
                self.never(root)
            });
        }
        self.known.neutral_operands.add(chain, 1);
        let neutral = self.known.neutral_operands.get(chain);
        Ok(match disjunction {
            true if neutral == total => self.never(root),
            true if bit(self.known.sure, root) && neutral + 1 == total => {
                self.unit(index, chain, work)?
            }
            false if neutral == total => self.sure(root),
            false if bit(self.known.never, root) && neutral + 1 == total => {
                self.unit(index, chain, work)?
            }
            _ => Step::Unchanged,
        })
    }

    /// The one operand of a chain not yet known learns what the chain's
    /// own knowledge leaves it: to hold, in a disjunction known to hold
    /// whose others fail; to fail, in a conjunction known to fail whose
    /// others hold. A counted witness already at the required polarity
    /// makes the scan redundant. Otherwise the operands are scanned once;
    /// a node's bit is set when it learns and counted when it is revisited,
    /// so the scan
    /// may find none, every operand being known with one count pending,
    /// and then the pending step decides the chain.
    fn unit(&mut self, index: &Narrower, chain: usize, work: &mut Work<'_>) -> Result<Step, Stop> {
        let source = &index.chains[chain];
        let disjunction = source.disjunction;
        let operands = source.leaves(&index.extrema);
        // Callers have counted all but one operand at the opposite polarity.
        // A processed witness is therefore the sole possible target, already
        // known as required. Bits precede their events, but counts never do.
        if bit(self.known.witnessed, chain) {
            return Ok(Step::Unchanged);
        }
        for &operand in operands {
            work.tick()?;
            let open = if disjunction {
                !bit(self.known.never, operand)
            } else {
                !bit(self.known.sure, operand)
            };
            if open {
                return Ok(if disjunction {
                    self.sure(operand)
                } else {
                    self.never(operand)
                });
            }
        }
        Ok(Step::Unchanged)
    }

    /// A known node teaches its operands what its knowledge leaves them and
    /// tells its atom. A chain known to hold forces its one
    /// open operand (disjunction) or every operand (conjunction); known to
    /// fail, every operand (disjunction) or its one open operand
    /// (conjunction).
    fn teach_operands(
        &mut self,
        subject: Subject<'_>,
        index: &Narrower,
        node: usize,
        work: &mut Work<'_>,
    ) -> Result<Step, Stop> {
        let Subject {
            theory,
            producers,
            frozen,
            choices: _,
            generator_order: _,
        } = subject;
        let nodes = theory.view();
        if let Some(chain) = index.chain_of[node] {
            // A chain link identifies an And/Or root. Its complete downward
            // rule is the chain's; the generic connective arms teach nothing.
            let chain = chain_position(chain);
            let step = self.teach_chain(index, chain, work)?;
            if step == Step::Contradiction {
                return Ok(step);
            }
            // This event already fixes the root, so only an expanded unit
            // obligation can remain; relay-only chains have nothing to teach.
            if witnesses::expands(index, chain)
                && self.needs_witness_notification(index, chain, frozen)
            {
                self.lists.witnesses.queue_dirty(chain);
            }
            let step = step.join(self.extremum_order(index, chain, frozen, work)?);
            if step == Step::Contradiction {
                return Ok(step);
            }
            let step = step.join(self.equal_extremum_witnesses(index, chain, frozen, work)?);
            if step == Step::Contradiction || frozen.is_some() {
                return Ok(step);
            }
            return Ok(step.join(self.threshold_connective(
                index,
                chain,
                bit(self.known.sure, node),
                work,
            )?));
        }
        Ok(match nodes.node(node).expect("admitted node") {
            NodeView::Atom(atom) => {
                if bit(self.known.sure, node) {
                    // A held head blocks the other heads of its producers.
                    if let Some(producers) = producers
                        && producers.has_competing_heads
                    {
                        for &producer in &producers.by_head[atom] {
                            for &head in &producers.rules[producer].heads {
                                if head != atom {
                                    self.lists.queue_recheck(head);
                                }
                            }
                        }
                    }
                    self.atom(index, producers, frozen, atom, true)
                } else if bit(self.known.never, node) {
                    self.atom(index, producers, frozen, atom, false)
                } else {
                    Step::Unchanged
                }
            }
            NodeView::False if bit(self.known.sure, node) => Step::Contradiction,
            NodeView::False | NodeView::And(..) | NodeView::Or(..) | NodeView::ExtremumCut(_) => {
                Step::Unchanged
            }
            NodeView::Implies(a, b) => self.teach_implication(node, a, b),
            NodeView::Threshold(row) => {
                self.threshold_guard(index, theory, node, row.bound, frozen, work)?
            }
        })
    }

    /// The same downward implication rules serve a node's own event and a
    /// known parent's response to an operand. Neither adds a charged visit.
    fn teach_implication(&mut self, node: usize, a: usize, b: usize) -> Step {
        if bit(self.known.sure, node) {
            return if bit(self.known.sure, a) {
                self.sure(b)
            } else if bit(self.known.never, b) {
                self.never(a)
            } else {
                Step::Unchanged
            };
        }
        if bit(self.known.never, node) {
            let step = self.sure(a);
            if step == Step::Contradiction {
                return step;
            }
            return step.join(self.never(b));
        }
        Step::Unchanged
    }

    /// What a chain's own knowledge leaves its operands. A complete processed
    /// count means every operand already has the required bit, so its downward
    /// scan teaches nothing. Counts never lead bits; the root's own event,
    /// parents and support wakeups still run even when this scan is omitted.
    /// A contradictory operand ends the scan: the region is already refuted,
    /// and neither the remaining operands nor their work permits are needed.
    fn teach_chain(
        &mut self,
        index: &Narrower,
        chain: usize,
        work: &mut Work<'_>,
    ) -> Result<Step, Stop> {
        let source = &index.chains[chain];
        let (disjunction, root) = (source.disjunction, source.root);
        let operands = source.leaves(&index.extrema);
        let total = operands.len();
        let mut step = Step::Unchanged;
        if bit(self.known.sure, root) {
            if disjunction {
                if self.known.neutral_operands.get(chain) + 1 == total {
                    step = step.join(self.unit(index, chain, work)?);
                }
            } else if self.known.neutral_operands.get(chain) != total {
                for &operand in operands {
                    work.tick()?;
                    step = step.join(self.sure(operand));
                    if step == Step::Contradiction {
                        return Ok(step);
                    }
                }
            }
        } else if bit(self.known.never, root) {
            if disjunction {
                if self.known.neutral_operands.get(chain) != total {
                    for &operand in operands {
                        work.tick()?;
                        step = step.join(self.never(operand));
                        if step == Step::Contradiction {
                            return Ok(step);
                        }
                    }
                }
            } else if self.known.neutral_operands.get(chain) + 1 == total {
                step = step.join(self.unit(index, chain, work)?);
            }
        }
        Ok(step)
    }

    /// An operand event already establishes that operand's truth. An
    /// antecedent known false or consequent known true satisfies the parent.
    /// Otherwise its truth is that of the other operand (antecedent true),
    /// or its negation (consequent false). Learn upward first; if the other
    /// operand is open, the parent's knowledge teaches it. A newly learned
    /// parent still has its own queued event and wakeups.
    fn revisit_implication(
        &mut self,
        nodes: FormulaView<'_>,
        node: usize,
        changed: usize,
        value: bool,
    ) -> Step {
        let NodeView::Implies(a, b) = nodes.node(node).expect("admitted node") else {
            unreachable!("a non-chain parent is an implication");
        };
        debug_assert!(changed == a || changed == b);
        debug_assert!(if value {
            bit(self.known.sure, changed)
        } else {
            bit(self.known.never, changed)
        });
        // An aliased operand a -> a also takes this branch at either truth.
        if (changed == a && !value) || (changed == b && value) {
            return self.sure(node);
        }
        let other = if changed == a { b } else { a };
        let satisfied = if value {
            bit(self.known.sure, other)
        } else {
            bit(self.known.never, other)
        };
        if satisfied {
            return self.sure(node);
        }
        let refuted = if value {
            bit(self.known.never, other)
        } else {
            bit(self.known.sure, other)
        };
        if refuted {
            return self.never(node);
        }
        // The changed operand already has its required bit, and the other
        // is open. Only the latter can learn from a known parent.
        if bit(self.known.sure, node) {
            if value {
                self.sure(other)
            } else {
                self.never(other)
            }
        } else if bit(self.known.never, node) {
            if value {
                self.never(other)
            } else {
                self.sure(other)
            }
        } else {
            Step::Unchanged
        }
    }

    /// An atom none of its producers can support is known to fail, and an
    /// atom known to hold with exactly one producer able to support it
    /// forces that producer's body (`unsupported_cut`,
    /// `sole_support_forces`). A producer can support its atom when its
    /// body is not known to fail and, unless it is a choice, no other of
    /// its heads is known to hold. An open atom needs only one possible
    /// supporter to remain open; a held atom with two cannot force a sole
    /// body. Stop at those witnesses and charge only visited producers.
    /// Inherited positions skip only prefixes already proved unavailable in
    /// this region or an ancestor; zero or sole support still requires complete
    /// coverage. Later body failure, a competing held head or this atom becoming
    /// held queues a recheck, which revalidates its remaining endpoints.
    fn recheck(
        &mut self,
        index: &Narrower,
        producers: &Producers,
        atom: usize,
        work: &mut Work<'_>,
    ) -> Result<Step, Stop> {
        if bit(self.known.atom_never, atom) {
            return Ok(Step::Unchanged);
        }
        let held = bit(self.known.atom_sure, atom);
        let support = self.known.support_progress.check(
            producers,
            atom,
            held,
            self.known.never,
            self.known.atom_sure,
            work,
        )?;
        Ok(match support {
            support::Support::Unavailable => self.atom(index, Some(producers), None, atom, false),
            support::Support::Sole(Some(body)) if held => self.sure(body),
            support::Support::Sole(_) | support::Support::Sufficient => Step::Unchanged,
        })
    }
}
