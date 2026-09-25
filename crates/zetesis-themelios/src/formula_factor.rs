//! Existential body components over fixed head bindings in completed support.

mod plan;

use zetesis_core::catalog::AssignmentError;
use zetesis_core::{PatternRef, TemplateTerm as Term};
use zetesis_ferraris::Node;

use crate::FormulaFailure;
use crate::formula_binding::Binding;
use crate::formula_ground::{Builder, FALSUM, VERUM};
use crate::formula_ir::{HeadIr, LiteralIr, RuleIr};
use crate::formula_support::{Context, Join, Support};

/// Return false only when the original complete-join path should be used.
pub(super) fn rule(
    builder: &mut Builder<'_, '_, '_>,
    rule: &RuleIr,
    support: &Support,
) -> Result<bool, FormulaFailure> {
    let HeadIr::Normal(head) = &rule.head else {
        return Ok(false);
    };
    let head = head
        .map(|head| {
            builder.computation.static_pattern(
                head,
                builder.limits,
                &mut builder.counters,
                rule.location,
            )
        })
        .transpose()?;
    let Some(components) = plan::components(builder, rule, head)? else {
        return Ok(false);
    };
    if let Some(head) = head {
        // Every possible firing's head occurs in completed U. Other producers
        // may add extra rows, which this rule's complete component joins filter.
        for atom in support.rows(head.predicate()) {
            builder.work(rule.location)?;
            if let Some(fixed) = head_binding(builder, rule, head, atom)? {
                emit(builder, rule, support, &components, &fixed)?;
            }
        }
    } else {
        let fixed = fixed_frame(builder, rule)?;
        emit(builder, rule, support, &components, &fixed)?;
    }
    Ok(true)
}

struct Component {
    literals: Vec<LiteralIr>,
    used: Vec<usize>,
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
                        &mut builder.counters,
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
        &mut builder.counters,
        rule.location,
    )?;
    fixed.extend_scope(
        rule.variables,
        builder.computation,
        builder.limits,
        &mut builder.counters,
        rule.location,
    )?;
    Ok(fixed)
}

fn emit(
    builder: &mut Builder<'_, '_, '_>,
    rule: &RuleIr,
    support: &Support,
    components: &plan::Plan,
    fixed: &Binding,
) -> Result<(), FormulaFailure> {
    let mut body = VERUM;
    for component in &components.components {
        let mut alternatives = FALSUM;
        let mut bindings = Join::component(
            &component.literals,
            rule.variables,
            &component.used,
            fixed,
            support,
            builder.budget,
            Context::new(
                &*builder.computation,
                builder.limits,
                &mut builder.counters,
                rule.location,
            ),
        )?;
        while let Some(binding) = bindings.next(
            builder.computation,
            builder.limits,
            builder.budget,
            &mut builder.counters,
            rule.location,
        )? {
            let conjunction =
                builder.body(&component.literals, &binding, rule.location, support)?;
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
    let formula = builder.node(Node::Implies(body, head), rule.location)?;
    builder.root(formula, rule)?;
    if head != FALSUM {
        builder.producer(head, body, rule)?;
    }
    Ok(())
}
