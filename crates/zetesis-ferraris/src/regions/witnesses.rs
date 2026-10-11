//! Lazy distinct-leaf views of shared associative connectives.
//!
//! Single-parent chain counters remain the ordinary path. A shared same-kind
//! child is a compact DAG boundary, not a logical boundary for associativity or
//! idempotence. The optional view unfolds only those same-kind links. It keeps
//! no leaf-to-ancestor transpose and no flattened prefix table.
//!
//! Preparation marks affected chains in O(C + L), with C chain roots and L
//! existing chain leaves. A requested view costs O(V + E) for its reachable
//! same-kind cone, using generation marks, an explicit stack and one leaf vector.
//! Retained walker scratch is O(N + E + C), with N theory nodes. Reservations are
//! fallible, each initialized mark and visited node/edge is charged, and the
//! caller's normal work quota/cancellation contract applies. The scratch never
//! enters cloned region knowledge; every narrowing clears abandoned dirty work.
//!
//! When an inner leaf changes without deciding its shared parent, dirty notices
//! follow only existing immediate same-kind parent edges. Coalescing bounds the
//! pending queue by C. Only a required nested witness scans its distinct leaves;
//! no unrelated flat chain changes its counter algorithm. Repeated notices can
//! revisit cones, so this trades runtime work for compact retained storage.
//! Settled roots do not relay possibility notices: ordinary truth events have
//! already informed their parents and forced any required descendants. Their own
//! required witness check remains independent of that notification rule.
//! Enqueue sites apply the same relevance rule after updating operand counters.
//! A settled root enters the queue only for a remaining expanded unit obligation;
//! a later root truth event seeds that obligation if it becomes required later.
//! Frozen closure enables associative witness views for native extrema too;
//! a masked same-kind child is an opaque falsum. Numeric bound bridges remain
//! original-only and are never transplanted into a frozen reduct.

use std::num::NonZeroUsize;
use std::ops::ControlFlow;

use zetesis_cpu::Stop;

use super::{Chain, Closure, Count, Narrower, Step, Work, bit, chain_position, flag_words};

const EXPAND: u8 = 1;
const DIRTY: u8 = 2;

pub(super) fn nested(
    chains: &[Chain],
    chain_of: &[Option<NonZeroUsize>],
    extrema: &super::extrema::Extrema,
) -> Result<Vec<u8>, Stop> {
    let has_child = |chain: &Chain| {
        chain.leaves(extrema).iter().any(|&operand| {
            chain_of[operand]
                .is_some_and(|link| chains[chain_position(link)].disjunction == chain.disjunction)
        })
    };
    if !chains.iter().any(has_child) {
        return Ok(Vec::new());
    }
    let mut roles = Vec::new();
    roles
        .try_reserve_exact(chains.len())
        .map_err(|_| Stop::Allocation)?;
    roles.resize(chains.len(), 0);
    for (parent, chain) in chains.iter().enumerate() {
        for &operand in chain.leaves(extrema) {
            if let Some(link) = chain_of[operand] {
                let child = chain_position(link);
                if chains[child].disjunction == chain.disjunction {
                    roles[parent] |= EXPAND | DIRTY;
                    roles[child] |= DIRTY;
                }
            }
        }
    }
    Ok(roles)
}

pub(super) fn expands(index: &Narrower, chain: usize) -> bool {
    index
        .witness_chains
        .get(chain)
        .is_some_and(|role| role & EXPAND != 0)
}

pub(super) fn participates(index: &Narrower, chain: usize) -> bool {
    index
        .witness_chains
        .get(chain)
        .is_some_and(|role| role & DIRTY != 0)
}

/// A suspended same-kind chain: children before `next_child` were scheduled in
/// order; the remaining borrowed operands have not been read. Only IDs and an
/// offset persist, so no borrow or formula authority enters worker scratch.
#[derive(Debug)]
struct Frame {
    chain: usize,
    next_child: usize,
}

#[derive(Debug, Default)]
pub(super) struct Scratch {
    seen: Vec<u32>,
    generation: u32,
    stack: Vec<usize>,
    frames: Vec<Frame>,
    leaves: Vec<usize>,
    candidates: Vec<usize>,
    dirty: Vec<usize>,
    pending: Vec<u64>,
}

impl Scratch {
    pub(super) fn retained_bytes(&self) -> u128 {
        (self.stack.capacity()
            + self.leaves.capacity()
            + self.candidates.capacity()
            + self.dirty.capacity()) as u128
            * size_of::<usize>() as u128
            + self.frames.capacity() as u128 * size_of::<Frame>() as u128
            + self.seen.capacity() as u128 * size_of::<u32>() as u128
            + self.pending.capacity() as u128 * size_of::<u64>() as u128
    }

