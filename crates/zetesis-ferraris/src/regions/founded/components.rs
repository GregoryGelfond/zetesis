//! Exact cyclic heads from existing positive incidences, without another graph.
//!
//! Reverse DFS reads each vertex's one predecessor list: producer bodies for
//! atoms, the denoted atom for an Atom node, or flattened leaves for a Boolean
//! node. Opaque bodies have no predecessors. Atom cursors reuse dependencies;
//! node cursors use Visiting while owner is absent. Queue holds the DFS stack
//! and `body_nodes` temporarily holds finishing order, at most A+B entries.
//! Forward searches then reuse queue and the same original outgoing postings.
//! No component IDs survive: only the packed cyclic-atom mask is retained.
//!
//! A component of the complete dependencies keeps its heads only when one of
//! its cycles survives without the circular producers ([`super::super::circular`]).
//! A component whose every cycle passes through a circular producer needs no
//! closure: the support cut, which passes over those producers, already
//! decides its atoms, so they are seeds like any acyclic head. A component
//! with a surviving cycle is kept whole, the atoms it holds only through a
//! circular producer included, and its closure reads every producer as before.
//! With marked producers the search therefore runs twice, first over the
//! supporting dependencies to find the surviving cycles, then over all of
//! them; an inventory without marks is searched once, exactly as before.

use zetesis_algebra::{Failure, Quota};

use zetesis_algebra::words::Words;

use super::{
    Meter, Narrower, NodeView, Producers, Scratch, State, Theory, bit, chain_position, flag_words,
    reserve,
};
use zetesis_algebra::Stop;

/// The producer dependencies one component search follows.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Dependencies {
    /// Every producer's dependency of its heads on its body.
    Every,
    /// Only those of producers that can support their head.
    Supporting,
}

impl Dependencies {
    fn follows(self, producers: &Producers, row: usize) -> bool {
        self == Self::Every || !producers.circular(row)
    }
}

impl Scratch {
    pub(super) fn prepare_components<Q: Quota + ?Sized>(
        &mut self,
        theory: &Theory,
        index: &Narrower,
        producers: &Producers,
        work: &mut Meter<'_, Q>,
    ) -> Result<(), Failure<Q::Error>> {
        debug_assert!(
            self.owner.is_none(),
            "component scratch is never authoritative"
        );
        let included = self.body_nodes.len();
        if !producers.any_circular() {
            return self.search_components(
                (theory, index, producers),
                included,
                Dependencies::Every,
                None,
                work,
            );
        }
        self.search_components(
            (theory, index, producers),
            included,
            Dependencies::Supporting,
            None,
            work,
        )?;
        // The atoms on a cycle that survives without circular producers.
        let mut surviving = Vec::new();
        reserve(&mut surviving, self.cyclic_atoms.len())?;
        for &word in &self.cyclic_atoms {
            work.tick()?;
            surviving.push(word);
        }
        // The first search left every included node discovered.
        for state in &mut self.states {
            work.tick()?;
            if matches!(state, State::True) {
                *state = State::Awaiting(0);
            }
        }
        self.search_components(
            (theory, index, producers),
            included,
            Dependencies::Every,
            Some(&surviving),
            work,
        )
    }

    /// One search for the components of the followed dependencies. With
    /// `surviving`, a component keeps its heads only when it holds one of
    /// those atoms.
    fn search_components<Q: Quota + ?Sized>(
        &mut self,
        (theory, index, producers): (&Theory, &Narrower, &Producers),
        included: usize,
        dependencies: Dependencies,
        surviving: Option<&[u64]>,
        work: &mut Meter<'_, Q>,
    ) -> Result<(), Failure<Q::Error>> {
        let atoms = theory.atom_count();
        let projected = atoms.checked_add(included).ok_or(Stop::Allocation)?;
        let vertices = atoms
            .checked_add(theory.nodes().len())
            .ok_or(Stop::Allocation)?;
        self.queue.clear();
        reserve(&mut self.queue, projected)?;
        reserve(&mut self.body_nodes, projected)?;
        self.body_nodes.clear();
        self.dependencies.clear();
        reserve(&mut self.dependencies, atoms)?;
        for _ in 0..atoms {
            work.tick()?;
            self.dependencies.push(0);
        }
        Self::zero_words(&mut self.atoms, atoms, work)?;
        Self::zero_words(&mut self.cyclic_atoms, atoms, work)?;
        for root in 0..vertices {
            work.tick()?;
            if (root >= atoms && matches!(self.states[root - atoms], State::Outside))
                || self.discovered(root, atoms)
            {
                continue;
            }
            self.discover_reverse(root, atoms);
            while let Some(&vertex) = self.queue.last() {
                work.tick()?;
                if let Some(previous) =
                    self.predecessor(vertex, (theory, index, producers), dependencies, work)?
                {
                    if !self.discovered(previous, atoms) {
                        self.discover_reverse(previous, atoms);
                    }
                } else {
                    self.queue.pop();
                    self.body_nodes.push(vertex);
                }
            }
        }
        if self.body_nodes.len() != projected {
            return Err(Failure::Stopped(Stop::InvalidProgram));
        }
        Self::zero_words(&mut self.atoms, atoms, work)?;
        for &vertex in &self.body_nodes {
            work.tick()?;
            if vertex >= atoms {
                self.states[vertex - atoms] = State::Awaiting(0);
            }
        }
        self.classify_components((theory, index, producers), dependencies, surviving, work)
    }

