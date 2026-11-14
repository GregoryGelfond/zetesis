//! The component schedule of rich producers.
//!
//! A rich producer is traversed in full whenever a predicate it reads gains
//! rows. When every predicate it reads lies in a dependency component before
//! the components of its heads, those reads stop changing once their
//! components reach their fixed point, and a single traversal then derives
//! every head the per-change traversals would. Such a producer is *deferred*:
//! it waits for that fixed point and is traversed once, in the round in which
//! the components it reads reach it, over that round's snapshot. Rich
//! producers that read their own component, or a later one, keep the
//! per-change traversal.
//!
//! Components come from the dependency graph's dependency-before-dependent
//! order. A component's dependencies, for this schedule, are the components of
//! every positive input of a certified producer of one of its predicates and of
//! every predicate a rich producer of one of its predicates reads, of either
//! sign. A component is *decidable* when each dependency precedes it or is
//! itself, and every earlier dependency is decidable; a producer reading an
//! undecidable component is never deferred.
//!
//! After a round's other producers have run, a component has reached its fixed
//! point when it gained no atom this round, every earlier dependency has
//! reached its fixed point, and every deferred producer of its predicates has
//! been traversed. Its producers then ran over final inputs and derived nothing
//! new, so no later round can derive anything for it. The test runs in
//! component order, so one pass decides every component; deferred producers
//! whose reads all passed are traversed, and the test repeats until none is
//! ready. Each deferred producer is traversed exactly once. When a round
//! derives nothing, every component not waiting on an untraversed deferred
//! producer passes, and the deferred producer of the earliest head component
//! among those still waiting reads only passed components, so completion never
//! ends with a deferred producer untraversed.
//!
//! Per round the test costs one visit per pending atom, a binary search for each
//! run of atoms of one predicate, and one visit per component, dependency and
//! deferred read. It is skipped once every deferred producer was traversed.

use std::cell::{Cell, RefCell};
use std::mem::size_of;
use std::ops::Range;

use zetesis_core::TemplateComponentsRef;
use zetesis_core::catalog::PredicateRef;

use super::super::super::relations::{Memory, SupportAppend};
use super::super::{Node, lookup};
use super::{Producer, Registration, predicate};
use crate::formula_ir::{HeadIr, LiteralIr, Prepared, RuleIr};
use crate::{FormulaFailure, FormulaLimits, ProgramSite};

use super::super::super::Counters;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum State {
    Waiting,
    Ready,
    Traversed,
}

/// Which deferred producers wait for which components, and the scratch the
/// per-round test fills.
pub(super) struct Schedule<'source> {
    nodes: Vec<Node<'source>>,
    decidable: Vec<bool>,
    dependencies: Vec<usize>,
    dependency_ranges: Vec<Range<usize>>,
    producers_of: Vec<usize>,
    producer_ranges: Vec<Range<usize>>,
    reads: Vec<usize>,
    /// Per registered producer: its read components, when it is deferred.
    deferred: Vec<Option<Range<usize>>>,
    state: Vec<Cell<State>>,
    changed: RefCell<Vec<bool>>,
    reached: RefCell<Vec<bool>>,
}

/// Scratch for one preparation: (head, input) and (head, producer) pairs, per
/// producer its first head and whether every predicate it names has a
/// component, per component whether all of its producers' predicates do, and
/// one producer's resolved components.
#[derive(Default)]
struct Scratch {
    edges: Vec<(usize, usize)>,
    owners: Vec<(usize, usize)>,
    first_heads: Vec<Option<usize>>,
    known_producers: Vec<bool>,
    known: Vec<bool>,
    inputs: Vec<usize>,
    heads: Vec<usize>,
}

impl Scratch {
    fn bytes(&self) -> usize {
        (self.edges.capacity() + self.owners.capacity()) * size_of::<(usize, usize)>()
            + self.first_heads.capacity() * size_of::<Option<usize>>()
            + self.known_producers.capacity()
            + self.known.capacity()
            + (self.inputs.capacity() + self.heads.capacity()) * size_of::<usize>()
    }
}

/// Per registered producer its read components' range, when it is deferred,
/// and the read components themselves.
type Deferred = (Vec<Option<Range<usize>>>, Vec<usize>);

/// A component's dependencies, as ranges into one list, and whether its fixed
/// point can be decided in component order.
struct Dependencies {
    list: Vec<usize>,
    ranges: Vec<Range<usize>>,
    decidable: Vec<bool>,
}

