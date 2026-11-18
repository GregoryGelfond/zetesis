//! Head-row morsels of one factored rule stage each instance's passing rows.
//!
//! A worker reads only the completed support. For every head row of its block
//! it fixes the head binding, joins each component under it and copies the
//! matched source row of every positive atom literal, in literal order. The
//! coordinator replays the staged instances in head order through the one
//! Builder, so every node, origin and atom identity keeps its serial order.

use std::ops::Range;

use zetesis_core::PatternRef;
use zetesis_core::TemplateTerm as Term;
use zetesis_core::catalog::AssignmentError;
use zetesis_core::relation::Row;

use super::coordinator::{self, Completed};
use super::{Binding, Buffer, Computation, Context, Counters, LiteralIr, StorageLease, Support};
use crate::expansion::Budget;
use crate::formula_ir::RuleIr;
use crate::formula_support::{ComponentPlans, ComponentScope, Executor};
use crate::{FormulaFailure, FormulaLimits, JoinStrategy, ProgramSite};
use themelios_program::program::DefaultNegation;

/// Positive atom literals one component may hold on this route.
const WIDTH: usize = 16;

/// The kept instances of one block. `shape` holds, per instance, its head
/// row's position and then each component's passing-row count; `rows` holds,
/// per passing row, its positive atom literals' matched rows in literal order.
pub(crate) struct Staged<'source> {
    pub(crate) declined: bool,
    pub(crate) shape: Buffer<usize>,
    pub(crate) rows: Buffer<Row<'source, 'source>>,
    _lease: StorageLease,
}

/// Whether every component holds only positive atoms and comparisons, with at
/// most `WIDTH` atoms each, so a staged row determines its whole conjunction.
pub(crate) fn eligible(components: &[ComponentScope<'_>]) -> bool {
    components.iter().all(|scope| {
        scope.literals.iter().all(|literal| {
            matches!(
                literal,
                LiteralIr::Atom(DefaultNegation::None, _) | LiteralIr::Compare(_)
            )
        }) && scope
            .literals
            .iter()
            .filter(|literal| matches!(literal, LiteralIr::Atom(..)))
            .count()
            <= WIDTH
    })
}

/// Head rows of `head`'s predicate in the completed support.
pub(crate) fn head_rows(support: &Support<'_>, head: PatternRef<'_>) -> usize {
    support.row_count(head.predicate())
}

/// Stage every head row's instance in parallel blocks of the fixed task
/// geometry. `None` when a worker met a row this route does not stage; the
/// caller then emits the rule on its ordinary path.
pub(crate) fn stage<'source>(
    executor: &Executor,
    (rule, head, components): (&RuleIr, PatternRef<'_>, &[ComponentScope<'_>]),
    support: &Support<'source>,
    computation: &Computation<'_, 'source>,
    budget: &mut Budget,
    (limits, counters): (&FormulaLimits, &mut Counters),
) -> Result<Option<Completed<Staged<'source>>>, FormulaFailure> {
    let location = rule.location;
    if support.strategy() == JoinStrategy::Table {
        return Ok(None);
    }
    let Some(append) = computation.pending_reader() else {
        return Ok(None);
    };
    let relations = support.relations();
    let width = head_rows(support, head);
    let completed = coordinator::run(
        executor,
        width,
        budget,
        Context::new(computation, limits, counters, location),
        |block, limits, budget, counters| {
            let support = Support::indexed(relations, limits, counters, location)?;
            let mut computation =
                Computation::borrowed(append, &support, limits, counters, location)?;
            block_instances(
                (rule, head, components),
                block,
                &support,
                &mut computation,
                budget,
                (limits, counters),
            )
        },
    )?;
    if completed.results().any(|staged| staged.declined) {
        return Ok(None);
    }
    Ok(Some(completed))
}

