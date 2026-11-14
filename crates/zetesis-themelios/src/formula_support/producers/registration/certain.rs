//! The certain predicates of the registered rules.
//!
//! [`crate::formula_support::certain`] states what a certain predicate is and
//! what its supported atoms establish. This is the analysis: one pass
//! classifies every rule and resolves what it derives and reads, a second
//! removes predicates until none reads a removed one, and a third lists the
//! assignment aggregate elements whose conditions hold certain atoms and
//! decided literals only, in the rules that read their relations complete.
//!
//! A deferred producer may itself be definite through an exact assignment
//! aggregate. The schedule traverses it once, after every predicate it reads
//! reached its fixed point, so when those predicates are certain each of its
//! elements is listed, every tuple is required, and the one value it proposes
//! is the aggregate's value in every interpretation. Its heads are then
//! supported exactly when they hold, as a definite rule's are.
//!
//! *Certain* is a property of a predicate. It is unrelated to
//! `Producer::Certified`, the registration's name for a rule whose body
//! support completion can extend row by row.

#[cfg(test)]
mod tests;

use std::ops::Range;

use themelios_program::program::DefaultNegation;
use zetesis_core::TemplateComponentsRef;

use super::super::super::certain::{Certainty, Signature};
use super::super::super::relations::Memory;
use super::super::{Node, graph_nodes, lookup};
use super::{Producer, Registration, components, positive};
use crate::formula_ir::{
    AggregateElementIr, AggregateIr, AggregateKey, HeadIr, LiteralIr, Prepared, RuleIr, decided,
};
use crate::formula_support::Counters;
use crate::formula_support::components::Pattern;
use crate::{FormulaFailure, FormulaLimits, ProgramSite};