    pub(super) fn clear(&mut self) {
        for &chain in &self.dirty {
            self.pending[chain / 64] &= !(1u64 << (chain % 64));
        }
        self.dirty.clear();
        self.candidates.clear();
        self.stack.clear();
        self.frames.clear();
        self.leaves.clear();
    }

    pub(super) fn prepare_dirty(&mut self, chains: usize) -> Result<(), Stop> {
        self.dirty
            .try_reserve_exact(chains.saturating_sub(self.dirty.len()))
            .map_err(|_| Stop::Allocation)?;
        let words = flag_words(chains);
        if self.pending.len() < words {
            self.pending
                .try_reserve_exact(words - self.pending.len())
                .map_err(|_| Stop::Allocation)?;
            self.pending.resize(words, 0);
        }
        Ok(())
    }

    pub(super) fn queue_dirty(&mut self, chain: usize) {
        let (word, flag) = (chain / 64, 1u64 << (chain % 64));
        if self.pending[word] & flag == 0 {
            self.pending[word] |= flag;
            self.dirty.push(chain);
        }
    }

    pub(super) fn next_dirty(&mut self) -> Option<usize> {
        let chain = self.dirty.pop()?;
        self.pending[chain / 64] &= !(1u64 << (chain % 64));
        Some(chain)
    }

