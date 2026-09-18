//! Regions of candidates over a formula theory, narrowed by its readings.
//!
//! A region (`zetesis_cpu::regions::Region`) holds some atoms in, cuts some
//! out and leaves the rest open; here it decides over the theory's atoms. A
//! formula has two readings under a region, decided by one pass over the
//! DAG: it is *sure* when every seed of the region satisfies it and
//! *impossible* when none does, with a held atom sure, a cut atom
//! impossible, and the connectives combining the readings as the closure
//! route's definite and possible gates do (`FormulaBounds.read`).
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
//! This is unit propagation over a clause form of the theory, read on the
//! theory itself (`FormulaBounds.Known`, `known_sound`). In the producer
//! fragment of the support restriction, an atom none of whose producers
//! can support it, each having a body known to fail or another head known
//! to hold, is known to fail (`unsupported_cut`), and an atom known to
//! hold with exactly one producer able to support it demands that
//! producer's body (`sole_support_forces`). A node or atom known both to
//! hold and to fail refutes the region. The open atoms known are then
//! held or cut.
//!
//! Work is charged per node read, per root tested and per producer checked,
//! against `RegionLimits`, and control is polled once per pass.

use std::collections::BTreeSet;

use zetesis_cpu::regions::{Narrowing, Region};
use zetesis_cpu::{Control, Stop};

use crate::{Node, Theory};

