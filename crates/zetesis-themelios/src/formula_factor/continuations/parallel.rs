//! Stage the continuation base join; family evaluation stays on the owner.

use super::{emit, next_selected, witness};
use crate::FormulaFailure;
use crate::formula_binding::Binding;
use crate::formula_ground::Builder;
use crate::formula_ir::RuleIr;
use crate::formula_support::{
    Buffer, Context, Continuations, Executor, Join, Support,
    row_blocks::{
        coordinator,
        cursor_rows::CursorRows,
        witness_rows::{Replay, RowBinding, WitnessPlan},
    },
};

pub(super) fn rule<'source>(
    builder: &mut Builder<'_, '_, 'source>,
    rule: &'source RuleIr,
    join: &mut Join<'_, 'source>,
    support: &Support<'source>,
    executor: &Executor,
) -> Result<bool, FormulaFailure> {
    let Some(inputs) = rule
        .bindings
        .as_ref()
        .and_then(|plan| plan.continuation_inputs.as_deref())
    else {
        return Ok(false);
    };
    let Some(bases) = stage_bases(builder, rule, join, support, executor)? else {
        return Ok(false);
    };
    let Some(mut rows) = Continuations::new(
        join,
        rule,
        Context::new(
            &*builder.computation,
            builder.limits,
            builder.counters,
            rule.location,
        ),
    )?
    else {
        return Ok(false);
    };
    emit_runs(builder, rule, support, &mut rows, inputs, bases.slice())?;
    Ok(true)
}

fn stage_bases<'source>(
    builder: &mut Builder<'_, '_, 'source>,
    rule: &'source RuleIr,
    join: &mut Join<'_, 'source>,
    support: &Support<'source>,
    executor: &Executor,
) -> Result<Option<Buffer<Binding<'static>>>, FormulaFailure> {
    let Some(prepared) = CursorRows::prepare_continuation(
        rule,
        join,
        builder.computation,
        builder.limits,
        builder.counters,
    )?
    else {
        return Ok(None);
    };
    let plan = WitnessPlan::for_continuations(
        prepared.descriptor(),
        support,
        builder.computation,
        builder.limits,
        builder.counters,
    )?;
    let Some(mut completed) = coordinator::stage_emission_witnesses(
        executor,
        &plan,
        support,
        builder.budget,
        Context::new(
            builder.computation,
            builder.limits,
            builder.counters,
            rule.location,
        ),
    )?
    else {
        return Ok(None);
    };
    completed.attach_reservation(Context::new(
        &*builder.computation,
        builder.limits,
        builder.counters,
        rule.location,
    ))?;
    Ok(Some(collect_bindings(
        builder,
        support,
        &plan,
        &mut completed,
    )?))
}

fn collect_bindings<'plan, 'source: 'plan>(
    builder: &mut Builder<'_, '_, 'source>,
    support: &Support<'source>,
    plan: &'plan WitnessPlan<'plan, 'source>,
    completed: &mut coordinator::Completed<
        crate::formula_support::row_blocks::witness_rows::Batch<'plan, 'source>,
    >,
) -> Result<Buffer<Binding<'static>>, FormulaFailure> {
    let location = rule_location(plan);
    let mut replay = Replay::new(
        plan,
        support,
        Context::new(
            &*builder.computation,
            builder.limits,
            builder.counters,
            location,
        ),
    )?;
    let mut bases = Buffer::new(
        builder.computation,
        builder.limits,
        builder.counters,
        location,
    )?;
    let mut reader = None;
    for batch in completed.drain() {
        let mut selected = replay.batch(&batch, builder.limits, builder.counters)?;
        while let Some(row) = selected.next(Context::new(
            &*builder.computation,
            builder.limits,
            builder.counters,
            location,
        ))? {
            let values = if let Some(reader) = reader.as_mut() {
                RowBinding::read(
                    reader,
                    &row,
                    Context::new(
                        &*builder.computation,
                        builder.limits,
                        builder.counters,
                        location,
                    ),
                )?
            } else {
                let built = RowBinding::new(
                    plan,
                    &row,
                    Context::new(
                        &*builder.computation,
                        builder.limits,
                        builder.counters,
                        location,
                    ),
                )?;
                reader = Some(built);
                let reader = reader.as_mut().expect("just stored");
                RowBinding::read(
                    reader,
                    &row,
                    Context::new(
                        &*builder.computation,
                        builder.limits,
                        builder.counters,
                        location,
                    ),
                )?
            };
            let copied = values.copied(
                builder.computation,
                builder.limits,
                builder.counters,
                location,
            )?;
            bases.push(
                copied,
                builder.computation,
                builder.limits,
                builder.counters,
                location,
            )?;
        }
    }
    Ok(bases)
}

fn rule_location(plan: &WitnessPlan<'_, '_>) -> crate::ProgramSite {
    plan.location()
}

fn emit_runs<'source>(
    builder: &mut Builder<'_, '_, 'source>,
    rule: &'source RuleIr,
    support: &Support<'source>,
    rows: &mut Continuations<'_, '_, 'source>,
    inputs: &[usize],
    bases: &[Binding<'static>],
) -> Result<(), FormulaFailure> {
    let mut index = 0;
    while index < bases.len() {
        let start = index;
        index += 1;
        while index < bases.len()
            && Continuations::equal_inputs(
                &bases[start],
                &bases[index],
                inputs,
                builder.limits,
                builder.counters,
                rule.location,
            )?
        {
            index += 1;
        }
        emit_run(
            builder,
            rule,
            support,
            rows,
            &bases[start],
            &bases[start + 1..index],
        )?;
    }
    Ok(())
}

fn emit_run<'source>(
    builder: &mut Builder<'_, '_, 'source>,
    rule: &'source RuleIr,
    support: &Support<'source>,
    rows: &mut Continuations<'_, '_, 'source>,
    start: &Binding<'static>,
    extras: &[Binding<'static>],
) -> Result<(), FormulaFailure> {
    #[cfg(test)]
    super::super::testing::record(super::super::testing::Point::Run);
    rows.start_from(
        start,
        Context::new(
            &*builder.computation,
            builder.limits,
            builder.counters,
            rule.location,
        ),
    )?;
    let selected = next_selected(builder, rule, support, rows)?;
    let Some(selected) = selected else {
        return Ok(());
    };
    let mut alternatives = witness(builder, rule, support, &selected.values)?;
    for extra in extras {
        let conjunction = witness(builder, rule, support, extra)?;
        alternatives = builder.or(alternatives, conjunction, rule.location)?;
    }
    emit(
        builder,
        rule,
        support,
        &selected.values,
        alternatives,
        rows.carriers(),
    )?;
    drop(selected);
    while let Some(selected) = next_selected(builder, rule, support, rows)? {
        emit(
            builder,
            rule,
            support,
            &selected.values,
            alternatives,
            rows.carriers(),
        )?;
    }
    Ok(())
}
