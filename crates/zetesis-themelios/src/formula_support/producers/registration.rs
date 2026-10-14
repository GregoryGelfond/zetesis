//! What each producer reads, prepared once per completion on the full schedule.
//!
//! Without a producer plan every rule is visited in every round. Its
//! registration records once what a visit needs:
//! - a certified producer's positive body occurrences, flat and structural,
//!   whose delta variants it takes;
//! - a rich producer's read set: the distinct predicates of every atom
//!   occurrence its frames hold, at any depth, of either sign
//!   ([`occurrences::rule`]).
//!
//! After the first round, a rich producer none of whose read predicates gained
//! rows is not traversed. Every relation it reads is then the one its last
//! traversal read, and support membership only grows, so another traversal
//! would follow the same plan over the same rows and derive only heads already
//! supported. Facts have no producer slot: a fact product enters support in the
//! first round, and its rows wake a reading rich producer in the second.

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
use crate::formula_ir::{HeadIr, LiteralIr, RuleIr};
use crate::grounding_observer::Event;
use crate::{FormulaFailure, FormulaLimits, ProgramSite};
use themelios_program::program::DefaultNegation;

/// One registration per producer rule, borrowing the rules it was prepared for.
pub(in crate::formula_support) struct Registration<'source> {
    rules: &'source [RuleIr],
    producers: Vec<Producer>,
    inputs: Vec<usize>,
    reads: Vec<Occurrence>,
    bytes: usize,
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
            let producer = match classify(rule, limits, counters)? {
                Kind::Constraint => Producer::Constraint,
                Kind::Certified(count) => {
                    let start = inputs.len();
                    memory.reserve(&mut inputs, count)?;
                    inputs.extend(rule.body.iter().enumerate().filter_map(
                        |(occurrence, literal)| positive(literal).then_some(occurrence),
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
            bytes: memory.bytes,
        })
    }

    pub(in crate::formula_support) fn rules(&self) -> &'source [RuleIr] {
        self.rules
    }

    pub(in crate::formula_support) fn bytes(&self) -> usize {
        self.bytes
    }

    /// The variants of the producer at `index`, which must be `rule`. A
    /// certified producer takes its registered occurrences' delta variants. A
    /// rich producer is traversed in the first round, and later exactly when
    /// one of its read predicates gained rows in `support`; a visit that ends
    /// at that test is recorded as unchanged. The owner check is a pointer
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
            Producer::Certified(inputs) => Ok(delta::prepared_variants(
                rule,
                support,
                &self.inputs[inputs.clone()],
                first,
            )),
            Producer::Rich(reads) => {
                let traversed = first
                    || every_round_traversed()
                    || affected(&self.reads[reads.clone()], rule, support, limits, counters)?;
                if !traversed {
                    counters.record(Event::SupportUnchangedVisit);
                }
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
        HeadIr::Normal(Some(_)) => {}
        HeadIr::Disjunction(_) | HeadIr::Choice(_) | HeadIr::ConditionalDisjunction { .. } => {
            return Ok(Kind::Rich);
        }
    }
    let mut inputs = 0;
    for literal in &rule.body {
        counters.work(limits, rule.location)?;
        match literal {
            LiteralIr::Atom(DefaultNegation::None, _) | LiteralIr::PatternAtom(_) => inputs += 1,
            // Join::new collects only positive inputs. Flat negative atoms
            // neither generate bindings nor filter possible heads; any argument
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

/// A positive flat or structural body occurrence: an input of a certified rule.
fn positive(literal: &LiteralIr) -> bool {
    matches!(
        literal,
        LiteralIr::Atom(DefaultNegation::None, _) | LiteralIr::PatternAtom(_)
    )
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