impl<'source> Schedule<'source> {
    /// The schedule of `registration`'s rich producers, or `None` when none
    /// can be deferred. Retained vectors are admitted through `memory`; the
    /// scratch is released before returning.
    pub(super) fn prepare(
        registration: &Registration<'source>,
        prepared: &'source Prepared,
        view: TemplateComponentsRef<'_>,
        memory: &mut Memory<'_>,
        work: (&FormulaLimits, &mut Counters, ProgramSite),
    ) -> Result<Option<Self>, FormulaFailure> {
        // The test-only reference traverses every rich producer every round.
        if super::every_round_traversed() {
            return Ok(None);
        }
        let (limits, counters, location) = work;
        let nodes = super::super::graph_nodes(prepared, memory, limits, counters, location)?;
        let count = nodes
            .iter()
            .filter_map(|node| node.component)
            .max()
            .map_or(0, |last| last + 1);
        let mut scratch = Scratch::default();
        let inputs = (registration, nodes.as_slice(), view);
        resolve_all(
            inputs,
            count,
            &mut scratch,
            memory,
            (limits, counters, location),
        )?;
        let dependencies = dependencies(&mut scratch, count, memory, (limits, counters, location))?;
        let (deferred, reads) = classify(
            inputs,
            &dependencies.decidable,
            &mut scratch,
            memory,
            (limits, counters, location),
        )?;
        let (producers_of, producer_ranges) = if deferred.iter().any(Option::is_some) {
            by_component(
                &mut scratch.owners,
                count,
                memory,
                (limits, counters, location),
            )?
        } else {
            (Vec::new(), Vec::new())
        };
        memory.release(scratch.bytes());
        drop(scratch);
        if deferred.iter().all(Option::is_none) {
            return Ok(None);
        }
        let mut state = Vec::new();
        memory.reserve(&mut state, deferred.len())?;
        state.extend(deferred.iter().map(|_| Cell::new(State::Waiting)));
        let mut changed = Vec::new();
        let mut reached = Vec::new();
        memory.reserve(&mut changed, count)?;
        memory.reserve(&mut reached, count)?;
        changed.resize(count, false);
        reached.resize(count, false);
        Ok(Some(Self {
            nodes,
            decidable: dependencies.decidable,
            dependencies: dependencies.list,
            dependency_ranges: dependencies.ranges,
            producers_of,
            producer_ranges,
            reads,
            deferred,
            state,
            changed: RefCell::new(changed),
            reached: RefCell::new(reached),
        }))
    }

    /// The producer's state, when it is deferred.
    pub(super) fn state(&self, index: usize) -> Option<State> {
        self.deferred
            .get(index)
            .and_then(Option::as_ref)
            .map(|_| self.state[index].get())
    }

    /// Whether a deferred producer has not yet been traversed.
    pub(super) fn pending(&self) -> bool {
        self.state
            .iter()
            .zip(&self.deferred)
            .any(|(state, deferred)| deferred.is_some() && state.get() != State::Traversed)
    }

    /// Mark ready every waiting producer whose read components reached their
    /// fixed point, given this round's pending atoms in `delta`. Returns
    /// whether any producer became ready.
    pub(super) fn mark_ready(
        &self,
        delta: &SupportAppend<'_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<bool, FormulaFailure> {
        if !self.state.iter().any(|state| state.get() == State::Waiting) {
            return Ok(false);
        }
        let mut changed = self.changed.borrow_mut();
        changed.fill(false);
        let mut previous: Option<PredicateRef<'_>> = None;
        for atom in delta.atoms() {
            counters.work(limits, location)?;
            let atom_predicate = atom.predicate();
            if let Some(previous) = previous
                && previous.equals_ref_with(atom_predicate, || counters.work(limits, location))?
            {
                continue;
            }
            previous = Some(atom_predicate);
            let found = lookup(
                self.nodes.len(),
                |index| self.nodes[index].signature,
                atom_predicate,
                limits,
                counters,
                location,
            )?;
            if let Some(component) = found.and_then(|index| self.nodes[index].component) {
                changed[component] = true;
            }
        }
        let mut reached = self.reached.borrow_mut();
        for component in 0..reached.len() {
            let dependencies = &self.dependencies[self.dependency_ranges[component].clone()];
            let producers = &self.producers_of[self.producer_ranges[component].clone()];
            counters.charge_work(
                1 + dependencies.len() as u128 + producers.len() as u128,
                limits,
                location,
            )?;
            reached[component] = !changed[component]
                && self.decidable[component]
                && dependencies
                    .iter()
                    .all(|&other| other == component || reached[other])
                && producers
                    .iter()
                    .all(|&producer| self.state[producer].get() == State::Traversed);
        }
        let mut any = false;
        for (index, reads) in self.deferred.iter().enumerate() {
            let Some(reads) = reads else {
                continue;
            };
            counters.charge_work(1 + reads.len() as u128, limits, location)?;
            if self.state[index].get() == State::Waiting
                && self.reads[reads.clone()].iter().all(|&read| reached[read])
            {
                self.state[index].set(State::Ready);
                any = true;
            }
        }
        Ok(any)
    }

