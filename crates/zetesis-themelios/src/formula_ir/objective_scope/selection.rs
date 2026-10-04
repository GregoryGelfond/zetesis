//! Select scoped binding from syntax before applying either compiler.

use crate::ProgramSite;
use crate::expansion::Budget;
use crate::{ExpansionResource, FormulaFailure};
use std::collections::BTreeSet;
use themelios_program::program::{
    BodyElement, Condition, DefaultNegation, Literal, LiteralInner, WeakConstraint,
};
use themelios_program::term::{Term, Variable};

pub(super) fn scoped(
    weak: &WeakConstraint,
    budget: &mut Budget,
    location: ProgramSite,
) -> Result<bool, FormulaFailure> {
    for element in weak.body().get().elements() {
        budget.charge(ExpansionResource::TermWork, 1, location)?;
        if !matches!(element.get(), BodyElement::Literal(_)) {
            return Ok(true);
        }
    }
    literals(
        || {
            weak.body()
                .get()
                .elements()
                .filter_map(|element| match element.get() {
                    BodyElement::Literal(literal) => Some(literal),
                    _ => None,
                })
        },
        budget,
        location,
    )
}

pub(super) fn condition(
    condition: &Condition,
    budget: &mut Budget,
    location: ProgramSite,
) -> Result<bool, FormulaFailure> {
    literals(
        || {
            condition
                .literals()
                .map(themelios_program::WithProvenance::get)
        },
        budget,
        location,
    )
}

fn literals<'a, I: Iterator<Item = &'a Literal>>(
    source: impl Fn() -> I,
    budget: &mut Budget,
    location: ProgramSite,
) -> Result<bool, FormulaFailure> {
    let mut bound = BTreeSet::new();
    for literal in source() {
        budget.charge(ExpansionResource::TermWork, 1, location)?;
        if let LiteralInner::Atom(atom) = &literal.inner {
            for term in atom.get().alternatives().flatten() {
                budget.charge(ExpansionResource::TermWork, 1, location)?;
                match term {
                    Term::Variable(Variable::Named(name))
                        if literal.negation == DefaultNegation::None =>
                    {
                        bound.insert(name.as_str());
                    }
                    Term::Variable(Variable::Anonymous)
                        if literal.negation != DefaultNegation::None =>
                    {
                        return Ok(true);
                    }
                    Term::Symbolic(_) | Term::Variable(_) => {}
                    _ => return Ok(true),
                }
            }
        }
    }
    for literal in source() {
        if let LiteralInner::Comparison(comparison) = &literal.inner {
            for term in std::iter::once(comparison.get().first())
                .chain(comparison.get().steps().map(|(_, term)| term))
            {
                for term in term.subterms() {
                    budget.charge(ExpansionResource::TermWork, 1, location)?;
                    if let Term::Variable(variable) = term {
                        match variable {
                            Variable::Named(name) if bound.contains(name.as_str()) => {}
                            _ => return Ok(true),
                        }
                    }
                }
            }
        }
    }
    Ok(false)
}
