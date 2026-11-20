//! Enumerate range-and-arithmetic rules through the shared columnar operators.
//!
//! Bind, Filter, and Project live in `zetesis-algebra`. A rule whose body is
//! only ranges, residual comparisons, and invertible arithmetic is compiled
//! into that Template, enumerated under the generating schedule when a
//! generator applies (t116u: that schedule visited far fewer candidates than
//! the filter-only product in the algebra tests, with identical bindings),
//! and emitted through the ordinary `Builder::rule` path so only retained
//! rows become formula nodes. Rules the compiler declines keep the Join
//! cursor.

use themelios_program::program::Relation;
use themelios_program::term::BinaryOp;
use zetesis_algebra::{Atoms, Bindings, EnumerationStats, Schedule, Template, TemplateError, rows};
use zetesis_core::ValueNodeRef;
use zetesis_core::catalog::TermKey;

use super::Builder;
use crate::formula_binding::Binding;
use crate::formula_ir::{Expression, HeadIr, LiteralIr, Operation, RuleIr};
use crate::formula_support::components::Scalar;
use crate::formula_support::{Counters, Support};
use crate::{FormulaFailure, FormulaLimits, ProgramSite};

/// Try to ground `rule` with the columnar operators.
///
/// Returns `Ok(true)` when the rule was compiled and emitted, `Ok(false)`
/// when this path does not apply. Failures are resource or catalog refusals
/// from charging, interning, or the ordinary emitter.
pub(super) fn instantiate<'source>(
    builder: &mut Builder<'_, '_, 'source>,
    rule: &'source RuleIr,
    support: &Support<'source>,
) -> Result<bool, FormulaFailure> {
    let Some((template, width)) = compile(rule, builder)? else {
        return Ok(false);
    };
    let mut stats = EnumerationStats::default();
    let bindings = enumerate(builder, rule, &template, &mut stats)?;
    emit(builder, rule, support, &bindings, width)?;
    let _ = (width, stats);
    Ok(true)
}

fn enumerate(
    builder: &mut Builder<'_, '_, '_>,
    rule: &RuleIr,
    template: &Template,
    stats: &mut EnumerationStats,
) -> Result<Bindings, FormulaFailure> {
    let location = rule.location;
    let limits = builder.limits;
    let counters = &mut *builder.counters;
    let mut account = |amount: u64| charge(counters, limits, location, amount);
    rows(template, &Atoms::empty(1), &mut account, stats)
}

fn emit<'source>(
    builder: &mut Builder<'_, '_, 'source>,
    rule: &'source RuleIr,
    support: &Support<'source>,
    bindings: &Bindings,
    width: usize,
) -> Result<(), FormulaFailure> {
    let location = rule.location;
    let mut assignment = Binding::new(
        builder.computation,
        builder.limits,
        builder.counters,
        location,
    )?;
    assignment.extend_scope(
        rule.variables,
        builder.computation,
        builder.limits,
        builder.counters,
        location,
    )?;
    let mut buffer = vec![0i32; width];
    for row in bindings.row_range() {
        bindings.row_into(row, &mut buffer);
        bind_row(builder, &mut assignment, &buffer, location)?;
        builder.rule(rule, &assignment, support, None, None)?;
    }
    Ok(())
}

fn bind_row(
    builder: &mut Builder<'_, '_, '_>,
    assignment: &mut Binding<'static>,
    row: &[i32],
    location: ProgramSite,
) -> Result<(), FormulaFailure> {
    for (slot, value) in row.iter().enumerate() {
        let key = intern_number(builder, *value, location)?;
        assignment.set(slot, &key, builder.limits, builder.counters, location)?;
    }
    Ok(())
}

fn intern_number(
    builder: &mut Builder<'_, '_, '_>,
    value: i32,
    location: ProgramSite,
) -> Result<TermKey, FormulaFailure> {
    let key = builder
        .computation
        .number(value, builder.limits, builder.counters, location)?;
    // The join cursor records each generated numeric identity in the
    // GeneratedValues population before it is assigned. Keep that charge so
    // later rules, including constraints that still use the cursor, accumulate
    // against the same ceiling.
    builder
        .counters
        .generated(&key, builder.computation, builder.limits, location)?;
    Ok(key)
}

fn charge(
    counters: &mut Counters,
    limits: &FormulaLimits,
    location: ProgramSite,
    amount: u64,
) -> Result<(), FormulaFailure> {
    let mut remaining = amount;
    while remaining > 0 {
        counters.work(limits, location)?;
        remaining -= 1;
    }
    Ok(())
}

