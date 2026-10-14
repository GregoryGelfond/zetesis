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
use crate::formula_facts::{FactBlock, Item, Row};
use crate::formula_ir::{HeadIr, Prepared, RuleIr};
use crate::{FormulaFailure, ProgramSite};
use zetesis_core::catalog::PredicateRef;

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
    producers.reserve(prepared.items().len(), context)?;
    let mut selected = Scratch::new(context)?;
    selected.reserve(prepared.items().len(), context)?;
    for item in prepared.items().iter() {
        context.work()?;
        let rule = match item {
            Item::Rule(_, rule) => rule,
            Item::Facts(block) => {
                let Some(selected_head) = block_head(
                    block,
                    &defined,
                    &mut covered.values,
                    &mut cursors.values,
                    context,
                )?
                else {
                    return Ok(None);
                };
                if selected_head {
                    producers.push(item, context)?;
                }
                selected.push(u8::from(selected_head), context)?;
                continue;
            }
        };
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
                Occurrence::Rule(rule),
                &defined,
                &mut covered.values,
                &mut cursors.values,
                context,
            )? {
                return Ok(None);
            }
            producers.push(item, context)?;
        }
        selected.push(u8::from(selected_head), context)?;
    }
    for (position, definition) in definitions.iter().enumerate() {
        context.work()?;
        if covered.values[position] != 0 {
            continue;
        }
        let mut matched = false;
        for producer in &producers.values {
            context.work()?;
            if matches(reads::source(definition), *producer, context)? {
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

/// Whether a fact block's head is selected, or `None` when one of its rows
/// corresponds to no definition. A fact reads nothing; each row of a selected
/// block must correspond to its analyzed fact statement, as its fact rule did.
fn block_head(
    block: FactBlock<'_>,
    defined: &Defined<'_>,
    covered: &mut [u8],
    cursors: &mut [usize],
    context: &mut Context<'_, '_>,
) -> Result<Option<bool>, FormulaFailure> {
    let predicate = context.predicate(block.predicate())?;
    if !reads::selected(predicate, defined, context)? {
        return Ok(Some(false));
    }
    for row in block.rows() {
        if !correspond(
            Occurrence::Row(row, predicate),
            defined,
            covered,
            cursors,
            context,
        )? {
            return Ok(None);
        }
    }
    Ok(Some(true))
}

/// Whether `definition` matches the producer: a rule, or some row of a block.
fn matches(
    definition: &themelios_program::program::Rule,
    producer: Item<'_>,
    context: &mut Context<'_, '_>,
) -> Result<bool, FormulaFailure> {
    match producer {
        Item::Rule(_, rule) => Occurrence::Rule(rule).matches(definition, context),
        Item::Facts(block) => {
            let predicate = context.predicate(block.predicate())?;
            for row in block.rows() {
                context.work()?;
                if Occurrence::Row(row, predicate).matches(definition, context)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
    }
}

/// One producer occurrence: a rule, or one row of a fact block with the
/// block's predicate, read once for the block.
#[derive(Clone, Copy)]
enum Occurrence<'a, 'p> {
    Rule(&'a RuleIr),
    Row(Row<'a>, PredicateRef<'p>),
}

impl<'a> Occurrence<'a, '_> {
    /// The occurrence's statement evidence: a rule's origins, or the evidence
    /// of the row's statement.
    fn evidence(self) -> &'a [ProgramSite] {
        match self {
            Self::Rule(rule) => &rule.origins,
            Self::Row(row, _) => row.evidence(),
        }
    }

    /// Whether `definition` matches the occurrence, by whole-rule matching.
    fn matches(
        self,
        definition: &themelios_program::program::Rule,
        context: &mut Context<'_, '_>,
    ) -> Result<bool, FormulaFailure> {
        match self {
            Self::Rule(rule) => matching::rule(definition, rule, context),
            Self::Row(row, predicate) => matching::fact(definition, predicate, row, context),
        }
    }
}

/// Whether some definition matches `occurrence`, marking what it finds
/// covered: positionally first, by the occurrence's statement evidence, and
/// otherwise against every definition.
fn correspond(
    occurrence: Occurrence<'_, '_>,
    defined: &Defined<'_>,
    covered: &mut [u8],
    cursors: &mut [usize],
    context: &mut Context<'_, '_>,
) -> Result<bool, FormulaFailure> {
    for &origin in occurrence.evidence() {
        context.work()?;
        let range = defined.origin(origin, context)?;
        if range.is_empty() {
            continue;
        }
        let hint = range.start + cursors[range.start];
        if hint < range.end {
            cursors[range.start] += 1;
            let position = defined.position(hint);
            if occurrence.matches(reads::source(defined.all[position]), context)? {
                covered[position] = 1;
                return Ok(true);
            }
        }
    }
    let mut matched = false;
    for (position, definition) in defined.all.iter().enumerate() {
        context.work()?;
        if occurrence.matches(reads::source(definition), context)? {
            covered[position] = 1;
            matched = true;
        }
    }
    Ok(matched)
}