fn block_instances<'source>(
    (rule, head, components): (&RuleIr, PatternRef<'_>, &[ComponentScope<'_>]),
    block: Range<usize>,
    support: &Support<'source>,
    computation: &mut Computation<'_, 'source>,
    budget: &mut Budget,
    (limits, counters): (&FormulaLimits, &mut Counters),
) -> Result<Staged<'source>, FormulaFailure> {
    let location = rule.location;
    let mut lease = computation.lease();
    let header = size_of::<Staged<'source>>()
        - size_of::<Buffer<usize>>()
        - size_of::<Buffer<Row<'source, 'source>>>();
    lease.observe(header, location)?;
    computation.storage_observed(&lease, 0, header, limits, counters, location)?;
    let mut staged = Staged {
        declined: false,
        shape: Buffer::new(computation, limits, counters, location)?,
        rows: Buffer::new(computation, limits, counters, location)?,
        _lease: lease,
    };
    // Planned from the first admitted head binding, as the serial route plans.
    let mut plans = None;
    let start = block.start;
    for (offset, atom) in support
        .rows(head.predicate())
        .skip(start)
        .take(block.len())
        .enumerate()
    {
        counters.work(limits, location)?;
        let Some(fixed) = head_binding(rule, head, atom, computation, limits, counters)? else {
            continue;
        };
        if plans.is_none() {
            plans = Some(ComponentPlans::new(
                components.len(),
                rule.variables,
                &fixed,
                support,
                Context::new(&*computation, limits, counters, location),
            )?);
        }
        let plans = plans.as_mut().expect("planned above");
        let (shape, rows) = (staged.shape.len(), staged.rows.len());
        staged
            .shape
            .push(start + offset, computation, limits, counters, location)?;
        let mut inhabited = true;
        for (index, scope) in components.iter().enumerate() {
            let mut join = plans.join(
                index,
                *scope,
                &fixed,
                support,
                budget,
                Context::new(&*computation, limits, counters, location),
            )?;
            join.select_numeric_local(Context::new(&*computation, limits, counters, location))?;
            join.retain_witnesses(Context::new(&*computation, limits, counters, location))?;
            let mut passing = 0_usize;
            while let Some(row) = join.next_matched_row(
                budget,
                Context::new(computation, limits, counters, location),
            )? {
                if !row.passes {
                    continue;
                }
                let Some(witnesses) = row.witnesses.as_ref() else {
                    staged.declined = true;
                    return Ok(staged);
                };
                if !stage_row(
                    witnesses,
                    scope,
                    &mut staged.rows,
                    computation,
                    (limits, counters, location),
                )? {
                    staged.declined = true;
                    return Ok(staged);
                }
                passing += 1;
            }
            if passing == 0 {
                inhabited = false;
                break;
            }
            staged
                .shape
                .push(passing, computation, limits, counters, location)?;
        }
        if !inhabited {
            while staged.shape.len() > shape {
                staged.shape.pop();
            }
            while staged.rows.len() > rows {
                staged.rows.pop();
            }
        }
    }
    Ok(staged)
}

/// Copy one passing row's positive atom rows in literal order. False when the
/// witness does not hold exactly one row per positive atom literal.
fn stage_row<'source>(
    witnesses: &crate::formula_support::witnesses::WitnessRow<'_, 'source>,
    scope: &ComponentScope<'_>,
    rows: &mut Buffer<Row<'source, 'source>>,
    computation: &Computation<'_, 'source>,
    (limits, counters, location): (&FormulaLimits, &mut Counters, ProgramSite),
) -> Result<bool, FormulaFailure> {
    let mut atoms = 0;
    for (literal, entry) in scope.literals.iter().enumerate() {
        if !matches!(entry, LiteralIr::Atom(..)) {
            continue;
        }
        counters.work(limits, location)?;
        atoms += 1;
        let Some(depth) = witnesses.sources().position(|source| source == literal) else {
            return Ok(false);
        };
        rows.push(
            witnesses.rows[depth],
            computation,
            limits,
            counters,
            location,
        )?;
    }
    Ok(witnesses.rows.len() == atoms)
}

fn frame(
    rule: &RuleIr,
    computation: &Computation<'_, '_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
) -> Result<Binding<'static>, FormulaFailure> {
    let mut fixed = Binding::new(computation, limits, counters, rule.location)?;
    fixed.extend_scope(rule.variables, computation, limits, counters, rule.location)?;
    Ok(fixed)
}

/// The head binding of one head row, as the serial factored route fixes it;
/// `None` when a constant or repeated variable disagrees with the row.
fn head_binding(
    rule: &RuleIr,
    head: PatternRef<'_>,
    atom: Row<'_, '_>,
    computation: &Computation<'_, '_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
) -> Result<Option<Binding<'static>>, FormulaFailure> {
    let location: ProgramSite = rule.location;
    let mut fixed = frame(rule, computation, limits, counters)?;
    for column in 0..head.terms().len() {
        counters.work(limits, location)?;
        let term = head.terms().at(column).expect("checked head arity");
        let value = atom.value(column).expect("checked head arity");
        match term {
            Term::Constant(constant) => {
                if !constant.equals_ref_with(value, || counters.work(limits, location))? {
                    return Ok(None);
                }
            }
            Term::Variable(variable) => {
                if fixed.is_bound(variable, location)? {
                    let previous = fixed.read(variable, computation.read(), location)?;
                    if !previous.equals_ref_with(value, || counters.work(limits, location))? {
                        return Ok(None);
                    }
                } else {
                    counters.work(limits, location)?;
                    let key = computation.read().term_key(value).map_err(|error| {
                        crate::formula_binding::assignment(AssignmentError::Read(error), location)
                    })?;
                    fixed.set(variable, &key, limits, counters, location)?;
                }
            }
        }
    }
    Ok(Some(fixed))
}