/// Ceilings on one narrowing and on producer extraction.
#[derive(Clone, Copy, Debug)]
pub struct RegionLimits {
    /// Charged node reads, root tests and producer checks.
    pub max_work: u64,
    /// Propagation events in one narrowing: a node or atom learned and
    /// its neighbours revisited, or an atom's support rechecked.
    pub max_propagations: u64,
}
impl Default for RegionLimits {
    fn default() -> Self {
        Self {
            max_work: 100_000_000,
            max_propagations: 1_000_000_000,
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
    control: &Control,
) -> Result<Extraction, Stop> {
    control.poll()?;
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
    pub forced: u64,
    /// Atoms cut by the narrowing.
    pub cut: u64,
}

struct Work {
    spent: u64,
    ceiling: u64,
}
impl Work {
    fn new(ceiling: u64) -> Self {
        Self { spent: 0, ceiling }
    }
    fn tick(&mut self) -> Result<(), Stop> {
        if self.spent >= self.ceiling {
            return Err(Stop::WorkLimit);
        }
        self.spent += 1;
        Ok(())
    }
}

/// The shape of a theory a narrowing walks: which nodes have a node as an
/// operand, and which nodes carry each atom. Built once per theory in one
/// pass over its nodes.
#[derive(Clone, Debug)]
pub struct Narrower {
    parents: Vec<Vec<usize>>,
    atom_nodes: Vec<Vec<usize>>,
}

impl Narrower {
    /// Index the theory's DAG for narrowing.
    #[must_use]
    pub fn new(theory: &Theory) -> Self {
        let nodes = theory.nodes();
        let mut parents = vec![Vec::new(); nodes.len()];
        let mut atom_nodes = vec![Vec::new(); theory.atom_count()];
        for (index, node) in nodes.iter().enumerate() {
            match *node {
                Node::Atom(atom) => atom_nodes[atom].push(index),
                Node::False => {}
                Node::And(a, b) | Node::Or(a, b) | Node::Implies(a, b) => {
                    parents[a].push(index);
                    if b != a {
                        parents[b].push(index);
                    }
                }
            }
        }
        Self {
            parents,
            atom_nodes,
        }
    }

    /// The work indexing charged: one visit per node.
    #[must_use]
    pub fn work(&self) -> u64 {
        self.parents.len() as u64
    }

    /// Narrow the region to the fixed point of the three rules.
    ///
    /// # Errors
    /// Returns the stop when the narrowing exceeds its work or propagation
    /// ceiling, or control stops it; the region is then unchanged.
    pub fn narrow(
        &self,
        theory: &Theory,
        producers: Option<&Producers>,
        region: &mut Region,
        limits: RegionLimits,
        control: &Control,
    ) -> Result<(Narrowing, NarrowingStatistics), Stop> {
        self.narrow_with(theory, producers, None, region, limits, control)
    }

    /// Narrow a region of the theory's frozen reduct under a candidate: a
    /// node false in `truth`, the candidate's truth of every node, reads as
    /// falsum (`FerrarisMask`), and the rest of the DAG is read unchanged.
    /// No support cut applies, since a model of the reduct need not be
    /// supported: this narrows the proper-subset query, not the candidate
    /// tree.
    ///
    /// # Errors
    /// As [`Self::narrow`].
    pub fn narrow_frozen(
        &self,
        theory: &Theory,
        truth: &[bool],
        region: &mut Region,
        limits: RegionLimits,
        control: &Control,
    ) -> Result<(Narrowing, NarrowingStatistics), Stop> {
        self.narrow_with(theory, None, Some(truth), region, limits, control)
    }

    fn narrow_with(
        &self,
        theory: &Theory,
        producers: Option<&Producers>,
        frozen: Option<&[bool]>,
        region: &mut Region,
        limits: RegionLimits,
        control: &Control,
    ) -> Result<(Narrowing, NarrowingStatistics), Stop> {
        control.poll()?;
        let mut known = Known::empty(theory.nodes().len(), theory.atom_count());
        let mut work = Work::new(limits.max_work);
        let mut statistics = NarrowingStatistics::default();
        let closed = known.close(
            theory,
            self,
            producers,
            frozen,
            region,
            &mut work,
            limits.max_propagations,
            &mut statistics,
        )?;
        statistics.work = work.spent;
        if closed == Sweep::Contradiction {
            return Ok((Narrowing::Refuted, statistics));
        }
        let mut changed = false;
        for atom in region.open().collect::<Vec<_>>() {
            if known.atom_sure[atom] {
                region.hold(atom);
                statistics.forced += 1;
                changed = true;
            } else if known.atom_never[atom] {
                region.cut(atom);
                statistics.cut += 1;
                changed = true;
            }
        }
        if let Some(atom) = self.most_constrained(region, &known, &mut work)? {
            region.prefer(atom);
        }
        statistics.work = work.spent;
        Ok((Narrowing::Fixed { changed }, statistics))
    }

    /// The open atom with the most parents still unknown, the one whose
    /// decision the theory is most sensitive to; ties go to the lower atom.
    /// This chooses the split, as the clause search branches on the
    /// variable with the most unresolved occurrences.
    fn most_constrained(
        &self,
        region: &Region,
        known: &Known,
        work: &mut Work,
    ) -> Result<Option<usize>, Stop> {
        let mut best: Option<(usize, usize)> = None;
        for atom in region.open() {
            let mut unknown = 0;
            for &node in &self.atom_nodes[atom] {
                for &parent in &self.parents[node] {
                    work.tick()?;
                    if !known.sure[parent] && !known.never[parent] {
                        unknown += 1;
                    }
                }
            }
            if best.is_none_or(|(_, count)| unknown > count) {
                best = Some((atom, unknown));
            }
        }
        Ok(best.map(|(atom, _)| atom))
    }
}

/// Narrow the region with a fresh index of the theory; for one narrowing
/// of a theory. A proposer keeps a [`Narrower`] and narrows many regions.
///
/// # Errors
/// As [`Narrower::narrow`].
pub fn narrow(
    theory: &Theory,
    producers: Option<&Producers>,
    region: &mut Region,
    limits: RegionLimits,
    control: &Control,
) -> Result<(Narrowing, NarrowingStatistics), Stop> {
    Narrower::new(theory).narrow(theory, producers, region, limits, control)
}

/// What every candidate of the region must make of each node and each
/// atom: known to hold, known to fail, or open. One bit pair over the
/// DAG and one over the atoms, closed under the upward rules from a node's
/// operands and the downward rules from a node's own knowledge until
/// nothing changes: unit propagation on the theory itself, with a node
/// shared by several parents known once for all of them
/// (`FormulaBounds.Known`, `known_sound`). The closure is driven by a
/// worklist: a node or atom that learns something is revisited once, and
/// only its parents, operands and dependent producers are read.
struct Known {
    sure: Vec<bool>,
    never: Vec<bool>,
    atom_sure: Vec<bool>,
    atom_never: Vec<bool>,
    /// Nodes that learned something and have not been revisited.
    nodes: Vec<usize>,
    /// Atoms whose support must be rechecked.
    heads: Vec<usize>,
}

/// What a step of the closure did.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Sweep {
    Unchanged,
    Changed,
    /// A node or atom became known both to hold and to fail.
    Contradiction,
}

impl Sweep {
    fn join(self, other: Self) -> Self {
        match (self, other) {
            (Self::Contradiction, _) | (_, Self::Contradiction) => Self::Contradiction,
            (Self::Changed, _) | (_, Self::Changed) => Self::Changed,
            (Self::Unchanged, Self::Unchanged) => Self::Unchanged,
        }
    }
}

fn learn(known: &mut [bool], opposite: &[bool], index: usize) -> Sweep {
    if known[index] {
        Sweep::Unchanged
    } else if opposite[index] {
        Sweep::Contradiction
    } else {
        known[index] = true;
        Sweep::Changed
    }
}

impl Known {
    fn empty(nodes: usize, atoms: usize) -> Self {
        Self {
            sure: vec![false; nodes],
            never: vec![false; nodes],
            atom_sure: vec![false; atoms],
            atom_never: vec![false; atoms],
            nodes: Vec::new(),
            heads: Vec::new(),
        }
    }

