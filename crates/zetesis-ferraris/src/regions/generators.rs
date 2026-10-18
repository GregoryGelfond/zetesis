//! Original generator heads ordered by ground dependency components.
//!
//! This is a split preference, never a truth, support, class or minimality
//! certificate. Both children of every selected open atom must still be visited.
//! Complete producers supply ordinary/atomic-choice heads. An atomic choice is
//! preferred even if another producer derives it; an ordinary disjunctive head
//! is preferred only when none of its producers has an ordinary singleton head.
//!
//! Dependency vertices are original atoms and formula nodes. Atoms precede their
//! occurrences, every operand precedes its parent (including both implication
//! operands and every native condition), and a producer body precedes all its
//! heads. Facts have no incoming producer dependency; constraints create none.
//! Two iterative graph passes assign components in topological order. Temporary
//! compact incidence tables reuse `Adjacency`; only atom ranks and packed head
//! marks survive. A component can contain arbitrary cycles: no shifting occurs.
//!
//! Construction is O(A + N + E + H), including repeated original incidences;
//! temporary storage is O(A + N + E + H), retained storage O(A). Every table
//! pass, initialized cell, inspected vertex and edge is charged. Named headers
//! and actual vector capacities are admitted before publication; borrowed theory
//! payload, allocator metadata and other stack locals are excluded. Selection
//! reads each eligible open atom once and has constant additional space.

use std::{fmt, mem::size_of};

use zetesis_cpu::{Cancellation, Stop};

use super::{
    NarrowingQuota, NodeView, OriginalSubject, PreferenceAttempt, Producers, Region, Theory, Work,
    adjacency::{Adjacency, Construction},
    flag_words,
};

#[cfg(test)]
mod tests;

/// Independent bounds for optional original generator ordering.
#[derive(Clone, Copy, Debug)]
pub struct GeneratorOrderLimits {
    /// Named simultaneous headers and actual vector capacities; not process RSS.
    pub max_bytes: usize,
    /// Charged head classification, graph construction and component work.
    pub max_work: u64,
}

/// An incomplete preference preparation, never a semantic verdict.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeneratorOrderError {
    /// Named storage exceeds its independent allowance.
    Storage {
        /// Complete request or actual capacity just observed.
        required: u128,
        /// Requested allowance; zero is a real limit.
        limit: usize,
    },
    /// Cancellation, deadline, work exhaustion, allocation or malformed identity.
    Stopped(Stop),
}

impl fmt::Display for GeneratorOrderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage { required, limit } => write!(
                formatter,
                "generator order needs {required} bytes, limit {limit}"
            ),
            Self::Stopped(stop) => stop.fmt(formatter),
        }
    }
}

impl std::error::Error for GeneratorOrderError {}

/// Preparation receipt, including an incomplete prefix.
#[derive(Debug)]
pub struct GeneratorOrderAttempt {
    /// No order when complete producer coverage or a generating head is absent.
    pub result: Result<Option<GeneratorOrder>, GeneratorOrderError>,
    /// Head, cell, vertex and edge operations charged on every outcome.
    pub work: u64,
    /// Largest named simultaneous request or capacity observed in this attempt.
    pub peak_bytes: u128,
}

/// Immutable original-theory-bound preference; not a formula transformation.
#[derive(Debug)]
pub struct GeneratorOrder {
    theory: Theory,
    heads: Vec<u64>,
    ranks: Vec<usize>,
}

impl GeneratorOrder {
    /// Prepare generator precedence from complete original producers.
    ///
    /// The subject's producer correspondence is the same precondition as
    /// original narrowing. No source spelling, predicate class or argument
    /// value is read. A theory without choices or competing heads declines
    /// without graph allocation. Cancellation precedes that fast path.
    #[must_use]
    pub fn prepare(
        original: OriginalSubject<'_>,
        limits: GeneratorOrderLimits,
        cancellation: &Cancellation,
    ) -> GeneratorOrderAttempt {
        Self::prepare_with(
            original,
            limits.max_bytes,
            cancellation,
            Work::new(limits.max_work),
        )
    }

