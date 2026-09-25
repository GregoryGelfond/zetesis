//! Conservative variable components with leased integer planning metadata.
//! Component source IR copying keeps its existing explicit expansion charges;
//! execution assignments themselves contain only canonical IDs.

use crate::formula_support::Context;

use std::ops::Range;

use crate::formula_support::components::Term;
use zetesis_core::PatternRef;

use super::Component;
use crate::FormulaFailure;
use crate::formula_ground::Builder;
use crate::formula_ir::{Expression, LiteralIr, Operation, Projection, RuleIr};
use crate::formula_support::{StorageLease, reserve};

pub(super) struct Plan {
    pub(super) components: Vec<Component>,
    lease: StorageLease,
}

struct Scratch {
    fixed: Vec<bool>,
    parents: Vec<usize>,
    scopes: Vec<Range<usize>>,
    variables: Vec<usize>,
    groups: Vec<(Option<usize>, usize)>,
    bound: Vec<bool>,
    lease: StorageLease,
}

fn lease(
    builder: &mut Builder<'_, '_, '_>,
    bytes: usize,
    rule: &RuleIr,
) -> Result<StorageLease, FormulaFailure> {
    let mut lease = builder.computation.lease();
    lease.observe(bytes, rule.location)?;
    builder.computation.storage_observed(
        &lease,
        0,
        bytes,
        builder.limits,
        &builder.counters,
        rule.location,
    )?;
    Ok(lease)
}

/// All buffers registered in this lease coexist. Replacing one buffer charges
/// its old/replacement overlap without adding any other allocation twice.
fn push<T>(
    values: &mut Vec<T>,
    value: T,
    lease: &mut StorageLease,
    builder: &mut Builder<'_, '_, '_>,
    rule: &RuleIr,
) -> Result<(), FormulaFailure> {
    let other = lease.bytes() - values.capacity() * size_of::<T>();
    let additional = if values.len() == values.capacity() {
        values.capacity().max(1)
    } else {
        1
    };
    reserve(
        values,
        additional,
        lease,
        other,
        Context::new(
            &*builder.computation,
            builder.limits,
            &mut builder.counters,
            rule.location,
        ),
    )?;
    builder.work(rule.location)?;
    values.push(value);
    Ok(())
}

pub(super) fn components(
    builder: &mut Builder<'_, '_, '_>,
    rule: &RuleIr,
    head: Option<PatternRef<'_>>,
) -> Result<Option<Plan>, FormulaFailure> {
    let Some(mut scratch) = partition(builder, rule, head)? else {
        return Ok(None);
    };
    let mut plan = Plan {
        components: Vec::new(),
        lease: lease(builder, size_of::<Plan>(), rule)?,
    };
    let mut start = 0;
    while start < scratch.groups.len() {
        builder.work(rule.location)?;
        let key = scratch.groups[start].0;
        let mut end = start + 1;
        while end < scratch.groups.len() {
            builder.work(rule.location)?;
            if scratch.groups[end].0 != key {
                break;
            }
            end += 1;
        }
        let mut component = Component {
            literals: Vec::new(),
            used: Vec::new(),
        };
        for &(_, index) in &scratch.groups[start..end] {
            for &variable in &scratch.variables[scratch.scopes[index].clone()] {
                push(
                    &mut component.used,
                    variable,
                    &mut plan.lease,
                    builder,
                    rule,
                )?;
            }
            let literal = copy_literal(builder, rule, &rule.body[index])?;
            push(
                &mut component.literals,
                literal,
                &mut plan.lease,
                builder,
                rule,
            )?;
        }
        sort(&mut component.used, builder, rule)?;
        for _ in &component.used {
            builder.work(rule.location)?;
        }
        component.used.dedup();
        for bound in &mut scratch.bound {
            builder.work(rule.location)?;
            *bound = false;
        }
        for literal in &component.literals {
            if let LiteralIr::Atom(themelios_program::program::DefaultNegation::None, pattern) =
                literal
            {
                let pattern = builder.computation.static_pattern(
                    *pattern,
                    builder.limits,
                    &mut builder.counters,
                    rule.location,
                )?;
                for variable in pattern.terms().variables() {
                    builder.work(rule.location)?;
                    scratch.bound[variable] = true;
                }
            }
        }
        // Local values require a binder inside this component. Fixed head
        // values retain their existing canonical assignment and need none here.
        for &variable in &component.used {
            builder.work(rule.location)?;
            if !scratch.fixed[variable] && !scratch.bound[variable] {
                return Ok(None);
            }
        }
        push(
            &mut plan.components,
            component,
            &mut plan.lease,
            builder,
            rule,
        )?;
        start = end;
    }
    Ok(Some(plan))
}

