//! A completed objective's exact prepared cursor is partitioned only after
//! its consumer has proved that no authored per-row scalar failure is possible.
//! No binding, objective template or canonical value crosses the task boundary;
//! all tasks exhaust their rows before the numeric-presence reduction is used.

use std::ops::Range;

#[cfg(test)]
mod tests;

use zetesis_algebra::reduction::{Any, Reduction};

use super::{
    Binding, Buffer, Computation, Context, Counters, GroundingWork, Ir, Join, JoinPlan, Probe,
    StorageLease, Support, checked_body, cursor_rows, delta, order, relations, selected_rows,
};
use crate::formula_ir::ObjectiveIr;
use crate::formula_support::{
    Coverage, SupportAppend, family::Evidence, parallel_executor::Executor,
};
use crate::{FormulaFailure, FormulaLimits, JoinStrategy, ProgramSite, expansion::Budget};

struct ObjectiveRows<'cursor, 'source> {
    objective: &'source ObjectiveIr,
    plan: &'cursor order::Plan<'cursor>,
    relations: &'source relations::Relations<'source>,
    rows: Buffer<usize>,
    partition_depth: usize,
    _lease: StorageLease,
}

impl<'cursor, 'source> ObjectiveRows<'cursor, 'source> {
    fn prepare<'plan>(
        objective: &'source ObjectiveIr,
        cursor: &'cursor mut Join<'plan, 'source>,
        budget: &mut Budget,
        computation: &mut Computation<'_, 'source>,
        limits: &FormulaLimits,
        counters: &mut Counters,
    ) -> Result<Option<Self>, FormulaFailure>
    where
        'source: 'plan,
        'plan: 'cursor,
    {
        let location = objective.location;
        counters.work(limits, location)?;
        if !std::ptr::eq(cursor.literals.get(), objective.condition.literals()) {
            return Err(owner(location));
        }
        counters.work(limits, location)?;
        if !std::ptr::eq(cursor.support, computation.query_support()) {
            return Err(owner(location));
        }
        counters.work(limits, location)?;
        if cursor.support.strategy() != JoinStrategy::Indexed {
            return Ok(None);
        }
        counters.work(limits, location)?;
        if objective.condition.literals().is_empty() || cursor.bindings.is_some() {
            return Ok(None);
        }
        counters.work(limits, location)?;
        if cursor.values.len() != objective.variables || cursor.plan.pivot.is_some() {
            return Ok(None);
        }
        if !cursor_rows::initial(cursor, Coverage::Complete, limits, counters, location)?
            || !checked_body(
                objective.condition.literals(),
                objective.variables,
                computation,
                limits,
                counters,
                location,
            )?
        {
            return Ok(None);
        }
        let Some(_) = cursor.plan.patterns.first() else {
            return Ok(None);
        };
        let mut lease = computation.lease();
        let header = size_of::<Self>() - size_of::<Buffer<usize>>();
        lease.observe(header, location)?;
        computation.storage_observed(&lease, 0, header, limits, counters, location)?;
        let partition_depth =
            partition_probe(cursor, budget, computation, limits, counters, location)?;
        let rows = selected_rows(
            cursor.probes[partition_depth]
                .as_ref()
                .expect("prepared partition probe"),
            computation,
            limits,
            counters,
            location,
        )?;
        Ok(Some(Self {
            objective,
            plan: &cursor.plan,
            relations: cursor.support.relations(),
            rows,
            partition_depth,
            _lease: lease,
        }))
    }

    fn execute<F>(
        &self,
        block: Range<usize>,
        append: &SupportAppend<'source>,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
        consume: &F,
    ) -> Result<(bool, Evidence), FormulaFailure>
    where
        F: Fn(
            &mut Join<'_, 'source>,
            &Support<'source>,
            &mut Budget,
            Context<'_, &mut Computation<'_, 'source>>,
        ) -> Result<(bool, Evidence), FormulaFailure>,
    {
        let location = self.objective.location;
        let rows = self
            .rows
            .slice()
            .get(block)
            .ok_or_else(|| owner(location))?;
        let support = Support::indexed(self.relations, limits, counters, location)?;
        let mut computation = Computation::borrowed(append, &support, limits, counters, location)?;
        let prefix = Binding::new(&computation, limits, counters, location)?;
        let mut join = Join::with_plan(
            self.objective.condition.literals(),
            &prefix,
            self.objective.variables,
            &support,
            JoinPlan::Borrowed(self.plan),
            None,
            Context::new(&computation, limits, counters, location),
        )?;
        join.literals = Ir::Source(self.objective.condition.literals());
        join.evidence();
        // The original fully bound singleton prefix still runs through Join
        // in every task. Its matching, exhaustion and repeated reads are charged
        // there; only this one later probe is replaced by the assigned rows.
        let pattern = join.plan.patterns[self.partition_depth];
        join.depth = self.partition_depth;
        let resolved =
            join.resolve_probe_source(pattern, GroundingWork::new(limits, counters, location));
        join.depth = 0;
        resolved?;
        join.probes[self.partition_depth] = Some(Probe::Indexed(delta::Rows::Posting(rows)));
        consume(
            &mut join,
            &support,
            budget,
            Context::new(&mut computation, limits, counters, location),
        )
    }
}

