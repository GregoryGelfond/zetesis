//! Conservative source-independent variable components and bounded IR copying.

use std::collections::{BTreeMap, BTreeSet};

use zetesis_core::{AtomPattern, Term};

use super::{Component, positive_variables};
use crate::formula_ground::Builder;
use crate::formula_ir::{Expression, LiteralIr, Operation, Projection, RuleIr, value_bytes};
use crate::formula_support::copy;
use crate::{ExpansionResource, FormulaFailure};

pub(super) fn components(
    builder: &mut Builder<'_>,
    rule: &RuleIr,
    head: Option<&AtomPattern>,
) -> Result<Option<Vec<Component>>, FormulaFailure> {
    let fixed: BTreeSet<_> = head
        .into_iter()
        .flat_map(AtomPattern::terms)
        .filter_map(|term| {
            if let Term::Variable(variable) = term {
                Some(*variable)
            } else {
                None
            }
        })
        .collect();
    let mut scopes = Vec::new();
    let mut parents = Vec::with_capacity(rule.variables);
    for variable in 0..rule.variables {
        builder.work(rule.location)?;
        parents.push(variable);
    }
    for literal in &rule.body {
        let Some(scope) = scope(builder, rule, literal)? else {
            return Ok(None);
        };
        let mut local = scope.iter().filter(|variable| !fixed.contains(variable));
        if let Some(&first) = local.next() {
            for &variable in local {
                let left = root(builder, rule, &parents, first)?;
                let right = root(builder, rule, &parents, variable)?;
                parents[right] = left;
            }
        }
        scopes.push(scope);
    }
    let mut groups: BTreeMap<Option<usize>, Vec<usize>> = BTreeMap::new();
    for (index, scope) in scopes.iter().enumerate() {
        builder.work(rule.location)?;
        let representative = scope.iter().find(|variable| !fixed.contains(variable));
        let key = representative
            .map(|&variable| root(builder, rule, &parents, variable))
            .transpose()?;
        groups.entry(key).or_default().push(index);
    }
    if groups.len() < 2 {
        return Ok(None);
    }
    let mut components = Vec::new();
    for indices in groups.values() {
        let mut used = BTreeSet::new();
        let mut literals = Vec::new();
        for &index in indices {
            builder.work(rule.location)?;
            used.extend(scopes[index].iter().copied());
            literals.push(copy_literal(builder, rule, &rule.body[index])?);
        }
        // Every local value must still have a positive relation binder inside
        // this component. Fixed head values need no new binder here.
        let bound: BTreeSet<_> = positive_variables(&literals).collect();
        if used
            .iter()
            .any(|variable| !fixed.contains(variable) && !bound.contains(variable))
        {
            return Ok(None);
        }
        components.push(Component { literals, used });
    }
    Ok(Some(components))
}

fn root(
    builder: &mut Builder<'_>,
    rule: &RuleIr,
    parents: &[usize],
    mut index: usize,
) -> Result<usize, FormulaFailure> {
    loop {
        builder.work(rule.location)?;
        if parents[index] == index {
            return Ok(index);
        }
        index = parents[index];
    }
}

fn scope(
    builder: &mut Builder<'_>,
    rule: &RuleIr,
    literal: &LiteralIr,
) -> Result<Option<BTreeSet<usize>>, FormulaFailure> {
    let mut used = BTreeSet::new();
    let mut term = |term: &Term| -> Result<(), FormulaFailure> {
        builder.work(rule.location)?;
        if let Term::Variable(variable) = term {
            used.insert(*variable);
        }
        Ok(())
    };
    match literal {
        LiteralIr::Atom(_, pattern) => {
            for item in pattern.terms() {
                term(item)?;
            }
        }
        LiteralIr::ProjectedAtom(_, projection) => {
            for item in projection.terms.iter().flatten() {
                term(item)?;
            }
        }
        LiteralIr::Compare(left, _, right) => {
            if !expression_scope(builder, rule, left, &mut used)?
                || !expression_scope(builder, rule, right, &mut used)?
            {
                return Ok(None);
            }
        }
        LiteralIr::TupleCompare(left, _, right) => {
            for expression in left.iter().chain(right) {
                if !expression_scope(builder, rule, expression, &mut used)? {
                    return Ok(None);
                }
            }
        }
        _ => return Ok(None),
    }
    Ok(Some(used))
}