fn compile(
    rule: &RuleIr,
    builder: &mut Builder<'_, '_, '_>,
) -> Result<Option<(Template, usize)>, FormulaFailure> {
    if !matches!(rule.head, HeadIr::Normal(_)) || rule.variables != rule.body_variables {
        return Ok(None);
    }
    let width = rule.body_variables;
    let mut template = Template::new(width);
    let mut saw_domain = width == 0;
    for literal in &rule.body {
        match literal {
            LiteralIr::Range {
                target,
                lower,
                upper,
                binder: true,
            } => {
                let Some(template_next) =
                    add_range(template, *target, lower, upper, builder, rule.location)?
                else {
                    return Ok(None);
                };
                template = template_next;
                saw_domain = true;
            }
            LiteralIr::Compare(comparison) => {
                let Some(template_next) = add_comparison(
                    template,
                    comparison.left(),
                    comparison.relation(),
                    comparison.right(),
                    builder,
                    rule.location,
                )?
                else {
                    return Ok(None);
                };
                template = template_next;
            }
            LiteralIr::Atom(..)
            | LiteralIr::PatternAtom(_)
            | LiteralIr::AnonymousAtom(..)
            | LiteralIr::ArgumentCheck(_)
            | LiteralIr::TupleCompare(..)
            | LiteralIr::Guard(_)
            | LiteralIr::HeadGuard(_)
            | LiteralIr::Conditional(_)
            | LiteralIr::Aggregate(_)
            | LiteralIr::Bind { .. }
            | LiteralIr::Range { binder: false, .. } => return Ok(None),
        }
    }
    if !saw_domain {
        return Ok(None);
    }
    let template = template.with_head_slots(&(0..width).collect::<Vec<_>>());
    let template = match template {
        Ok(template) => template,
        Err(TemplateError::Slot | TemplateError::MissingHead) => return Ok(None),
    };
    // Generating Bind is the measured winner whenever an arithmetic test can
    // produce a slot (remainder, quotient, sum, product, absolute value).
    // Filter-only remains available on the Template for dual-option tests.
    let template = template.with_schedule(Schedule::Generating);
    Ok(Some((template, width)))
}

fn add_range(
    template: Template,
    target: usize,
    lower: &Expression,
    upper: &Expression,
    builder: &mut Builder<'_, '_, '_>,
    location: ProgramSite,
) -> Result<Option<Template>, FormulaFailure> {
    let Some(lo) = constant_i32(lower, builder, location)? else {
        return Ok(None);
    };
    if let Some(hi) = constant_i32(upper, builder, location)? {
        return Ok(Some(map_template(template.with_range(target, lo, hi))));
    }
    if let Some((parent, delta)) = affine(upper, builder, location)? {
        return Ok(Some(map_template(
            template.with_dependent_range(target, lo, parent, delta),
        )));
    }
    Ok(None)
}

fn add_comparison(
    template: Template,
    left: &Expression,
    relation: Relation,
    right: &Expression,
    builder: &mut Builder<'_, '_, '_>,
    location: ProgramSite,
) -> Result<Option<Template>, FormulaFailure> {
    if relation == Relation::Eq {
        if let Some(template) = arithmetic(template.clone(), left, right, builder, location)? {
            return Ok(Some(template));
        }
        if let Some(template) = arithmetic(template.clone(), right, left, builder, location)? {
            return Ok(Some(template));
        }
    }
    slot_relation(template, left, relation, right, builder, location)
}

fn arithmetic(
    template: Template,
    expression: &Expression,
    other: &Expression,
    builder: &mut Builder<'_, '_, '_>,
    location: ProgramSite,
) -> Result<Option<Template>, FormulaFailure> {
    if constant_i32(other, builder, location)? == Some(0)
        && let Some((dividend, divisor)) = remainder_of(expression)
    {
        return Ok(Some(map_template(
            template.with_remainder(dividend, divisor),
        )));
    }
    if let Some(result) = slot(other) {
        if let Some((left, right)) = binary(expression, BinaryOp::Add) {
            return Ok(Some(map_template(template.with_plus(left, right, result))));
        }
        if let Some((left, right)) = binary(expression, BinaryOp::Mul) {
            return Ok(Some(map_template(template.with_times(left, right, result))));
        }
        if let Some((dividend, divisor)) = binary(expression, BinaryOp::Div) {
            return Ok(Some(map_template(
                template.with_quotient(dividend, divisor, result),
            )));
        }
        if let Some(input) = absolute(expression) {
            return Ok(Some(map_template(template.with_abs(input, result))));
        }
    }
    Ok(None)
}