/// Connect non-head variables and group literals by their admitted component.
/// This scratch remains live while the plan copies the selected literal scopes.
fn partition(
    builder: &mut Builder<'_, '_, '_>,
    rule: &RuleIr,
    head: Option<PatternRef<'_>>,
) -> Result<Option<Scratch>, FormulaFailure> {
    let mut scratch = Scratch {
        fixed: Vec::new(),
        parents: Vec::new(),
        scopes: Vec::new(),
        variables: Vec::new(),
        groups: Vec::new(),
        bound: Vec::new(),
        lease: lease(builder, size_of::<Scratch>(), rule)?,
    };
    for variable in 0..rule.variables {
        push(&mut scratch.fixed, false, &mut scratch.lease, builder, rule)?;
        push(
            &mut scratch.parents,
            variable,
            &mut scratch.lease,
            builder,
            rule,
        )?;
        push(&mut scratch.bound, false, &mut scratch.lease, builder, rule)?;
    }
    for variable in head.into_iter().flat_map(|head| head.terms().variables()) {
        builder.work(rule.location)?;
        scratch.fixed[variable] = true;
    }
    for literal in &rule.body {
        let start = scratch.variables.len();
        if !scope(
            builder,
            rule,
            literal,
            &mut scratch.variables,
            &mut scratch.lease,
        )? {
            return Ok(None);
        }
        let range = start..scratch.variables.len();
        // Match the prior sorted scope convention, so representatives and
        // component order remain independent of argument visitation order.
        sort(&mut scratch.variables[range.clone()], builder, rule)?;
        let mut first = None;
        for &variable in &scratch.variables[range.clone()] {
            builder.work(rule.location)?;
            if scratch.fixed[variable] {
                continue;
            }
            if let Some(first) = first {
                let left = root(builder, rule, &scratch.parents, first)?;
                let right = root(builder, rule, &scratch.parents, variable)?;
                builder.work(rule.location)?;
                scratch.parents[right] = left;
            } else {
                first = Some(variable);
            }
        }
        push(
            &mut scratch.scopes,
            range,
            &mut scratch.lease,
            builder,
            rule,
        )?;
    }
    for (index, scope) in scratch.scopes.iter().enumerate() {
        let mut representative = None;
        for &variable in &scratch.variables[scope.clone()] {
            builder.work(rule.location)?;
            if !scratch.fixed[variable] {
                representative = Some(variable);
                break;
            }
        }
        let key = representative
            .map(|variable| root(builder, rule, &scratch.parents, variable))
            .transpose()?;
        push(
            &mut scratch.groups,
            (key, index),
            &mut scratch.lease,
            builder,
            rule,
        )?;
    }
    sort(&mut scratch.groups, builder, rule)?;
    builder.work(rule.location)?;
    if scratch.groups.first().map(|group| group.0) == scratch.groups.last().map(|group| group.0) {
        return Ok(None);
    }
    Ok(Some(scratch))
}

fn root(
    builder: &mut Builder<'_, '_, '_>,
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
    builder: &mut Builder<'_, '_, '_>,
    rule: &RuleIr,
    literal: &LiteralIr,
    used: &mut Vec<usize>,
    lease: &mut StorageLease,
) -> Result<bool, FormulaFailure> {
    match literal {
        LiteralIr::Atom(_, pattern) => {
            let pattern = builder.computation.static_pattern(
                *pattern,
                builder.limits,
                &mut builder.counters,
                rule.location,
            )?;
            for variable in pattern.terms().variables() {
                push(used, variable, lease, builder, rule)?;
            }
        }
        LiteralIr::ProjectedAtom(_, projection) => {
            let Projection::Arguments { terms, .. } = projection else {
                return Ok(false);
            };
            for term in terms.iter().flatten() {
                builder.work(rule.location)?;
                if let Term::Variable(variable) = term {
                    push(used, *variable, lease, builder, rule)?;
                }
            }
        }
        LiteralIr::Compare(left, _, right) => {
            if !expression_scope(builder, rule, left, used, lease)?
                || !expression_scope(builder, rule, right, used, lease)?
            {
                return Ok(false);
            }
        }
        LiteralIr::TupleCompare(left, _, right) => {
            for expression in left.iter().chain(right) {
                if !expression_scope(builder, rule, expression, used, lease)? {
                    return Ok(false);
                }
            }
        }
        _ => return Ok(false),
    }
    Ok(true)
}

fn expression_scope(
    builder: &mut Builder<'_, '_, '_>,
    rule: &RuleIr,
    expression: &Expression,
    used: &mut Vec<usize>,
    lease: &mut StorageLease,
) -> Result<bool, FormulaFailure> {
    for operation in &expression.nodes {
        builder.work(rule.location)?;
        match operation {
            Operation::Variable(variable) => push(used, *variable, lease, builder, rule)?,
            Operation::Constant(_) => {}
            _ => return Ok(false),
        }
    }
    Ok(true)
}

/// Integer metadata uses the same checked sorting permits as typed views.
fn sort<T: Ord>(
    values: &mut [T],
    builder: &mut Builder<'_, '_, '_>,
    rule: &RuleIr,
) -> Result<(), FormulaFailure> {
    crate::formula_support::sort::by(
        values,
        crate::formula_support::GroundingWork::new(
            builder.limits,
            &mut builder.counters,
            rule.location,
        ),
        |left, right, _| Ok(left.cmp(right)),
    )
}

fn copy_expression(
    builder: &mut Builder<'_, '_, '_>,
    rule: &RuleIr,
    expression: &Expression,
) -> Result<Expression, FormulaFailure> {
    let mut nodes = Vec::with_capacity(expression.nodes.len());
    for operation in &expression.nodes {
        builder.work(rule.location)?;
        nodes.push(match *operation {
            Operation::Constant(value) => Operation::Constant(value),
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
    builder: &mut Builder<'_, '_, '_>,
    rule: &RuleIr,
    literal: &LiteralIr,
) -> Result<LiteralIr, FormulaFailure> {
    Ok(match literal {
        LiteralIr::Atom(negation, pattern) => LiteralIr::Atom(*negation, *pattern),
        LiteralIr::ProjectedAtom(negation, projection) => {
            let Projection::Arguments {
                predicate,
                terms: original,
            } = projection
            else {
                unreachable!("factor scope excludes structural projections")
            };
            let mut terms = Vec::new();
            crate::formula_pattern::reserve(
                &mut terms,
                original.len(),
                builder.budget,
                rule.location,
            )?;
            for term in original {
                builder.work(rule.location)?;
                terms.push(*term);
            }
            LiteralIr::ProjectedAtom(
                *negation,
                Projection::Arguments {
                    predicate: *predicate,
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