    /// Borrow the ordinary leaves or prepare one expanded view. Every view has
    /// distinct node IDs in first left-to-right occurrence order. A shared child
    /// may occur on several paths, but its descendants are expanded only once.
    pub(super) fn read<'a>(
        &'a mut self,
        index: &'a Narrower,
        chain: usize,
        frozen: Option<&[bool]>,
        work: &mut Work<'_>,
    ) -> Result<&'a [usize], Stop> {
        if !expands(index, chain) {
            return Ok(index.chains[chain].leaves(&index.extrema));
        }
        // The visitor's begin clears the scratch's leaf vector. Lend its
        // reusable allocation to the collecting adapter, then restore it on
        // every outcome, including a refused walk or fallible leaf push.
        let mut leaves = std::mem::take(&mut self.leaves);
        leaves.clear();
        let result = self.visit_expanded(index, chain, frozen, work, |leaf, _| {
            push(&mut leaves, leaf)?;
            Ok(ControlFlow::Continue(()))
        });
        self.leaves = leaves;
        result.map(|_| ())?;
        Ok(&self.leaves)
    }

    /// Fold distinct witnesses in the collecting view's traversal order.
    /// The caller has selected an expanding chain. One continuation frame per
    /// active same-kind ancestor reads child edges only as they are reached;
    /// stopping after a witness or two open leaves does not prepare later siblings.
    /// Node and edge visits are charged here, while the visitor charges its own
    /// leaf reads. A complete walk retains its node/edge receipt; an early stop
    /// omits unread edges as well as nodes. First-use marks still cost O(N).
    /// Frames use O(D) scratch for active depth D, counted at actual capacity.
    /// Breaks and errors may leave frames; the next walk clears them and advances
    /// the same generation marks. A fold never allocates a leaf-result vector.
    fn visit_expanded(
        &mut self,
        index: &Narrower,
        chain: usize,
        frozen: Option<&[bool]>,
        work: &mut Work<'_>,
        mut leaf: impl FnMut(usize, &mut Work<'_>) -> Result<ControlFlow<()>, Stop>,
    ) -> Result<ControlFlow<()>, Stop> {
        self.begin(index.parents.len(), work)?;
        let mut node = index.chains[chain].root;
        let disjunction = index.chains[chain].disjunction;
        loop {
            work.tick()?;
            if self.seen[node] != self.generation {
                self.seen[node] = self.generation;
                let inner = index.chain_of[node].map(chain_position);
                if let Some(inner) = inner.filter(|&inner| {
                    index.chains[inner].disjunction == disjunction
                        && frozen.is_none_or(|truth| truth[node])
                }) {
                    self.frames.try_reserve(1).map_err(|_| Stop::Allocation)?;
                    self.frames.push(Frame {
                        chain: inner,
                        next_child: 0,
                    });
                } else if let ControlFlow::Break(()) = leaf(node, work)? {
                    return Ok(ControlFlow::Break(()));
                }
            }
            let Some(next) = self.next_child(index, work)? else {
                return Ok(ControlFlow::Continue(()));
            };
            node = next;
        }
    }

    /// Resume the deepest unfinished chain, discarding completed frames.
    /// Each advance consumes one previously unread edge; a completed frame is
    /// popped once. A refusal precedes the edge read and leaves its cursor intact.
    fn next_child(&mut self, index: &Narrower, work: &mut Work<'_>) -> Result<Option<usize>, Stop> {
        while let Some(frame) = self.frames.last_mut() {
            let children = index.chains[frame.chain].leaves(&index.extrema);
            if frame.next_child == children.len() {
                self.frames.pop();
                continue;
            }
            work.tick()?;
            let child = children[frame.next_child];
            frame.next_child += 1;
            return Ok(Some(child));
        }
        Ok(None)
    }

    pub(super) fn prepared(&self) -> &[usize] {
        &self.leaves
    }

    pub(super) fn clear_candidates(&mut self) {
        self.candidates.clear();
    }

    pub(super) fn push_candidate(&mut self, candidate: usize) -> Result<(), Stop> {
        push(&mut self.candidates, candidate)
    }

    pub(super) fn candidates(&self) -> &[usize] {
        &self.candidates
    }

    /// Collect the same-kind cone, including compound conditions and its root.
    /// A native row can name a cut through this cone, rather than just its
    /// opaque or fully unfolded boundary. Distinct nodes occur once.
    pub(super) fn cone(
        &mut self,
        index: &Narrower,
        chain: usize,
        work: &mut Work<'_>,
    ) -> Result<(), Stop> {
        self.walk(index, chain, work, |_, _| Ok(false), true)?;
        Ok(())
    }

    pub(super) fn visited(&self, node: usize) -> bool {
        self.seen[node] == self.generation
    }

    /// Every terminal path must meet a caller-approved condition. Stop at such
    /// a condition before unfolding it: a row may name a compound condition.
    /// Failure at an opaque endpoint refutes this structural implication only.
    pub(super) fn covered_by(
        &mut self,
        index: &Narrower,
        chain: usize,
        work: &mut Work<'_>,
        covers: impl FnMut(usize, &mut Work<'_>) -> Result<bool, Stop>,
    ) -> Result<bool, Stop> {
        self.walk(index, chain, work, covers, false)
    }

    /// Iterative same-kind traversal shared by cone discovery and cut
    /// authentication. Generation marks coalesce repeated paths. A cover walk
    /// stops at accepted nodes; a cone walk records every distinct visited node.
    fn walk(
        &mut self,
        index: &Narrower,
        chain: usize,
        work: &mut Work<'_>,
        mut covers: impl FnMut(usize, &mut Work<'_>) -> Result<bool, Stop>,
        collect: bool,
    ) -> Result<bool, Stop> {
        self.begin(index.parents.len(), work)?;
        push(&mut self.stack, index.chains[chain].root)?;
        let disjunction = index.chains[chain].disjunction;
        while let Some(node) = self.stack.pop() {
            work.tick()?;
            if self.visited(node) {
                continue;
            }
            self.seen[node] = self.generation;
            if collect {
                push(&mut self.leaves, node)?;
            }
            if covers(node, work)? {
                continue;
            }
            let inner = index.chain_of[node].map(chain_position);
            if let Some(inner) =
                inner.filter(|&inner| index.chains[inner].disjunction == disjunction)
            {
                for &child in index.chains[inner].leaves(&index.extrema).iter().rev() {
                    work.tick()?;
                    push(&mut self.stack, child)?;
                }
            } else if !collect {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Discover candidate chain roots from one row anchor using the existing
    /// immediate-parent transpose. Initial parents may use either connective;
    /// each subsequent path retains that connective. The anchor itself is also
    /// a candidate when it names a live chain. Results are distinct, with no
    /// retained row-by-chain or leaf-by-ancestor relation. The caller may prune
    /// a branch only when no same-kind ancestor needs this requirement: either
    /// its polarity is irrelevant, or this root already has the absorbing truth
    /// that ordinary node events propagate to every same-kind ancestor.
    pub(super) fn parent_chains(
        &mut self,
        index: &Narrower,
        condition: usize,
        work: &mut Work<'_>,
        mut relevant: impl FnMut(&Chain) -> bool,
    ) -> Result<(), Stop> {
        self.begin(index.parents.len(), work)?;
        self.candidates.clear();
        if index.chain_of[condition].is_some() {
            push(&mut self.stack, condition)?;
        }
        for parent in index.logical_parents(condition).rev() {
            work.tick()?;
            if index.chain_of[parent].is_some() {
                push(&mut self.stack, parent)?;
            }
        }
        while let Some(node) = self.stack.pop() {
            work.tick()?;
            if self.visited(node) {
                continue;
            }
            self.seen[node] = self.generation;
            let chain = chain_position(index.chain_of[node].expect("queued live chain"));
            if !relevant(&index.chains[chain]) {
                continue;
            }
            self.push_candidate(chain)?;
            for parent in index.logical_parents(node).rev() {
                work.tick()?;
                if let Some(link) = index.chain_of[parent]
                    && index.chains[chain_position(link)].disjunction
                        == index.chains[chain].disjunction
                {
                    push(&mut self.stack, parent)?;
                }
            }
        }
        Ok(())
    }

    fn begin(&mut self, nodes: usize, work: &mut Work<'_>) -> Result<(), Stop> {
        self.stack.clear();
        self.frames.clear();
        self.leaves.clear();
        self.seen
            .try_reserve_exact(nodes.saturating_sub(self.seen.len()))
            .map_err(|_| Stop::Allocation)?;
        while self.seen.len() < nodes {
            work.tick()?;
            self.seen.push(0);
        }
        if self.generation == u32::MAX {
            for seen in &mut self.seen {
                work.tick()?;
                *seen = 0;
            }
            self.generation = 0;
        }
        self.generation += 1;
        Ok(())
    }
}

fn push(values: &mut Vec<usize>, value: usize) -> Result<(), Stop> {
    values.try_reserve(1).map_err(|_| Stop::Allocation)?;
    values.push(value);
    Ok(())
}

impl<C: Count> Closure<'_, C> {
    /// Select a change that can still inform a shared ancestor or establish
    /// an expanded unit consequence. Counts describe processed events, so callers
    /// first account for a changed operand. Truth bits may precede their events:
    /// a newly required root also reaches this operation from `teach_operands`.
    /// The caller has selected a participating shared chain before consulting
    /// dynamic truth: `participates`, `expands` or a shared same-kind edge
    /// establishes that immutable role. Unrelated chains never need this test.
    /// Frozen falsum roots and unrelated chains have no associative obligation.
    #[inline]
    pub(super) fn needs_witness_notification(
        &self,
        index: &Narrower,
        chain: usize,
        frozen: Option<&[bool]>,
    ) -> bool {
        if frozen.is_some() && index.extrema.is_empty() {
            return false;
        }
        debug_assert!(participates(index, chain));
        let role = index.witness_chains[chain];
        // A processed witness establishes the required root truth in the same
        // operand event. Contradictory events stop before reaching this helper;
        // otherwise that truth persists, so neither relay nor unit work remains.
        if bit(self.known.witnessed, chain) {
            return false;
        }
        let source = &index.chains[chain];
        if frozen.is_some_and(|truth| !truth[source.root]) {
            return false;
        }
        let (required, neutral) = if source.disjunction {
            (&self.known.sure, &self.known.never)
        } else {
            (&self.known.never, &self.known.sure)
        };
        !bit(neutral, source.root) && (role & EXPAND != 0 || !bit(required, source.root))
    }

    pub(super) fn witness_changed(
        &mut self,
        index: &Narrower,
        chain: usize,
        frozen: Option<&[bool]>,
        work: &mut Work<'_>,
    ) -> Result<Step, Stop> {
        work.tick()?;
        let source = &index.chains[chain];
        if frozen.is_some_and(|truth| !truth[source.root]) {
            return Ok(Step::Unchanged);
        }
        // Ordinary node events drain first. A settled source has therefore
        // already informed its parents: an absorbing value settles them, and a
        // neutral value forces this source's operands. Only an unknown source
        // must relay a change in its possible witnesses through the shared DAG.
        if !bit(self.known.sure, source.root) && !bit(self.known.never, source.root) {
            for parent in index.logical_parents(source.root) {
                work.tick()?;
                if let Some(link) = index.chain_of[parent] {
                    let parent = chain_position(link);
                    if index.chains[parent].disjunction == source.disjunction
                        && self.needs_witness_notification(index, parent, frozen)
                    {
                        self.lists.witnesses.queue_dirty(parent);
                    }
                }
            }
        }
        if !expands(index, chain)
            || !if source.disjunction {
                bit(self.known.sure, source.root)
            } else {
                bit(self.known.never, source.root)
            }
            || bit(self.known.witnessed, chain)
        {
            return Ok(Step::Unchanged);
        }
        let mut candidate = None;
        let known = &self.known;
        let outcome =
            self.lists
                .witnesses
                .visit_expanded(index, chain, frozen, work, |leaf, work| {
                    work.tick()?;
                    let (witness, neutral) = if source.disjunction {
                        (bit(known.sure, leaf), bit(known.never, leaf))
                    } else {
                        (bit(known.never, leaf), bit(known.sure, leaf))
                    };
                    if witness {
                        return Ok(ControlFlow::Break(()));
                    }
                    if !neutral {
                        if candidate.is_some() {
                            return Ok(ControlFlow::Break(()));
                        }
                        candidate = Some(leaf);
                    }
                    Ok(ControlFlow::Continue(()))
                })?;
        if outcome.is_break() {
            return Ok(Step::Unchanged);
        }
        Ok(match candidate {
            None => Step::Contradiction,
            Some(leaf) if source.disjunction => self.sure(leaf),
            Some(leaf) => self.never(leaf),
        })
    }
}

#[cfg(test)]
mod tests;
