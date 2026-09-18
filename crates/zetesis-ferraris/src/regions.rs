//! Regions of candidates over a formula theory, narrowed by its readings.
//!
//! A region holds some atoms in, cuts some out and leaves the rest open. A
//! formula has two readings under a region, decided by one pass over the
//! DAG: it is *sure* when every seed of the region satisfies it and
//! *impossible* when none does, with a held atom sure, a cut atom
//! impossible, and the connectives combining the readings as the closure
//! route's definite and possible gates do (`FormulaBounds.read`).
//!
//! One narrowing pass applies three rules, each sound for stable models:
//! an impossible root refutes the region (`never_root_refutes`); a root not
//! yet sure with exactly one open atom is decided by that atom, held when
//! the root is impossible with it cut and cut when impossible with it held,
//! which covers a rule whose body is sure and a constraint with one open
//! premise (`sure_body_forces`, `sure_body_refutes`, both classical
//! consequences); and in the producer fragment of the support restriction,
//! an atom none of whose producers can support it, each having an impossible
//! body or another head held, is cut (`unsupported_cut`). A held atom that
//! is cut refutes. Passes repeat until one changes nothing, so the result is
//! a fixed point; each pass decides at least one more atom or is the last.
//!
//! Work is charged per node read, per root tested and per producer checked,
//! against `RegionLimits`, and control is polled once per pass.

use std::collections::BTreeSet;

use zetesis_cpu::{Control, Stop};

use crate::{Node, Theory};

/// Ceilings on one narrowing and on producer extraction.
#[derive(Clone, Copy, Debug)]
pub struct RegionLimits {
    /// Charged node reads, root tests and producer checks.
    pub max_work: u64,
    /// Passes over the theory in one narrowing; each decides an atom or is the last.
    pub max_passes: u64,
}
impl Default for RegionLimits {
    fn default() -> Self {
        Self {
            max_work: 100_000_000,
            max_passes: 1_000_000,
        }
    }
}

/// A region of candidates: every atom held, cut or open.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Region {
    decided: Vec<Option<bool>>,
}
impl Region {
    /// The region in which nothing is decided: every candidate lies in it.
    #[must_use]
    pub fn undecided(theory: &Theory) -> Self {
        Self {
            decided: vec![None; theory.atom_count()],
        }
    }
    /// Hold an atom in every seed; `false` when the atom is cut.
    pub fn hold(&mut self, atom: usize) -> bool {
        self.decide(atom, true)
    }
    /// Cut an atom from every seed; `false` when the atom is held.
    pub fn cut(&mut self, atom: usize) -> bool {
        self.decide(atom, false)
    }
    fn decide(&mut self, atom: usize, value: bool) -> bool {
        match self.decided.get(atom) {
            Some(None) => {
                self.decided[atom] = Some(value);
                true
            }
            Some(Some(decided)) => *decided == value,
            None => false,
        }
    }
    /// Whether every seed holds the atom.
    #[must_use]
    pub fn is_held(&self, atom: usize) -> bool {
        self.decided.get(atom).copied().flatten() == Some(true)
    }
    /// Whether no seed holds the atom.
    #[must_use]
    pub fn is_cut(&self, atom: usize) -> bool {
        self.decided.get(atom).copied().flatten() == Some(false)
    }
    /// Whether the atom is undecided: some seeds hold it and some do not.
    #[must_use]
    pub fn is_open(&self, atom: usize) -> bool {
        self.decided.get(atom).is_some_and(Option::is_none)
    }
    /// The highest open atom, the one a split decides first so that leaves
    /// come in the counter's order.
    #[must_use]
    pub fn highest_open(&self) -> Option<usize> {
        self.decided.iter().rposition(Option::is_none)
    }
    /// The atoms every seed holds, ascending.
    pub fn held(&self) -> impl Iterator<Item = usize> + '_ {
        self.decided
            .iter()
            .enumerate()
            .filter(|(_, decision)| **decision == Some(true))
            .map(|(atom, _)| atom)
    }
    /// The two regions a fresh atom splits this one into: out, then in
    /// (`Search.split_partition`, `split_disjoint`).
    #[must_use]
    pub fn split(&self, atom: usize) -> (Self, Self) {
        let mut cut = self.clone();
        cut.decided[atom] = Some(false);
        let mut held = self.clone();
        held.decided[atom] = Some(true);
        (cut, held)
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
    producers: Vec<Producer>,
    by_head: Vec<Vec<usize>>,
}

