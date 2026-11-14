//! What each producer reads, prepared once per completion on the full schedule.
//!
//! Without a producer plan every rule is registered, and its registration
//! records once what a visit needs:
//! - a certified producer's positive body occurrences, flat and structural,
//!   whose delta variants it takes;
//! - a rich producer's read set: the distinct predicates of every atom
//!   occurrence its frames hold, at any depth, of either sign
//!   ([`occurrences::rule`]).
//!
//! Every producer is visited in the first round. Afterwards a certified
//! producer is visited only in a round after a predicate of one of its
//! positive occurrences gained rows ([`Visits`]): its delta variants join
//! rows of the last publication alone, so any other visit has no variant. A
//! rich producer is visited in every round. A certified producer the
//! component schedule holds back is visited like any other: its visit takes
//! no variant and is counted as deferred, and the schedule traverses it
//! outside the round's visits, whether or not it was visited.
//!
//! After the first round, a rich producer none of whose read predicates gained
//! rows is not traversed. Every relation it reads is then the one its last
//! traversal read, and support membership only grows, so another traversal
//! would follow the same plan over the same rows and derive only heads already
//! supported. Facts have no producer slot: a fact block enters support in the
//! first round, and its rows wake a reading rich producer in the second.
//!
//! A rich producer whose reads all lie in dependency components before its
//! heads' is instead traversed once, when those components reach their fixed
//! point ([`components`]). Any other rich producer that proposes an
//! assignment's values from elements over earlier decidable components is
//! held back until those reach their fixed point, and traversed per change
//! from then on. A producer of either kind is also held back for the atoms
//! it negates in earlier decidable components, until those are complete.

mod certain;
mod components;
#[cfg(test)]
mod tests;

use std::mem::size_of;
use std::ops::Range;

use zetesis_core::TemplateComponentsRef;
use zetesis_core::catalog::PredicateRef;
use zetesis_core::relation::Failure;

use super::super::delta::{self, Variants};
use super::super::occurrences::{self, Occurrence};
use super::super::relations::Memory;
use super::super::{Counters, Support};
use super::invalid;
use super::wake::Wake;
use crate::formula_ir::{HeadIr, LiteralIr, RuleIr};
use crate::formula_support::components::Pattern;
use crate::grounding_observer::Event;
use crate::{FormulaFailure, FormulaLimits, ProgramSite};
use themelios_program::program::DefaultNegation;

/// One registration per producer rule, borrowing the rules it was prepared for.
pub(in crate::formula_support) struct Registration<'source> {
    rules: &'source [RuleIr],
    producers: Vec<Producer>,
    inputs: Vec<usize>,
    reads: Vec<Occurrence>,
    /// The component schedule, when some rich producer can wait for its
    /// reads or for the elements its assignments propose from, or some
    /// certified producer for the atoms it negates.
    schedule: Option<Box<components::Schedule<'source>>>,
    /// Which producers each round visits, when the dependency graph names
    /// the predicates they read ([`Self::wake`]).
    wake: Option<Visits<'source>>,
    bytes: usize,
}

/// The producers a round visits. After the first round a certified producer
/// is visited only when a predicate of one of its positive occurrences
/// gained rows at the last publication: its delta variants join new rows
/// alone, so without one it has no variant and a visit derives nothing.
struct Visits<'source> {
    /// The certified producers' positive occurrences, by predicate.
    wake: Wake<'source>,
    /// Visited in every round: each rich producer, whose registration
    /// decides at the visit whether it is traversed, and each certified
    /// producer with an occurrence whose predicate the graph does not name.
    always: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Producer {
    /// A normal head over positive, negated and scalar body literals: its
    /// positive occurrences in `inputs`.
    Certified(Range<usize>),
    /// Every other rule with a head: its read set in `reads`.
    Rich(Range<usize>),
    /// A constraint proposes no head and is never scheduled.
    Constraint,
}