    /// The next ready producer, which the caller traverses now.
    pub(super) fn take_ready(&self) -> Option<usize> {
        let index = self
            .state
            .iter()
            .position(|state| state.get() == State::Ready)?;
        self.state[index].set(State::Traversed);
        Some(index)
    }
}

/// Resolve every producer: its (head, input) edges, its first head, and
/// whether every predicate it names has a component.
fn resolve_all(
    (registration, nodes, view): (&Registration<'_>, &[Node<'_>], TemplateComponentsRef<'_>),
    count: usize,
    scratch: &mut Scratch,
    memory: &mut Memory<'_>,
    (limits, counters, location): (&FormulaLimits, &mut Counters, ProgramSite),
) -> Result<(), FormulaFailure> {
    let producers = registration.producers.len();
    memory.reserve(&mut scratch.first_heads, producers)?;
    memory.reserve(&mut scratch.known_producers, producers)?;
    memory.reserve(&mut scratch.known, count)?;
    scratch.known.resize(count, true);
    for (index, producer) in registration.producers.iter().enumerate() {
        counters.work(limits, location)?;
        let rule = &registration.rules[index];
        scratch.inputs.clear();
        scratch.heads.clear();
        let complete = resolve(
            registration,
            producer,
            rule,
            nodes,
            view,
            (&mut scratch.inputs, &mut scratch.heads, memory),
            (limits, counters, rule.location),
        )?;
        memory.reserve(
            &mut scratch.edges,
            scratch.heads.len() * scratch.inputs.len(),
        )?;
        for &head in &scratch.heads {
            scratch.known[head] &= complete;
            scratch
                .edges
                .extend(scratch.inputs.iter().map(|&input| (head, input)));
        }
        scratch
            .first_heads
            .push(scratch.heads.iter().copied().min());
        scratch.known_producers.push(complete);
    }
    Ok(())
}

/// Each component's dependencies, and whether its fixed point is decidable:
/// every dependency precedes it or is itself, and each earlier one is.
fn dependencies(
    scratch: &mut Scratch,
    count: usize,
    memory: &mut Memory<'_>,
    (limits, counters, location): (&FormulaLimits, &mut Counters, ProgramSite),
) -> Result<Dependencies, FormulaFailure> {
    counters.charge_work(scratch.edges.len() as u128 + 1, limits, location)?;
    scratch.edges.sort_unstable();
    scratch.edges.dedup();
    let mut list = Vec::new();
    let mut ranges = Vec::new();
    let mut decidable = Vec::new();
    memory.reserve(&mut list, scratch.edges.len())?;
    memory.reserve(&mut ranges, count)?;
    memory.reserve(&mut decidable, count)?;
    let mut edges = scratch.edges.iter().peekable();
    for (component, &known) in scratch.known.iter().enumerate() {
        counters.work(limits, location)?;
        let start = list.len();
        let mut earlier = true;
        while let Some(&(_, other)) = edges.next_if(|&&(head, _)| head == component) {
            earlier &= other == component || (other < component && decidable[other]);
            list.push(other);
        }
        ranges.push(start..list.len());
        decidable.push(known && earlier);
    }
    Ok(Dependencies {
        list,
        ranges,
        decidable,
    })
}

/// The deferred producers: rich, with every read decidable and earlier than
/// every head. Returns each producer's read range and the reads; records each
/// deferred producer's (head, producer) pairs in the scratch.
fn classify(
    (registration, nodes, view): (&Registration<'_>, &[Node<'_>], TemplateComponentsRef<'_>),
    decidable: &[bool],
    scratch: &mut Scratch,
    memory: &mut Memory<'_>,
    (limits, counters, location): (&FormulaLimits, &mut Counters, ProgramSite),
) -> Result<Deferred, FormulaFailure> {
    let mut deferred = Vec::new();
    let mut reads = Vec::new();
    memory.reserve(&mut deferred, registration.producers.len())?;
    for (index, producer) in registration.producers.iter().enumerate() {
        counters.work(limits, location)?;
        let Producer::Rich(_) = producer else {
            deferred.push(None);
            continue;
        };
        let rule = &registration.rules[index];
        scratch.inputs.clear();
        scratch.heads.clear();
        resolve(
            registration,
            producer,
            rule,
            nodes,
            view,
            (&mut scratch.inputs, &mut scratch.heads, memory),
            (limits, counters, rule.location),
        )?;
        let waits = scratch.known_producers[index]
            && scratch.first_heads[index].is_some_and(|first| {
                scratch
                    .inputs
                    .iter()
                    .all(|&read| read < first && decidable[read])
            });
        if !waits {
            deferred.push(None);
            continue;
        }
        let start = reads.len();
        memory.reserve(&mut reads, scratch.inputs.len())?;
        reads.extend_from_slice(&scratch.inputs);
        deferred.push(Some(start..reads.len()));
        memory.reserve(&mut scratch.owners, scratch.heads.len())?;
        scratch
            .owners
            .extend(scratch.heads.iter().map(|&head| (head, index)));
    }
    Ok((deferred, reads))
}

