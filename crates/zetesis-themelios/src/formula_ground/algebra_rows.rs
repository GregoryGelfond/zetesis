//! Enumerate range-and-arithmetic rules through the shared columnar operators.
//!
//! Bind, Filter, and Project live in `zetesis-algebra`. A rule whose body is
//! only ranges, residual comparisons, and invertible arithmetic is compiled
//! into that Template, enumerated under the generating schedule when a
//! generator applies (t116u: that schedule visited far fewer candidates than
//! the filter-only product in the algebra tests, with identical bindings),
//! and emitted through the ordinary `Builder::rule` path so only retained
//! rows become formula nodes. The same compilation is used during support
//! completion to derive heads, so a producer of this class does not walk
//! failing pairs on the Join cursor. A positive 1-ary atom whose predicate
//! is already in the snapshot, is not this rule's head, and holds only
//! integers is a complete lower-stratum domain (Guerin's `num(X)` form):
//! nothing is narrowed without those complete bounds, so the compiler
//! declines a missing or growing (recursive) domain. Rules the compiler
//! declines keep the Join cursor.

use std::collections::{BTreeMap, BTreeSet};

use themelios_program::program::{DefaultNegation, Relation};
use themelios_program::term::BinaryOp;
use zetesis_algebra::{Atoms, Bindings, EnumerationStats, Schedule, Template, TemplateError, rows};
use zetesis_core::TemplateTerm;
use zetesis_core::ValueNodeRef;
use zetesis_core::catalog::{PredicateRef, TermKey};

use super::Builder;
use crate::formula::ceiling;
use crate::formula_binding::Binding;
use crate::formula_ir::{Expression, HeadIr, LiteralIr, Operation, RuleIr};
use crate::formula_support::components::Scalar;
use crate::formula_support::{Computation, Counters, Support};
use crate::{FormulaFailure, FormulaLimits, FormulaResource, ProgramSite};

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
    let Some((template, width)) = compile(
        rule,
        builder.computation,
        builder.limits,
        builder.counters,
        support,
    )?
    else {
        return Ok(false);
    };
    let bindings = enumerate(&template, builder.limits, builder.counters, rule.location)?;
    emit(builder, rule, support, &bindings, width)?;
    Ok(true)
}

fn admissible(rule: &RuleIr) -> bool {
    if !matches!(rule.head, HeadIr::Normal(_)) || rule.variables != rule.body_variables {
        return false;
    }
    let mut saw_range = rule.body_variables == 0;
    let mut saw_compare = false;
    let mut saw_atom = false;
    for literal in &rule.body {
        match literal {
            LiteralIr::Range { binder: true, .. } => saw_range = true,
            LiteralIr::Compare(_) => saw_compare = true,
            LiteralIr::Atom(DefaultNegation::None, _) => saw_atom = true,
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
            | LiteralIr::Range { binder: false, .. } => return false,
        }
    }
    // Binder ranges already supply the domain. Atom facts are a complete
    // lower-stratum domain only when they replace that range, not when they
    // sit beside one: those programs keep the Join cursor so existing range
    // selection and sparse probes still run. Constraints with atom domains
    // also stay on the Join cursor: their pairwise exclusion certificates
    // are collected there, not from Bind rows.
    if saw_atom && !matches!(rule.head, HeadIr::Normal(Some(_))) {
        return false;
    }
    (saw_range && !saw_atom) || (saw_atom && saw_compare && !saw_range)
}

fn complete_atom_domains(
    rule: &RuleIr,
    support: &Support<'_>,
) -> Option<BTreeMap<usize, Vec<i32>>> {
    let mut facts: BTreeMap<usize, Vec<i32>> = BTreeMap::new();
    let mut needed = false;
    for literal in &rule.body {
        if matches!(literal, LiteralIr::Atom(DefaultNegation::None, _)) {
            needed = true;
            break;
        }
    }
    if !needed {
        return Some(facts);
    }
    let view = support.components()?;
    let head = match &rule.head {
        HeadIr::Normal(Some(pattern)) => pattern
            .resolve(view)
            .map(zetesis_core::PatternRef::predicate),
        _ => None,
    };
    for literal in &rule.body {
        let LiteralIr::Atom(DefaultNegation::None, pattern) = literal else {
            continue;
        };
        let pattern = pattern.resolve(view)?;
        if pattern.predicate().arity() != 1 {
            return None;
        }
        let Some(TemplateTerm::Variable(slot)) = pattern.terms().at(0) else {
            return None;
        };
        let predicate = pattern.predicate();
        if head.is_some_and(|head| same_predicate(head, predicate)) {
            // A recursive domain is still growing; delta variants stay on Join.
            return None;
        }
        if !published(support, predicate) {
            return None;
        }
        let values = numeric_column(support, predicate)?;
        match facts.entry(slot) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(values);
            }
            std::collections::btree_map::Entry::Occupied(mut entry) => {
                entry.get_mut().retain(|value| values.contains(value));
            }
        }
    }
    Some(facts)
}