impl<'source> Registration<'source> {
    /// Register every rule of `rules`, resolving read predicates through
    /// `view` to deduplicate them. Charges one unit per rule and per body
    /// literal classified; a rich rule's walk charges one unit per literal it
    /// visits, at every depth, and deduplication one unit per occurrence
    /// resolved and per comparison, with the compared names' lengths. The
    /// retained vectors are admitted against the support byte ceiling beside
    /// the `external` bytes other owners hold, and scratch is released before
    /// returning.
    pub(in crate::formula_support) fn prepare(
        rules: &'source [RuleIr],
        view: Option<TemplateComponentsRef<'_>>,
        external: u128,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Self, FormulaFailure> {
        let mut memory = Memory::new(size_of::<Self>(), external, limits, counters, location);
        memory.add(0)?;
        let mut producers = Vec::new();
        let mut inputs = Vec::new();
        let mut reads = Vec::new();
        memory.reserve(&mut producers, rules.len())?;
        let mut found = Vec::new();
        let mut distinct: Vec<(PredicateRef<'_>, Occurrence)> = Vec::new();
        for rule in rules {
            counters.work(limits, rule.location)?;
            let producer =
                match classify(rule, limits, counters)? {
                    Kind::Constraint => Producer::Constraint,
                    Kind::Certified(count) => {
                        let start = inputs.len();
                        memory.reserve(&mut inputs, count)?;
                        inputs.extend(rule.body.iter().enumerate().filter_map(
                            |(occurrence, literal)| positive(literal).map(|_| occurrence),
                        ));
                        Producer::Certified(start..inputs.len())
                    }
                    Kind::Rich => {
                        found.clear();
                        let mut walk = Walk {
                            found: &mut found,
                            memory: &mut memory,
                            limits,
                            counters,
                            location: rule.location,
                        };
                        occurrences::rule(rule, &mut walk)?;
                        let start = reads.len();
                        if let Some(view) = view {
                            distinct.clear();
                            memory.reserve(&mut distinct, found.len())?;
                            for &occurrence in &found {
                                insert(
                                    &mut distinct,
                                    occurrence,
                                    view,
                                    limits,
                                    counters,
                                    rule.location,
                                )?;
                            }
                            memory.reserve(&mut reads, distinct.len())?;
                            reads.extend(distinct.iter().map(|&(_, occurrence)| occurrence));
                        } else {
                            // No component view: keep every coordinate, a superset.
                            memory.reserve(&mut reads, found.len())?;
                            reads.extend_from_slice(&found);
                        }
                        Producer::Rich(start..reads.len())
                    }
                };
            producers.push(producer);
        }
        let scratch_bytes = found.capacity() * size_of::<Occurrence>()
            + distinct.capacity() * size_of::<(PredicateRef<'_>, Occurrence)>();
        drop(distinct);
        drop(found);
        memory.release(scratch_bytes);
        Ok(Self {
            rules,
            producers,
            inputs,
            reads,
            schedule: None,
            wake: None,
            bytes: memory.bytes,
        })
    }

    /// Schedule by dependency component every producer that can wait for
    /// components it reads. Its vectors are admitted beside the registration's
    /// own bytes and `external`, and counted with them.
    pub(in crate::formula_support) fn schedule(
        &mut self,
        prepared: &'source crate::formula_ir::Prepared,
        view: TemplateComponentsRef<'_>,
        external: u128,
        (limits, counters, location): (&FormulaLimits, &mut Counters, ProgramSite),
    ) -> Result<(), FormulaFailure> {
        let mut memory = Memory::new(0, external + self.bytes as u128, limits, counters, location);
        memory.add(0)?;
        let schedule = components::Schedule::prepare(
            self,
            prepared,
            view,
            &mut memory,
            (limits, counters, location),
        )?;
        // Without a waiting producer every vector was scratch and is gone.
        if let Some(schedule) = schedule {
            memory.add(size_of::<components::Schedule<'_>>())?;
            self.schedule = Some(Box::new(schedule));
            self.bytes += memory.bytes;
        }
        Ok(())
    }

    /// Prepare the producers each round visits ([`Visits`]). Every producer
    /// is visited in the first round, in rule order, so that a fact block
    /// still enters support at its item position. The test-only reference
    /// that traverses every rich producer in every round keeps visiting
    /// every rule. The retained vectors are admitted beside the
    /// registration's own bytes and `external`, and counted with them; the
    /// graph's nodes are scratch. One unit per producer and per positive
    /// occurrence, with each occurrence's resolution.
    pub(in crate::formula_support) fn wake(
        &mut self,
        prepared: &'source crate::formula_ir::Prepared,
        view: TemplateComponentsRef<'_>,
        external: u128,
        (limits, counters, location): (&FormulaLimits, &mut Counters, ProgramSite),
    ) -> Result<(), FormulaFailure> {
        if every_round_traversed() {
            return Ok(());
        }
        let mut memory = Memory::new(
            size_of::<Visits<'_>>(),
            external + self.bytes as u128,
            limits,
            counters,
            location,
        );
        memory.add(0)?;
        let nodes = super::graph_nodes(prepared, &mut memory, limits, counters, location)?;
        let resolver = certain::Resolver {
            nodes: &nodes,
            view,
            limits,
        };
        let mut rules = Vec::new();
        let mut predicates = Vec::new();
        let mut always = Vec::new();
        // The two scratch vectors' headers; the retained ones are fields.
        memory.add(size_of::<Vec<Option<Range<usize>>>>() + size_of::<Vec<usize>>())?;
        memory.reserve(&mut rules, self.producers.len())?;
        for (index, producer) in self.producers.iter().enumerate() {
            counters.work(limits, location)?;
            let rule = &self.rules[index];
            let start = predicates.len();
            match producer {
                Producer::Constraint => {
                    rules.push(None);
                    continue;
                }
                Producer::Rich(_) => {
                    memory.reserve(&mut always, 1)?;
                    always.push(index);
                }
                Producer::Certified(inputs) => {
                    let mut named = true;
                    for &occurrence in &self.inputs[inputs.clone()] {
                        counters.work(limits, rule.location)?;
                        let pattern = positive(&rule.body[occurrence])
                            .expect("a registered input is a positive occurrence");
                        if let Some(node) = resolver.node(pattern, counters, rule.location)? {
                            memory.reserve(&mut predicates, 1)?;
                            predicates.push(node);
                        } else {
                            named = false;
                        }
                    }
                    if !named {
                        predicates.truncate(start);
                        memory.reserve(&mut always, 1)?;
                        always.push(index);
                    }
                }
            }
            rules.push(Some(start..predicates.len()));
        }
        let mut wake = Wake::prepare(
            &nodes,
            (&rules, &predicates),
            false,
            &mut memory,
            limits,
            counters,
            location,
        )?;
        for (index, inputs) in rules.iter().enumerate() {
            if inputs.is_some() {
                wake.mark(index, limits, counters, location)?;
            }
        }
        let scratch_bytes = size_of::<Vec<super::Node<'_>>>()
            + nodes.capacity() * size_of::<super::Node<'_>>()
            + size_of::<Vec<Option<Range<usize>>>>()
            + rules.capacity() * size_of::<Option<Range<usize>>>()
            + size_of::<Vec<usize>>()
            + predicates.capacity() * size_of::<usize>();
        drop(rules);
        drop(predicates);
        drop(nodes);
        memory.release(scratch_bytes);
        self.wake = Some(Visits { wake, always });
        self.bytes += memory.bytes;
        Ok(())
    }

    /// This round's producers, in rule order: the ones [`Self::advance`]
    /// selected, or every rule without a wake set.
    pub(in crate::formula_support) fn round(&self) -> super::Schedule<'_> {
        match &self.wake {
            Some(visits) => visits.wake.schedule(),
            None => super::Schedule::all(self.rules),
        }
    }

    /// Select the next round's producers from this complete round's
    /// proposed new atoms: the certified producers one of whose positive
    /// occurrences' predicates gained an atom, and every producer visited in
    /// every round. One unit per producer selected, beside the wake set's.
    pub(in crate::formula_support) fn advance<'atoms>(
        &mut self,
        delta: impl Iterator<Item = zetesis_core::catalog::AtomRef<'atoms>>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        let Some(visits) = &mut self.wake else {
            return Ok(());
        };
        visits.wake.advance(delta, limits, counters, location)?;
        for &producer in &visits.always {
            visits.wake.mark(producer, limits, counters, location)?;
        }
        Ok(())
    }

    /// See [`components::Schedule::mark_ready`].
    pub(in crate::formula_support) fn mark_ready(
        &self,
        delta: &super::super::relations::SupportAppend<'_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<bool, FormulaFailure> {
        match &self.schedule {
            Some(schedule) => schedule.mark_ready(delta, limits, counters, location),
            None => Ok(false),
        }
    }

    /// See [`components::Schedule::take_ready`].
    pub(in crate::formula_support) fn take_ready(&self) -> Option<usize> {
        self.schedule.as_ref()?.take_ready()
    }

    /// See [`components::Schedule::pending`].
    pub(in crate::formula_support) fn pending(&self) -> bool {
        self.schedule
            .as_ref()
            .is_some_and(|schedule| schedule.pending())
    }

    pub(in crate::formula_support) fn bytes(&self) -> usize {
        self.bytes
    }

    /// The variants of the producer at `index`, which must be `rule`. A
    /// certified producer takes its registered occurrences' delta variants. A
    /// rich producer is traversed in the first round, and later exactly when
    /// one of its read predicates gained rows in `support`; a visit that ends
    /// at that test is recorded as unchanged. A deferred producer is never
    /// traversed here, and a gated one only after the schedule traversed it,
    /// and then by the same test. A held-back certified producer takes no
    /// variant before the schedule traversed it and its delta variants
    /// after: that traversal read every row of its round's snapshot, so the
    /// rows published since are the new ones. The owner check is a pointer
    /// comparison, covered by the visit's own charge.
    pub(in crate::formula_support) fn variants<'a, 'rows>(
        &'a self,
        index: usize,
        rule: &'a RuleIr,
        support: &'a Support<'rows>,
        first: bool,
        limits: &FormulaLimits,
        counters: &mut Counters,
    ) -> Result<Variants<'a, 'rows>, FormulaFailure> {
        let (Some(producer), Some(registered)) = (self.producers.get(index), self.rules.get(index))
        else {
            return Err(invalid(Failure::Owner, rule.location));
        };
        if !std::ptr::eq(rule, registered) {
            return Err(invalid(Failure::Owner, rule.location));
        }
        match producer {
            Producer::Certified(inputs) => {
                let held = self
                    .schedule
                    .as_ref()
                    .and_then(|schedule| schedule.state(index));
                match held {
                    Some(components::State::Waiting | components::State::Ready) => {
                        counters.record(Event::SupportDeferredVisit);
                        Ok(delta::traversal(rule, support, false))
                    }
                    // The schedule's traversal stood for the first round's.
                    Some(components::State::Traversed) => Ok(delta::prepared_variants(
                        rule,
                        support,
                        &self.inputs[inputs.clone()],
                        false,
                    )),
                    // Never held. `Schedule::state` reports a released
                    // producer as never held, so its state only completes
                    // the match.
                    None | Some(components::State::Released) => Ok(delta::prepared_variants(
                        rule,
                        support,
                        &self.inputs[inputs.clone()],
                        first,
                    )),
                }
            }
            Producer::Rich(_)
                if let Some(state) = self.schedule.as_ref().and_then(|schedule| {
                    schedule
                        .state(index)
                        .filter(|&state| !schedule.resumed(index, state))
                }) =>
            {
                // A deferred producer is traversed only by the component
                // schedule, after its round's other producers, and so is a
                // gated one until that traversal.
                counters.record(if state == components::State::Traversed {
                    Event::SupportUnchangedVisit
                } else {
                    Event::SupportDeferredVisit
                });
                Ok(delta::traversal(rule, support, false))
            }
            Producer::Rich(reads) => {
                // A gated producer reaches this arm only after the schedule
                // traversed it in full, which follows its round's last visit
                // here: never in the first round.
                let traversed = first
                    || every_round_traversed()
                    || affected(&self.reads[reads.clone()], rule, support, limits, counters)?;
                counters.record(if traversed {
                    Event::SupportRichTraversal
                } else {
                    Event::SupportUnchangedVisit
                });
                Ok(delta::traversal(rule, support, traversed))
            }
            Producer::Constraint => Err(invalid(Failure::Owner, rule.location)),
        }
    }
}

enum Kind {
    Certified(usize),
    Rich,
    Constraint,
}

/// A rule's kind, from its head and the body literals it holds at top level,
/// one unit per literal. Under the test-only structural reference, a
/// structural occurrence makes the rule rich.
fn classify(
    rule: &RuleIr,
    limits: &FormulaLimits,
    counters: &mut Counters,
) -> Result<Kind, FormulaFailure> {
    match &rule.head {
        HeadIr::Normal(None) => return Ok(Kind::Constraint),
        HeadIr::Normal(Some(_)) | HeadIr::Disjunction(_) => {}
        // A choice element's condition is a join of its own: its rows can grow
        // after a body binding was derived.
        HeadIr::Choice(choice)
            if choice
                .elements
                .iter()
                .all(|element| element.condition.is_empty()) => {}
        HeadIr::Choice(_) | HeadIr::ConditionalDisjunction { .. } => return Ok(Kind::Rich),
    }
    let mut inputs = 0;
    for literal in &rule.body {
        counters.work(limits, rule.location)?;
        match literal {
            LiteralIr::Atom(DefaultNegation::None, _) | LiteralIr::PatternAtom(_) => inputs += 1,
            // Join::new collects only positive inputs. Flat negative atoms
            // generate no bindings and are no join filters: a decided one is
            // tested against each complete row after the join. Any argument
            // evaluation is a separate, retained scalar operation in the IR.
            LiteralIr::Atom(DefaultNegation::Not | DefaultNegation::NotNot, _)
            | LiteralIr::Compare(..)
            | LiteralIr::ArgumentCheck { .. }
            | LiteralIr::TupleCompare(..)
            | LiteralIr::Guard(_)
            | LiteralIr::HeadGuard(_)
            | LiteralIr::Bind { .. }
            | LiteralIr::Range { .. } => {}
            LiteralIr::ProjectedAtom(..) | LiteralIr::Conditional(_) | LiteralIr::Aggregate(_) => {
                return Ok(Kind::Rich);
            }
        }
    }
    #[cfg(test)]
    if !delta::structural_enabled()
        && rule
            .body
            .iter()
            .any(|literal| matches!(literal, LiteralIr::PatternAtom(_)))
    {
        return Ok(Kind::Rich);
    }
    Ok(Kind::Certified(inputs))
}

/// The pattern of a positive flat or structural body occurrence: an input of
/// a certified rule, and what a definite rule reads.
fn positive(literal: &LiteralIr) -> Option<Pattern> {
    match literal {
        LiteralIr::Atom(DefaultNegation::None, atom) => Some(*atom),
        LiteralIr::PatternAtom(pattern) => Some(pattern.atom),
        _ => None,
    }
}

/// Collects a rich rule's occurrences, charging one unit per literal visited.
struct Walk<'a, 'limits> {
    found: &'a mut Vec<Occurrence>,
    memory: &'a mut Memory<'limits>,
    limits: &'a FormulaLimits,
    counters: &'a mut Counters,
    location: ProgramSite,
}

impl occurrences::Visitor for Walk<'_, '_> {
    fn work(&mut self) -> Result<(), FormulaFailure> {
        self.counters.work(self.limits, self.location)
    }