/// The deferred producers of each component, as ranges into one list.
fn by_component(
    owners: &mut [(usize, usize)],
    count: usize,
    memory: &mut Memory<'_>,
    (limits, counters, location): (&FormulaLimits, &mut Counters, ProgramSite),
) -> Result<(Vec<usize>, Vec<Range<usize>>), FormulaFailure> {
    counters.charge_work(owners.len() as u128 + 1, limits, location)?;
    owners.sort_unstable();
    let mut list = Vec::new();
    let mut ranges = Vec::new();
    memory.reserve(&mut list, owners.len())?;
    memory.reserve(&mut ranges, count)?;
    let mut entries = owners.iter().peekable();
    for component in 0..count {
        counters.work(limits, location)?;
        let start = list.len();
        while let Some(&(_, producer)) = entries.next_if(|&&(head, _)| head == component) {
            if list.last() != Some(&producer) || list.len() == start {
                list.push(producer);
            }
        }
        ranges.push(start..list.len());
    }
    Ok((list, ranges))
}

/// A producer's head components into `heads` and the components its
/// derivations read into `inputs`: a certified producer's positive inputs, a
/// rich producer's reads. Returns whether every predicate it names has a
/// component.
fn resolve(
    registration: &Registration<'_>,
    producer: &Producer,
    rule: &RuleIr,
    nodes: &[Node<'_>],
    view: TemplateComponentsRef<'_>,
    (inputs, heads, memory): (&mut Vec<usize>, &mut Vec<usize>, &mut Memory<'_>),
    (limits, counters, location): (&FormulaLimits, &mut Counters, ProgramSite),
) -> Result<bool, FormulaFailure> {
    let component = |predicate: PredicateRef<'_>,
                     counters: &mut Counters|
     -> Result<Option<usize>, FormulaFailure> {
        Ok(lookup(
            nodes.len(),
            |index| nodes[index].signature,
            predicate,
            limits,
            counters,
            location,
        )?
        .and_then(|index| nodes[index].component))
    };
    let mut complete = true;
    let mut record = |found: Option<usize>,
                      into: &mut Vec<usize>,
                      memory: &mut Memory<'_>|
     -> Result<(), FormulaFailure> {
        match found {
            Some(found) => {
                memory.reserve(into, 1)?;
                into.push(found);
            }
            None => complete = false,
        }
        Ok(())
    };
    match producer {
        Producer::Constraint => return Ok(true),
        Producer::Certified(occurrences) => {
            for &occurrence in &registration.inputs[occurrences.clone()] {
                let atom = match &rule.body[occurrence] {
                    LiteralIr::Atom(_, atom) => *atom,
                    LiteralIr::PatternAtom(pattern) => pattern.atom,
                    _ => unreachable!("a certified input is a positive occurrence"),
                };
                let predicate = atom.get(view, limits, counters, location)?.predicate();
                record(component(predicate, counters)?, inputs, memory)?;
            }
        }
        Producer::Rich(reads) => {
            for &read in &registration.reads[reads.clone()] {
                let read = predicate(read, view, limits, counters, location)?;
                record(component(read, counters)?, inputs, memory)?;
            }
        }
    }
    for head in rule_heads(rule) {
        let predicate = head.get(view, limits, counters, location)?.predicate();
        record(component(predicate, counters)?, heads, memory)?;
    }
    Ok(complete)
}

/// Every atom a rule's head can derive: the normal head, each disjunct, each
/// conditional head and each choice element.
pub(super) fn rule_heads(
    rule: &RuleIr,
) -> impl Iterator<Item = crate::formula_support::components::Pattern> + '_ {
    let normal = match &rule.head {
        HeadIr::Normal(head) => *head,
        _ => None,
    };
    let elements = match &rule.head {
        HeadIr::Choice(choice) => choice.elements.as_slice(),
        _ => &[],
    };
    normal
        .into_iter()
        .chain(
            rule.head
                .disjuncts()
                .filter_map(|literal| literal.atom().copied()),
        )
        .chain(
            elements
                .iter()
                .filter_map(|element| element.head.atom().copied()),
        )
}
