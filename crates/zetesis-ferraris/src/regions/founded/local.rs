//! First-pass replay below inherited completed founded knowledge.
//!
//! Every old uncut cyclic atom belongs to the old least upper closure. Collect
//! all newly false selected inputs, then close their forward dependency cone.
//! Discovery stops at currently false destinations: newly false inputs already
//! have their own queued events, and older false inputs were already inactive.
//! No newly invalidated unblocked dependency can change a boundary seed, so
//! unaffected uncut cyclic atoms remain valid seeds. Recompute every producer
//! body for affected live heads, including unchanged support, and saturate those
//! heads under current masks. The final
//! atom mask contains the unaffected seeds and the complete local result.
//!
//! Only the inherited knowledge supplies this premise. Owner-bound scratch stores
//! disposable membership/worklists, never an ancestor's truth. A partial attempt
//! consumes the premise and leaves marks that require a full retry. Publication
//! and explanation capture retain their existing complete-result boundary.

use super::{
    Initial, Narrower, NodeView, Producers, Scratch, State, Theory, Work, bit, chain_position,
    flag_words, reserve,
};
use std::mem::size_of;
use zetesis_cpu::Stop;

#[cfg(test)]
mod tests;

const FORWARD: u8 = 1;
const BODY: u8 = 2;
const FACT: u8 = 4;

/// Worker-owned membership; original vertex IDs are atoms followed by nodes.
/// Touched entries permit sparse cleanup, including after a typed failure.
#[derive(Debug, Default)]
pub(super) struct Replay {
    collecting: bool,
    atoms: usize,
    cyclic: usize,
    marks: Vec<u8>,
    touched: Vec<usize>,
    #[cfg(test)]
    passes: usize,
}

impl Replay {
    pub(super) fn retained_bytes(&self) -> u128 {
        self.marks.capacity() as u128 + self.touched.capacity() as u128 * size_of::<usize>() as u128
    }

    pub(super) fn end(&mut self) {
        self.collecting = false;
    }

    /// Preparation publishes under the same exact owner as the immutable rows.
    /// Reserve false-event slots before infallible ordinary propagation begins.
    pub(super) fn prepare(
        &mut self,
        atoms: usize,
        nodes: usize,
        bodies: usize,
        cyclic: &[u64],
        work: &mut Work<'_>,
    ) -> Result<(), Stop> {
        self.end();
        self.clear(work)?;
        let vertices = atoms.checked_add(nodes).ok_or(Stop::Allocation)?;
        reserve(&mut self.marks, vertices)?;
        reserve(&mut self.touched, bodies)?;
        self.marks.truncate(vertices);
        while self.marks.len() < vertices {
            work.tick()?;
            self.marks.push(0);
        }
        self.atoms = atoms;
        self.cyclic = 0;
        for &word in cyclic {
            work.tick()?;
            self.cyclic += word.count_ones() as usize;
        }
        Ok(())
    }

    pub(super) fn clear(&mut self, work: &mut Work<'_>) -> Result<(), Stop> {
        while let Some(&vertex) = self.touched.last() {
            work.tick()?;
            self.marks[vertex] = 0;
            self.touched.pop();
        }
        Ok(())
    }

    pub(super) fn body(&self, node: usize) -> bool {
        self.marks[self.atoms + node] & BODY != 0
    }

    pub(super) fn head(&self, atom: usize) -> bool {
        self.marks[atom] & FORWARD != 0
    }

    /// Discovery can grow beyond the initial false inputs. Reserve before
    /// publishing a mark so every prefix remains clearable after allocation fails.
    fn enqueue(
        &mut self,
        queue: &mut Vec<usize>,
        vertex: usize,
        flag: u8,
        work: &mut Work<'_>,
    ) -> Result<(), Stop> {
        work.tick()?;
        if self.marks[vertex] & flag != 0 {
            return Ok(());
        }
        let count = queue.len().checked_add(1).ok_or(Stop::Allocation)?;
        reserve(queue, count)?;
        if self.marks[vertex] == 0 {
            let count = self.touched.len().checked_add(1).ok_or(Stop::Allocation)?;
            reserve(&mut self.touched, count)?;
            self.touched.push(vertex);
        }
        self.marks[vertex] |= flag;
        queue.push(vertex);
        Ok(())
    }
}

impl Scratch {
    pub(in crate::regions) fn end_local(&mut self) {
        self.local.end();
    }

    /// Called once before original events. Unchanged checks incur no traversal
    /// or allocation. Dirty scratch from a stopped sibling declines this route.
    pub(in crate::regions) fn begin_local(&mut self, theory: &Theory, inherited: bool) {
        self.local.end();
        if inherited
            && self
                .owner
                .as_ref()
                .is_some_and(|owner| owner.same_instance(theory))
            && self.projected.is_some()
            && self.local.touched.is_empty()
            && self.local.atoms == theory.atom_count()
            && self.local.marks.len() == theory.atom_count().saturating_add(theory.nodes().len())
        {
            self.queue.clear();
            self.local.collecting = true;
        }
    }

