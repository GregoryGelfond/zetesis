//! Immutable reconstruction patterns, authenticated once against the closed owner.
//!
//! The partition's rule and literal order is preserved. Each retained pattern
//! is the same borrowed component the join formerly resolved for each answer;
//! neither a pattern nor a frame's shape contains selected truth or a binding.

use std::ops::Range;

use themelios_program::program::DefaultNegation;
use zetesis_core::{Model, PatternRef, TemplateComponentsRef, atom_interner::AtomInterner};

use super::{ReconstructionError, Work, join};
use crate::ProgramSite;
use crate::formula_ir::{HeadIr, LiteralIr, RuleIr};
use crate::formula_support::components;

pub(super) struct Rule<'a> {
    pub(super) head: PatternRef<'a>,
    pub(super) variables: usize,
    pub(super) location: ProgramSite,
    body: Range<usize>,
}

/// O(R + L) immutable metadata for R rules and L body occurrences. Preparation
/// resolves each head/body once, with the component reader's bounded work.
/// Each answer still initializes its own variable slots and selected-row cursors.
pub(super) struct Plan<'a> {
    rules: Vec<Rule<'a>>,
    bodies: Vec<PatternRef<'a>>,
}

impl<'a> Plan<'a> {
    pub(super) fn new(
        rules: &[RuleIr],
        components: TemplateComponentsRef<'a>,
        writer: &AtomInterner,
        work: &mut Work<'_>,
    ) -> Result<Self, ReconstructionError> {
        let mut plan = Self {
            rules: Vec::new(),
            bodies: Vec::new(),
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
        for rule in rules {
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

    /// Owned vector capacity only. The inline header belongs to the session;
    /// pattern payloads are borrowed from its already counted closed source.
    pub(super) fn retained_bytes(&self) -> u128 {
        self.rules.capacity() as u128 * size_of::<Rule<'_>>() as u128
            + self.bodies.capacity() as u128 * size_of::<PatternRef<'_>>() as u128
    }

    pub(super) fn derive(
        &self,
        model: &Model,
        writer: &mut AtomInterner,
        work: &mut Work<'_>,
    ) -> Result<(), ReconstructionError> {
        for rule in &self.rules {
            work.location = rule.location;
            work.permit()?;
            join::derive(rule, &self.bodies[rule.body.clone()], model, writer, work)?;
        }
        Ok(())
    }
}
