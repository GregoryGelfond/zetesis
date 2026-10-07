//! Factor complete positive witness runs before expanding their common proposals.

use crate::FormulaFailure;
use crate::formula_binding::Binding;
use crate::formula_ground::{Builder, FALSUM};
use crate::formula_ir::{HeadIr, LiteralIr, RuleIr};
use crate::formula_support::{Context, Continuations, Join, Row, Support};
use themelios_program::program::DefaultNegation;

pub(crate) fn rule<'a>(
    builder: &mut Builder<'_, '_, '_>,
    rule: &'a RuleIr,
    join: &mut Join<'a, '_>,
    support: &Support<'_>,
) -> Result<bool, FormulaFailure> {
    #[cfg(test)]
    if !super::testing::enabled() {
        return Ok(false);
    }
    let Some(mut rows) = Continuations::new(
        join,
        rule,
        Context::new(
            &*builder.computation,
            builder.limits,
            &mut builder.counters,
            rule.location,
        ),
    )?
    else {
        return Ok(false);
    };
    while rows.start(
        builder.budget,
        Context::new(
            &mut *builder.computation,
            builder.limits,
            &mut builder.counters,
            rule.location,
        ),
    )? {
        #[cfg(test)]
        super::testing::record(super::testing::Point::Run);
        let selected = next_selected(builder, rule, support, &mut rows)?;
        let Some(selected) = selected else {
            // A completed empty/rejected family introduces no logical atom.
            while rows
                .witness(
                    builder.budget,
                    Context::new(
                        &mut *builder.computation,
                        builder.limits,
                        &mut builder.counters,
                        rule.location,
                    ),
                )?
                .is_some()
            {}
            continue;
        };
        let mut alternatives = witness(builder, rule, support, &selected.values)?;
        while let Some(binding) = rows.witness(
            builder.budget,
            Context::new(
                &mut *builder.computation,
                builder.limits,
                &mut builder.counters,
                rule.location,
            ),
        )? {
            let conjunction = witness(builder, rule, support, &binding)?;
            alternatives = builder.or(alternatives, conjunction, rule.location)?;
        }
        emit(builder, rule, support, &selected.values, alternatives)?;
        drop(selected);
        while let Some(selected) = next_selected(builder, rule, support, &mut rows)? {
            emit(builder, rule, support, &selected.values, alternatives)?;
        }
    }
    Ok(true)
}

/// A selected row has completed body selection and head arithmetic. Its logical
/// aggregate and negative conditions are still formulas, not a truth certificate.
fn next_selected(
    builder: &mut Builder<'_, '_, '_>,
    rule: &RuleIr,
    support: &Support<'_>,
    rows: &mut Continuations<'_, '_, '_>,
) -> Result<Option<Row<'static>>, FormulaFailure> {
    while let Some(row) = rows.next(
        builder.budget,
        Context::new(
            &mut *builder.computation,
            builder.limits,
            &mut builder.counters,
            rule.location,
        ),
    )? {
        if row.passes {
            return Ok(Some(row));
        }
        builder.validate_body(&rule.body, &row.values, support, rule.location)?;
    }
    Ok(None)
}

fn positive(literal: &LiteralIr) -> bool {
    matches!(literal, LiteralIr::Atom(DefaultNegation::None, _))
}

fn witness(
    builder: &mut Builder<'_, '_, '_>,
    rule: &RuleIr,
    support: &Support<'_>,
    binding: &Binding<'_>,
) -> Result<usize, FormulaFailure> {
    #[cfg(test)]
    super::testing::record(super::testing::Point::Witness);
    builder.body(
        rule.body.iter().filter(|literal| positive(literal)),
        &rule.body_binding(binding),
        rule.location,
        support,
    )
}

fn emit(
    builder: &mut Builder<'_, '_, '_>,
    rule: &RuleIr,
    support: &Support<'_>,
    binding: &Binding<'_>,
    alternatives: usize,
) -> Result<(), FormulaFailure> {
    #[cfg(test)]
    super::testing::record(super::testing::Point::Continuation);
    builder.work(rule.location)?;
    let common = builder.body(
        rule.body.iter().filter(|literal| !positive(literal)),
        &rule.body_binding(binding),
        rule.location,
        support,
    )?;
    let body = builder.and(alternatives, common, rule.location)?;
    if body != FALSUM {
        let HeadIr::Normal(head) = &rule.head else {
            unreachable!("checked ordinary continuation")
        };
        builder.normal(*head, rule, binding, body)?;
    }
    Ok(())
}
