//! Arithmetic certification for existential components over fixed head rows.
//!
//! A component may check a captured value against arithmetic whose complete
//! inputs belong to the head binding. Before publishing any root, evaluate
//! those expressions on every matching row in completed possible support.
//! Other producers can contribute unrelated or nonnumeric head rows: any
//! partial evaluation declines this optimization, never source admission.
//! Authoritative family diagnostics have already run before formula emission.
//!
//! Certification retains one reusable evaluation stack and one head frame,
//! not a row cache. The existing evaluator and frame account actual storage,
//! work and interruption. Successful certification proves defined values, not
//! body truth. Component joins still enumerate all their independent witnesses;
//! fixed assignments are equalities, and every emitted body remains a producer.

#[cfg(test)]
pub(super) mod testing;
#[cfg(test)]
mod tests;

use zetesis_core::PatternRef;
use zetesis_core::catalog::AssignmentError;

use crate::formula_binding::Binding;
use crate::formula_ground::Builder;
use crate::formula_ir::{Expression, LiteralIr, Operation, RuleIr};
use crate::formula_support::{Evaluation, Support};
use crate::{ExpansionFailure, FormulaFailure};

/// Pure scalar arithmetic may only consume already fixed head coordinates.
/// Structural constructors and local arithmetic generators retain the complete
/// original join. Checked evaluation, not this syntactic test, proves totality.
pub(super) fn fixed_expression(
    builder: &mut Builder<'_, '_, '_>,
    rule: &RuleIr,
    expression: &Expression,
    fixed: &[bool],
) -> Result<bool, FormulaFailure> {
    for operation in &expression.nodes {
        builder.work(rule.location)?;
        match operation {
            Operation::Variable(variable) if !fixed[*variable] => return Ok(false),
            Operation::Constructor(_) => return Ok(false),
            _ => {}
        }
    }
    Ok(true)
}

/// Complete the whole head carrier before the first root or producer is emitted.
pub(super) fn certify<'source>(
    builder: &mut Builder<'_, '_, 'source>,
    rule: &RuleIr,
    support: &Support<'source>,
    head: Option<PatternRef<'_>>,
    evaluation: &mut Evaluation,
) -> Result<bool, FormulaFailure> {
    if let Some(head) = head {
        for atom in support.rows(head.predicate()) {
            builder.work(rule.location)?;
            if let Some(fixed) = super::head_binding(builder, rule, head, atom)?
                && !certify_binding(builder, rule, &fixed, evaluation)?
            {
                return Ok(false);
            }
        }
    } else {
        let fixed = super::fixed_frame(builder, rule)?;
        if !certify_binding(builder, rule, &fixed, evaluation)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn certify_binding(
    builder: &mut Builder<'_, '_, '_>,
    rule: &RuleIr,
    fixed: &Binding<'_>,
    evaluation: &mut Evaluation,
) -> Result<bool, FormulaFailure> {
    #[cfg(test)]
    testing::record(testing::Point::Head);
    for literal in &rule.body {
        builder.work(rule.location)?;
        match literal {
            LiteralIr::Compare(comparison) | LiteralIr::ArgumentCheck(comparison) => {
                for expression in [comparison.left(), comparison.right()] {
                    if !defined(builder, rule, expression, fixed, evaluation)? {
                        return Ok(false);
                    }
                }
            }
            LiteralIr::TupleCompare(left, _, right) => {
                for expression in left.iter().chain(right) {
                    if !defined(builder, rule, expression, fixed, evaluation)? {
                        return Ok(false);
                    }
                }
            }
            LiteralIr::Bind { value, .. } if !defined(builder, rule, value, fixed, evaluation)? => {
                return Ok(false);
            }
            _ => {}
        }
    }
    Ok(true)
}

fn defined(
    builder: &mut Builder<'_, '_, '_>,
    rule: &RuleIr,
    expression: &Expression,
    fixed: &Binding<'_>,
    evaluation: &mut Evaluation,
) -> Result<bool, FormulaFailure> {
    // Scalar leaves may denote local captured columns. Their ordinary join
    // supplies them; only non-leaf arithmetic needed a fixed-input certificate.
    if expression.nodes.len() == 1 {
        return Ok(true);
    }
    match evaluation.expression(
        expression,
        |variable| fixed.key(variable, rule.location),
        builder.computation,
        builder.limits,
        builder.counters,
        rule.location,
    ) {
        Ok(_) => Ok(true),
        Err(FormulaFailure::Expansion(ExpansionFailure::Evaluation { .. })) => Ok(false),
        Err(failure) => Err(failure),
    }
}

/// A generated head coordinate remains an equality with the completed row.
/// Never overwrite a fixed coordinate with a newly computed value: other
/// producers may have placed an incompatible row in the same head relation.
pub(super) fn matches(
    builder: &mut Builder<'_, '_, '_>,
    rule: &RuleIr,
    fixed: &Binding<'_>,
    evaluation: &mut Evaluation,
) -> Result<bool, FormulaFailure> {
    for literal in &rule.body {
        builder.work(rule.location)?;
        if let LiteralIr::Bind { target, value } = literal {
            let value = evaluation.expression(
                value,
                |variable| fixed.key(variable, rule.location),
                builder.computation,
                builder.limits,
                builder.counters,
                rule.location,
            )?;
            let read = builder.computation.read();
            let expected = fixed.read(*target, read, rule.location)?;
            let value = read.term(&value).map_err(|error| {
                crate::formula_binding::assignment(AssignmentError::Read(error), rule.location)
            })?;
            if !expected.equals_ref_with(value, || {
                builder.counters.work(builder.limits, rule.location)
            })? {
                return Ok(false);
            }
        }
    }
    Ok(true)
}
