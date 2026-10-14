//! Immutable reconstruction patterns, authenticated once against the closed owner.
//!
//! The partition's rule and literal order is preserved. Each retained pattern
//! is the same borrowed component the join formerly resolved for each answer;
//! neither a pattern nor a frame's shape contains selected truth or a binding.

use std::ops::Range;

use themelios_program::program::DefaultNegation;
use zetesis_core::catalog::PredicateRef;
use zetesis_core::{
    Model, PatternRef, TemplateComponentsRef, TemplateTerm, atom_interner::AtomInterner,
};

use super::{ReconstructionError, Work, join};
use crate::ProgramSite;
use crate::formula_facts::{Item, Items};
use crate::formula_ir::{HeadIr, LiteralIr};
use crate::formula_support::components;

pub(super) struct Rule<'a> {
    pub(super) head: PatternRef<'a>,
    pub(super) variables: usize,
    pub(super) location: ProgramSite,
    body: Range<usize>,
}

/// A deferred fact row: a body-free ground rule, derived without a join.
struct Fact<'a> {
    predicate: PredicateRef<'a>,
    location: ProgramSite,
    terms: Range<usize>,
}

/// One deferred item's plan entry, in the partition's item order.
enum Step {
    Rule(usize),
    Fact(usize),
}

/// O(R + L + F·A) immutable metadata for R rules, L body occurrences and F
/// fact rows of arity A. Preparation resolves each head/body and each fact
/// cell once, with the component reader's bounded work. Each answer still
/// initializes its own variable slots and selected-row cursors.
pub(super) struct Plan<'a> {
    rules: Vec<Rule<'a>>,
    bodies: Vec<PatternRef<'a>>,
    facts: Vec<Fact<'a>>,
    terms: Vec<TemplateTerm<'a>>,
    steps: Vec<Step>,
}

#[cfg(test)]
impl Plan<'_> {
    /// The plan's steps and the room reserved for them.
    pub(super) fn step_room(&self) -> (usize, usize) {
        (self.steps.len(), self.steps.capacity())
    }
}

