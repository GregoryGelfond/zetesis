//! Exact cyclic heads from existing positive incidences, without another graph.
//!
//! Reverse DFS reads each vertex's one predecessor list: producer bodies for
//! atoms, the denoted atom for an Atom node, or flattened leaves for a Boolean
//! node. Opaque bodies have no predecessors. Atom cursors reuse dependencies;
//! node cursors use Visiting while owner is absent. Queue holds the DFS stack
//! and `body_nodes` temporarily holds finishing order, at most A+B entries.
//! Forward searches then reuse queue and the same original outgoing postings.
//! No component IDs survive: only the packed cyclic-atom mask is retained.

use super::{
    Narrower, NodeView, Producers, Scratch, State, Theory, Work, bit, chain_position, flag_words,
    reserve,
};
use zetesis_cpu::Stop;

impl Scratch {
    pub(super) fn prepare_components(
        &mut self,
        theory: &Theory,
        index: &Narrower,
        producers: &Producers,
        work: &mut Work<'_>,
    ) -> Result<(), Stop> {
        debug_assert!(
            self.owner.is_none(),
            "component scratch is never authoritative"
        );
        let atoms = theory.atom_count();
        let projected = atoms
            .checked_add(self.body_nodes.len())
            .ok_or(Stop::Allocation)?;
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
                if let Some(previous) = self.predecessor(vertex, theory, index, producers, work)? {
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
            return Err(Stop::InvalidProgram);
        }
        Self::zero_words(&mut self.atoms, atoms, work)?;
        for &vertex in &self.body_nodes {
            work.tick()?;
            if vertex >= atoms {
                self.states[vertex - atoms] = State::Awaiting(0);
            }
        }
        self.classify_components(theory, index, producers, work)
    }

    fn zero_words(words: &mut Vec<u64>, atoms: usize, work: &mut Work<'_>) -> Result<(), Stop> {
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
            bit(&self.atoms, vertex)
        } else {
            matches!(
                self.states[vertex - atoms],
                State::Visiting(_) | State::True
            )
        }
    }

    fn discover_reverse(&mut self, vertex: usize, atoms: usize) {
        if vertex < atoms {
            self.atoms[vertex / 64] |= 1 << (vertex % 64);
        } else {
            self.states[vertex - atoms] = State::Visiting(0);
        }
        self.queue.push(vertex);
    }

    /// The cursor advances only after the inspected incidence is charged.
    fn predecessor(
        &mut self,
        vertex: usize,
        theory: &Theory,
        index: &Narrower,
        producers: &Producers,
        work: &mut Work<'_>,
    ) -> Result<Option<usize>, Stop> {
        let atoms = theory.atom_count();
        if vertex < atoms {
            while let Some(&row) = producers.by_head[vertex].get(self.dependencies[vertex]) {
                work.tick()?;
                self.dependencies[vertex] += 1;
                if let Some(body) = producers.rules[row].body {
                    if matches!(self.states[body], State::Outside) {
                        return Err(Stop::InvalidProgram);
                    }
                    return Ok(Some(atoms + body));
                }
            }
            return Ok(None);
        }
        let node = vertex - atoms;
        let State::Visiting(next) = self.states[node] else {
            return Err(Stop::InvalidProgram);
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

    fn classify_components(
        &mut self,
        theory: &Theory,
        index: &Narrower,
        producers: &Producers,
        work: &mut Work<'_>,
    ) -> Result<(), Stop> {
        let atoms = theory.atom_count();
        while let Some(root) = self.body_nodes.pop() {
            work.tick()?;
            if self.discovered(root, atoms) {
                continue;
            }
            self.discover_forward(root, atoms);
            let mut members = 0_usize;
            while let Some(vertex) = self.queue.pop() {
                work.tick()?;
                members += 1;
                self.successors(vertex, theory, index, producers, work)?;
            }
            // Atom vertices point only to separate formula vertices. Thus an
            // atom in a singleton component cannot have a positive self edge.
            if members == 1 && root < atoms {
                self.cyclic_atoms[root / 64] &= !(1 << (root % 64));
            }
        }
        Ok(())
    }

    fn discover_forward(&mut self, vertex: usize, atoms: usize) {
        if self.discovered(vertex, atoms) {
            return;
        }
        if vertex < atoms {
            self.atoms[vertex / 64] |= 1 << (vertex % 64);
            self.cyclic_atoms[vertex / 64] |= 1 << (vertex % 64);
        } else {
            self.states[vertex - atoms] = State::True;
        }
        self.queue.push(vertex);
    }

    fn successors(
        &mut self,
        vertex: usize,
        theory: &Theory,
        index: &Narrower,
        producers: &Producers,
        work: &mut Work<'_>,
    ) -> Result<(), Stop> {
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
            for &head in &producers.rules[row].heads {
                work.tick()?;
                self.discover_forward(head, atoms);
            }
        }
        Ok(())
    }
}
