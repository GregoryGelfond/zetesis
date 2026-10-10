//! Necessary occurrence reads refine a bounded set of changed atoms.
//!
//! This replaces the existing per-predicate flag, not the dependency owner.
//! Overflow keeps ordinary invalidation. A change that makes an occurrence
//! false only disables body instances, regardless of its default negation.
//! Enabling positive occurrences may anchor a completed negative scan; enabling
//! default-negated occurrences still require a full scan. Generated rules
//! decline relevant changes. Slots, including repeated
//! names and lowered constructor temporaries, are independent wildcards here:
//! failing a constant or constructor check excludes a read; passing proves none.

use super::{ChangedAtoms, PreparedConstraints, literal_atom};
use crate::FormulaFailure;
use crate::formula_ir::{LiteralIr, RuleIr};
use crate::formula_pattern::{ArgumentPattern, PatternNode};
use crate::formula_support::{Counters, GroundingWork};
use themelios_program::program::DefaultNegation;
use zetesis_core::catalog::{AtomRef, TermRef};
use zetesis_core::{TemplateComponentsRef, TemplateTerm};

#[derive(Clone, Copy)]
pub(super) enum Change {
    None,
    Atoms(ChangedAtoms),
    /// Changes exceeded the bounded set; no complete atom list is retained.
    Untracked,
}

impl Change {
    pub(super) fn include(&mut self, atom: usize) {
        *self = match *self {
            Self::None => Self::Atoms(ChangedAtoms::one(atom)),
            Self::Atoms(mut atoms) => {
                if atoms.include(atom) {
                    Self::Atoms(atoms)
                } else {
                    Self::Untracked
                }
            }
            Self::Untracked => Self::Untracked,
        };
    }
}

/// The ordered dependency row has one group per original atom occurrence.
/// Both it and the canonical changed IDs belong to this exact prepared core.
/// Clean means no possible read changed or every affected occurrence became
/// false. `PositiveDelta` covers enabling positive occurrences with retained
/// newly held atoms. Enabling default-negated occurrences require Full.
/// Reuse requires monotone, consistent bounds and completed negative evidence;
/// Clean also preserves a completed batch pending application of all its units.
/// Full declines reuse. Relevant generated frames also decline; unchanged
/// generated rules retain their prior clean evidence.
pub(super) fn classify(
    rule: &RuleIr,
    dependencies: &[usize],
    changes: &[Change],
    prepared: &PreparedConstraints<'_>,
    region: &zetesis_cpu::regions::Region,
    counters: &mut Counters,
) -> Result<super::Scan, FormulaFailure> {
    let mut groups = dependencies.iter();
    let mut required = super::Scan::Clean;
    let mut generated = false;
    let mut relevant = false;
    for literal in &rule.body {
        counters.work(&prepared.limits, rule.location)?;
        generated |= crate::formula_binding_cursor::target(literal).is_some();
        if literal_atom(literal).is_none() {
            continue;
        }
        let &group = groups.next().expect("one group per original occurrence");
        match changes[group] {
            Change::None => {}
            Change::Untracked => return Ok(super::Scan::Full),
            Change::Atoms(atoms) => {
                for &position in atoms.as_slice() {
                    counters.work(&prepared.limits, rule.location)?;
                    let atom = prepared
                        .index
                        .expect("prepared source owner")
                        .catalog()
                        .atoms()
                        .at(position)
                        .expect("authenticated changed atom");
                    let components = prepared.completed.components().ok_or_else(|| {
                        crate::formula_support::components::missing(rule.location)
                    })?;
                    if may_read(
                        literal,
                        atom,
                        components,
                        &mut GroundingWork::new(&prepared.limits, counters, rule.location),
                    )? {
                        relevant = true;
                        let (negation, _) = literal_atom(literal).expect("an atom occurrence");
                        match region.decision(position) {
                            // Disabling one read cannot create a violation or
                            // unit. Keep inspecting: an alias later in this
                            // body can read the same atom with another polarity.
                            Some(held) if held != (negation != DefaultNegation::Not) => {}
                            Some(true) if negation == DefaultNegation::None => required
                                .include(super::Scan::PositiveDelta(ChangedAtoms::one(position))),
                            Some(_) | None => return Ok(super::Scan::Full),
                        }
                        if required == super::Scan::Full {
                            return Ok(required);
                        }
                    }
                }
            }
        }
    }
    debug_assert!(groups.next().is_none());
    Ok(if generated && relevant {
        super::Scan::Full
    } else {
        required
    })
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