/// Extract the producers when every root is an implication, or a bare head,
/// whose head is a disjunction of atoms with falsum, an atomic choice, or
/// falsum: the fragment `DisjunctiveSupport.Covered` names, with a fact's
/// head standing alone. `None` when a
/// root lies outside it, in which case the support rule does not apply.
///
/// # Errors
/// Returns the stop when extraction exceeds `limits.max_work` or control stops.
pub fn producers(
    theory: &Theory,
    limits: RegionLimits,
    control: &Control,
) -> Result<Option<Producers>, Stop> {
    control.poll()?;
    let mut work = Work::new(limits.max_work);
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
    let mut producers = Vec::new();
    let mut by_head = vec![Vec::new(); theory.atom_count()];
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
        let index = producers.len();
        for &atom in &heads {
            by_head[atom].push(index);
        }
        producers.push(Producer {
            body,
            heads,
            choice,
        });
    }
    Ok(Some(Producers { producers, by_head }))
}

/// The outcome of one narrowing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Narrowing {
    /// No seed of the region is a stable model.
    Refuted,
    /// The fixed point, and whether a pass decided an atom.
    Fixed {
        /// Whether any atom was decided by the narrowing.
        changed: bool,
    },
}

/// The work one narrowing charged.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NarrowingStatistics {
    /// Charged node reads, root tests and producer checks.
    pub work: u64,
    /// Completed passes.
    pub passes: u64,
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

/// The readings of every node under a region: sure, impossible, and the
/// number of open-atom occurrences that can still move it, saturating at
/// two, with the one open atom when that number is one. A node already
/// sure or impossible has none: its open atoms below cannot move it.
struct Readings {
    sure: Vec<bool>,
    never: Vec<bool>,
    open: Vec<u8>,
    atom: Vec<usize>,
}

impl Readings {
    fn empty(nodes: usize) -> Self {
        Self {
            sure: vec![false; nodes],
            never: vec![false; nodes],
            open: vec![0; nodes],
            atom: vec![0; nodes],
        }
    }

    /// Read every node of the theory under the region, in index order; a
    /// node's operands precede it. `tentative` reads one open atom as decided.
    fn read(
        &mut self,
        theory: &Theory,
        region: &Region,
        tentative: Option<(usize, bool)>,
        work: &mut Work,
    ) -> Result<(), Stop> {
        for (index, node) in theory.nodes().iter().enumerate() {
            work.tick()?;
            let (sure, never, open, atom) = match *node {
                Node::Atom(atom) => {
                    let decision = match tentative {
                        Some((tentative_atom, value)) if tentative_atom == atom => Some(value),
                        _ => region.decided[atom],
                    };
                    match decision {
                        Some(true) => (true, false, 0, 0),
                        Some(false) => (false, true, 0, 0),
                        None => (false, false, 1, atom),
                    }
                }
                Node::False => (false, true, 0, 0),
                Node::And(a, b) => (
                    self.sure[a] && self.sure[b],
                    self.never[a] || self.never[b],
                    self.open[a].saturating_add(self.open[b]),
                    if self.open[a] > 0 {
                        self.atom[a]
                    } else {
                        self.atom[b]
                    },
                ),
                Node::Or(a, b) => (
                    self.sure[a] || self.sure[b],
                    self.never[a] && self.never[b],
                    self.open[a].saturating_add(self.open[b]),
                    if self.open[a] > 0 {
                        self.atom[a]
                    } else {
                        self.atom[b]
                    },
                ),
                Node::Implies(a, b) => (
                    self.never[a] || self.sure[b],
                    self.sure[a] && self.never[b],
                    self.open[a].saturating_add(self.open[b]),
                    if self.open[a] > 0 {
                        self.atom[a]
                    } else {
                        self.atom[b]
                    },
                ),
            };
            self.sure[index] = sure;
            self.never[index] = never;
            self.open[index] = if sure || never { 0 } else { open.min(2) };
            self.atom[index] = atom;
        }
        Ok(())
    }
}

