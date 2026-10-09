//! Necessary occurrence reads refine one changed atom, never a dense change set.
//!
//! This replaces the existing per-predicate flag, not the dependency owner.
//! Multiple changed atoms keep ordinary invalidation. Slots, including repeated
//! names and lowered constructor temporaries, are independent wildcards here:
//! failing a constant or constructor check excludes a read; passing proves none.

use super::{PreparedConstraints, literal_atom};
use crate::FormulaFailure;
use crate::formula_ir::{LiteralIr, RuleIr};
use crate::formula_pattern::{ArgumentPattern, PatternNode};
use crate::formula_support::{Counters, GroundingWork};
use zetesis_core::catalog::{AtomRef, TermRef};
use zetesis_core::{TemplateComponentsRef, TemplateTerm};

#[derive(Clone, Copy)]
pub(super) enum Change {
    None,
    One(usize),
    Several,
}

impl Change {
    pub(super) fn include(&mut self, atom: usize) {
        *self = match self {
            Self::None => Self::One(atom),
            Self::One(_) | Self::Several => Self::Several,
        };
    }
}

/// The ordered dependency row has one group per original atom occurrence.
/// Both it and the canonical changed IDs belong to this exact prepared core.
pub(super) fn changed(
    rule: &RuleIr,
    dependencies: &[usize],
    changes: &[Change],
    prepared: &PreparedConstraints<'_>,
    counters: &mut Counters,
) -> Result<bool, FormulaFailure> {
    let mut groups = dependencies.iter();
    for literal in &rule.body {
        counters.work(&prepared.limits, rule.location)?;
        if literal_atom(literal).is_none() {
            continue;
        }
        let &group = groups.next().expect("one group per original occurrence");
        match changes[group] {
            Change::None => {}
            Change::Several => return Ok(true),
            Change::One(position) => {
                counters.work(&prepared.limits, rule.location)?;
                let atom = prepared
                    .index
                    .expect("prepared source owner")
                    .catalog()
                    .atoms()
                    .at(position)
                    .expect("authenticated changed atom");
                let components = prepared
                    .completed
                    .components()
                    .ok_or_else(|| crate::formula_support::components::missing(rule.location))?;
                if may_read(
                    literal,
                    atom,
                    components,
                    &mut GroundingWork::new(&prepared.limits, counters, rule.location),
                )? {
                    return Ok(true);
                }
            }
        }
    }
    debug_assert!(groups.next().is_none());
    Ok(false)
}

/// Only immutable constants and constructor descriptors constrain the read.
/// Canonical term comparison/navigation supplies equality and charges. There is
/// no binding, term construction, captured equality, allocation or truth read.
fn may_read(
    literal: &LiteralIr,
    atom: AtomRef<'_>,
    components: TemplateComponentsRef<'_>,
    work: &mut GroundingWork<'_>,
) -> Result<bool, FormulaFailure> {
    let (_, pattern) = literal_atom(literal).expect("an original atom occurrence");
    let pattern = pattern.get(components, work.limits, work.counters, work.location)?;
    // The plan already establishes signed predicate equality for this group.
    for (column, term) in pattern.terms().iter().enumerate() {
        work.counters.work(work.limits, work.location)?;
        if let TemplateTerm::Constant(expected) = term {
            let actual = atom.values().get(column).expect("same predicate arity");
            if !actual
                .equals_ref_with(expected, || work.counters.work(work.limits, work.location))?
            {
                return Ok(false);
            }
        }
    }
    if let LiteralIr::PatternAtom(pattern) = literal {
        for argument in &pattern.arguments {
            work.counters.work(work.limits, work.location)?;
            let value = atom
                .values()
                .get(argument.position)
                .expect("admitted argument");
            if !argument_matches(argument, value, components, work)? {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn argument_matches(
    pattern: &ArgumentPattern,
    value: TermRef<'_>,
    components: TemplateComponentsRef<'_>,
    work: &mut GroundingWork<'_>,
) -> Result<bool, FormulaFailure> {
    let mut offset = 0;
    for node in &pattern.nodes {
        let Some(actual) =
            value.subterm_with(offset, || work.counters.work(work.limits, work.location))?
        else {
            return Ok(false);
        };
        work.counters.work(work.limits, work.location)?;
        match node {
            PatternNode::Constructor(constructor) => {
                let descriptor = actual.descriptor();
                work.counters.charge_work(
                    descriptor.text_bytes() as u128,
                    work.limits,
                    work.location,
                )?;
                let expected =
                    constructor.get(components, work.limits, work.counters, work.location)?;
                if descriptor != expected {
                    return Ok(false);
                }
                offset += 1;
            }
            PatternNode::Constant(constant) => {
                let expected =
                    constant.get(components, work.limits, work.counters, work.location)?;
                if !actual
                    .equals_ref_with(expected, || work.counters.work(work.limits, work.location))?
                {
                    return Ok(false);
                }
                offset += actual.expanded_nodes();
            }
            PatternNode::Slot(_) | PatternNode::Wildcard => offset += actual.expanded_nodes(),
        }
    }
    Ok(offset == value.expanded_nodes())
}