    /// A node learns to hold; it is revisited if that is new.
    fn sure(&mut self, index: usize) -> Sweep {
        let step = learn(&mut self.sure, &self.never, index);
        if step == Sweep::Changed {
            self.nodes.push(index);
        }
        step
    }

    /// A node learns to fail; it is revisited if that is new.
    fn never(&mut self, index: usize) -> Sweep {
        let step = learn(&mut self.never, &self.sure, index);
        if step == Sweep::Changed {
            self.nodes.push(index);
        }
        step
    }

    /// An atom learns to hold or to fail, and every node carrying it
    /// learns the same. A newly held atom has its support rechecked.
    fn atom(&mut self, index: &Narrower, atom: usize, value: bool) -> Sweep {
        let step = if value {
            learn(&mut self.atom_sure, &self.atom_never, atom)
        } else {
            learn(&mut self.atom_never, &self.atom_sure, atom)
        };
        if step != Sweep::Changed {
            return step;
        }
        let mut step = step;
        for &node in &index.atom_nodes[atom] {
            step = step.join(if value {
                self.sure(node)
            } else {
                self.never(node)
            });
        }
        if value {
            self.heads.push(atom);
        }
        step
    }

    /// Close the knowledge from the region's decisions, falsum and the
    /// roots. Each event on the worklist follows a new bit, or is one of
    /// the initial seeds, so the events are bounded by the bits.
    #[allow(clippy::too_many_arguments)]
    fn close(
        &mut self,
        theory: &Theory,
        index: &Narrower,
        producers: Option<&Producers>,
        frozen: Option<&[bool]>,
        region: &Region,
        work: &mut Work,
        max_propagations: u64,
        statistics: &mut NarrowingStatistics,
    ) -> Result<Sweep, Stop> {
        let nodes = theory.nodes();
        let mut step = Sweep::Unchanged;
        for (node, kind) in nodes.iter().enumerate() {
            let falsum = matches!(kind, Node::False) || frozen.is_some_and(|truth| !truth[node]);
            if falsum {
                step = step.join(self.never(node));
            }
        }
        for &root in theory.roots() {
            step = step.join(self.sure(root));
        }
        for atom in 0..theory.atom_count() {
            if let Some(value) = region.decision(atom) {
                step = step.join(self.atom(index, atom, value));
            }
        }
        if producers.is_some() {
            self.heads.extend(0..theory.atom_count());
        }
        if step == Sweep::Contradiction {
            return Ok(step);
        }
        loop {
            if statistics.propagations >= max_propagations {
                return Err(Stop::WorkLimit);
            }
            let step = if let Some(node) = self.nodes.pop() {
                statistics.propagations += 1;
                self.revisit(nodes, index, producers, frozen, node, work)?
            } else if let Some(atom) = self.heads.pop() {
                statistics.propagations += 1;
                match producers {
                    Some(producers) => self.recheck(index, producers, atom, work)?,
                    None => Sweep::Unchanged,
                }
            } else {
                return Ok(Sweep::Changed);
            };
            if step == Sweep::Contradiction {
                return Ok(step);
            }
        }
    }