fn slot_relation(
    template: Template,
    left: &Expression,
    relation: Relation,
    right: &Expression,
    builder: &mut Builder<'_, '_, '_>,
    location: ProgramSite,
) -> Result<Option<Template>, FormulaFailure> {
    let Some(left) = slot(left) else {
        return Ok(None);
    };
    if let Some(right) = slot(right) {
        return Ok(Some(map_template(match relation {
            Relation::Eq => template.with_eq(left, right),
            Relation::Neq => template.with_ne(left, right),
            Relation::Lt => template.with_lt(left, right),
            Relation::Le => template.with_le(left, right),
            Relation::Gt => template.with_gt(left, right),
            Relation::Ge => template.with_ge(left, right),
        })));
    }
    if let Some(value) = constant_i32(right, builder, location)?
        && relation == Relation::Eq
    {
        return Ok(Some(map_template(template.with_eq_const(left, value))));
    }
    Ok(None)
}

fn remainder_of(expression: &Expression) -> Option<(usize, usize)> {
    let [
        Operation::Variable(dividend),
        Operation::Variable(divisor),
        Operation::Binary(BinaryOp::Mod, 0, 1),
    ] = expression.nodes.as_slice()
    else {
        return None;
    };
    Some((*dividend, *divisor))
}

fn binary(expression: &Expression, operator: BinaryOp) -> Option<(usize, usize)> {
    let [
        Operation::Variable(left),
        Operation::Variable(right),
        Operation::Binary(found, 0, 1),
    ] = expression.nodes.as_slice()
    else {
        return None;
    };
    (*found == operator).then_some((*left, *right))
}

fn absolute(expression: &Expression) -> Option<usize> {
    let [Operation::Variable(input), Operation::Absolute(0)] = expression.nodes.as_slice() else {
        return None;
    };
    Some(*input)
}

fn slot(expression: &Expression) -> Option<usize> {
    let [Operation::Variable(slot)] = expression.nodes.as_slice() else {
        return None;
    };
    Some(*slot)
}

fn constant_i32(
    expression: &Expression,
    builder: &mut Builder<'_, '_, '_>,
    location: ProgramSite,
) -> Result<Option<i32>, FormulaFailure> {
    let [Operation::Constant(value)] = expression.nodes.as_slice() else {
        return Ok(None);
    };
    read_scalar(*value, builder, location)
}

fn affine(
    expression: &Expression,
    builder: &mut Builder<'_, '_, '_>,
    location: ProgramSite,
) -> Result<Option<(usize, i32)>, FormulaFailure> {
    if let Some(slot) = slot(expression) {
        return Ok(Some((slot, 0)));
    }
    let nodes = expression.nodes.as_slice();
    let (parent, constant, operator) = match nodes {
        [
            Operation::Variable(parent),
            Operation::Constant(value),
            Operation::Binary(operator, 0, 1),
        ] => (*parent, *value, *operator),
        [
            Operation::Constant(value),
            Operation::Variable(parent),
            Operation::Binary(operator, 0, 1),
        ] if *operator == BinaryOp::Add => (*parent, *value, *operator),
        _ => return Ok(None),
    };
    let Some(delta) = read_scalar(constant, builder, location)? else {
        return Ok(None);
    };
    let delta = match operator {
        BinaryOp::Add => delta,
        BinaryOp::Sub => match 0i32.checked_sub(delta) {
            Some(negated) => negated,
            None => return Ok(None),
        },
        _ => return Ok(None),
    };
    Ok(Some((parent, delta)))
}

fn read_scalar(
    value: Scalar,
    builder: &mut Builder<'_, '_, '_>,
    location: ProgramSite,
) -> Result<Option<i32>, FormulaFailure> {
    let value =
        builder
            .computation
            .static_scalar(value, builder.limits, builder.counters, location)?;
    Ok(match value.descriptor() {
        ValueNodeRef::Number(number) => Some(number),
        _ => None,
    })
}

fn map_template(result: Result<Template, TemplateError>) -> Template {
    result.expect("slot indices are checked against the compiled width")
}