    fn occurrence(&mut self, occurrence: Occurrence) -> Result<(), FormulaFailure> {
        self.memory.reserve(self.found, 1)?;
        self.found.push(occurrence);
        Ok(())
    }
}

/// The signed predicate an occurrence names, through `view`: one unit.
fn predicate<'view>(
    occurrence: Occurrence,
    view: TemplateComponentsRef<'view>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
) -> Result<PredicateRef<'view>, FormulaFailure> {
    Ok(match occurrence {
        Occurrence::Atom(pattern) => pattern.get(view, limits, counters, location)?.predicate(),
        Occurrence::Projected(predicate) => predicate.get(view, limits, counters, location)?,
    })
}

/// Keep `occurrence` unless an entry already names its predicate. Entries stay
/// sorted by predicate, which compares sign, name and arity; each comparison
/// charges one unit and the two names' lengths.
fn insert<'view>(
    distinct: &mut Vec<(PredicateRef<'view>, Occurrence)>,
    occurrence: Occurrence,
    view: TemplateComponentsRef<'view>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
) -> Result<(), FormulaFailure> {
    let named = predicate(occurrence, view, limits, counters, location)?;
    let mut start = 0;
    let mut end = distinct.len();
    while start < end {
        let middle = start + (end - start) / 2;
        let entry = distinct[middle].0;
        counters.charge_work(
            1 + entry.name().len() as u128 + named.name().len() as u128,
            limits,
            location,
        )?;
        match entry.cmp(&named) {
            std::cmp::Ordering::Less => start = middle + 1,
            std::cmp::Ordering::Greater => end = middle,
            std::cmp::Ordering::Equal => return Ok(()),
        }
    }
    distinct.insert(start, (named, occurrence));
    Ok(())
}

