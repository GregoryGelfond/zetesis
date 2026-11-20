//! Formula factoring over completed possible support.
//!
//! Independent body components share fixed head bindings; complete positive
//! witness runs share their common continuations. Independent fixed-input
//! arithmetic is certified over the complete head carrier before emission.
//! Both routes retain all witness alternatives and source provenance.

mod arithmetic;
mod continuations;
mod parallel;
mod plan;
pub(super) use continuations::rule as continuations;
#[cfg(test)]
pub(crate) mod testing;
#[cfg(test)]
mod witnesses;

use zetesis_core::catalog::AssignmentError;
use zetesis_core::{PatternRef, TemplateTerm as Term};
use zetesis_ferraris::NodeView as Node;

use crate::FormulaFailure;
use crate::formula_binding::Binding;
use crate::formula_ground::{Builder, FALSUM, VERUM};
use crate::formula_ir::{HeadIr, LiteralIr, RuleIr};
use crate::formula_support::{ComponentPlans, ComponentScope, Context, Join, Support};

/// Return false only when the original complete-join path should be used.
pub(super) fn rule<'source>(
    builder: &mut Builder<'_, '_, 'source>,
    rule: &RuleIr,
    support: &Support<'source>,
) -> Result<bool, FormulaFailure> {
    let HeadIr::Normal(head) = &rule.head else {
        return Ok(false);
    };
    let head = head
        .map(|head| {
            builder.computation.static_pattern(
                head,
                builder.limits,
                builder.counters,
                rule.location,
            )
        })
        .transpose()?;
    let Some(components) = plan::components(builder, rule, head)? else {
        return Ok(false);
    };
    let mut evaluation = crate::formula_support::Evaluation::default();
    if components.arithmetic && !arithmetic::certify(builder, rule, support, head, &mut evaluation)?
    {
        return Ok(false);
    }
    #[cfg(test)]
    if components.arithmetic {
        arithmetic::testing::record(arithmetic::testing::Point::Certified);
    }
    if let Some(head) = head
        && !components.arithmetic
        && parallel::rule(builder, rule, support, head, &components)?
    {
        return Ok(true);
    }
    let mut prepared = None;
    if let Some(head) = head {
        // Every possible firing's head occurs in completed U. Other producers
        // may add extra rows, which this rule's complete component joins filter.
        for atom in support.rows(head.predicate()) {
            builder.work(rule.location)?;
            if let Some(fixed) = head_binding(builder, rule, head, atom)?
                && (!components.arithmetic
                    || arithmetic::matches(builder, rule, &fixed, &mut evaluation)?)
            {
                emit(builder, rule, support, &components, &mut prepared, &fixed)?;
            }
        }
    } else {
        let fixed = fixed_frame(builder, rule)?;
        if !components.arithmetic || arithmetic::matches(builder, rule, &fixed, &mut evaluation)? {
            emit(builder, rule, support, &components, &mut prepared, &fixed)?;
        }
    }
    Ok(true)
}

struct Component {
    literals: Vec<LiteralIr>,
    /// Sorted, distinct slots read by these literals, including fixed head slots.
    used: Vec<usize>,
}

/// How a component cursor reaches the rule's prepared component plans.
enum Plans<'cursor, 'plan, 'source> {
    /// Component plans are disabled (tests): each cursor owns its plan.
    Owned,
    /// Plan the component on its first traversal.
    Lazy(&'cursor mut ComponentPlans<'plan, 'source>),
    /// The component is already planned; other cursors may share the owner.
    Shared(&'cursor ComponentPlans<'plan, 'source>),
}

impl<'plan, 'source> Plans<'_, 'plan, 'source> {
    fn reborrow(&mut self) -> Plans<'_, 'plan, 'source> {
        match self {
            Self::Owned => Plans::Owned,
            Self::Lazy(plans) => Plans::Lazy(plans),
            Self::Shared(plans) => Plans::Shared(plans),
        }
    }
}

impl Component {
    fn join<'plan: 'cursor, 'cursor, 'source>(
        &'plan self,
        builder: &mut Builder<'_, '_, 'source>,
        rule: &RuleIr,
        support: &'plan Support<'source>,
        fixed: &Binding<'_>,
        index: usize,
        plans: Plans<'cursor, 'plan, 'source>,
    ) -> Result<Join<'cursor, 'source>, FormulaFailure> {
        let scope = ComponentScope {
            literals: &self.literals,
            used: &self.used,
        };
        let context = Context::new(
            &*builder.computation,
            builder.limits,
            builder.counters,
            rule.location,
        );
        let mut join = match plans {
            Plans::Lazy(prepared) => {
                prepared.join(index, scope, fixed, support, builder.budget, context)?
            }
            Plans::Shared(prepared) => {
                prepared.join_prepared(index, scope, fixed, support, context)?
            }
            Plans::Owned => Join::component(
                &self.literals,
                rule.variables,
                &self.used,
                fixed,
                support,
                builder.budget,
                context,
            )?,
        };
        // Both constructors preserve the same plan and install the current
        // fixed head values. The local selector certifies only the scalar
        // conditions of this component; it grants no whole-rule coverage.
        join.select_numeric_local(Context::new(
            &*builder.computation,
            builder.limits,
            builder.counters,
            rule.location,
        ))?;
        Ok(join)
    }

