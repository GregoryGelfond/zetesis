//! A whole-rule variable bijection, separate from matching ground model rows.

use themelios_program::program::{
    Arguments, Atom, BodyElement, DefaultNegation, Head, LiteralInner, Rule,
};
use themelios_program::term::{Term, Variable};
use zetesis_core::{PatternRef, TemplateTerm, catalog::PredicateRef};

use super::{
    symbols,
    workspace::{Context, Scratch},
};
use crate::FormulaFailure;
use crate::formula_ir::{HeadIr, LiteralIr, RuleIr};

pub(super) fn head(rule: &Rule) -> Option<&Atom> {
    let Head::Literal(literal) = rule.head().get() else {
        return None;
    };
    let LiteralInner::Atom(atom) = &literal.inner else {
        return None;
    };
    (literal.negation == DefaultNegation::None).then(|| atom.get())
}

pub(super) fn signature(
    source: &Atom,
    actual: PredicateRef<'_>,
    ignore_sign: bool,
    context: &mut Context<'_, '_>,
) -> Result<bool, FormulaFailure> {
    context.work()?;
    let Arguments::Single(arguments) = &source.arguments else {
        return Ok(false);
    };
    if arguments.len() != actual.arity()
        || (!ignore_sign && crate::coherence::core_sign(source.sign) != actual.sign())
    {
        return Ok(false);
    }
    context.text(source.name.as_str(), actual.name())
}

#[derive(Clone, Copy)]
enum Slot<'a> {
    Vacant,
    Named(&'a str),
    Anonymous,
}

pub(super) fn rule(
    source: &Rule,
    actual: &RuleIr,
    context: &mut Context<'_, '_>,
) -> Result<bool, FormulaFailure> {
    context.work()?;
    let Some(source_head) = head(source) else {
        return Ok(false);
    };
    let HeadIr::Normal(Some(actual_head)) = actual.head else {
        return Ok(false);
    };
    if actual.bindings.is_some() || actual.body_variables != actual.variables {
        return Ok(false);
    }
    let mut slots = Scratch::new(context)?;
    for _ in 0..actual.variables {
        slots.push(Slot::Vacant, context)?;
    }
    let mut source_body = source.body().get().elements();
    for literal in &actual.body {
        context.work()?;
        let Some(element) = source_body.next() else {
            return Ok(false);
        };
        let BodyElement::Literal(source_literal) = element.get() else {
            return Ok(false);
        };
        let LiteralInner::Atom(source_atom) = &source_literal.inner else {
            return Ok(false);
        };
        let LiteralIr::Atom(DefaultNegation::None, actual_atom) = literal else {
            return Ok(false);
        };
        if source_literal.negation != DefaultNegation::None {
            return Ok(false);
        }
        let pattern = context.pattern(*actual_atom)?;
        if !atom(source_atom.get(), pattern, &mut slots.values, true, context)? {
            return Ok(false);
        }
    }
    context.work()?;
    if source_body.next().is_some() {
        return Ok(false);
    }
    let pattern = context.pattern(actual_head)?;
    if !atom(source_head, pattern, &mut slots.values, false, context)? {
        return Ok(false);
    }
    for slot in &slots.values {
        context.work()?;
        if matches!(slot, Slot::Vacant) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn atom<'source, 'read>(
    source: &'source Atom,
    actual: PatternRef<'read>,
    slots: &mut [Slot<'source>],
    binds: bool,
    context: &mut Context<'read, '_>,
) -> Result<bool, FormulaFailure> {
    if !signature(source, actual.predicate(), false, context)? {
        return Ok(false);
    }
    let Arguments::Single(arguments) = &source.arguments else {
        return Ok(false);
    };
    let terms = actual.terms();
    for (column, source) in arguments.iter().enumerate() {
        context.work()?;
        let Some(actual) = terms.at(column) else {
            return Ok(false);
        };
        let matched = match (source, actual) {
            (Term::Symbolic(symbol), TemplateTerm::Constant(value)) => {
                symbols::matches(symbol, value, context)?
            }
            (Term::Variable(variable), TemplateTerm::Variable(slot)) => {
                variable_slot(variable, slot, slots, binds, context)?
            }
            _ => false,
        };
        if !matched {
            return Ok(false);
        }
    }
    Ok(true)
}

fn variable_slot<'source>(
    variable: &'source Variable,
    position: usize,
    slots: &mut [Slot<'source>],
    binds: bool,
    context: &mut Context<'_, '_>,
) -> Result<bool, FormulaFailure> {
    context.work()?;
    let Some(current) = slots.get(position).copied() else {
        return Ok(false);
    };
    match variable {
        Variable::Anonymous => {
            if !binds || !matches!(current, Slot::Vacant) {
                return Ok(false);
            }
            slots[position] = Slot::Anonymous;
        }
        Variable::Named(name) => match current {
            Slot::Named(existing) => return context.text(existing, name.as_str()),
            Slot::Anonymous => return Ok(false),
            Slot::Vacant => {
                if !binds {
                    return Ok(false);
                }
                for other in slots.iter() {
                    context.work()?;
                    if let Slot::Named(existing) = other
                        && context.text(existing, name.as_str())?
                    {
                        return Ok(false);
                    }
                }
                slots[position] = Slot::Named(name.as_str());
            }
        },
    }
    Ok(true)
}
