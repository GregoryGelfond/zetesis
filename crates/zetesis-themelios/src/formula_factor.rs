//! Existential body components over fixed head bindings in completed support.

mod plan;

use std::collections::BTreeSet;

use themelios_program::program::DefaultNegation;
use zetesis_core::{Atom, AtomPattern, Term, Value};
use zetesis_ferraris::Node;

use crate::FormulaFailure;
use crate::formula_ground::Builder;
use crate::formula_ir::{HeadIr, LiteralIr, RuleIr};
use crate::formula_support::{Join, Support, copy};

/// Return false only when the original complete-join path should be used.
pub(super) fn rule(
    builder: &mut Builder<'_>,
    rule: &RuleIr,
    support: &Support,
) -> Result<bool, FormulaFailure> {
    let HeadIr::Normal(head) = &rule.head else {
        return Ok(false);
    };
    let Some(components) = plan::components(builder, rule, head.as_ref())? else {
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
        let fixed = vec![None; rule.variables];
        emit(builder, rule, support, &components, &fixed)?;
    }
    Ok(true)
}

struct Component {
    literals: Vec<LiteralIr>,
    used: BTreeSet<usize>,
}

fn head_binding(
    builder: &mut Builder<'_>,
    rule: &RuleIr,
    head: &AtomPattern,
    atom: &Atom,
) -> Result<Option<Vec<Option<Value>>>, FormulaFailure> {
    let mut fixed = Vec::with_capacity(rule.variables);
    for _ in 0..rule.variables {
        builder.work(rule.location)?;
        fixed.push(None);
    }
    for (term, value) in head.terms().iter().zip(atom.values()) {
        builder.work(rule.location)?;
        match term {
            Term::Constant(constant) if constant != value => return Ok(None),
            Term::Variable(variable) => {
                if let Some(previous) = &fixed[*variable] {
                    if previous != value {
                        return Ok(None);
                    }
                } else {
                    fixed[*variable] = Some(copy(value, builder.budget, rule.location)?);
                }
            }
            Term::Constant(_) => {}
        }
    }
    Ok(Some(fixed))
}

fn emit(
    builder: &mut Builder<'_>,
    rule: &RuleIr,
    support: &Support,
    components: &[Component],
    fixed: &[Option<Value>],
) -> Result<(), FormulaFailure> {
    let mut body = 1;
    for component in components {
        let mut alternatives = 0;
        let mut bindings = Join::component(
            &component.literals,
            rule.variables,
            &component.used,
            fixed,
            support,
            builder.budget,
            rule.location,
        )?;
        while let Some(binding) = bindings.next(
            builder.limits,
            builder.budget,
            &mut builder.counters,
            rule.location,
        )? {
            let conjunction =
                builder.body(&component.literals, &binding, rule.location, support)?;
            alternatives = builder.or(alternatives, conjunction, rule.location)?;
        }
        if alternatives == 0 {
            return Ok(());
        }
        body = builder.and(body, alternatives, rule.location)?;
    }
    let HeadIr::Normal(head) = &rule.head else {
        unreachable!("only normal rules have a component plan");
    };
    let head = if let Some(head) = head {
        let mut assignment = Vec::with_capacity(rule.variables);
        for slot in fixed {
            builder.work(rule.location)?;
            assignment.push(match slot {
                Some(value) => copy(value, builder.budget, rule.location)?,
                None => Value::Number(0), // Only head variables are read here.
            });
        }
        builder.atom(head, &assignment, rule.location)?
    } else {
        0
    };
    let formula = builder.node(Node::Implies(body, head), rule.location)?;
    builder.root(formula, rule)?;
    if head != 0 {
        builder.producer(head, body, rule)?;
    }
    Ok(())
}

fn positive_variables(literals: &[LiteralIr]) -> impl Iterator<Item = usize> + '_ {
    literals
        .iter()
        .filter_map(|literal| match literal {
            LiteralIr::Atom(DefaultNegation::None, pattern) => Some(pattern.terms()),
            _ => None,
        })
        .flatten()
        .filter_map(|term| match term {
            Term::Variable(variable) => Some(*variable),
            Term::Constant(_) => None,
        })
}
