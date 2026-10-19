//! Possible founded atoms of an original candidate region.
//!
//! Complete `Producers` coverage is mandatory. Relax a body by keeping its
//! positive atoms and Boolean chains, replacing nodes already known false by
//! falsum, and replacing other implications and aggregates by truth. Exact
//! positive dependency components identify cyclic heads. Conceptually seed every
//! other uncut atom, then derive cyclic heads to closure. A boundary Atom node
//! reads that seed directly; no outside atom or its unrelated postings is queued.
//! Held cyclic atoms are not seeds. Disjunctive producers never exclude a head
//! because another head is held.
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
//! Each worker prepares the immutable full positive incidence graph through the
//! existing postings, ignoring region false masks. Iterative reverse/forward DFS
//! retains only a cyclic-atom mask, then selects complete bodies of every producer
//! with a cyclic head. Omitted nodes cannot affect a cyclic-head body; ordinary
//! support still propagates along acyclic paths. Opaque bodies remain leaves.
//! Node states, atom cursors, queue and body-node IDs supply the temporary DFS
//! storage; the component pass retains no graph or component map. Body-node IDs
//! temporarily hold at most A+B finishing entries, where B is the full body cone.
//! Their actual capacity remains charged after selecting the smaller cone. Owner
//! publication authenticates the cyclic mask, selected cone and any completed
//! incidence indexes. The first pass keeps original parent and producer rows. A later preparation
//! for the same exact owner builds the filtered rows, so one-use scratch pays no
//! incidence-index construction. A refused build clears owner authority; retry
//! conservatively repeats body preparation before another attempt. Reused preparation
//! also records each selected node's immutable initial role. Every pass applies
//! current node/atom masks to those roles; no prior closure truth is reused.
//!
//! One pass reuses the original atom/node and producer body/head rows. Once
//! prepared, its parent rows retain only included, nonabsorbed Boolean destinations,
//! in original occurrence order and multiplicity. Outside destinations never change;
//! included opaque destinations are initialized True or Inactive and cannot
//! consume a parent event. No region-dependent state filters this index.
//! Body-to-head rows flatten the original producer rows and retain exactly their
//! cyclic head occurrences; noncyclic heads were already ignored by activation.
//! One additional row holds the selected facts' heads in the same occurrence order.
//! Neither projection consults region knowledge or changes producer information.
//! Each atom/node activates at most once. For A atoms, N nodes, E indexed Boolean
//! incidences, R producers, H head occurrences and P retained parent occurrences,
//! preparation takes O(A + N + E + R + H) work and O(A + N + P + H) exclusive storage.
//! Each incidence construction also temporarily holds at most N+1 row cursors.
//! Original parent and producer postings are scanned only for selected body nodes;
//! empty offset rows and fact-row inspections remain initialized and charged. A later pass resets only selected body nodes/fact
//! rows and atom-mask words; only cyclic atom postings, the chosen parent rows
//! and projected cyclic-head incidences are read after preparation. Publication reads cyclic
//! mask words and missing uncut cyclic heads. All actual reads are charged; cuts are
//! published together before ordinary propagation resumes. A later pass can
//! exploit newly false bodies; no pass installs a rule or changes the reduct.
//! Successfully closed region knowledge can omit a later pass while no projected
//! body input has become false. A completed exact-owner projection
//! distinguishes an outside node from an included but blocked node. Its outside
//! cells never change during closure; unknown or foreign preparation invalidates
//! conservatively. This includes held decisions that indirectly falsify bodies.
//! The completion authority belongs to inherited knowledge under its same-original
//! subject contract, never worker scratch. An atom with no positive occurrence
//! in the projection can only remove itself from the completed upper closure:
//! no projected body reads it, so all remaining uncut atoms stay included. Every
//! included occurrence still invalidates, even when already blocked. Newly false
//! opaque bodies invalidate separately after ordinary propagation. With unknown
//! preparation every false atom invalidates. These rules preserve closedness and
//! coverage of all uncut atoms, even when the operator loses an output-only head.
//!
//! A restricted pass may be weaker than the global least closure: a conceptual
//! acyclic seed can temporarily support a downstream cycle. Ordinary propagation
//! and foundedness therefore still alternate to saturation. At that fixed point,
//! every uncut acyclic head has a nonfalse producer. Boolean saturation makes a
//! relaxed-false body known false; opaque nonfalse leaves read true. Induction on
//! the actual SCC condensation order then puts each acyclic head and each cyclic
//! component into the global least closure. A boundary dependency must come from
//! an earlier component, or a return path would put it in that cyclic component.
//! Thus the saturated restriction matches global closure, even when passes differ.
//! This executable correspondence remains a proof obligation, tested against the
//! same event engine with every atom forced to be a closure head.
//!
//! A cached Kahn pass over the relaxed positive dependencies skips all later
//! foundedness work when they are acyclic. Local support already suffices there:
//! every nonfalse atom has a possible producer, whose positive inputs have lower
//! dependency order and therefore enter P. The cycle test reuses exactly these
//! immutable incidences and adds no graph. Its O(A + N + E + R + H) work is charged
//! to the original narrowing that completes it; concurrent first users may each
//! perform a charged test. Only a completed Boolean result is shared.

