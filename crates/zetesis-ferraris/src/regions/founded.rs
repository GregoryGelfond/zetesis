//! Possible founded atoms of an original candidate region.
//!
//! Complete `Producers` coverage is mandatory. Relax a body by keeping its
//! positive atoms and Boolean chains, replacing nodes already known false by
//! falsum, and replacing other implications and aggregates by truth. Starting
//! empty, derive every uncut head of each enabled producer. Held atoms are not
//! seeds; disjunctive producers do not exclude competing held heads.
//!
//! Let P be the completed closure and M a stable interpretation compatible with
//! the original knowledge. For J = M intersect P, body reduct truth in J implies
//! relaxed body truth in P. Each enabled ordinary producer therefore puts all
//! its uncut heads in P, including a head true in M; its reduct holds in J.
//! A selected atomic choice works identically; an unselected choice's negative
//! alternative is true. Constraints satisfied by M have tautological reducts.
//! Thus J models the full reduct, and stability forces J = M. Consequently an
//! atom outside P is absent from every stable completion. This argument needs
//! complete roots and an upper relaxation, not head-cycle freedom or shifting.
//! The representation and executable refinement remain proof obligations.
//!
//! Each worker caches the positive Boolean cone of complete producer bodies,
//! bound to the exact original theory. Omitted formula positions stay inactive:
//! they have no relaxation path to any producer body and cannot affect P. Opaque
//! bodies remain leaves, and every region's known-false mask remains authoritative.
//! Preparation initializes node cells once; later passes reset only this cone.
//! The cache adds body-node/fact-row IDs, not a second incidence graph. Headers
//! are part of the owning scratch and capacities are included in retained bytes.
//!
//! One pass reuses the original atom/node and flattened-chain incidences and
//! producer body/head rows. Each atom/node activates at most once. For A atoms,
//! N nodes, E indexed Boolean incidences, R producers and H head occurrences,
//! preparation takes O(A + N + E + R + H) work and O(A + N) exclusive scratch.
//! A later pass initializes only cached body nodes/fact rows; existing atom-node
//! and parent postings are still visited and charged. All new cuts are
//! published together before ordinary propagation resumes. A later pass can
//! exploit newly false bodies; no pass installs a rule or changes the reduct.
//! Successfully closed region knowledge can omit a later pass while no atom
//! or node has become false. This conservative invalidation includes held
//! decisions that indirectly falsify bodies. The authority belongs to inherited
//! knowledge under its same-original subject contract, never worker scratch.
//! Unchanged false masks define the same upper-closure operator, and a pass
//! publishing no cuts has already included every uncut atom. True knowledge
//! alone changes neither premise; a later false bit requires recomputation.
//!
//! A cached Kahn pass over the relaxed positive dependencies skips all later
//! foundedness work when they are acyclic. Local support already suffices there:
//! every nonfalse atom has a possible producer, whose positive inputs have lower
//! dependency order and therefore enter P. The cycle test reuses exactly these
//! immutable incidences and adds no graph. Its O(A + N + E + R + H) work is charged
//! to the original narrowing that completes it; concurrent first users may each
//! perform a charged test. Only a completed Boolean result is shared.

use super::{Narrower, NodeView, Producers, Theory, Work, bit, chain_position, flag_words};
use std::mem::size_of;
use zetesis_cpu::Stop;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug)]
enum State {
    Inactive,
    Awaiting(usize),
    True,
}

/// Exclusive worklists and owner-bound body preparation. This is neither
/// inherited region knowledge nor a second incidence graph.
#[derive(Debug, Default)]
pub(super) struct Scratch {
    states: Vec<State>,
    atoms: Vec<u64>,
    queue: Vec<usize>,
    /// Atom indegrees for the once-per-original positive dependency test.
    dependencies: Vec<usize>,
    /// Only completed preparation authenticates these body-node/fact rows.
    owner: Option<Theory>,
    body_nodes: Vec<usize>,
    fact_rows: Vec<usize>,
    /// Test-only operation count; no closure authority is stored here.
    #[cfg(test)]
    passes: usize,
}