fn published(support: &Support<'_>, predicate: PredicateRef<'_>) -> bool {
    support
        .predicates()
        .any(|found| same_predicate(found, predicate))
}

fn same_predicate(left: PredicateRef<'_>, right: PredicateRef<'_>) -> bool {
    left.sign() == right.sign() && left.name() == right.name() && left.arity() == right.arity()
}

fn numeric_column(support: &Support<'_>, predicate: PredicateRef<'_>) -> Option<Vec<i32>> {
    let mut values = Vec::new();
    for row in support.rows(predicate) {
        let term = row.value(0)?;
        match term.descriptor() {
            ValueNodeRef::Number(number) => values.push(number),
            _ => return None,
        }
    }
    Some(values)
}

/// Compile a range-and-arithmetic rule into the shared Template.
///
/// Returns `Ok(None)` when the compiler declines the body or head. Failures
/// are catalog or resource refusals from reading static scalars.
pub(crate) fn compile(
    rule: &RuleIr,
    computation: &mut Computation<'_, '_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    support: &Support<'_>,
) -> Result<Option<(Template, usize)>, FormulaFailure> {
    if !admissible(rule) {
        return Ok(None);
    }
    // Load atom domains before any scalar work so a declined incomplete
    // domain charges nothing and leaves the Join cursor's accounting intact.
    let Some(facts) = complete_atom_domains(rule, support) else {
        return Ok(None);
    };
    let width = rule.body_variables;
    // A fact column whose slot never appears in a comparison is a free
    // cross-product, the same class as a range sitting beside an atom.
    // Those bodies stay on the Join cursor so join order and pairwise
    // work remain visible. Every Guerin-style num(X), num(Y) slot is
    // mentioned in a comparison and still compiles.
    if !facts.is_empty() {
        let compared = comparison_slots(rule);
        if facts.keys().any(|slot| !compared.contains(slot)) {
            return Ok(None);
        }
    }
    let mut intern = Intern {
        computation,
        limits,
        counters,
    };
    let mut template = Template::new(width);
    let mut saw_domain = width == 0 || !facts.is_empty();
    for (slot, values) in facts {
        // AssignmentValues is the width of one domain, just as the Join
        // cursor charges an inclusive range before walking it.
        ceiling(
            FormulaResource::AssignmentValues,
            values.len() as u128,
            intern.limits.max_assignment_values as u128,
            rule.location,
        )?;
        template = match template.with_facts(slot, values) {
            Ok(template) => template,
            Err(TemplateError::Slot | TemplateError::MissingHead) => return Ok(None),
        };
    }
    for literal in &rule.body {
        match literal {
            LiteralIr::Range {
                target,
                lower,
                upper,
                binder: true,
            } => {
                let Some(template_next) =
                    add_range(template, *target, lower, upper, &mut intern, rule.location)?
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
                    &mut intern,
                    rule.location,
                )?
                else {
                    return Ok(None);
                };
                template = template_next;
            }
            LiteralIr::Atom(DefaultNegation::None, _) => {}
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

/// Enumerate `template` under the generating schedule, charging one unit of
/// formula work per operator step.
pub(crate) fn enumerate(
    template: &Template,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
) -> Result<Bindings, FormulaFailure> {
    let mut stats = EnumerationStats::default();
    let mut account = |amount: u64| charge(counters, limits, location, amount);
    rows(template, &Atoms::empty(1), &mut account, &mut stats)
}

/// Intern each slot of a retained row and assign it, charging `GeneratedValues`
/// for every numeric identity the way the Join cursor does.
pub(crate) fn bind_row(
    computation: &mut Computation<'_, '_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    assignment: &mut Binding<'static>,
    row: &[i32],
    location: ProgramSite,
) -> Result<(), FormulaFailure> {
    for (slot, value) in row.iter().enumerate() {
        let key = intern_number(computation, limits, counters, *value, location)?;
        assignment.set(slot, &key, limits, counters, location)?;
    }
    Ok(())
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
        bind_row(
            builder.computation,
            builder.limits,
            builder.counters,
            &mut assignment,
            &buffer,
            location,
        )?;
        builder.rule(rule, &assignment, support, None, None)?;
    }
    Ok(())
}

fn intern_number(
    computation: &mut Computation<'_, '_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    value: i32,
    location: ProgramSite,
) -> Result<TermKey, FormulaFailure> {
    let key = computation.number(value, limits, counters, location)?;
    // The join cursor records each generated numeric identity in the
    // GeneratedValues population before it is assigned. Keep that charge so
    // later rules, including constraints that still use the cursor, accumulate
    // against the same ceiling.
    counters.generated(&key, computation, limits, location)?;
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

struct Intern<'a, 'terms, 'source> {
    computation: &'a mut Computation<'terms, 'source>,
    limits: &'a FormulaLimits,
    counters: &'a mut Counters,
}

fn comparison_slots(rule: &RuleIr) -> BTreeSet<usize> {
    let mut slots = BTreeSet::new();
    for literal in &rule.body {
        let LiteralIr::Compare(comparison) = literal else {
            continue;
        };
        expression_slots(comparison.left(), &mut slots);
        expression_slots(comparison.right(), &mut slots);
    }
    slots
}

fn expression_slots(expression: &Expression, slots: &mut BTreeSet<usize>) {
    for operation in &expression.nodes {
        if let Operation::Variable(slot) = operation {
            slots.insert(*slot);
        }
    }
}

fn charge_assignment_range(
    lower: i32,
    upper: i32,
    limits: &FormulaLimits,
    location: ProgramSite,
) -> Result<(), FormulaFailure> {
    let Some(range) = crate::integer_range::inclusive(Some(lower), Some(upper)) else {
        return Ok(());
    };
    ceiling(
        FormulaResource::AssignmentValues,
        u128::from(crate::integer_range::width(&range)),
        limits.max_assignment_values as u128,
        location,
    )
}

fn add_range(
    template: Template,
    target: usize,
    lower: &Expression,
    upper: &Expression,
    intern: &mut Intern<'_, '_, '_>,
    location: ProgramSite,
) -> Result<Option<Template>, FormulaFailure> {
    let Some(lo) = constant_i32(lower, intern, location)? else {
        return Ok(None);
    };
    if let Some(hi) = constant_i32(upper, intern, location)? {
        charge_assignment_range(lo, hi, intern.limits, location)?;
        return Ok(Some(map_template(template.with_range(target, lo, hi))));
    }
    if let Some((parent, delta)) = affine(upper, intern, location)? {
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
    intern: &mut Intern<'_, '_, '_>,
    location: ProgramSite,
) -> Result<Option<Template>, FormulaFailure> {
    if relation == Relation::Eq {
        if let Some(template) = arithmetic(template.clone(), left, right, intern, location)? {
            return Ok(Some(template));
        }
        if let Some(template) = arithmetic(template.clone(), right, left, intern, location)? {
            return Ok(Some(template));
        }
    }
    slot_relation(template, left, relation, right, intern, location)
}

fn arithmetic(
    template: Template,
    expression: &Expression,
    other: &Expression,
    intern: &mut Intern<'_, '_, '_>,
    location: ProgramSite,
) -> Result<Option<Template>, FormulaFailure> {
    if constant_i32(other, intern, location)? == Some(0)
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
    intern: &mut Intern<'_, '_, '_>,
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
    if let Some(value) = constant_i32(right, intern, location)?
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
    intern: &mut Intern<'_, '_, '_>,
    location: ProgramSite,
) -> Result<Option<i32>, FormulaFailure> {
    let [Operation::Constant(value)] = expression.nodes.as_slice() else {
        return Ok(None);
    };
    read_scalar(*value, intern, location)
}

fn affine(
    expression: &Expression,
    intern: &mut Intern<'_, '_, '_>,
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
    let Some(delta) = read_scalar(constant, intern, location)? else {
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
    intern: &mut Intern<'_, '_, '_>,
    location: ProgramSite,
) -> Result<Option<i32>, FormulaFailure> {
    let value =
        intern
            .computation
            .static_scalar(value, intern.limits, intern.counters, location)?;
    Ok(match value.descriptor() {
        ValueNodeRef::Number(number) => Some(number),
        _ => None,
    })
}

fn map_template(result: Result<Template, TemplateError>) -> Template {
    result.expect("slot indices are checked against the compiled width")
}