use super::{
    Adjacency, Narrower, NodeView, Producers, Theory, Work, bit, chain_position, flag_words,
};
use std::mem::size_of;
use zetesis_cpu::Stop;

mod components;
pub(super) mod explanations;
mod initialization;
pub(super) mod ledger;
mod projection;

use initialization::Initial;
use projection::Projection;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug)]
enum State {
    /// Not read by any prepared producer body; immutable during closure.
    Outside,
    /// Preparation-only reverse DFS cursor; never owner-authorized.
    Visiting(usize),
    Inactive,
    Awaiting(usize),
    True,
}

/// Exclusive worklists and owner-bound body/incidence preparation. These retain
/// original node IDs and carry no inherited region knowledge or second theory.
#[derive(Debug, Default)]
pub(super) struct Scratch {
    states: Vec<State>,
    atoms: Vec<u64>,
    /// Exact positive cyclic heads; immutable under the prepared owner.
    cyclic_atoms: Vec<u64>,
    queue: Vec<usize>,
    /// Kahn indegrees, then atom cursors during exact component preparation.
    dependencies: Vec<usize>,
    /// Authenticates the cyclic mask, body/fact rows and completed incidence indexes.
    owner: Option<Theory>,
    body_nodes: Vec<usize>,
    fact_rows: Vec<usize>,
    /// Original ordered parent and cyclic-head occurrences in the body projection.
    /// Absence keeps original rows for the first pass; exact-owner reuse builds it.
    projected: Option<Projection>,
    /// Test-only operation count; no closure authority is stored here.
    #[cfg(test)]
    passes: usize,
}

impl Scratch {
    pub(super) fn retained_bytes(&self) -> u128 {
        self.states.capacity() as u128 * size_of::<State>() as u128
            + (self.atoms.capacity() + self.cyclic_atoms.capacity()) as u128
                * size_of::<u64>() as u128
            + (self.queue.capacity() as u128
                + self.dependencies.capacity() as u128
                + self.body_nodes.capacity() as u128
                + self.fact_rows.capacity() as u128)
                * size_of::<usize>() as u128
            + self.projected.as_ref().map_or(0, Projection::payload_bytes)
    }

    /// Number of packed original-atom words inspected during publication.
    pub(super) fn publication_words(&self) -> usize {
        self.cyclic_atoms.len()
    }

    /// Completed closure omits only these permitted cyclic heads. Noncyclic
    /// heads are conceptual seeds and are never candidates for publication.
    pub(super) fn missing_in_word(&self, word: usize, never: u64) -> u64 {
        self.cyclic_atoms[word] & !self.atoms[word] & !never
    }

    fn contains(&self, atom: usize) -> bool {
        bit(&self.atoms, atom)
    }

    /// Every producer-backed original narrowing authenticates cached projection
    /// membership before any false event runs. Clearing a foreign owner keeps
    /// later queries conservative until preparation for this subject completes.
    /// Producer-free and frozen passes do not consume or replace this owner.
    pub(super) fn forget_foreign(&mut self, theory: &Theory) {
        if self
            .owner
            .as_ref()
            .is_some_and(|owner| !owner.same_instance(theory))
        {
            self.owner = None;
        }
    }

    /// Original support narrowing authenticated this projection before processing
    /// any event. Without it even an atom with no occurrences must invalidate.
    pub(super) const fn has_projection(&self) -> bool {
        self.owner.is_some()
    }

