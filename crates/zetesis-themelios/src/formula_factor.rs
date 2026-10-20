//! Formula factoring over completed possible support.
//!
//! Independent body components share fixed head bindings; complete positive
//! witness runs share their common continuations. Independent fixed-input
//! arithmetic is certified over the complete head carrier before emission.
//! Both routes retain all witness alternatives and source provenance.

mod arithmetic;
mod continuations;
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

impl Component {
    fn join<'plan: 'cursor, 'cursor, 'source>(
        &'plan self,
        builder: &mut Builder<'_, '_, 'source>,
        rule: &RuleIr,
        support: &'plan Support<'source>,
        fixed: &Binding<'_>,
        index: usize,
        prepared: Option<&'cursor mut ComponentPlans<'plan, 'source>>,
    ) -> Result<Join<'cursor, 'source>, FormulaFailure> {
        if let Some(prepared) = prepared {
            return prepared.join(
                index,
                ComponentScope {
                    literals: &self.literals,
                    used: &self.used,
                },
                fixed,
                support,
                builder.budget,
                Context::new(
                    &*builder.computation,
                    builder.limits,
                    builder.counters,
                    rule.location,
                ),
            );
        }
        Join::component(
            &self.literals,
            rule.variables,
            &self.used,
            fixed,
            support,
            builder.budget,
            Context::new(
                &*builder.computation,
                builder.limits,
                builder.counters,
                rule.location,
            ),
        )
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
    if !inhabited(builder, rule, support, components, prepared.as_mut(), fixed)? {
        return Ok(());
    }
    let mut body = VERUM;
    for (index, component) in components.components.iter().enumerate() {
        let mut alternatives = FALSUM;
        let mut bindings =
            component.join(builder, rule, support, fixed, index, prepared.as_mut())?;
        builder.retain_matched_rows(&mut bindings, rule.location)?;
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
            // name no atoms and must not advance the emitted-node prefix.
            if !row.passes {
                continue;
            }
            #[cfg(test)]
            if components.arithmetic {
                arithmetic::testing::record(arithmetic::testing::Point::Witness);
            }
            let conjunction = builder.body_from_row(
                &component.literals,
                &row,
                &mut matched,
                support,
                rule.location,
            )?;
            alternatives = builder.or(alternatives, conjunction, rule.location)?;
        }
        if alternatives == FALSUM {
            return Ok(());
        }
        body = builder.and(body, alternatives, rule.location)?;
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
        builder.producer(head, body, rule)?;
    }
    Ok(())
}

/// Establish a complete Cartesian witness before naming any component atoms.
/// Naming an early component records the rule as an atom's first origin; a
/// later empty component must not leave that unused origin in completion roots.
/// The ordinary profile has only scalar leaves; the extended profile has its
/// fixed-input arithmetic certificate. One selected row per independent component
/// proves existence, not body truth. Emission subsequently traverses every
/// selected row without retaining a cache. Work and typed refusal belong to the
/// same join as emission; only one cursor and its first frame coexist here.
fn inhabited<'plan, 'source>(
    builder: &mut Builder<'_, '_, 'source>,
    rule: &RuleIr,
    support: &'plan Support<'source>,
    components: &'plan plan::Plan,
    mut prepared: Option<&mut ComponentPlans<'plan, 'source>>,
    fixed: &Binding<'_>,
) -> Result<bool, FormulaFailure> {
    for (index, component) in components.components.iter().enumerate() {
        let mut bindings = component.join(
            builder,
            rule,
            support,
            fixed,
            index,
            prepared.as_deref_mut(),
        )?;
        if bindings
            .next(
                builder.computation,
                builder.limits,
                builder.budget,
                builder.counters,
                rule.location,
            )?
            .is_none()
        {
            return Ok(false);
        }
    }
    Ok(true)
}