impl Scratch {
    pub(super) fn retained_bytes(&self) -> u128 {
        self.states.capacity() as u128 * size_of::<State>() as u128
            + self.atoms.capacity() as u128 * size_of::<u64>() as u128
            + (self.queue.capacity() as u128
                + self.dependencies.capacity() as u128
                + self.body_nodes.capacity() as u128
                + self.fact_rows.capacity() as u128)
                * size_of::<usize>() as u128
    }

    pub(super) fn contains(&self, atom: usize) -> bool {
        bit(&self.atoms, atom)
    }

    /// An acyclic positive projection cannot strengthen ordinary support. The
    /// existing subject contract binds both inventories to this exact original.
    pub(super) fn needed(
        &mut self,
        theory: &Theory,
        index: &Narrower,
        producers: &Producers,
        work: &mut Work<'_>,
    ) -> Result<bool, Stop> {
        if let Some(&cyclic) = producers.founded_cycles.get() {
            return Ok(cyclic);
        }
        let cyclic = self.cyclic(theory, index, producers, work)?;
        // Another worker may have completed the same immutable test. Failed
        // tests never reach publication, and no work or scratch is shared.
        Ok(*producers.founded_cycles.get_or_init(|| cyclic))
    }

    fn cyclic(
        &mut self,
        theory: &Theory,
        index: &Narrower,
        producers: &Producers,
        work: &mut Work<'_>,
    ) -> Result<bool, Stop> {
        // Cycle preparation uses the same state cells as body preparation.
        // Even an interrupted cycle pass invalidates their previous authority.
        self.owner = None;
        self.states.clear();
        self.queue.clear();
        self.dependencies.clear();
        let atoms = theory.atom_count();
        let vertices = atoms
            .checked_add(theory.nodes().len())
            .ok_or(Stop::Allocation)?;
        reserve(&mut self.states, theory.nodes().len())?;
        reserve(&mut self.dependencies, atoms)?;
        reserve(&mut self.queue, vertices)?;
        for _ in 0..atoms {
            work.tick()?;
            self.dependencies.push(0);
        }
        let nodes = theory.view();
        for node in 0..nodes.len() {
            work.tick()?;
            let degree = match nodes.node(node).map_err(|_| Stop::InvalidProgram)? {
                NodeView::Atom(_) => 1,
                NodeView::And(_) | NodeView::Or(_) if !index.absorbed[node] => {
                    let chain = index.chain_of[node].ok_or(Stop::InvalidProgram)?;
                    index.chains[chain_position(chain)]
                        .leaves(&index.extrema)
                        .len()
                }
                _ => 0,
            };
            self.states.push(State::Awaiting(degree));
        }
        for producer in &producers.rules {
            work.tick()?;
            if producer.body.is_some() {
                for &head in &producer.heads {
                    work.tick()?;
                    self.dependencies[head] = self.dependencies[head]
                        .checked_add(1)
                        .ok_or(Stop::Allocation)?;
                }
            }
        }
        for atom in 0..atoms {
            work.tick()?;
            if self.dependencies[atom] == 0 {
                self.queue.push(atom);
            }
        }
        for node in 0..nodes.len() {
            work.tick()?;
            if matches!(self.states[node], State::Awaiting(0)) {
                self.activate_node(node, atoms, work)?;
            }
        }
        let mut next = 0;
        while let Some(&vertex) = self.queue.get(next) {
            work.tick()?;
            next += 1;
            if vertex < atoms {
                for &node in &index.atom_nodes[vertex] {
                    work.tick()?;
                    self.remove_dependency(node, atoms, work)?;
                }
                continue;
            }
            let node = vertex - atoms;
            for &parent in &index.parents[node] {
                work.tick()?;
                if matches!(
                    nodes.node(parent).map_err(|_| Stop::InvalidProgram)?,
                    NodeView::And(_) | NodeView::Or(_)
                ) {
                    self.remove_dependency(parent, atoms, work)?;
                }
            }
            for &producer in &producers.by_body[node] {
                work.tick()?;
                for &head in &producers.rules[producer].heads {
                    work.tick()?;
                    self.dependencies[head] = self.dependencies[head]
                        .checked_sub(1)
                        .ok_or(Stop::InvalidProgram)?;
                    if self.dependencies[head] == 0 {
                        self.queue.push(head);
                    }
                }
            }
        }
        Ok(next != vertices)
    }