    /// Prepare under a caller's streaming work quota. Every admitted operation
    /// keeps exactly one permit; unused permits are refunded on every outcome.
    /// `max_bytes` has the same ownership scope as [`GeneratorOrderLimits`].
    #[must_use]
    pub fn prepare_reserved(
        original: OriginalSubject<'_>,
        max_bytes: usize,
        cancellation: &Cancellation,
        quota: &mut dyn NarrowingQuota,
    ) -> GeneratorOrderAttempt {
        Self::prepare_with(original, max_bytes, cancellation, Work::reserved(quota))
    }

    fn prepare_with(
        original: OriginalSubject<'_>,
        max_bytes: usize,
        cancellation: &Cancellation,
        work: Work<'_>,
    ) -> GeneratorOrderAttempt {
        let mut budget = Budget {
            max_bytes,
            cancellation,
            work,
            bytes: 0,
            peak: 0,
            failure: None,
        };
        let result = cancellation.poll().and_then(|()| {
            let Some(producers) = original.producers else {
                return Ok(None);
            };
            if !producers.has_competing_heads && !producers.has_choice_heads {
                return Ok(None);
            }
            build(original.theory, producers, &mut budget)
        });
        let result = result.map_err(|stop| {
            budget
                .failure
                .take()
                .unwrap_or(GeneratorOrderError::Stopped(stop))
        });
        budget.work.refund();
        GeneratorOrderAttempt {
            result,
            work: budget.work.spent,
            peak_bytes: budget.peak,
        }
    }

    /// Whether these semantic atom coordinates belong to this exact theory.
    #[must_use]
    pub fn belongs_to(&self, theory: &Theory) -> bool {
        self.theory.same_instance(theory)
    }

    /// Header and retained vector capacities, excluding the shared theory.
    #[must_use]
    pub fn retained_bytes(&self) -> u128 {
        size_of::<Self>() as u128
            + self.heads.capacity() as u128 * size_of::<u64>() as u128
            + self.ranks.capacity() as u128 * size_of::<usize>() as u128
    }

    /// Select the earliest component among open generating heads, or all open
    /// atoms when none remain. Atom identity breaks component ties. The theory
    /// is authenticated before reading the region; region atom coordinates must
    /// belong to it. Both split children remain necessary for complete coverage.
    pub fn preferred_atom_reserved(
        &self,
        theory: &Theory,
        region: &Region,
        cancellation: &Cancellation,
        quota: &mut dyn NarrowingQuota,
    ) -> PreferenceAttempt {
        let mut work = Work::reserved(quota);
        let result = cancellation
            .poll()
            .and_then(|()| self.preferred(theory, region, &mut work));
        work.refund();
        PreferenceAttempt {
            result,
            work: work.spent,
        }
    }

    pub(super) fn preferred(
        &self,
        theory: &Theory,
        region: &Region,
        work: &mut Work<'_>,
    ) -> Result<Option<usize>, Stop> {
        if !self.belongs_to(theory) || region.len() != self.ranks.len() {
            return Err(Stop::InvalidProgram);
        }
        let mut marked = region.open_in(&self.heads).peekable();
        let any_marked = marked.peek().is_some();
        let eligible = marked.chain((!any_marked).then(|| region.open()).into_iter().flatten());
        let mut best = None;
        for atom in eligible {
            work.tick()?;
            let key = (self.ranks[atom], atom);
            if best.is_none_or(|old| key < old) {
                best = Some(key);
            }
        }
        Ok(best.map(|(_, atom)| atom))
    }
}

struct Graph {
    outgoing: Adjacency,
    incoming: Adjacency,
}

#[derive(Clone, Copy)]
struct Frame {
    vertex: usize,
    next: usize,
}