impl<'a> Plan<'a> {
    pub(super) fn new(
        items: &Items,
        components: TemplateComponentsRef<'a>,
        writer: &AtomInterner,
        work: &mut Work<'_>,
    ) -> Result<Self, ReconstructionError> {
        let rules = items.rules();
        let mut plan = Self {
            rules: Vec::new(),
            bodies: Vec::new(),
            facts: Vec::new(),
            terms: Vec::new(),
            steps: Vec::new(),
        };
        let mut lease = work.counters.lease();
        lease.observe(size_of::<Self>(), work.location)?;
        work.observe(writer.storage_bytes())?;
        let mut body_count = 0_usize;
        for rule in rules {
            work.permit()?;
            body_count = body_count
                .checked_add(rule.body.len())
                .ok_or_else(|| super::super::storage::overflow(work.location))?;
        }
        work.reserve(
            &mut plan.rules,
            rules.len(),
            &mut lease,
            size_of::<Self>(),
            writer,
        )?;
        let fixed = size_of::<Self>()
            .checked_add(plan.rules.capacity() * size_of::<Rule<'_>>())
            .ok_or_else(|| super::super::storage::overflow(work.location))?;
        work.reserve(&mut plan.bodies, body_count, &mut lease, fixed, writer)?;
        let (rows, cells) = items
            .blocks()
            .fold((0_usize, 0_usize), |(rows, cells), block| {
                (
                    rows.saturating_add(block.len()),
                    cells.saturating_add(block.len().saturating_mul(block.arity())),
                )
            });
        let fixed = fixed
            .checked_add(plan.bodies.capacity() * size_of::<PatternRef<'_>>())
            .ok_or_else(|| super::super::storage::overflow(work.location))?;
        work.reserve(&mut plan.facts, rows, &mut lease, fixed, writer)?;
        let fixed = fixed
            .checked_add(plan.facts.capacity() * size_of::<Fact<'_>>())
            .ok_or_else(|| super::super::storage::overflow(work.location))?;
        work.reserve(&mut plan.terms, cells, &mut lease, fixed, writer)?;
        let fixed = fixed
            .checked_add(plan.terms.capacity() * size_of::<TemplateTerm<'_>>())
            .ok_or_else(|| super::super::storage::overflow(work.location))?;
        // One step per producer occurrence: each rule, and each row of each
        // fact block, which `facts` pushes one by one.
        work.reserve(
            &mut plan.steps,
            items.occurrences(),
            &mut lease,
            fixed,
            writer,
        )?;
        for item in items.iter() {
            let rule = match item {
                Item::Rule(_, rule) => rule,
                Item::Facts(block) => {
                    plan.facts(block, components, work)?;
                    continue;
                }
            };
            work.location = rule.location;
            work.permit()?;
            let HeadIr::Normal(Some(head)) = &rule.head else {
                return Err(components::missing(rule.location).into());
            };
            let head = head.get(components, work.limits, work.counters, rule.location)?;
            let start = plan.bodies.len();
            for literal in &rule.body {
                work.permit()?;
                let LiteralIr::Atom(DefaultNegation::None, pattern) = literal else {
                    return Err(components::missing(rule.location).into());
                };
                plan.bodies.push(pattern.get(
                    components,
                    work.limits,
                    work.counters,
                    rule.location,
                )?);
            }
            plan.steps.push(Step::Rule(plan.rules.len()));
            plan.rules.push(Rule {
                head,
                variables: rule.variables,
                location: rule.location,
                body: start..plan.bodies.len(),
            });
        }
        // Only a fully prepared plan escapes. The lease measures construction;
        // its caller transfers the capacities to the session's external owner.
        Ok(plan)
    }

    /// Resolve a deferred block's rows: its predicate once, each row's
    /// cells, and the row's statement site.
    fn facts(
        &mut self,
        block: crate::formula_facts::FactBlock<'_>,
        components: TemplateComponentsRef<'a>,
        work: &mut Work<'_>,
    ) -> Result<(), ReconstructionError> {
        work.location = block.location();
        work.permit()?;
        let predicate =
            block
                .predicate()
                .get(components, work.limits, work.counters, work.location)?;
        for row in block.rows() {
            work.location = row.location();
            work.permit()?;
            let start = self.terms.len();
            for cell in row.cells() {
                work.permit()?;
                self.terms.push(TemplateTerm::Constant(cell.get(
                    components,
                    work.limits,
                    work.counters,
                    work.location,
                )?));
            }
            self.steps.push(Step::Fact(self.facts.len()));
            self.facts.push(Fact {
                predicate,
                location: work.location,
                terms: start..self.terms.len(),
            });
        }
        Ok(())
    }

    /// Owned vector capacity only. The inline header belongs to the session;
    /// pattern payloads are borrowed from its already counted closed source.
    pub(super) fn retained_bytes(&self) -> u128 {
        self.rules.capacity() as u128 * size_of::<Rule<'_>>() as u128
            + self.bodies.capacity() as u128 * size_of::<PatternRef<'_>>() as u128
            + self.facts.capacity() as u128 * size_of::<Fact<'_>>() as u128
            + self.terms.capacity() as u128 * size_of::<TemplateTerm<'_>>() as u128
            + self.steps.capacity() as u128 * size_of::<Step>() as u128
    }

    pub(super) fn derive(
        &self,
        model: &Model,
        writer: &mut AtomInterner,
        work: &mut Work<'_>,
    ) -> Result<(), ReconstructionError> {
        for step in &self.steps {
            match *step {
                Step::Rule(index) => {
                    let rule = &self.rules[index];
                    work.location = rule.location;
                    work.permit()?;
                    join::derive(rule, &self.bodies[rule.body.clone()], model, writer, work)?;
                }
                Step::Fact(index) => {
                    // A body-free ground rule: its head under the empty assignment.
                    let fact = &self.facts[index];
                    work.location = fact.location;
                    work.permit()?;
                    let head =
                        PatternRef::from_parts(fact.predicate, &self.terms[fact.terms.clone()])
                            .map_err(|_| components::missing(fact.location))?;
                    join::emit(head, &[], writer, work)?;
                }
            }
        }
        Ok(())
    }
}
