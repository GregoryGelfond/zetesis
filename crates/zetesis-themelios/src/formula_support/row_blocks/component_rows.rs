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
use zetesis_core::relation::Row;

use super::coordinator::{self, Completed};
use super::{Binding, Buffer, Computation, Context, Counters, LiteralIr, StorageLease, Support};
use crate::expansion::Budget;
use crate::formula_ir::RuleIr;
use crate::formula_support::{ComponentPlans, ComponentScope, Executor, Join};
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
    // One head frame per block, rebound row by row, so no row takes its own
    // storage lease or vocabulary handle.
    let mut fixed = frame(rule, computation, limits, counters)?;
    let start = block.start;
    let mut atoms = support
        .rows(head.predicate())
        .skip(start)
        .take(block.len())
        .enumerate();
    let mut next = |fixed: &mut Binding<'static>,
                    computation: &Computation<'_, 'source>,
                    counters: &mut Counters|
     -> Result<Option<usize>, FormulaFailure> {
        for (offset, atom) in atoms.by_ref() {
            counters.work(limits, location)?;
            if head_binding(rule, head, atom, fixed, computation, limits, counters)? {
                return Ok(Some(start + offset));
            }
        }
        Ok(None)
    };
    let Some(mut row) = next(&mut fixed, computation, counters)? else {
        return Ok(staged);
    };
    // Planned from the first admitted head binding, as the serial route plans;
    // every component is planned once here so the block's joins can borrow
    // the plans together.
    let mut plans = ComponentPlans::new(
        components.len(),
        rule.variables,
        &fixed,
        support,
        Context::new(&*computation, limits, counters, location),
    )?;
    for (index, scope) in components.iter().enumerate() {
        plans.prepare(
            index,
            *scope,
            budget,
            Context::new(&*computation, limits, counters, location),
        )?;
    }
    let plans = plans;
    // One join per component, lent on its first head row and rebound after.
    let mut joins = Buffer::new(computation, limits, counters, location)?;
    for _ in components {
        joins.push(None, computation, limits, counters, location)?;
    }
    loop {
        if stage_head_row(
            row,
            (components, &plans, &mut joins),
            (&fixed, support),
            &mut staged,
            computation,
            budget,
            (limits, counters, location),
        )? {
            staged.declined = true;
            return Ok(staged);
        }
        let Some(following) = next(&mut fixed, computation, counters)? else {
            break;
        };
        row = following;
    }
    Ok(staged)
}

/// Stage one admitted head row's component rows through the block's joins.
/// True when a worker met a row this route does not stage.
fn stage_head_row<'plans, 'source>(
    row: usize,
    (components, plans, joins): (
        &[ComponentScope<'plans>],
        &'plans ComponentPlans<'plans, 'source>,
        &mut Buffer<Option<Join<'plans, 'source>>>,
    ),
    (fixed, support): (&Binding<'static>, &'plans Support<'source>),
    staged: &mut Staged<'source>,
    computation: &mut Computation<'_, 'source>,
    budget: &mut Budget,
    (limits, counters, location): (&FormulaLimits, &mut Counters, ProgramSite),
) -> Result<bool, FormulaFailure> {
    let (shape, rows) = (staged.shape.len(), staged.rows.len());
    staged
        .shape
        .push(row, computation, limits, counters, location)?;
    let mut inhabited = true;
    for (index, scope) in components.iter().enumerate() {
        let slot = &mut joins.slice_mut()[index];
        if let Some(join) = slot.as_mut() {
            plans.rejoin(
                join,
                index,
                *scope,
                fixed,
                support,
                Context::new(&*computation, limits, counters, location),
            )?;
        } else {
            *slot = Some(plans.join_prepared(
                index,
                *scope,
                fixed,
                support,
                Context::new(&*computation, limits, counters, location),
            )?);
        }
        let join = slot.as_mut().expect("lent above");
        join.select_numeric_local(Context::new(&*computation, limits, counters, location))?;
        join.retain_witnesses(Context::new(&*computation, limits, counters, location))?;
        let mut passing = 0_usize;
        while let Some(matched) = join.next_matched_row(
            budget,
            Context::new(computation, limits, counters, location),
        )? {
            if !matched.passes {
                continue;
            }
            let Some(witnesses) = matched.witnesses.as_ref() else {
                return Ok(true);
            };
            if !stage_row(
                witnesses,
                scope,
                &mut staged.rows,
                computation,
                (limits, counters, location),
            )? {
                return Ok(true);
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
    Ok(false)
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

/// Rebind `fixed` to one head row, as the serial factored route fixes it;
/// false when a constant or repeated variable disagrees with the row. Only
/// head variables are ever bound in `fixed`, so clearing them first leaves the
/// frame exactly as a fresh one.
fn head_binding(
    rule: &RuleIr,
    head: PatternRef<'_>,
    atom: Row<'_, '_>,
    fixed: &mut Binding<'static>,
    computation: &Computation<'_, '_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
) -> Result<bool, FormulaFailure> {
    let location: ProgramSite = rule.location;
    for column in 0..head.terms().len() {
        if let Some(Term::Variable(variable)) = head.terms().at(column) {
            fixed.clear(variable, limits, counters, location)?;
        }
    }
    for column in 0..head.terms().len() {
        counters.work(limits, location)?;
        let term = head.terms().at(column).expect("checked head arity");
        let value = atom.value(column).expect("checked head arity");
        match term {
            Term::Constant(constant) => {
                if !constant.equals_ref_with(value, || counters.work(limits, location))? {
                    return Ok(false);
                }
            }
            Term::Variable(variable) => {
                if fixed.is_bound(variable, location)? {
                    let previous = fixed.read(variable, computation.read(), location)?;
                    if !previous.equals_ref_with(value, || counters.work(limits, location))? {
                        return Ok(false);
                    }
                } else {
                    counters.work(limits, location)?;
                    fixed.set_term_with(variable, value, computation.read(), location, || {
                        counters.work(limits, location)
                    })?;
                }
            }
        }
    }
    Ok(true)
}