    fn remove_dependency(
        &mut self,
        node: usize,
        atoms: usize,
        work: &mut Work<'_>,
    ) -> Result<(), Stop> {
        match self.states[node] {
            State::Awaiting(1) => self.activate_node(node, atoms, work),
            State::Awaiting(remaining) if remaining > 1 => {
                self.states[node] = State::Awaiting(remaining - 1);
                Ok(())
            }
            _ => Err(Stop::InvalidProgram),
        }
    }

    /// Complete the upper closure before its caller may cut any atom. No state
    /// from an earlier theory, region or interrupted prefix is authoritative.
    pub(super) fn complete(
        &mut self,
        theory: &Theory,
        index: &Narrower,
        producers: &Producers,
        never: &[u64],
        atom_never: &[u64],
        work: &mut Work<'_>,
    ) -> Result<(), Stop> {
        #[cfg(test)]
        {
            self.passes += 1;
        }
        self.prepare_bodies(theory, index, producers, work)?;
        self.initialize(theory, index, never, work)?;
        for position in 0..self.fact_rows.len() {
            work.tick()?;
            let producer = &producers.rules[self.fact_rows[position]];
            for &head in &producer.heads {
                work.tick()?;
                self.activate_atom(head, atom_never, work)?;
            }
        }
        let atoms = theory.atom_count();
        let mut next = 0;
        while let Some(&vertex) = self.queue.get(next) {
            work.tick()?;
            next += 1;
            if vertex < atoms {
                for &node in &index.atom_nodes[vertex] {
                    work.tick()?;
                    if matches!(self.states[node], State::Awaiting(_)) {
                        self.activate_node(node, atoms, work)?;
                    }
                }
                continue;
            }
            let node = vertex - atoms;
            for &parent in &index.parents[node] {
                work.tick()?;
                match self.states[parent] {
                    State::Awaiting(1) => self.activate_node(parent, atoms, work)?,
                    State::Awaiting(remaining) => {
                        self.states[parent] = State::Awaiting(remaining - 1);
                    }
                    State::Inactive | State::True => {}
                }
            }
            for &producer in &producers.by_body[node] {
                work.tick()?;
                for &head in &producers.rules[producer].heads {
                    work.tick()?;
                    self.activate_atom(head, atom_never, work)?;
                }
            }
        }
        Ok(())
    }

    /// Restrict the relaxation to nodes that can affect a producer body.
    /// Boolean paths use the same flattened chain leaves as runtime propagation;
    /// opaque implications/aggregates are leaves, just as in the relaxation.
    /// Roots without heads and head-only subformulas cannot contribute to P.
    /// Existing state cells mark discovery, so no second graph or bitset is held.
    fn prepare_bodies(
        &mut self,
        theory: &Theory,
        index: &Narrower,
        producers: &Producers,
        work: &mut Work<'_>,
    ) -> Result<(), Stop> {
        if self
            .owner
            .as_ref()
            .is_some_and(|owner| owner.same_instance(theory))
        {
            return Ok(());
        }
        self.owner = None;
        self.body_nodes.clear();
        self.fact_rows.clear();
        self.states.clear();
        reserve(&mut self.states, theory.nodes().len())?;
        // This complete initialization occurs once per worker/owner. Subsequent
        // regions reset only body_nodes; omitted cells remain inactive forever.
        for _ in 0..theory.nodes().len() {
            work.tick()?;
            self.states.push(State::Inactive);
        }
        for (row, producer) in producers.rules.iter().enumerate() {
            work.tick()?;
            if producer.heads.is_empty() {
                continue;
            }
            if let Some(body) = producer.body {
                self.include_body(body, work)?;
            } else {
                self.fact_rows
                    .try_reserve(1)
                    .map_err(|_| Stop::Allocation)?;
                self.fact_rows.push(row);
            }
        }
        let nodes = theory.view();
        let mut next = 0;
        while let Some(&node) = self.body_nodes.get(next) {
            work.tick()?;
            next += 1;
            if matches!(
                nodes.node(node).map_err(|_| Stop::InvalidProgram)?,
                NodeView::And(_) | NodeView::Or(_)
            ) {
                // A body root has an implication parent; a flattened chain leaf
                // is never absorbed. Both are active original index positions.
                if index.absorbed[node] {
                    return Err(Stop::InvalidProgram);
                }
                let chain = index.chain_of[node].ok_or(Stop::InvalidProgram)?;
                for &child in index.chains[chain_position(chain)].leaves(&index.extrema) {
                    self.include_body(child, work)?;
                }
            }
        }
        // No interrupted prefix can be reused as a complete projection.
        self.owner = Some(theory.clone());
        Ok(())
    }