/// Whether a read predicate gained rows at the last publication: `old_rows <
/// row_count` in this round's snapshot, as the certified test reads it. One unit
/// per entry tested, and the resolution's unit; it stops at the first changed
/// predicate.
fn affected(
    reads: &[Occurrence],
    rule: &RuleIr,
    support: &Support<'_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
) -> Result<bool, FormulaFailure> {
    if reads.is_empty() {
        return Ok(false);
    }
    let view = support
        .components()
        .ok_or_else(|| super::super::components::missing(rule.location))?;
    for &read in reads {
        counters.work(limits, rule.location)?;
        let predicate = predicate(read, view, limits, counters, rule.location)?;
        if support.old_rows(predicate) < support.row_count(predicate) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Whether the test-only reference traverses every rich producer in every
/// round, as before the registration's test. Always false outside tests.
fn every_round_traversed() -> bool {
    #[cfg(test)]
    {
        testing::every_round_traversed()
    }
    #[cfg(not(test))]
    {
        false
    }
}

/// The reference the skip is compared against: a scoped, thread-local switch
/// that traverses every rich producer in every round. It is restored after an
/// unwind and confined to its thread, as the structural switch is.
#[cfg(test)]
pub(crate) mod testing {
    use std::cell::Cell;

    thread_local! {
        static EVERY_ROUND: Cell<bool> = const { Cell::new(false) };
    }

    pub(super) fn every_round_traversed() -> bool {
        EVERY_ROUND.get()
    }

    /// Run `action` with every rich producer traversed in every round.
    pub(crate) fn with_every_round_traversed<T>(action: impl FnOnce() -> T) -> T {
        struct Restore(bool);
        impl Drop for Restore {
            fn drop(&mut self) {
                EVERY_ROUND.set(self.0);
            }
        }
        let _restore = Restore(EVERY_ROUND.replace(true));
        action()
    }
}