/// Choose a later partition only across actual singleton probes whose flat
/// patterns bind no new slot and whose one candidate passes ordinary matching. Preparing a probe never advances its position;
/// the original cursor remains available on a declined or refused attempt.
fn partition_probe(
    cursor: &mut Join<'_, '_>,
    budget: &mut Budget,
    computation: &mut Computation<'_, '_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
) -> Result<usize, FormulaFailure> {
    for depth in 0..cursor.plan.patterns.len() {
        counters.work(limits, location)?;
        let pattern = cursor.plan.patterns[depth];
        // prepare_probe indexes by depth. Restore the real cursor position
        // before propagating every typed refusal, as well as on success.
        let original_depth = cursor.depth;
        cursor.depth = depth;
        let prepared = cursor.prepare_probe(
            pattern,
            Context::new(computation, limits, counters, location),
        );
        cursor.depth = original_depth;
        prepared?;
        if depth + 1 == cursor.plan.patterns.len()
            || !fully_bound(pattern, &cursor.values, limits, counters, location)?
        {
            return Ok(depth);
        }
        let probe = cursor.probes[depth]
            .as_ref()
            .expect("prepared prefix probe");
        let mut position = 0;
        counters.work(limits, location)?;
        let Some(row) = probe.next(&mut position, limits, counters, location)? else {
            return Ok(depth);
        };
        counters.work(limits, location)?;
        if probe
            .next(&mut position, limits, counters, location)?
            .is_some()
        {
            return Ok(depth);
        }
        // A singleton posting is only a candidate. Use the original matcher
        // before preparing any later relation behind a false constant test.
        let atom = cursor.resolutions[depth]
            .rows()
            .and_then(|source| source.row(row))
            .ok_or_else(|| owner(location))?;
        let original_depth = cursor.depth;
        cursor.depth = depth;
        let matched = cursor.match_row(
            pattern.pattern,
            atom,
            budget,
            Context::new(computation, limits, counters, location),
        );
        cursor.depth = original_depth;
        if !matched? {
            return Ok(depth);
        }
        // The prefix matches one original row and cannot extend the current
        // binding. Every task will replay it before reaching the next depth.
    }
    unreachable!("the admitted objective has at least one positive occurrence")
}

fn fully_bound(
    occurrence: crate::formula_support::PatternOccurrence<'_>,
    binding: &Binding<'_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
) -> Result<bool, FormulaFailure> {
    let crate::formula_support::PositivePattern::Flat(pattern) = occurrence.pattern else {
        return Ok(false);
    };
    let terms = pattern.terms();
    for column in 0..terms.len() {
        counters.work(limits, location)?;
        if let zetesis_core::TemplateTerm::Variable(slot) = terms.at(column).expect("checked arity")
            && !binding.is_bound(slot, location)?
        {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Private, statically chosen complete numeric-row consumer. This operation is
/// not available to downstream callers as a configurable source/search policy.
pub(crate) fn try_numeric<'source, F>(
    executor: &Executor,
    objective: &'source ObjectiveIr,
    cursor: &mut Join<'_, 'source>,
    budget: &mut Budget,
    context: Context<'_, &mut Computation<'_, 'source>>,
    consume: F,
) -> Result<Option<(bool, Evidence)>, FormulaFailure>
where
    F: Fn(
            &mut Join<'_, 'source>,
            &Support<'source>,
            &mut Budget,
            Context<'_, &mut Computation<'_, 'source>>,
        ) -> Result<(bool, Evidence), FormulaFailure>
        + Sync,
{
    let Context {
        computation,
        work:
            GroundingWork {
                limits,
                counters,
                location,
            },
    } = context;
    counters.work(limits, location)?;
    if computation.pending_reader().is_none() {
        return Ok(None);
    }
    let Some(rows) =
        ObjectiveRows::prepare(objective, cursor, budget, computation, limits, counters)?
    else {
        return Ok(None);
    };
    if rows.rows.len() == 0 {
        return Ok(None);
    }
    let append = computation
        .pending_reader()
        .expect("admitted immutable appender loan");
    let mut completed = super::coordinator::run(
        executor,
        rows.rows.len(),
        budget,
        Context::new(&*computation, limits, counters, location),
        |block, limits, budget, counters| {
            rows.execute(block, append, limits, budget, counters, &consume)
        },
    )?;
    Ok(Some(combine_results(
        &mut completed,
        limits,
        counters,
        location,
    )?))
}

/// Every task receipt has settled before these fixed result operations begin.
/// A refused combination returns no value to the one-template publisher.
fn combine_results(
    completed: &mut super::coordinator::Completed<(bool, Evidence)>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
) -> Result<(bool, Evidence), FormulaFailure> {
    let mut numeric = Any.unit();
    let mut evidence = Evidence::default();
    for (value, family) in completed.results_mut() {
        counters.work(limits, location)?;
        numeric = Any.combine(numeric, *value);
        evidence.merge(std::mem::take(family));
    }
    // All row reads, receipts and outcomes are complete before the result can
    // authorize retaining the one lifted template. No early absorption occurs.
    Ok((numeric, evidence))
}

fn owner(location: ProgramSite) -> FormulaFailure {
    FormulaFailure::SupportRelation {
        error: zetesis_core::relation::Failure::Owner,
        location,
    }
}