/// Narrow the region to the fixed point of the three rules.
///
/// # Errors
/// Returns the stop when the narrowing exceeds its work or pass ceiling, or
/// control stops it; the region then keeps the completed passes' decisions.
pub fn narrow(
    theory: &Theory,
    producers: Option<&Producers>,
    region: &mut Region,
    limits: RegionLimits,
    control: &Control,
) -> Result<(Narrowing, NarrowingStatistics), Stop> {
    let mut pass = Pass {
        theory,
        producers,
        work: Work::new(limits.max_work),
        readings: Readings::empty(theory.nodes().len()),
        tentative: Readings::empty(theory.nodes().len()),
        decided: Vec::new(),
    };
    let mut statistics = NarrowingStatistics::default();
    let mut changed = false;
    let narrowing = loop {
        control.poll()?;
        if statistics.passes >= limits.max_passes {
            return Err(Stop::WorkLimit);
        }
        let narrowing = pass.run(region)?;
        statistics.passes += 1;
        statistics.work = pass.work.spent;
        if narrowing == Narrowing::Refuted {
            break Narrowing::Refuted;
        }
        if pass.decided.is_empty() {
            break Narrowing::Fixed { changed };
        }
        for &(atom, value) in &pass.decided {
            if region.is_open(atom) {
                region.decided[atom] = Some(value);
                if value {
                    statistics.forced += 1;
                } else {
                    statistics.cut += 1;
                }
                changed = true;
            } else if region.is_held(atom) != value {
                return Ok((Narrowing::Refuted, statistics));
            }
        }
    };
    Ok((narrowing, statistics))
}

/// One pass of the three rules over a region, with its working storage.
struct Pass<'t> {
    theory: &'t Theory,
    producers: Option<&'t Producers>,
    work: Work,
    readings: Readings,
    tentative: Readings,
    /// The decisions the pass proposes, applied after it: the readings a
    /// pass tests are those of the region it started from.
    decided: Vec<(usize, bool)>,
}

impl Pass<'_> {
    /// Read the region and propose its decisions, or refute it. `Fixed` here
    /// reports whether the pass proposed any decision.
    fn run(&mut self, region: &Region) -> Result<Narrowing, Stop> {
        self.decided.clear();
        self.readings
            .read(self.theory, region, None, &mut self.work)?;
        // Roots: an impossible root refutes; a unit root decides its atom.
        for &root in self.theory.roots() {
            self.work.tick()?;
            if self.readings.never[root] {
                return Ok(Narrowing::Refuted);
            }
            if self.readings.sure[root] || self.readings.open[root] != 1 {
                continue;
            }
            let atom = self.readings.atom[root];
            let never_cut = self.impossible_with(region, root, atom, false)?;
            let never_held = self.impossible_with(region, root, atom, true)?;
            match (never_cut, never_held) {
                (true, true) => return Ok(Narrowing::Refuted),
                (true, false) => self.decided.push((atom, true)),
                (false, true) => self.decided.push((atom, false)),
                (false, false) => {}
            }
        }
        // Producers: an atom none can support is cut.
        if let Some(producers) = self.producers {
            for atom in (0..self.theory.atom_count()).filter(|&atom| region.is_open(atom)) {
                if !self.supportable(producers, region, atom)? {
                    self.decided.push((atom, false));
                }
            }
        }
        Ok(Narrowing::Fixed {
            changed: !self.decided.is_empty(),
        })
    }

    /// Whether the root is impossible once the atom is read as decided.
    fn impossible_with(
        &mut self,
        region: &Region,
        root: usize,
        atom: usize,
        value: bool,
    ) -> Result<bool, Stop> {
        self.tentative
            .read(self.theory, region, Some((atom, value)), &mut self.work)?;
        Ok(self.tentative.never[root])
    }

    /// Whether some producer of the atom may still support it: its body is
    /// not impossible and, unless it is a choice, no other head is held.
    fn supportable(
        &mut self,
        producers: &Producers,
        region: &Region,
        atom: usize,
    ) -> Result<bool, Stop> {
        for &index in &producers.by_head[atom] {
            self.work.tick()?;
            let producer = &producers.producers[index];
            let body_impossible = producer.body.is_some_and(|body| self.readings.never[body]);
            let other_held = !producer.choice
                && producer
                    .heads
                    .iter()
                    .any(|&head| head != atom && region.is_held(head));
            if !body_impossible && !other_held {
                return Ok(true);
            }
        }
        Ok(false)
    }
}