    fn scope(&self) -> ComponentScope<'_> {
        ComponentScope {
            literals: &self.literals,
            used: &self.used,
        }
    }
}

fn head_binding(
    builder: &mut Builder<'_, '_, '_>,
    rule: &RuleIr,
    head: PatternRef<'_>,
    atom: zetesis_core::relation::Row<'_, '_>,
) -> Result<Option<Binding<'static>>, FormulaFailure> {
    let mut fixed = fixed_frame(builder, rule)?;
    for column in 0..head.terms().len() {
        builder.work(rule.location)?;
        let term = head.terms().at(column).expect("checked head arity");
        let value = atom.value(column).expect("checked head arity");
        match term {
            Term::Constant(constant) => {
                if !constant.equals_ref_with(value, || {
                    builder.counters.work(builder.limits, rule.location)
                })? {
                    return Ok(None);
                }
            }
            Term::Variable(variable) => {
                if fixed.is_bound(variable, rule.location)? {
                    let previous =
                        fixed.read(variable, builder.computation.read(), rule.location)?;
                    if !previous.equals_ref_with(value, || {
                        builder.counters.work(builder.limits, rule.location)
                    })? {
                        return Ok(None);
                    }
                } else {
                    builder.work(rule.location)?;
                    let key = builder
                        .computation
                        .read()
                        .term_key(value)
                        .map_err(|error| {
                            crate::formula_binding::assignment(
                                AssignmentError::Read(error),
                                rule.location,
                            )
                        })?;
                    fixed.set(
                        variable,
                        &key,
                        builder.limits,
                        builder.counters,
                        rule.location,
                    )?;
                }
            }
        }
    }
    Ok(Some(fixed))
}

fn fixed_frame(
    builder: &mut Builder<'_, '_, '_>,
    rule: &RuleIr,
) -> Result<Binding<'static>, FormulaFailure> {
    let mut fixed = Binding::new(
        builder.computation,
        builder.limits,
        builder.counters,
        rule.location,
    )?;
    fixed.extend_scope(
        rule.variables,
        builder.computation,
        builder.limits,
        builder.counters,
        rule.location,
    )?;
    Ok(fixed)
}

fn emit<'plan, 'source>(
    builder: &mut Builder<'_, '_, 'source>,
    rule: &RuleIr,
    support: &'plan Support<'source>,
    components: &'plan plan::Plan,
    prepared: &mut Option<ComponentPlans<'plan, 'source>>,
    fixed: &Binding,
) -> Result<(), FormulaFailure> {
    #[cfg(test)]
    let enabled = crate::formula_support::component_plan::testing::enabled();
    #[cfg(not(test))]
    let enabled = true;
    if enabled && prepared.is_none() {
        *prepared = Some(ComponentPlans::new(
            components.components.len(),
            rule.variables,
            fixed,
            support,
            Context::new(
                &*builder.computation,
                builder.limits,
                builder.counters,
                rule.location,
            ),
        )?);
    }
    // A rule with a head stops at its first component without an alternative,
    // before that component or any later one names an atom, so only the
    // components after the first need a witness in advance. A constraint
    // names every component's atoms, so all of its components need one.
    let first = usize::from(!matches!(rule.head, HeadIr::Normal(None)));
    if let Some(plans) = prepared.as_mut() {
        for (index, component) in components.components.iter().enumerate().skip(first) {
            plans.prepare(
                index,
                component.scope(),
                builder.budget,
                Context::new(
                    &*builder.computation,
                    builder.limits,
                    builder.counters,
                    rule.location,
                ),
            )?;
        }
    }
    // Components before `first` are planned on their first emission, as
    // before; until then the witnesses' cursors cannot share the owner with
    // that planning and are rebuilt for emission.
    let ready = prepared
        .as_ref()
        .is_none_or(|plans| (0..first).all(|index| plans.is_prepared(index)));
    let shared = || prepared.as_ref().map_or(Plans::Owned, Plans::Shared);
    let Some(mut held) = inhabited(builder, rule, support, components, shared, fixed, first)?
    else {
        return Ok(());
    };
    if !ready {
        drop(held);
        return body(
            builder,
            rule,
            support,
            components,
            prepared.as_mut().map_or(Plans::Owned, Plans::Lazy),
            fixed,
            &mut Vec::new(),
        );
    }
    body(
        builder,
        rule,
        support,
        components,
        shared(),
        fixed,
        &mut held,
    )
}

