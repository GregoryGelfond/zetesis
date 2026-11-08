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

use zetesis_algebra::{
    Cancellation, Failure, Meter, MeterBytes, POLL_INTERVAL, Quota, Stop, words::Words,
};

use super::{
    NodeView, OriginalSubject, PreferenceAttempt, Producers, Region, Theory,
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
pub enum GeneratorOrderError<E = Stop> {
    /// Named storage exceeds its independent allowance.
    Storage {
        /// Complete request or actual capacity just observed.
        required: u128,
        /// Requested allowance; zero is a real limit.
        limit: usize,
    },
    /// Cancellation, deadline, work exhaustion, allocation or malformed identity.
    Stopped(E),
}

impl<E: fmt::Display> fmt::Display for GeneratorOrderError<E> {
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

impl<E: std::error::Error> std::error::Error for GeneratorOrderError<E> {}

impl<E> From<Stop> for GeneratorOrderError<Failure<E>> {
    fn from(stop: Stop) -> Self {
        Self::Stopped(Failure::Stopped(stop))
    }
}

impl<E> From<Failure<E>> for GeneratorOrderError<Failure<E>> {
    fn from(failure: Failure<E>) -> Self {
        Self::Stopped(failure)
    }
}

/// Preparation receipt, including an incomplete prefix.
#[derive(Debug)]
pub struct GeneratorOrderAttempt<E = Stop> {
    /// No order when complete producer coverage or a generating head is absent.
    pub result: Result<Option<GeneratorOrder>, GeneratorOrderError<E>>,
    /// Head, cell, vertex and edge operations charged on every outcome.
    pub work: u64,
    /// Largest named simultaneous request or capacity observed in this attempt.
    pub peak_bytes: u128,
}

/// Immutable original-theory-bound preference; not a formula transformation.
#[derive(Debug)]
pub struct GeneratorOrder {
    theory: Theory,
    heads: Words<u64>,
    ranks: Vec<usize>,
}

impl GeneratorOrder {
    /// Prepare generator precedence from complete original producers.
    ///
    /// The subject's producer correspondence is the same precondition as
    /// original narrowing. No source spelling, predicate class or argument
    /// value is read. A theory without choices or competing heads declines
    /// without graph allocation. Cancellation precedes that fast path.
    ///
    /// Preparation polls for cancellation at entry, before reserving vector
    /// storage and before reporting a refused work charge. With
    /// [`Self::prepare_reserved`], a polling quota also observes cancellation
    /// when the meter refills.
    #[must_use]
    pub fn prepare(
        original: OriginalSubject<'_>,
        limits: GeneratorOrderLimits,
        cancellation: &Cancellation,
    ) -> GeneratorOrderAttempt {
        let attempt = Self::prepare_with(
            original,
            limits.max_bytes,
            cancellation,
            &mut Meter::local(limits.max_work),
        );
        GeneratorOrderAttempt {
            result: attempt.result.map_err(|error| match error {
                GeneratorOrderError::Storage { required, limit } => {
                    GeneratorOrderError::Storage { required, limit }
                }
                GeneratorOrderError::Stopped(failure) => {
                    GeneratorOrderError::Stopped(failure.into_stop())
                }
            }),
            work: attempt.work,
            peak_bytes: attempt.peak_bytes,
        }
    }

    /// Prepare under a caller's streaming work meter. Every admitted operation
    /// keeps exactly one permit; unused permits are refunded on every outcome.
    /// `max_bytes` has the same ownership scope as [`GeneratorOrderLimits`].
    #[must_use]
    pub fn prepare_reserved<Q: Quota + ?Sized>(
        original: OriginalSubject<'_>,
        max_bytes: usize,
        cancellation: &Cancellation,
        work: &mut Meter<'_, Q>,
    ) -> GeneratorOrderAttempt<Failure<Q::Error>> {
        Self::prepare_with(original, max_bytes, cancellation, work)
    }

    fn prepare_with<Q: Quota + ?Sized>(
        original: OriginalSubject<'_>,
        max_bytes: usize,
        cancellation: &Cancellation,
        work: &mut Meter<'_, Q>,
    ) -> GeneratorOrderAttempt<Failure<Q::Error>> {
        let before = work.charged();
        let mut budget = Budget {
            max_bytes,
            cancellation,
            work,
            bytes: 0,
            peak: 0,
        };
        let result = cancellation
            .poll()
            .map_err(GeneratorOrderError::from)
            .and_then(|()| {
                let Some(producers) = original.producers else {
                    return Ok(None);
                };
                if !producers.has_competing_heads && !producers.has_choice_heads {
                    return Ok(None);
                }
                build(
                    original.theory,
                    producers,
                    edges(original.theory, producers),
                    &mut budget,
                )
            });
        budget.work.refund();
        GeneratorOrderAttempt {
            result,
            work: budget.work.charged() - before,
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
    pub fn preferred_atom_reserved<Q: Quota + ?Sized>(
        &self,
        theory: &Theory,
        region: &Region,
        cancellation: &Cancellation,
        work: &mut Meter<'_, Q>,
    ) -> PreferenceAttempt<Failure<Q::Error>> {
        let before = work.charged();
        let result = cancellation
            .poll()
            .map_err(Failure::from)
            .and_then(|()| self.preferred(theory, region, work));
        work.refund();
        PreferenceAttempt {
            result,
            work: work.charged() - before,
        }
    }

    pub(super) fn preferred<Q: Quota + ?Sized>(
        &self,
        theory: &Theory,
        region: &Region,
        work: &mut Meter<'_, Q>,
    ) -> Result<Option<usize>, Failure<Q::Error>> {
        if !self.belongs_to(theory) || region.len() != self.ranks.len() {
            return Err(Failure::Stopped(Stop::InvalidProgram));
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

/// Classified original heads and whether graph preparation has a consumer.
struct GeneratorHeads {
    heads: Words<u64>,
    any: bool,
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

fn build<Q: Quota + ?Sized>(
    theory: &Theory,
    producers: &Producers,
    incidences: impl Iterator<Item = Option<(usize, usize)>> + Clone,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<Option<GeneratorOrder>, GeneratorOrderError<Failure<Q::Error>>> {
    let headers = size_of::<GeneratorOrder>()
        + size_of::<Graph>()
        + size_of::<Vec<u8>>() * 2
        + size_of::<Vec<usize>>() * 4
        + size_of::<Vec<Frame>>();
    budget.observe(headers as u128)?;
    let GeneratorHeads { heads, any } = generator_heads(theory.atom_count(), producers, budget)?;
    if !any {
        return Ok(None);
    }
    let vertices = theory
        .atom_count()
        .checked_add(theory.nodes().len())
        .ok_or(Stop::Allocation)?;
    let (outgoing, incoming) = Adjacency::try_build_pair_in(vertices, incidences.map(Ok), budget)?;
    let graph = Graph { outgoing, incoming };
    let finished = finishing_order(&graph, vertices, budget)?;
    let components = components(&graph, &finished, vertices, budget)?;
    // The graph walk's coordinates are checked before any order is published.
    for from in 0..vertices {
        budget.tick()?;
        for &to in &graph.outgoing[from] {
            budget.tick()?;
            if components[from] > components[to] {
                return Err(Stop::InvalidProgram.into());
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

fn generator_heads<Q: Quota + ?Sized>(
    atoms: usize,
    producers: &Producers,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<GeneratorHeads, GeneratorOrderError<Failure<Q::Error>>> {
    let mut any = false;
    let mut heads = Words::new(budget.filled(flag_words(atoms), 0_u64)?);
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
            heads.insert(atom);
            any = true;
        }
    }
    Ok(GeneratorHeads { heads, any })
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

fn finishing_order<Q: Quota + ?Sized>(
    graph: &Graph,
    vertices: usize,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<Vec<usize>, GeneratorOrderError<Failure<Q::Error>>> {
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

fn components<Q: Quota + ?Sized>(
    graph: &Graph,
    finished: &[usize],
    vertices: usize,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<Vec<usize>, GeneratorOrderError<Failure<Q::Error>>> {
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

struct Budget<'a, 'q, Q: Quota + ?Sized> {
    max_bytes: usize,
    cancellation: &'a Cancellation,
    work: &'a mut Meter<'q, Q>,
    bytes: u128,
    peak: u128,
}

impl<Q: Quota + ?Sized> Budget<'_, '_, Q> {
    /// Charge one operation. Control is observed as a metered operation
    /// observes it: the order's entry before the first charge, the reserved
    /// meter's account at every refill of `POLL_INTERVAL` permits, and this
    /// tick before a refused charge is reported.
    fn tick(&mut self) -> Result<(), GeneratorOrderError<Failure<Q::Error>>> {
        if let Err(failure) = self.work.tick() {
            self.poll()?;
            return Err(GeneratorOrderError::from(failure));
        }
        Ok(())
    }

    /// Observe control, out of line so that the tick it guards stays small
    /// enough to inline where it is hot.
    #[cold]
    #[inline(never)]
    fn poll(&self) -> Result<(), GeneratorOrderError<Failure<Q::Error>>> {
        self.cancellation.poll()?;
        Ok(())
    }

    fn observe(&mut self, bytes: u128) -> Result<(), GeneratorOrderError<Failure<Q::Error>>> {
        MeterBytes::observe_peak(bytes, self.max_bytes as u128, &mut self.peak).map_err(|_| {
            GeneratorOrderError::Storage {
                required: bytes,
                limit: self.max_bytes,
            }
        })?;
        self.bytes = bytes;
        Ok(())
    }

    fn reserve<T>(
        &mut self,
        count: usize,
    ) -> Result<Vec<T>, GeneratorOrderError<Failure<Q::Error>>> {
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

    /// `count` copies of `value`, one charged operation each. Between
    /// observation boundaries the copies already admitted are charged as one
    /// run; the boundary's own operation, and any that needs a refill, is a
    /// tick. So the polls, refills and the refused operation fall where
    /// `count` ticks would put them, and the receipt is the same.
    fn filled<T: Copy>(
        &mut self,
        count: usize,
        value: T,
    ) -> Result<Vec<T>, GeneratorOrderError<Failure<Q::Error>>> {
        let mut values = self.reserve(count)?;
        while values.len() < count {
            let charged = self.work.charged();
            let offset = charged % POLL_INTERVAL;
            let run = if offset == 0 {
                0
            } else {
                (POLL_INTERVAL - offset)
                    .min(self.work.remaining())
                    .min((count - values.len()) as u64)
            };
            if run == 0 {
                self.tick()?;
                values.push(value);
                continue;
            }
            if let Err(failure) = self.work.bounded(run, || ((), run)) {
                self.poll()?;
                return Err(GeneratorOrderError::from(failure));
            }
            let run = usize::try_from(run).map_err(|_| Stop::Allocation)?;
            values.resize(values.len() + run, value);
        }
        Ok(values)
    }

    fn release<T>(&mut self, values: Vec<T>) -> Result<(), GeneratorOrderError<Failure<Q::Error>>> {
        self.bytes = self
            .bytes
            .checked_sub(values.capacity() as u128 * size_of::<T>() as u128)
            .ok_or(Stop::InvalidProgram)?;
        drop(values);
        Ok(())
    }
}

impl<Q: Quota + ?Sized> Construction for Budget<'_, '_, Q> {
    type Error = GeneratorOrderError<Failure<Q::Error>>;
    fn tick(&mut self) -> Result<(), GeneratorOrderError<Failure<Q::Error>>> {
        Self::tick(self)
    }
    fn zeros(
        &mut self,
        count: usize,
    ) -> Result<Vec<usize>, GeneratorOrderError<Failure<Q::Error>>> {
        self.filled(count, 0)
    }
    fn reserve(
        &mut self,
        count: usize,
    ) -> Result<Vec<usize>, GeneratorOrderError<Failure<Q::Error>>> {
        Self::reserve(self, count)
    }
    fn release(
        &mut self,
        values: Vec<usize>,
    ) -> Result<(), GeneratorOrderError<Failure<Q::Error>>> {
        Self::release(self, values)
    }
}