    /// Ordinary evidence only appends selected node IDs, once each. Storage was
    /// reserved with the immutable projection. Atom events also call this for
    /// already-false occurrences, preserving conservative invalidation coverage.
    pub(in crate::regions) fn record_false(&mut self, node: usize) {
        if !self.local.collecting || matches!(self.states[node], State::Outside) {
            return;
        }
        let vertex = self.local.atoms + node;
        if self.local.marks[vertex] == 0 {
            self.local.marks[vertex] = FORWARD;
            self.local.touched.push(vertex);
            self.queue.push(vertex);
        }
    }

    /// Consume first-pass authority before any fallible work. A declined local
    /// plan leaves only disposable marks; the caller must complete a full pass.
    pub(super) fn complete_local(
        &mut self,
        theory: &Theory,
        index: &Narrower,
        producers: &Producers,
        never: &[u64],
        atom_never: &[u64],
        work: &mut Work<'_>,
    ) -> Result<bool, Stop> {
        let inherited = std::mem::take(&mut self.local.collecting);
        if !inherited
            || !self
                .owner
                .as_ref()
                .is_some_and(|owner| owner.same_instance(theory))
            || self.projected.is_none()
        {
            return Ok(false);
        }
        let heads = self.local_forward(index, (never, atom_never), work)?;
        // Cut heads cannot activate and are omitted from forward discovery.
        // Keep the full-pass fallback when no live cyclic boundary remains.
        let mut live = self.local.cyclic;
        for (&cyclic, &cut) in self.cyclic_atoms.iter().zip(atom_never) {
            work.tick()?;
            live -= (cyclic & cut).count_ones() as usize;
        }
        if heads == live {
            return Ok(false);
        }
        let forward_end = self.queue.len();
        self.partition_heads(heads, work)?;
        self.local_bodies(
            theory,
            index,
            producers,
            (heads, forward_end),
            (never, atom_never),
            work,
        )?;
        let body_end = self.queue.len();
        self.local_initialize(
            theory,
            index,
            (forward_end, body_end, heads),
            never,
            atom_never,
            work,
        )?;
        self.local_events(index, producers, body_end, atom_never, work)?;
        self.local.clear(work)?;
        #[cfg(test)]
        {
            self.local.passes += 1;
        }
        Ok(true)
    }

    /// Do not filter queued inputs. Every newly false included node, including
    /// each occurrence of a newly cut atom, was recorded before discovery.
    /// A false destination therefore either has its own input event or was
    /// already inactive in the inherited closure. Neither case needs another
    /// incoming traversal. Current masks, not sibling scratch truth, decide this.
    fn local_forward(
        &mut self,
        index: &Narrower,
        (never, atom_never): (&[u64], &[u64]),
        work: &mut Work<'_>,
    ) -> Result<usize, Stop> {
        let projected = self.projected.as_ref().ok_or(Stop::InvalidProgram)?;
        let mut heads = 0;
        let mut next = 0;
        while let Some(&vertex) = self.queue.get(next) {
            work.tick()?;
            next += 1;
            if vertex < self.local.atoms {
                heads += 1;
                for &node in &index.atom_nodes[vertex] {
                    work.tick()?;
                    if !matches!(self.states[node], State::Outside) && !bit(never, node) {
                        self.local.enqueue(
                            &mut self.queue,
                            self.local.atoms + node,
                            FORWARD,
                            work,
                        )?;
                    }
                }
            } else {
                let node = vertex - self.local.atoms;
                for &parent in &projected.parents[node] {
                    work.tick()?;
                    if bit(never, parent) {
                        continue;
                    }
                    self.local.enqueue(
                        &mut self.queue,
                        self.local.atoms + parent,
                        FORWARD,
                        work,
                    )?;
                }
                for &head in &projected.heads[node] {
                    work.tick()?;
                    if !bit(atom_never, head) {
                        self.local.enqueue(&mut self.queue, head, FORWARD, work)?;
                    }
                }
            }
        }
        Ok(heads)
    }

    /// Group discovered heads before producer selection. Discovery has finished,
    /// so its vertex order is no longer needed; marks and the forward boundary
    /// remain unchanged. A stopped partition is disposable on the full retry.
    fn partition_heads(&mut self, heads: usize, work: &mut Work<'_>) -> Result<(), Stop> {
        let mut placed = 0;
        for position in 0..self.queue.len() {
            if placed == heads {
                return Ok(());
            }
            work.tick()?;
            if self.queue[position] < self.local.atoms {
                if position != placed {
                    work.tick()?;
                    self.queue.swap(position, placed);
                }
                placed += 1;
            }
        }
        if placed == heads {
            Ok(())
        } else {
            Err(Stop::InvalidProgram)
        }
    }