/// Emit the rule instance: each component's alternatives, in order, then the
/// implication. `witnesses[i]` is component `first + i`'s witness cursor, holding
/// its first passing completion; an absent one is built here.
fn body<'cursor, 'plan: 'cursor, 'source>(
    builder: &mut Builder<'_, '_, 'source>,
    rule: &RuleIr,
    support: &'plan Support<'source>,
    components: &'plan plan::Plan,
    mut plans: Plans<'cursor, 'plan, 'source>,
    fixed: &Binding,
    witnesses: &mut Vec<Join<'cursor, 'source>>,
) -> Result<(), FormulaFailure> {
    let first = usize::from(!matches!(rule.head, HeadIr::Normal(None)));
    let mut witnesses = witnesses.drain(..);
    let mut body = VERUM;
    for (index, component) in components.components.iter().enumerate() {
        let mut alternatives = FALSUM;
        let mut bindings = if let Some(join) = (index >= first).then(|| witnesses.next()).flatten()
        {
            join
        } else {
            let mut join =
                component.join(builder, rule, support, fixed, index, plans.reborrow())?;
            builder.retain_matched_rows(&mut join, rule.location)?;
            join
        };
        let mut matched = None;
        while let Some(row) = bindings.next_matched_row(
            builder.budget,
            Context::new(
                builder.computation,
                builder.limits,
                builder.counters,
                rule.location,
            ),
        )? {
            // The borrowed cursor exposes scalar-rejected completions. They
            // name no atoms and must not advance the unchanged prefix. Unlike
            // ordinary emission, they need no rejected-row validation: every
            // component variable is a fixed head slot or bound by a positive
            // atom of this component, and its fixed-input arithmetic was
            // certified over the head carrier, so no atom key can be unbound.
            if !row.passes {
                continue;
            }
            #[cfg(test)]
            if components.arithmetic {
                arithmetic::testing::record(arithmetic::testing::Point::Witness);
            }
            let conjunction =
                builder.body_from_row((rule, &component.literals), &row, &mut matched, support)?;
            alternatives = builder.or(alternatives, conjunction, rule.location)?;
        }
        if alternatives == FALSUM && !matches!(rule.head, HeadIr::Normal(None)) {
            return Ok(());
        }
        // Every component is inhabited, so a constraint's component without
        // an alternative had its rows decided false. The instance is dropped
        // below, after the other components have named their atoms: the
        // complete join names every atom of every row that passes, as a
        // streamed constraint's capture does, and the routes publish the
        // same atoms.
        body = builder.and(body, alternatives, rule.location)?;
    }
    if body == FALSUM {
        return Ok(());
    }
    let HeadIr::Normal(head) = &rule.head else {
        unreachable!("only normal rules have a component plan");
    };
    let head = if let Some(head) = head {
        builder.atom(*head, fixed, rule.location)?
    } else {
        FALSUM
    };
    #[cfg(test)]
    if components.arithmetic {
        arithmetic::testing::record(arithmetic::testing::Point::Emission);
    }
    let formula = builder.node(Node::Implies(body, head), rule.location)?;
    builder.root(formula, rule)?;
    if head != FALSUM {
        builder.ordinary_producer(head, body, rule)?;
    }
    Ok(())
}

/// Establish a complete Cartesian witness, from component `first` on, before
/// naming any component atoms.
/// Naming an early component records the rule as an atom's first origin; a
/// later empty component must not leave that unused origin in completion roots.
/// The ordinary profile has only scalar leaves; the extended profile has its
/// fixed-input arithmetic certificate. One selected row per independent component
/// proves existence, not body truth. Each witness cursor is the one emission
/// then consumes: it stops at its first passing completion, unlent, and
/// emission resumes it there, so no row is traversed twice. The held cursors
/// coexist, each charged by its own workspace lease.
fn inhabited<'cursor, 'plan: 'cursor, 'source>(
    builder: &mut Builder<'_, '_, 'source>,
    rule: &RuleIr,
    support: &'plan Support<'source>,
    components: &'plan plan::Plan,
    plans: impl Fn() -> Plans<'cursor, 'plan, 'source>,
    fixed: &Binding<'_>,
    first: usize,
) -> Result<Option<Vec<Join<'cursor, 'source>>>, FormulaFailure> {
    let count = components.components.len().saturating_sub(first);
    builder.work(rule.location)?;
    let mut held = Vec::with_capacity(count);
    for (index, component) in components.components.iter().enumerate().skip(first) {
        let mut join = component.join(builder, rule, support, fixed, index, plans())?;
        builder.retain_matched_rows(&mut join, rule.location)?;
        if !join.hold_passing_row(
            builder.budget,
            Context::new(
                builder.computation,
                builder.limits,
                builder.counters,
                rule.location,
            ),
        )? {
            return Ok(None);
        }
        held.push(join);
    }
    Ok(Some(held))
}