fn build(
    theory: &Theory,
    producers: &Producers,
    budget: &mut Budget<'_, '_>,
) -> Result<Option<GeneratorOrder>, Stop> {
    let headers = size_of::<GeneratorOrder>()
        + size_of::<Graph>()
        + size_of::<Vec<u8>>() * 2
        + size_of::<Vec<usize>>() * 4
        + size_of::<Vec<Frame>>();
    budget.observe(headers as u128)?;
    let (heads, any) = generator_heads(theory.atom_count(), producers, budget)?;
    if !any {
        return Ok(None);
    }
    let vertices = theory
        .atom_count()
        .checked_add(theory.nodes().len())
        .ok_or(Stop::Allocation)?;
    let incidences = edges(theory, producers);
    let graph = Graph {
        outgoing: Adjacency::try_build_in(vertices, incidences.clone().map(Ok), budget)?,
        incoming: Adjacency::try_build_in(
            vertices,
            incidences.map(|event| Ok(event.map(|(from, to)| (to, from)))),
            budget,
        )?,
    };
    let finished = finishing_order(&graph, vertices, budget)?;
    let components = components(&graph, &finished, vertices, budget)?;
    // The graph walk's coordinates are checked before any order is published.
    for from in 0..vertices {
        budget.tick()?;
        for &to in &graph.outgoing[from] {
            budget.tick()?;
            if components[from] > components[to] {
                return Err(Stop::InvalidProgram);
            }
        }
    }
    let mut ranks = budget.reserve(theory.atom_count())?;
    for &component in &components[..theory.atom_count()] {
        budget.tick()?;
        ranks.push(component);
    }
    budget.release(components)?;
    budget.release(finished)?;
    graph.outgoing.release_in(budget)?;
    graph.incoming.release_in(budget)?;
    Ok(Some(GeneratorOrder {
        theory: theory.clone(),
        heads,
        ranks,
    }))
}

fn generator_heads(
    atoms: usize,
    producers: &Producers,
    budget: &mut Budget<'_, '_>,
) -> Result<(Vec<u64>, bool), Stop> {
    let mut any = false;
    let mut heads = budget.filled(flag_words(atoms), 0_u64)?;
    for atom in 0..atoms {
        budget.tick()?;
        let rows = &producers.by_head[atom];
        let mut choice = false;
        let mut pure = !rows.is_empty();
        for &row in rows {
            budget.tick()?;
            let rule = &producers.rules[row];
            choice |= rule.choice;
            pure &= rule.choice || rule.heads.len() > 1;
        }
        if choice || pure {
            heads[atom / 64] |= 1_u64 << (atom % 64);
            any = true;
        }
    }
    Ok((heads, any))
}

/// A marker charges each node/producer inspection even when it has no edge.
/// Original occurrence order is retained, including duplicate operands.
fn edges<'a>(
    theory: &'a Theory,
    producers: &'a Producers,
) -> impl Iterator<Item = Option<(usize, usize)>> + Clone + 'a {
    let atoms = theory.atom_count();
    let nodes = theory.view();
    (0..nodes.len())
        .flat_map(move |node| {
            std::iter::once(None).chain(
                inputs(nodes.node(node).expect("admitted node"), atoms)
                    .map(move |source| Some((source, atoms + node))),
            )
        })
        .chain(producers.rules.iter().flat_map(move |rule| {
            std::iter::once(None).chain(rule.body.into_iter().flat_map(move |body| {
                rule.heads
                    .iter()
                    .map(move |&head| Some((atoms + body, head)))
            }))
        }))
}

