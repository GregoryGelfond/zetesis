//! All producer occurrences must agree with the exact analyzed source carriers.
//!
//! A selected rule corresponds to a definition when whole-rule matching
//! certifies it. Compilation emits a statement's rules in the order of its
//! definitions, so a rule first tries the definition at its own place among
//! those of its parsed origin, and otherwise every definition. A definition no
//! rule matched on that path is checked against every selected rule. Both
//! questions (does each selected rule match some definition; is each definition
//! matched by some selected rule) are therefore answered exactly as by checking
//! every pair, at a cost linear in rules and definitions when that order holds.

use themelios_program::program::Statement;
use themelios_program::provenance::WithProvenance;

use super::{
    index::Defined,
    matching, reads,
    workspace::{Context, Scratch},
};
use crate::FormulaFailure;
use crate::formula_ir::{HeadIr, Prepared, RuleIr};

pub(super) fn select(
    prepared: &Prepared,
    definitions: &[&WithProvenance<Statement>],
    context: &mut Context<'_, '_>,
) -> Result<Option<Scratch<u8>>, FormulaFailure> {
    let defined = Defined::index(definitions, context)?;
    let mut covered = Scratch::new(context)?;
    covered.reserve(definitions.len(), context)?;
    for _ in definitions {
        context.work()?;
        covered.values.push(0_u8);
    }
    // How many rules of each origin have taken their positional candidate.
    let mut cursors = Scratch::new(context)?;
    cursors.reserve(defined.origin_entries(), context)?;
    for _ in 0..defined.origin_entries() {
        context.work()?;
        cursors.values.push(0_usize);
    }
    // One reservation each: pushing into exact scratch one cell at a time
    // would copy and charge every earlier cell again.
    let mut producers = Scratch::new(context)?;
    producers.reserve(prepared.rules.len(), context)?;
    let mut selected = Scratch::new(context)?;
    selected.reserve(prepared.rules.len(), context)?;
    for rule in &prepared.rules {
        context.work()?;
        if reads::rule(rule, &defined, context)? {
            return Ok(None);
        }
        let selected_head = match &rule.head {
            HeadIr::Normal(Some(head)) => reads::pattern(*head, &defined, context)?,
            HeadIr::Normal(None) => false,
            HeadIr::Choice(group) => {
                for element in &group.elements {
                    context.work()?;
                    if let Some(head) = element.head.atom()
                        && reads::pattern(*head, &defined, context)?
                    {
                        return Ok(None);
                    }
                }
                false
            }
            HeadIr::Disjunction(_) | HeadIr::ConditionalDisjunction { .. } => {
                for head in rule.head.disjuncts() {
                    context.work()?;
                    if let Some(head) = head.atom()
                        && reads::pattern(*head, &defined, context)?
                    {
                        return Ok(None);
                    }
                }
                false
            }
        };
        if selected_head {
            if !correspond(
                rule,
                &defined,
                &mut covered.values,
                &mut cursors.values,
                context,
            )? {
                return Ok(None);
            }
            producers.push(rule, context)?;
        }
        selected.push(u8::from(selected_head), context)?;
    }
    for (position, definition) in definitions.iter().enumerate() {
        context.work()?;
        if covered.values[position] != 0 {
            continue;
        }
        let mut matched = false;
        for rule in &producers.values {
            context.work()?;
            if matching::rule(reads::source(definition), rule, context)? {
                matched = true;
                break;
            }
        }
        if !matched {
            return Ok(None);
        }
    }
    Ok(Some(selected))
}

/// Whether some definition matches `rule`, marking what it finds covered.
fn correspond(
    rule: &RuleIr,
    defined: &Defined<'_>,
    covered: &mut [u8],
    cursors: &mut [usize],
    context: &mut Context<'_, '_>,
) -> Result<bool, FormulaFailure> {
    for &origin in &rule.origins {
        context.work()?;
        let range = defined.origin(origin, context)?;
        if range.is_empty() {
            continue;
        }
        let hint = range.start + cursors[range.start];
        if hint < range.end {
            cursors[range.start] += 1;
            let position = defined.position(hint);
            if matching::rule(reads::source(defined.all[position]), rule, context)? {
                covered[position] = 1;
                return Ok(true);
            }
        }
    }
    let mut matched = false;
    for (position, definition) in defined.all.iter().enumerate() {
        context.work()?;
        if matching::rule(reads::source(definition), rule, context)? {
            covered[position] = 1;
            matched = true;
        }
    }
    Ok(matched)
}