    fn include_body(&mut self, node: usize, work: &mut Work<'_>) -> Result<(), Stop> {
        work.tick()?;
        if matches!(self.states[node], State::Inactive) {
            self.body_nodes
                .try_reserve(1)
                .map_err(|_| Stop::Allocation)?;
            self.body_nodes.push(node);
            self.states[node] = State::Awaiting(0);
        }
        Ok(())
    }

    fn initialize(
        &mut self,
        theory: &Theory,
        index: &Narrower,
        never: &[u64],
        work: &mut Work<'_>,
    ) -> Result<(), Stop> {
        self.atoms.clear();
        self.queue.clear();
        let vertices = theory
            .atom_count()
            .checked_add(self.body_nodes.len())
            .ok_or(Stop::Allocation)?;
        reserve(&mut self.atoms, flag_words(theory.atom_count()))?;
        reserve(&mut self.queue, vertices)?;
        for _ in 0..flag_words(theory.atom_count()) {
            work.tick()?;
            self.atoms.push(0);
        }
        let nodes = theory.view();
        for &node in &self.body_nodes {
            work.tick()?;
            let state = if index.absorbed[node] || bit(never, node) {
                State::Inactive
            } else {
                match nodes.node(node).map_err(|_| Stop::InvalidProgram)? {
                    NodeView::False => State::Inactive,
                    NodeView::Atom(_) => State::Awaiting(1),
                    NodeView::And(_) | NodeView::Or(_) => {
                        let chain = index.chain_of[node].ok_or(Stop::InvalidProgram)?;
                        let chain = &index.chains[chain_position(chain)];
                        if chain.disjunction {
                            State::Awaiting(1)
                        } else {
                            State::Awaiting(chain.leaves(&index.extrema).len())
                        }
                    }
                    // Unknown nonpositive bodies can only add possible support.
                    // Original known-false masks above still suppress them.
                    NodeView::Implies(_, _) | NodeView::Threshold(_) | NodeView::ExtremumCut(_) => {
                        State::True
                    }
                }
            };
            self.states[node] = state;
            if matches!(state, State::True | State::Awaiting(0)) {
                // The seed is recorded explicitly; subsequent incidences cannot
                // publish it again, including empty conjunctions.
                work.tick()?;
                self.states[node] = State::True;
                self.queue.push(theory.atom_count() + node);
            }
        }
        Ok(())
    }

    fn activate_node(
        &mut self,
        node: usize,
        atoms: usize,
        work: &mut Work<'_>,
    ) -> Result<(), Stop> {
        if !matches!(self.states[node], State::True) {
            work.tick()?;
            self.states[node] = State::True;
            self.queue.push(atoms + node);
        }
        Ok(())
    }

    fn activate_atom(
        &mut self,
        atom: usize,
        atom_never: &[u64],
        work: &mut Work<'_>,
    ) -> Result<(), Stop> {
        if !bit(atom_never, atom) && !self.contains(atom) {
            work.tick()?;
            self.atoms[atom / 64] |= 1u64 << (atom % 64);
            self.queue.push(atom);
        }
        Ok(())
    }
}

fn reserve<T>(values: &mut Vec<T>, count: usize) -> Result<(), Stop> {
    values
        .try_reserve_exact(count.saturating_sub(values.len()))
        .map_err(|_| Stop::Allocation)
}
