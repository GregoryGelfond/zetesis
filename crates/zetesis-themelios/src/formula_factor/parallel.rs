//! Factored emission over head-row morsels.
//!
//! Workers stage each head row's passing component rows against the completed
//! support (`row_blocks::component_rows`). This coordinator then replays the
//! instances in head order with exactly the Builder calls of the serial route:
//! the same atoms are named in the same order, so nodes, origins and producers
//! are those of `emit`. Small head relations stay on the serial route.

use zetesis_core::PatternRef;
use zetesis_ferraris::NodeView as Node;

use super::{Component, head_binding, plan};
use crate::FormulaFailure;
use crate::GroundingOperator;
use crate::formula_ground::{Builder, FALSUM, VERUM};
use crate::formula_ir::{HeadIr, LiteralIr, RuleIr};
use crate::formula_support::row_blocks::component_rows;
use crate::formula_support::row_blocks::coordinator::Completed;
use crate::formula_support::{ComponentScope, Context, Support};

/// Head rows below which a rule stays on the serial route. It depends only on
/// the data, so every thread count takes the same route.
const MIN_HEAD_ROWS: usize = 1024;

/// False when this route does not apply; nothing has then been emitted.
pub(super) fn rule<'source>(
    builder: &mut Builder<'_, '_, 'source>,
    rule: &RuleIr,
    support: &Support<'source>,
    head: PatternRef<'_>,
    components: &plan::Plan,
) -> Result<bool, FormulaFailure> {
    let Some(executor) = builder.executor else {
        return Ok(false);
    };
    let HeadIr::Normal(Some(pattern)) = rule.head else {
        return Ok(false);
    };
    let scopes: Vec<ComponentScope<'_>> =
        components.components.iter().map(Component::scope).collect();
    if !component_rows::eligible(&scopes)
        || component_rows::head_rows(support, head) < MIN_HEAD_ROWS
    {
        return Ok(false);
    }
    let location = rule.location;
    let operators = builder.counters.operators();
    let staged = crate::grounding_observer::operator(
        operators.clone(),
        GroundingOperator::EmitStage,
        &mut *builder,
        |builder| {
            component_rows::stage(
                executor,
                (rule, head, &scopes),
                support,
                builder.computation,
                builder.budget,
                (builder.limits, builder.counters),
            )
        },
    )?;
    let Some(mut completed) = staged else {
        return Ok(false);
    };
    completed.attach_reservation(Context::new(
        &*builder.computation,
        builder.limits,
        builder.counters,
        location,
    ))?;
    crate::grounding_observer::operator(
        operators,
        GroundingOperator::EmitReplay,
        &mut *builder,
        |builder| {
            replay(
                builder,
                rule,
                (head, pattern),
                components,
                support,
                &completed,
            )
        },
    )?;
    Ok(true)
}

/// Replay every staged instance in head order with the serial route's calls.
fn replay<'source>(
    builder: &mut Builder<'_, '_, 'source>,
    rule: &RuleIr,
    (head, pattern): (PatternRef<'_>, crate::formula_support::components::Pattern),
    components: &plan::Plan,
    support: &Support<'source>,
    completed: &Completed<component_rows::Staged<'source>>,
) -> Result<(), FormulaFailure> {
    let location = rule.location;
    let mut heads = support.rows(head.predicate()).enumerate();
    for staged in completed.results() {
        let shape = staged.shape.slice();
        let rows = staged.rows.slice();
        let (mut at, mut row) = (0, 0);
        while at < shape.len() {
            let position = shape[at];
            at += 1;
            let atom = loop {
                let (index, atom) = heads.next().ok_or_else(|| owner(location))?;
                if index == position {
                    break atom;
                }
            };
            builder.work(location)?;
            let fixed = head_binding(builder, rule, head, atom)?.ok_or_else(|| owner(location))?;
            let mut body = VERUM;
            let mut dropped = false;
            for component in &components.components {
                let passing = shape[at];
                at += 1;
                let width = component
                    .literals
                    .iter()
                    .filter(|literal| matches!(literal, LiteralIr::Atom(..)))
                    .count();
                if dropped {
                    row += passing * width;
                    continue;
                }
                let mut alternatives = FALSUM;
                for _ in 0..passing {
                    let mut conjunction = VERUM;
                    for literal in &component.literals {
                        if matches!(literal, LiteralIr::Atom(..)) {
                            let node = builder.matched_atom(rows[row], location)?;
                            row += 1;
                            let node = builder.fixed_body_atom(node, location)?;
                            conjunction = builder.and(conjunction, node, location)?;
                        }
                    }
                    alternatives = builder.or(alternatives, conjunction, location)?;
                }
                if alternatives == FALSUM {
                    dropped = true;
                    continue;
                }
                body = builder.and(body, alternatives, location)?;
            }
            if dropped || body == FALSUM {
                continue;
            }
            let head = builder.atom(pattern, &fixed, location)?;
            let formula = builder.node(Node::Implies(body, head), location)?;
            builder.root(formula, rule)?;
            builder.ordinary_producer(head, body, rule)?;
        }
        if row != rows.len() {
            return Err(owner(location));
        }
    }
    Ok(())
}

fn owner(location: crate::ProgramSite) -> FormulaFailure {
    FormulaFailure::SupportRelation {
        error: zetesis_core::relation::Failure::Owner,
        location,
    }
}