    fn zero_words<Q: Quota + ?Sized>(
        words: &mut Words<u64>,
        atoms: usize,
        work: &mut Meter<'_, Q>,
    ) -> Result<(), Failure<Q::Error>> {
        let words = words.storage_mut();
        words.clear();
        reserve(words, flag_words(atoms))?;
        for _ in 0..flag_words(atoms) {
            work.tick()?;
            words.push(0);
        }
        Ok(())
    }

    fn discovered(&self, vertex: usize, atoms: usize) -> bool {
        if vertex < atoms {
            self.atoms.contains(vertex)
        } else {
            matches!(
                self.states[vertex - atoms],
                State::Visiting(_) | State::True
            )
        }
    }

    fn discover_reverse(&mut self, vertex: usize, atoms: usize) {
        if vertex < atoms {
            self.atoms.insert(vertex);
        } else {
            self.states[vertex - atoms] = State::Visiting(0);
        }
        self.queue.push(vertex);
    }

    /// The cursor advances only after the inspected incidence is charged.
    fn predecessor<Q: Quota + ?Sized>(
        &mut self,
        vertex: usize,
        (theory, index, producers): (&Theory, &Narrower, &Producers),
        dependencies: Dependencies,
        work: &mut Meter<'_, Q>,
    ) -> Result<Option<usize>, Failure<Q::Error>> {
        let atoms = theory.atom_count();
        if vertex < atoms {
            while let Some(&row) = producers.by_head[vertex].get(self.dependencies[vertex]) {
                work.tick()?;
                self.dependencies[vertex] += 1;
                if !dependencies.follows(producers, row) {
                    continue;
                }
                if let Some(body) = producers.rules[row].body {
                    if matches!(self.states[body], State::Outside) {
                        return Err(Failure::Stopped(Stop::InvalidProgram));
                    }
                    return Ok(Some(atoms + body));
                }
            }
            return Ok(None);
        }
        let node = vertex - atoms;
        let State::Visiting(next) = self.states[node] else {
            return Err(Failure::Stopped(Stop::InvalidProgram));
        };
        let previous = match theory.view().node(node).map_err(|_| Stop::InvalidProgram)? {
            NodeView::Atom(atom) => (next == 0).then_some(atom),
            NodeView::And(_) | NodeView::Or(_) => {
                let chain = index.chain_of[node].ok_or(Stop::InvalidProgram)?;
                index.chains[chain_position(chain)]
                    .leaves(&index.extrema)
                    .get(next)
                    .map(|&child| atoms + child)
            }
            _ => None,
        };
        if previous.is_some() {
            work.tick()?;
            self.states[node] = State::Visiting(next + 1);
        }
        Ok(previous)
    }

    fn classify_components<Q: Quota + ?Sized>(
        &mut self,
        (theory, index, producers): (&Theory, &Narrower, &Producers),
        dependencies: Dependencies,
        surviving: Option<&[u64]>,
        work: &mut Meter<'_, Q>,
    ) -> Result<(), Failure<Q::Error>> {
        let atoms = theory.atom_count();
        // The atoms of the component being searched, kept only to release
        // them together when none of them is on a surviving cycle.
        let mut component = Vec::new();
        if surviving.is_some() {
            reserve(&mut component, atoms)?;
        }
        while let Some(root) = self.body_nodes.pop() {
            work.tick()?;
            if self.discovered(root, atoms) {
                continue;
            }
            self.discover_forward(root, atoms);
            let mut members = 0_usize;
            let mut survives = surviving.is_none();
            component.clear();
            while let Some(vertex) = self.queue.pop() {
                work.tick()?;
                members += 1;
                if let Some(surviving) = surviving
                    && vertex < atoms
                {
                    work.tick()?;
                    survives |= bit(surviving, vertex);
                    component.push(vertex);
                }
                self.successors(vertex, (theory, index, producers), dependencies, work)?;
            }
            // Atom vertices point only to separate formula vertices. Thus an
            // atom in a singleton component cannot have a positive self edge.
            if members == 1 && root < atoms {
                self.cyclic_atoms.remove(root);
            } else if !survives {
                for &atom in &component {
                    work.tick()?;
                    self.cyclic_atoms.remove(atom);
                }
            }
        }
        Ok(())
    }

    fn discover_forward(&mut self, vertex: usize, atoms: usize) {
        if self.discovered(vertex, atoms) {
            return;
        }
        if vertex < atoms {
            self.atoms.insert(vertex);
            self.cyclic_atoms.insert(vertex);
        } else {
            self.states[vertex - atoms] = State::True;
        }
        self.queue.push(vertex);
    }

    fn successors<Q: Quota + ?Sized>(
        &mut self,
        vertex: usize,
        (theory, index, producers): (&Theory, &Narrower, &Producers),
        dependencies: Dependencies,
        work: &mut Meter<'_, Q>,
    ) -> Result<(), Failure<Q::Error>> {
        let atoms = theory.atom_count();
        if vertex < atoms {
            for &node in &index.atom_nodes[vertex] {
                work.tick()?;
                if !matches!(self.states[node], State::Outside) {
                    self.discover_forward(atoms + node, atoms);
                }
            }
            return Ok(());
        }
        let node = vertex - atoms;
        for &parent in &index.parents[node] {
            work.tick()?;
            if !matches!(self.states[parent], State::Outside)
                && matches!(
                    theory
                        .view()
                        .node(parent)
                        .map_err(|_| Stop::InvalidProgram)?,
                    NodeView::And(_) | NodeView::Or(_)
                )
            {
                self.discover_forward(atoms + parent, atoms);
            }
        }
        for &row in &producers.by_body[node] {
            work.tick()?;
            if !dependencies.follows(producers, row) {
                continue;
            }
            for &head in &producers.rules[row].heads {
                work.tick()?;
                self.discover_forward(head, atoms);
            }
        }
        Ok(())
    }
}