fn expression_scope(
    builder: &mut Builder<'_>,
    rule: &RuleIr,
    expression: &Expression,
    used: &mut BTreeSet<usize>,
) -> Result<bool, FormulaFailure> {
    for operation in &expression.nodes {
        builder.work(rule.location)?;
        match operation {
            Operation::Variable(variable) => {
                used.insert(*variable);
            }
            Operation::Constant(_) => {}
            _ => return Ok(false),
        }
    }
    Ok(true)
}

fn copy_pattern(
    builder: &mut Builder<'_>,
    rule: &RuleIr,
    pattern: &AtomPattern,
) -> Result<AtomPattern, FormulaFailure> {
    let mut bytes = pattern.predicate().name().len() as u128;
    for term in pattern.terms() {
        builder.work(rule.location)?;
        if let Term::Constant(value) = term {
            bytes += value_bytes(value);
        }
    }
    builder
        .budget
        .charge(ExpansionResource::ScalarBytes, bytes, rule.location)?;
    Ok(pattern.clone())
}

fn copy_expression(
    builder: &mut Builder<'_>,
    rule: &RuleIr,
    expression: &Expression,
) -> Result<Expression, FormulaFailure> {
    let mut nodes = Vec::with_capacity(expression.nodes.len());
    for operation in &expression.nodes {
        builder.work(rule.location)?;
        nodes.push(match *operation {
            Operation::Constant(ref value) => {
                Operation::Constant(copy(value, builder.budget, rule.location)?)
            }
            Operation::Constructor(ref constructor) => {
                Operation::Constructor(constructor.copy(builder.budget, rule.location)?)
            }
            Operation::Variable(variable) => Operation::Variable(variable),
            Operation::Unary(operator, value) => Operation::Unary(operator, value),
            Operation::Binary(operator, left, right) => Operation::Binary(operator, left, right),
            Operation::Absolute(value) => Operation::Absolute(value),
        });
    }
    Ok(Expression { nodes })
}

fn copy_literal(
    builder: &mut Builder<'_>,
    rule: &RuleIr,
    literal: &LiteralIr,
) -> Result<LiteralIr, FormulaFailure> {
    Ok(match literal {
        LiteralIr::Atom(negation, pattern) => {
            LiteralIr::Atom(*negation, copy_pattern(builder, rule, pattern)?)
        }
        LiteralIr::ProjectedAtom(negation, projection) => {
            builder.budget.charge(
                ExpansionResource::ScalarBytes,
                projection.predicate.name().len() as u128,
                rule.location,
            )?;
            let mut terms = Vec::new();
            for term in &projection.terms {
                builder.work(rule.location)?;
                terms.push(match term {
                    Some(Term::Constant(value)) => {
                        Some(Term::Constant(copy(value, builder.budget, rule.location)?))
                    }
                    Some(Term::Variable(variable)) => Some(Term::Variable(*variable)),
                    None => None,
                });
            }
            LiteralIr::ProjectedAtom(
                *negation,
                Projection {
                    predicate: projection.predicate.clone(),
                    terms,
                },
            )
        }
        LiteralIr::Compare(left, relation, right) => LiteralIr::Compare(
            copy_expression(builder, rule, left)?,
            *relation,
            copy_expression(builder, rule, right)?,
        ),
        LiteralIr::TupleCompare(left, relation, right) => LiteralIr::TupleCompare(
            left.iter()
                .map(|expression| copy_expression(builder, rule, expression))
                .collect::<Result<_, _>>()?,
            *relation,
            right
                .iter()
                .map(|expression| copy_expression(builder, rule, expression))
                .collect::<Result<_, _>>()?,
        ),
        _ => unreachable!("the complete factor plan only copies its supported literal profile"),
    })
}
