//! All producer occurrences must agree with the exact analyzed source carriers.

use themelios_program::program::Statement;
use themelios_program::provenance::WithProvenance;

use super::{
    matching, reads,
    workspace::{Context, Scratch},
};
use crate::FormulaFailure;
use crate::formula_ir::{HeadIr, Prepared};

pub(super) fn select(
    prepared: &Prepared,
    definitions: &[&WithProvenance<Statement>],
    context: &mut Context<'_, '_>,
) -> Result<Option<Scratch<u8>>, FormulaFailure> {
    let mut covered = Scratch::new(context)?;
    for _ in definitions {
        covered.push(0_u8, context)?;
    }
    let mut selected = Scratch::new(context)?;
    for rule in &prepared.rules {
        context.work()?;
        if reads::rule(rule, definitions, context)? {
            return Ok(None);
        }
        let selected_head = match &rule.head {
            HeadIr::Normal(Some(head)) => reads::pattern(*head, definitions, context)?,
            HeadIr::Normal(None) => false,
            HeadIr::Choice(group) => {
                for element in &group.elements {
                    context.work()?;
                    if let Some(head) = element.head.atom()
                        && reads::pattern(*head, definitions, context)?
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
                        && reads::pattern(*head, definitions, context)?
                    {
                        return Ok(None);
                    }
                }
                false
            }
        };
        if selected_head {
            let mut matched = false;
            for (position, definition) in definitions.iter().enumerate() {
                context.work()?;
                if matching::rule(reads::source(definition), rule, context)? {
                    covered.values[position] = 1;
                    matched = true;
                }
            }
            if !matched {
                return Ok(None);
            }
        }
        selected.push(u8::from(selected_head), context)?;
    }
    for covered in &covered.values {
        context.work()?;
        if *covered == 0 {
            return Ok(None);
        }
    }
    Ok(Some(selected))
}