    /// Only completed, authenticated preparation can exclude a node from the
    /// relaxation's inputs. Included false nodes stay Inactive, not Outside.
    /// Failed preparation clears owner before changing any state. The caller
    /// must authenticate the original support subject with `forget_foreign` first.
    pub(super) fn may_affect_body(&self, node: usize) -> bool {
        self.owner.is_none() || !matches!(self.states.get(node), Some(State::Outside))
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
        self.initialize(theory, index, never, atom_never, work)?;
        if let Some(projected) = &self.projected {
            for &head in projected.fact_heads() {
                work.tick()?;
                activate_atom(
                    &mut self.atoms,
                    &self.cyclic_atoms,
                    &mut self.queue,
                    head,
                    atom_never,
                    work,
                )?;
            }
        } else {
            for position in 0..self.fact_rows.len() {
                work.tick()?;
                let producer = &producers.rules[self.fact_rows[position]];
                for &head in &producer.heads {
                    work.tick()?;
                    self.activate_atom(head, atom_never, work)?;
                }
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
            let parents = self
                .projected
                .as_ref()
                .map_or(&index.parents, |projected| &projected.parents);
            for &parent in &parents[node] {
                work.tick()?;
                match self.states[parent] {
                    State::Awaiting(1) => {
                        activate_node(&mut self.states, &mut self.queue, parent, atoms, work)?;
                    }
                    State::Awaiting(remaining) => {
                        self.states[parent] = State::Awaiting(remaining - 1);
                    }
                    State::Outside | State::Inactive | State::True => {}
                    State::Visiting(_) => return Err(Stop::InvalidProgram),
                }
            }
            if let Some(projected) = &self.projected {
                for &head in &projected.heads[node] {
                    work.tick()?;
                    activate_atom(
                        &mut self.atoms,
                        &self.cyclic_atoms,
                        &mut self.queue,
                        head,
                        atom_never,
                        work,
                    )?;
                }
            } else {
                for &producer in &producers.by_body[node] {
                    work.tick()?;
                    for &head in &producers.rules[producer].heads {
                        work.tick()?;
                        self.activate_atom(head, atom_never, work)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Prepare exact cyclic heads, then complete bodies feeding those heads.
    /// Boolean paths use the same flattened chain leaves as runtime propagation;
    /// opaque implications/aggregates are leaves, just as in the relaxation.
    /// Roots without heads and head-only subformulas cannot contribute to P.
    /// Existing state cells mark discovery; only cyclic head membership adds a mask.
    /// Prepare incidence rows only when the same exact body projection is reused.
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
            if self.projected.is_none() {
                // No partial construction may retain body-projection authority.
                self.owner = None;
                self.projected = Some(Projection::prepare(theory, index, producers, self, work)?);
                self.owner = Some(theory.clone());
            }
            return Ok(());
        }
        self.owner = None;
        self.prepare_cone(theory, index, producers, false, work)?;
        self.prepare_components(theory, index, producers, work)?;
        self.prepare_cone(theory, index, producers, true, work)?;
        // Publish the completed bodies; the first pass uses original parent rows.
        // A failed phase cannot authorize classification or inherited reuse.
        self.owner = Some(theory.clone());
        Ok(())
    }

    fn prepare_cone(
        &mut self,
        theory: &Theory,
        index: &Narrower,
        producers: &Producers,
        cyclic_only: bool,
        work: &mut Work<'_>,
    ) -> Result<(), Stop> {
        // No incidence row may survive replacement of its included body set.
        self.projected = None;
        self.body_nodes.clear();
        self.fact_rows.clear();
        self.states.clear();
        reserve(&mut self.states, theory.nodes().len())?;
        // This complete initialization occurs once per worker/owner. Subsequent
        // regions reset only body_nodes; omitted cells remain Outside forever.
        for _ in 0..theory.nodes().len() {
            work.tick()?;
            self.states.push(State::Outside);
        }
        for (row, producer) in producers.rules.iter().enumerate() {
            work.tick()?;
            let mut selected = false;
            for &head in &producer.heads {
                work.tick()?;
                selected |= !cyclic_only || bit(&self.cyclic_atoms, head);
            }
            if !selected {
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
        Ok(())
    }

    fn include_body(&mut self, node: usize, work: &mut Work<'_>) -> Result<(), Stop> {
        work.tick()?;
        if matches!(self.states[node], State::Outside) {
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
        atom_never: &[u64],
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
        for (position, &node) in self.body_nodes.iter().enumerate() {
            work.tick()?;
            let state = if bit(never, node) {
                State::Inactive
            } else {
                let initial = if let Some(plan) = &self.projected {
                    plan.initial[position]
                } else {
                    Initial::read(theory, index, &self.cyclic_atoms, node)?
                };
                initial.state(atom_never)
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
        activate_node(&mut self.states, &mut self.queue, node, atoms, work)
    }

    fn activate_atom(
        &mut self,
        atom: usize,
        atom_never: &[u64],
        work: &mut Work<'_>,
    ) -> Result<(), Stop> {
        activate_atom(
            &mut self.atoms,
            &self.cyclic_atoms,
            &mut self.queue,
            atom,
            atom_never,
            work,
        )
    }
}

/// Split atom/worklist borrows retain one immutable projected-head row while
/// applying the same activation as the original producer traversal.
fn activate_atom(
    atoms: &mut [u64],
    cyclic_atoms: &[u64],
    queue: &mut Vec<usize>,
    atom: usize,
    atom_never: &[u64],
    work: &mut Work<'_>,
) -> Result<(), Stop> {
    if bit(cyclic_atoms, atom) && !bit(atom_never, atom) && !bit(atoms, atom) {
        work.tick()?;
        atoms[atom / 64] |= 1u64 << (atom % 64);
        queue.push(atom);
    }
    Ok(())
}

/// Split state/worklist borrows let a closure keep one immutable parent-row
/// borrow throughout traversal. This is the same activation used in preparation.
fn activate_node(
    states: &mut [State],
    queue: &mut Vec<usize>,
    node: usize,
    atoms: usize,
    work: &mut Work<'_>,
) -> Result<(), Stop> {
    if matches!(states[node], State::Outside) {
        return Err(Stop::InvalidProgram);
    }
    if !matches!(states[node], State::True) {
        work.tick()?;
        states[node] = State::True;
        queue.push(atoms + node);
    }
    Ok(())
}

fn reserve<T>(values: &mut Vec<T>, count: usize) -> Result<(), Stop> {
    values
        .try_reserve_exact(count.saturating_sub(values.len()))
        .map_err(|_| Stop::Allocation)
}