/// Whether `aggregate` is a plain assignment: one whose elements each have a
/// tuple key and a condition of positive atoms and decided literals. Its
/// value is then the measure of the tuples its elements yield over complete
/// relations, the same in every interpretation that holds their atoms. One
/// unit per element and per condition literal.
fn plain(
    aggregate: &AggregateIr,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
) -> Result<bool, FormulaFailure> {
    if aggregate.binding.is_none() {
        return Ok(false);
    }
    // A binding is granted only to an unnegated aggregate whose one guard is
    // the equality with its target (`Compiler::assignment_targets`), so no
    // other guard reads the value.
    debug_assert!(aggregate.negation == DefaultNegation::None && aggregate.guards.len() == 1);
    for element in &aggregate.elements {
        counters.work(limits, location)?;
        if !matches!(element.key, AggregateKey::Tuple(_)) {
            return Ok(false);
        }
        for literal in &element.condition {
            counters.work(limits, location)?;
            if positive(literal).is_none() && !decided(literal) {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

/// Whether `rule` has one ordinary head atom over positive atoms, literals
/// its join decides and, when the component schedule traverses it once over
/// complete relations (`deferred`), plain assignment aggregates, which are
/// exact once the predicates they read prove certain. One unit per body
/// literal, with an aggregate's own.
fn definite(
    rule: &RuleIr,
    deferred: bool,
    limits: &FormulaLimits,
    counters: &mut Counters,
) -> Result<bool, FormulaFailure> {
    if !matches!(rule.head, HeadIr::Normal(Some(_))) {
        return Ok(false);
    }
    for literal in &rule.body {
        counters.work(limits, rule.location)?;
        let admitted = match literal {
            LiteralIr::Aggregate(aggregate) => {
                deferred && plain(aggregate, limits, counters, rule.location)?
            }
            other => positive(other).is_some() || decided(other),
        };
        if !admitted {
            return Ok(false);
        }
    }
    Ok(true)
}

/// The positive atoms a definite rule reads: its body's, then those of each
/// plain assignment's element conditions.
fn atoms_read(rule: &RuleIr) -> impl Iterator<Item = Pattern> + '_ {
    rule.body.iter().flat_map(|literal| {
        let (own, elements): (Option<Pattern>, &[AggregateElementIr]) = match literal {
            LiteralIr::Aggregate(aggregate) => (None, aggregate.elements.as_slice()),
            other => (positive(other), &[]),
        };
        own.into_iter().chain(
            elements
                .iter()
                .flat_map(|element| element.condition.iter().filter_map(positive)),
        )
    })
}

/// Resolves an atom pattern's predicate to its node in the dependency graph.
struct Resolver<'a, 'source, 'view> {
    nodes: &'a [Node<'source>],
    view: TemplateComponentsRef<'view>,
    limits: &'a FormulaLimits,
}

impl Resolver<'_, '_, '_> {
    /// The node of `pattern`'s predicate, or `None` when the graph does not
    /// name it. The pattern's read and the lookup charge their own units.
    fn node(
        &self,
        pattern: Pattern,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Option<usize>, FormulaFailure> {
        let predicate = pattern
            .get(self.view, self.limits, counters, location)?
            .predicate();
        lookup(
            self.nodes.len(),
            |index| self.nodes[index].signature,
            predicate,
            self.limits,
            counters,
            location,
        )
    }
}

/// Per predicate node whether it is still certain, and per rule the head a
/// definite rule derives with the range of the nodes it reads.
struct Classified {
    certain: Vec<bool>,
    heads: Vec<Option<usize>>,
    ranges: Vec<Range<usize>>,
    reads: Vec<usize>,
}

impl<'source> Registration<'source> {
    /// The certain predicates of the registered rules, and the assignment
    /// aggregate elements they make required. Scratch is admitted beside the
    /// registration's own bytes and `external` and released before returning;
    /// the returned value's bytes are its owner's to admit.
    pub(in crate::formula_support) fn certainty(
        &self,
        prepared: &'source Prepared,
        view: TemplateComponentsRef<'_>,
        external: u128,
        (limits, counters, location): (&FormulaLimits, &mut Counters, ProgramSite),
    ) -> Result<Certainty, FormulaFailure> {
        let mut memory = Memory::new(0, external + self.bytes as u128, limits, counters, location);
        memory.add(0)?;
        let nodes = graph_nodes(prepared, &mut memory, limits, counters, location)?;
        let resolver = Resolver {
            nodes: &nodes,
            view,
            limits,
        };
        let mut classified = self.classify(&resolver, &mut memory, counters)?;
        remove_readers(&mut classified, limits, counters, location)?;
        let elements =
            self.required_elements(&resolver, &classified.certain, &mut memory, counters)?;
        let mut predicates = Vec::new();
        for (index, _) in classified
            .certain
            .iter()
            .enumerate()
            .filter(|&(_, &certain)| certain)
        {
            counters.work(limits, location)?;
            let signature = nodes[index].signature;
            memory.reserve(&mut predicates, 1)?;
            memory.add(signature.name.as_str().len())?;
            predicates.push(Signature {
                sign: match signature.sign {
                    themelios_program::symbol::Sign::Positive => zetesis_core::Sign::Positive,
                    themelios_program::symbol::Sign::Negative => zetesis_core::Sign::Negative,
                },
                name: signature.name.as_str().into(),
                arity: usize::try_from(signature.arity).map_err(|_| {
                    super::invalid(zetesis_core::relation::Failure::Overflow, location)
                })?,
            });
        }
        // The sort and deduplication of the two lists.
        counters.charge_work(
            1 + predicates.len() as u128 + elements.len() as u128,
            limits,
            location,
        )?;
        Ok(Certainty::new(predicates, elements))
    }

    /// Classify every rule and resolve what it derives and reads. A rule that
    /// is not definite removes each predicate it can derive; a definite rule
    /// that reads a predicate the graph does not name removes its head.
    fn classify(
        &self,
        resolver: &Resolver<'_, '_, '_>,
        memory: &mut Memory<'_>,
        counters: &mut Counters,
    ) -> Result<Classified, FormulaFailure> {
        let limits = resolver.limits;
        let mut certain = Vec::new();
        memory.reserve(&mut certain, resolver.nodes.len())?;
        certain.resize(resolver.nodes.len(), true);
        let mut heads: Vec<Option<usize>> = Vec::new();
        let mut ranges: Vec<Range<usize>> = Vec::new();
        let mut reads: Vec<usize> = Vec::new();
        memory.reserve(&mut heads, self.rules.len())?;
        memory.reserve(&mut ranges, self.rules.len())?;
        for (index, rule) in self.rules.iter().enumerate() {
            counters.work(limits, rule.location)?;
            let start = reads.len();
            let definite = definite(rule, self.deferred(index), limits, counters)?;
            let mut head = None;
            for pattern in components::rule_heads(rule) {
                let found = resolver.node(pattern, counters, rule.location)?;
                if definite {
                    head = found;
                } else if let Some(found) = found {
                    certain[found] = false;
                }
            }
            if let Some(head) = head {
                for pattern in atoms_read(rule) {
                    counters.work(limits, rule.location)?;
                    match resolver.node(pattern, counters, rule.location)? {
                        Some(read) => {
                            memory.reserve(&mut reads, 1)?;
                            reads.push(read);
                        }
                        // A predicate the dependency graph does not name.
                        None => certain[head] = false,
                    }
                }
            }
            heads.push(head);
            ranges.push(start..reads.len());
        }
        Ok(Classified {
            certain,
            heads,
            ranges,
            reads,
        })
    }

    /// Whether the component schedule traverses the producer at `index` once,
    /// after every predicate it reads reached its fixed point.
    fn deferred(&self, index: usize) -> bool {
        self.schedule
            .as_ref()
            .is_some_and(|schedule| schedule.state(index).is_some())
    }

    /// Whether the rule at `index` reads its relations complete: a deferred
    /// producer, or a constraint, which proposes no head and is grounded only
    /// after support completion.
    fn reads_complete_relations(&self, index: usize) -> bool {
        self.deferred(index) || self.producers[index] == Producer::Constraint
    }

    /// The assignment aggregate elements whose conditions hold certain atoms
    /// and decided literals only, in the rules that read their relations
    /// complete, as `(aggregate, element)` coordinates. A producer traversed
    /// in every round proposes from relations that may still grow, and lists
    /// nothing.
    fn required_elements(
        &self,
        resolver: &Resolver<'_, '_, '_>,
        certain: &[bool],
        memory: &mut Memory<'_>,
        counters: &mut Counters,
    ) -> Result<Vec<(usize, usize)>, FormulaFailure> {
        let limits = resolver.limits;
        let mut elements = Vec::new();
        for (index, rule) in self.rules.iter().enumerate() {
            counters.work(limits, rule.location)?;
            if !self.reads_complete_relations(index) {
                continue;
            }
            for literal in &rule.body {
                counters.work(limits, rule.location)?;
                let LiteralIr::Aggregate(aggregate) = literal else {
                    continue;
                };
                if aggregate.binding.is_none() {
                    continue;
                }
                for (position, element) in aggregate.elements.iter().enumerate() {
                    counters.work(limits, rule.location)?;
                    if element.required || !matches!(element.key, AggregateKey::Tuple(_)) {
                        continue;
                    }
                    let mut holds = true;
                    for literal in &element.condition {
                        counters.work(limits, rule.location)?;
                        holds = match positive(literal) {
                            Some(pattern) => resolver
                                .node(pattern, counters, rule.location)?
                                .is_some_and(|read| certain[read]),
                            None => decided(literal),
                        };
                        if !holds {
                            break;
                        }
                    }
                    if holds {
                        memory.reserve(&mut elements, 1)?;
                        elements.push((aggregate.id, position));
                    }
                }
            }
        }
        Ok(elements)
    }
}

/// Remove predicates until no remaining one reads a removed one. Each round
/// removes at least one predicate or ends the removal, and charges one unit
/// per rule and per node it reads: at most the predicates and one more times
/// the rules and their reads, when each round removes one predicate of a
/// chain written head first.
fn remove_readers(
    classified: &mut Classified,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
) -> Result<(), FormulaFailure> {
    loop {
        let mut changed = false;
        for (head, range) in classified.heads.iter().zip(&classified.ranges) {
            counters.charge_work(1 + range.len() as u128, limits, location)?;
            let Some(head) = *head else {
                continue;
            };
            if classified.certain[head]
                && classified.reads[range.clone()]
                    .iter()
                    .any(|&read| !classified.certain[read])
            {
                classified.certain[head] = false;
                changed = true;
            }
        }
        if !changed {
            return Ok(());
        }
    }
}