    fn local_bodies(
        &mut self,
        theory: &Theory,
        index: &Narrower,
        producers: &Producers,
        (heads, forward_end): (usize, usize),
        (never, atom_never): (&[u64], &[u64]),
        work: &mut Work<'_>,
    ) -> Result<(), Stop> {
        for position in 0..heads {
            work.tick()?;
            let head = self.queue[position];
            // A cut head cannot activate. Other uncut affected heads select
            // their complete rows independently, including shared producers.
            if bit(atom_never, head) {
                continue;
            }
            for &row in &producers.by_head[head] {
                work.tick()?;
                if let Some(body) = producers.rules[row].body {
                    self.local
                        .enqueue(&mut self.queue, self.local.atoms + body, BODY, work)?;
                } else {
                    self.local.marks[head] |= FACT;
                }
            }
        }
        let view = theory.view();
        let mut next = forward_end;
        while let Some(&vertex) = self.queue.get(next) {
            work.tick()?;
            next += 1;
            let node = vertex - self.local.atoms;
            if matches!(self.states[node], State::Outside) || index.absorbed[node] {
                return Err(Stop::InvalidProgram);
            }
            // Keep the body selected: current initialization makes it Inactive.
            // Its operands cannot enable it. A descendant needed through another
            // nonfalse producer or Boolean path is selected on that path instead.
            if bit(never, node) {
                continue;
            }
            if matches!(
                view.node(node).map_err(|_| Stop::InvalidProgram)?,
                NodeView::And(_) | NodeView::Or(_)
            ) {
                let chain = index.chain_of[node].ok_or(Stop::InvalidProgram)?;
                for &child in index.chains[chain_position(chain)].leaves(&index.extrema) {
                    work.tick()?;
                    self.local
                        .enqueue(&mut self.queue, self.local.atoms + child, BODY, work)?;
                }
            }
        }
        Ok(())
    }

    fn local_initialize(
        &mut self,
        theory: &Theory,
        index: &Narrower,
        (forward_end, body_end, heads): (usize, usize, usize),
        never: &[u64],
        atom_never: &[u64],
        work: &mut Work<'_>,
    ) -> Result<(), Stop> {
        let events = (body_end - forward_end)
            .checked_add(heads)
            .ok_or(Stop::Allocation)?;
        reserve(
            &mut self.queue,
            body_end.checked_add(events).ok_or(Stop::Allocation)?,
        )?;
        self.atoms.clear();
        reserve(&mut self.atoms, flag_words(theory.atom_count()))?;
        for (&cyclic, &never) in self.cyclic_atoms.iter().zip(atom_never) {
            work.tick()?;
            self.atoms.push(cyclic & !never);
        }
        for position in 0..heads {
            work.tick()?;
            let head = self.queue[position];
            self.atoms[head / 64] &= !(1_u64 << (head % 64));
        }
        for position in forward_end..body_end {
            work.tick()?;
            let node = self.queue[position] - self.local.atoms;
            self.states[node] = self.local_initial(theory, index, node, never, atom_never)?;
            if matches!(self.states[node], State::True | State::Awaiting(0)) {
                work.tick()?;
                self.states[node] = State::True;
                self.queue.push(self.local.atoms + node);
            }
        }
        for position in 0..heads {
            work.tick()?;
            let head = self.queue[position];
            if self.local.marks[head] & FACT != 0 {
                self.activate_atom(head, atom_never, work)?;
            }
        }
        Ok(())
    }

    fn local_initial(
        &self,
        theory: &Theory,
        index: &Narrower,
        node: usize,
        never: &[u64],
        atom_never: &[u64],
    ) -> Result<State, Stop> {
        if bit(never, node) {
            return Ok(State::Inactive);
        }
        if let NodeView::Atom(atom) = theory.view().node(node).map_err(|_| Stop::InvalidProgram)?
            && !self.local.head(atom)
        {
            return Ok(if bit(atom_never, atom) {
                State::Inactive
            } else {
                State::True
            });
        }
        Ok(Initial::read(theory, index, &self.cyclic_atoms, node)?.state(atom_never))
    }

    fn local_events(
        &mut self,
        index: &Narrower,
        producers: &Producers,
        mut next: usize,
        atom_never: &[u64],
        work: &mut Work<'_>,
    ) -> Result<(), Stop> {
        while let Some(&vertex) = self.queue.get(next) {
            work.tick()?;
            next += 1;
            if vertex < self.local.atoms {
                for &node in &index.atom_nodes[vertex] {
                    work.tick()?;
                    if self.local.body(node) && matches!(self.states[node], State::Awaiting(_)) {
                        super::activate_node(
                            &mut self.states,
                            &mut self.queue,
                            node,
                            self.local.atoms,
                            work,
                        )?;
                    }
                }
            } else {
                self.propagate_node::<true>(
                    index,
                    producers,
                    vertex - self.local.atoms,
                    self.local.atoms,
                    atom_never,
                    work,
                )?;
            }
        }
        Ok(())
    }
}