fn inputs(view: NodeView<'_>, atoms: usize) -> impl Iterator<Item = usize> + Clone + '_ {
    let (atom, pair, children): (Option<usize>, [Option<usize>; 2], &[usize]) = match view {
        NodeView::Atom(atom) => (Some(atom), [None, None], &[]),
        NodeView::False => (None, [None, None], &[]),
        NodeView::Implies(left, right) => (None, [Some(left), Some(right)], &[]),
        NodeView::And(children) | NodeView::Or(children) => (None, [None, None], children),
        NodeView::Threshold(row) => (None, [None, None], row.conditions),
        NodeView::ExtremumCut(cut) => (None, [None, None], cut.row.conditions),
    };
    atom.into_iter().chain(
        pair.into_iter()
            .flatten()
            .chain(children.iter().copied())
            .map(move |node| atoms + node),
    )
}

fn finishing_order(
    graph: &Graph,
    vertices: usize,
    budget: &mut Budget<'_, '_>,
) -> Result<Vec<usize>, Stop> {
    let mut seen = budget.filled(vertices, 0_u8)?;
    let mut stack: Vec<Frame> = budget.reserve(vertices)?;
    let mut finished = budget.reserve(vertices)?;
    for start in 0..vertices {
        budget.tick()?;
        if seen[start] != 0 {
            continue;
        }
        seen[start] = 1;
        stack.push(Frame {
            vertex: start,
            next: 0,
        });
        while let Some(frame) = stack.last().copied() {
            budget.tick()?;
            let Some(&child) = graph.outgoing[frame.vertex].get(frame.next) else {
                stack.pop();
                finished.push(frame.vertex);
                continue;
            };
            stack.last_mut().ok_or(Stop::InvalidProgram)?.next += 1;
            if seen[child] == 0 {
                seen[child] = 1;
                stack.push(Frame {
                    vertex: child,
                    next: 0,
                });
            }
        }
    }
    budget.release(seen)?;
    budget.release(stack)?;
    Ok(finished)
}

fn components(
    graph: &Graph,
    finished: &[usize],
    vertices: usize,
    budget: &mut Budget<'_, '_>,
) -> Result<Vec<usize>, Stop> {
    let mut assigned = budget.filled(vertices, 0_u8)?;
    let mut components = budget.filled(vertices, 0_usize)?;
    let mut queue = budget.reserve(vertices)?;
    let mut component = 0;
    for &start in finished.iter().rev() {
        budget.tick()?;
        if assigned[start] != 0 {
            continue;
        }
        queue.clear();
        queue.push(start);
        assigned[start] = 1;
        components[start] = component;
        let mut next = 0;
        while let Some(&vertex) = queue.get(next) {
            budget.tick()?;
            next += 1;
            for &source in &graph.incoming[vertex] {
                budget.tick()?;
                if assigned[source] == 0 {
                    assigned[source] = 1;
                    components[source] = component;
                    queue.push(source);
                }
            }
        }
        component += 1;
    }
    budget.release(assigned)?;
    budget.release(queue)?;
    Ok(components)
}

struct Budget<'a, 'q> {
    max_bytes: usize,
    cancellation: &'a Cancellation,
    work: Work<'q>,
    bytes: u128,
    peak: u128,
    failure: Option<GeneratorOrderError>,
}

impl Budget<'_, '_> {
    fn tick(&mut self) -> Result<(), Stop> {
        self.cancellation.poll()?;
        self.work.tick()
    }

    fn observe(&mut self, bytes: u128) -> Result<(), Stop> {
        self.peak = self.peak.max(bytes);
        if bytes > self.max_bytes as u128 {
            self.failure = Some(GeneratorOrderError::Storage {
                required: bytes,
                limit: self.max_bytes,
            });
            return Err(Stop::Allocation);
        }
        self.bytes = bytes;
        Ok(())
    }

    fn reserve<T>(&mut self, count: usize) -> Result<Vec<T>, Stop> {
        self.cancellation.poll()?;
        let prior = self.bytes;
        self.observe(prior + count as u128 * size_of::<T>() as u128)?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(count)
            .map_err(|_| Stop::Allocation)?;
        self.observe(prior + values.capacity() as u128 * size_of::<T>() as u128)?;
        Ok(values)
    }

    fn filled<T: Copy>(&mut self, count: usize, value: T) -> Result<Vec<T>, Stop> {
        let mut values = self.reserve(count)?;
        for _ in 0..count {
            self.tick()?;
            values.push(value);
        }
        Ok(values)
    }

    fn release<T>(&mut self, values: Vec<T>) -> Result<(), Stop> {
        self.bytes = self
            .bytes
            .checked_sub(values.capacity() as u128 * size_of::<T>() as u128)
            .ok_or(Stop::InvalidProgram)?;
        drop(values);
        Ok(())
    }
}

impl Construction for Budget<'_, '_> {
    fn tick(&mut self) -> Result<(), Stop> {
        Self::tick(self)
    }
    fn zeros(&mut self, count: usize) -> Result<Vec<usize>, Stop> {
        self.filled(count, 0)
    }
    fn reserve(&mut self, count: usize) -> Result<Vec<usize>, Stop> {
        Self::reserve(self, count)
    }
    fn release(&mut self, values: Vec<usize>) -> Result<(), Stop> {
        Self::release(self, values)
    }
}