    /// A node that learned something teaches its operands what the
    /// connective leaves them, tells its atom, lets each parent learn from
    /// its operands, and, when it is a body that fails, has the producers'
    /// heads rechecked. A node false under a frozen mask is falsum in the
    /// reduct, a constant with no operands: it teaches nothing and learns
    /// nothing from them, and a parent under the mask likewise.
    fn revisit(
        &mut self,
        nodes: &[Node],
        index: &Narrower,
        producers: Option<&Producers>,
        frozen: Option<&[bool]>,
        node: usize,
        work: &mut Work,
    ) -> Result<Sweep, Stop> {
        work.tick()?;
        let masked = |node: usize| frozen.is_some_and(|truth| !truth[node]);
        let mut step = Sweep::Unchanged;
        if masked(node) {
            for &parent in &index.parents[node] {
                work.tick()?;
                if !masked(parent) {
                    step = step.join(self.learn_from_operands(nodes, parent));
                }
            }
            return Ok(step);
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
                    self.atom(index, atom, true)
                }
                Node::False => Sweep::Contradiction,
                Node::And(a, b) => self.sure(a).join(self.sure(b)),
                Node::Or(a, b) => {
                    if self.never[a] {
                        self.sure(b)
                    } else if self.never[b] {
                        self.sure(a)
                    } else {
                        Sweep::Unchanged
                    }
                }
                Node::Implies(a, b) => {
                    if self.sure[a] {
                        self.sure(b)
                    } else if self.never[b] {
                        self.never(a)
                    } else {
                        Sweep::Unchanged
                    }
                }
            });
        }
        if self.never[node] {
            step = step.join(match nodes[node] {
                Node::Atom(atom) => self.atom(index, atom, false),
                Node::False => Sweep::Unchanged,
                Node::And(a, b) => {
                    if self.sure[a] {
                        self.never(b)
                    } else if self.sure[b] {
                        self.never(a)
                    } else {
                        Sweep::Unchanged
                    }
                }
                Node::Or(a, b) => self.never(a).join(self.never(b)),
                Node::Implies(a, b) => self.sure(a).join(self.never(b)),
            });
            if let Some(producers) = producers {
                for &producer in &producers.by_body[node] {
                    self.heads
                        .extend(producers.rules[producer].heads.iter().copied());
                }
            }
        }
        for &parent in &index.parents[node] {
            work.tick()?;
            if masked(parent) {
                continue;
            }
            step = step.join(self.learn_from_operands(nodes, parent));
        }
        Ok(step)
    }

    /// A node learns from its operands what the connective dictates.
    fn learn_from_operands(&mut self, nodes: &[Node], node: usize) -> Sweep {
        match nodes[node] {
            Node::Atom(_) | Node::False => Sweep::Unchanged,
            Node::And(a, b) => {
                let mut up = Sweep::Unchanged;
                if self.sure[a] && self.sure[b] {
                    up = up.join(self.sure(node));
                }
                if self.never[a] || self.never[b] {
                    up = up.join(self.never(node));
                }
                up
            }
            Node::Or(a, b) => {
                let mut up = Sweep::Unchanged;
                if self.sure[a] || self.sure[b] {
                    up = up.join(self.sure(node));
                }
                if self.never[a] && self.never[b] {
                    up = up.join(self.never(node));
                }
                up
            }
            Node::Implies(a, b) => {
                let mut up = Sweep::Unchanged;
                if self.never[a] || self.sure[b] {
                    up = up.join(self.sure(node));
                }
                if self.sure[a] && self.never[b] {
                    up = up.join(self.never(node));
                }
                up
            }
        }
    }

    /// An atom none of its producers can support is known to fail, and an
    /// atom known to hold with exactly one producer able to support it
    /// demands that producer's body (`unsupported_cut`,
    /// `sole_support_forces`). A producer can support its atom when its
    /// body is not known to fail and, unless it is a choice, no other of
    /// its heads is known to hold.
    fn recheck(
        &mut self,
        index: &Narrower,
        producers: &Producers,
        atom: usize,
        work: &mut Work,
    ) -> Result<Sweep, Stop> {
        if self.atom_never[atom] {
            return Ok(Sweep::Unchanged);
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
            (0, _) => self.atom(index, atom, false),
            (1, Some(body)) if self.atom_sure[atom] => self.sure(body),
            _ => Sweep::Unchanged,
        })
    }
}
