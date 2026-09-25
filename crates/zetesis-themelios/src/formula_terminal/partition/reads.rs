//! Exhaustive compiled semantic reads; scalar term occurrences are not atoms.

use themelios_program::program::{Rule, Statement};
use themelios_program::provenance::WithProvenance;
use zetesis_core::catalog::PredicateRef;

use super::{
    matching,
    workspace::{Context, Scratch},
};
use crate::FormulaFailure;
use crate::formula_conditional_ir::{Consequent, ConsequentOperand};
use crate::formula_ir::{AggregateKey, HeadIr, LiteralIr, Projection, RuleIr};
use crate::formula_support::components::Pattern;

pub(super) fn selected(
    predicate: PredicateRef<'_>,
    definitions: &[&WithProvenance<Statement>],
    context: &mut Context<'_, '_>,
) -> Result<bool, FormulaFailure> {
    for definition in definitions {
        context.work()?;
        let Statement::Rule(rule) = definition.get() else {
            continue;
        };
        if let Some(head) = matching::head(rule)
            && matching::signature(head, predicate, true, context)?
        {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(super) fn pattern(
    value: Pattern,
    definitions: &[&WithProvenance<Statement>],
    context: &mut Context<'_, '_>,
) -> Result<bool, FormulaFailure> {
    let predicate = context.pattern(value)?.predicate();
    selected(predicate, definitions, context)
}

pub(super) fn source(definition: &WithProvenance<Statement>) -> &Rule {
    let Statement::Rule(rule) = definition.get() else {
        unreachable!("terminal analysis selects only rules")
    };
    rule
}

pub(super) fn rule(
    rule: &RuleIr,
    definitions: &[&WithProvenance<Statement>],
    context: &mut Context<'_, '_>,
) -> Result<bool, FormulaFailure> {
    let mut pending = Scratch::new(context)?;
    pending.push(rule.body.as_slice(), context)?;
    match &rule.head {
        HeadIr::Choice(group) => {
            for element in &group.elements {
                context.work()?;
                pending.push(element.condition.as_slice(), context)?;
            }
        }
        HeadIr::ConditionalDisjunction { elements, .. } => {
            for element in elements {
                context.work()?;
                pending.push(element.condition.as_slice(), context)?;
            }
        }
        HeadIr::Normal(_) | HeadIr::Disjunction(_) => {}
    }
    while !pending.values.is_empty() {
        context.work()?;
        let literals = pending
            .values
            .pop()
            .expect("checked nonempty read frontier");
        for literal in literals {
            context.work()?;
            if literal_reads(literal, definitions, &mut pending, context)? {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn projection<'ir>(
    value: &'ir Projection,
    definitions: &[&WithProvenance<Statement>],
    pending: &mut Scratch<&'ir [LiteralIr]>,
    context: &mut Context<'_, '_>,
) -> Result<bool, FormulaFailure> {
    let predicate = value.predicate(
        context.components,
        context.limits,
        context.counters,
        context.location,
    )?;
    if selected(predicate, definitions, context)? {
        return Ok(true);
    }
    if let Projection::Witnesses { bindings, .. } = value {
        pending.push(bindings.as_slice(), context)?;
    }
    Ok(false)
}

fn literal_reads<'ir>(
    literal: &'ir LiteralIr,
    definitions: &[&WithProvenance<Statement>],
    pending: &mut Scratch<&'ir [LiteralIr]>,
    context: &mut Context<'_, '_>,
) -> Result<bool, FormulaFailure> {
    match literal {
        LiteralIr::Atom(_, atom) => pattern(*atom, definitions, context),
        LiteralIr::PatternAtom(atom) => pattern(atom.atom, definitions, context),
        LiteralIr::ProjectedAtom(_, value) => projection(value, definitions, pending, context),
        LiteralIr::Conditional(value) => {
            pending.push(value.condition.as_slice(), context)?;
            match &value.consequent {
                Consequent::Atoms(_, alternatives) => {
                    for alternative in alternatives {
                        context.work()?;
                        pending.push(alternative.bindings.as_slice(), context)?;
                        let found = match &alternative.operand {
                            ConsequentOperand::Atom(atom) => pattern(*atom, definitions, context)?,
                            ConsequentOperand::Projection(value) => {
                                projection(value, definitions, pending, context)?
                            }
                        };
                        if found {
                            return Ok(true);
                        }
                    }
                }
                Consequent::Guards(alternatives) => {
                    for alternative in alternatives {
                        context.work()?;
                        pending.push(alternative.bindings.as_slice(), context)?;
                    }
                }
                Consequent::Guard(_) => {}
            }
            Ok(false)
        }
        LiteralIr::Aggregate(value) => {
            for element in &value.elements {
                context.work()?;
                pending.push(element.condition.as_slice(), context)?;
                if let AggregateKey::Atom(atom) = &element.key
                    && pattern(*atom, definitions, context)?
                {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        LiteralIr::Compare(..)
        | LiteralIr::ArgumentCheck { .. }
        | LiteralIr::TupleCompare(..)
        | LiteralIr::Guard(_)
        | LiteralIr::Bind { .. }
        | LiteralIr::Range { .. } => Ok(false),
    }
}
