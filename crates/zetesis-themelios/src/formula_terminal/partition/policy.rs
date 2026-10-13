//! Defer complete certified signatures only when a producer enumerates bindings.

use themelios_program::program::Statement;
use themelios_program::provenance::WithProvenance;
use zetesis_core::catalog::PredicateRef;

use super::{
    matching, reads,
    workspace::{Context, Scratch},
};
use crate::FormulaFailure;
use crate::formula_facts::Item;
use crate::formula_ir::{HeadIr, Prepared};

/// Refine a complete source/IR certificate without splitting producer groups.
/// All rules of a selected signature move together, including its closed facts.
/// Ground-only signatures stay in the base because they avoid no binding work.
/// Signature probes cost O(rules + (rules + definitions) * selected signatures),
/// with compared source-name bytes charged separately. Scratch holds
/// O(selected signatures + selected source carriers) borrowed references, never
/// canonical payload.
pub(super) fn select<'source>(
    prepared: &Prepared,
    definitions: &[&'source WithProvenance<Statement>],
    selected: &mut [u8],
    context: &mut Context<'_, '_>,
) -> Result<Scratch<&'source WithProvenance<Statement>>, FormulaFailure> {
    let mut signatures = Scratch::new(context)?;
    for (item, selected) in prepared.items().iter().zip(selected.iter()) {
        context.work()?;
        // A fact product is ground: it adds no signature, and moves with one.
        if let Item::Rule(_, rule) = item
            && *selected != 0
            && rule.variables != 0
        {
            let predicate = predicate(item, context)?;
            if !contains(predicate, &signatures.values, context)? {
                signatures.push(predicate, context)?;
            }
        }
    }
    let mut deferred = Scratch::new(context)?;
    if signatures.values.is_empty() {
        return Ok(deferred);
    }
    for definition in definitions {
        context.work()?;
        let head = matching::head(reads::source(definition))
            .expect("certified definition has an ordinary atom head");
        for signature in &signatures.values {
            context.work()?;
            if matching::signature(head, *signature, false, context)? {
                deferred.push(*definition, context)?;
                break;
            }
        }
    }
    for (item, selected) in prepared.items().iter().zip(selected.iter_mut()) {
        context.work()?;
        if *selected != 0 {
            *selected = u8::from(contains(
                predicate(item, context)?,
                &signatures.values,
                context,
            )?);
        }
    }
    Ok(deferred)
}

fn predicate<'a>(
    item: Item<'_>,
    context: &mut Context<'a, '_>,
) -> Result<PredicateRef<'a>, FormulaFailure> {
    match item {
        Item::Rule(_, rule) => {
            let HeadIr::Normal(Some(head)) = rule.head else {
                unreachable!("certified selected producer has an ordinary atom head")
            };
            Ok(context.pattern(head)?.predicate())
        }
        Item::Facts(product) => context.predicate(product.predicate()),
    }
}

fn contains(
    predicate: PredicateRef<'_>,
    signatures: &[PredicateRef<'_>],
    context: &mut Context<'_, '_>,
) -> Result<bool, FormulaFailure> {
    for signature in signatures {
        context.work()?;
        if predicate.equals_ref_with(*signature, || context.work())? {
            return Ok(true);
        }
    }
    Ok(false)
}
