//! A fail-closed boundary for unqualified mixed-extrema priority presence.
//!
//! An extrema proposal can be numeric although a mandatory symbolic tuple
//! prevents every realized extremum from being numeric. Possible support alone
//! then cannot certify that a numeric objective priority survives grounding.
//! Keep this implementation gap explicit instead of publishing a false zero
//! cost slot. The implicit empty endpoint is not a contributing tuple.

use std::collections::{BTreeMap, BTreeSet};

use themelios_analysis::depend::DependencyGraph;
use themelios_program::program::AggregateFunction;
use themelios_program::symbol::Signature;
use zetesis_core::{Term, Value};

use crate::expansion::Budget;
use crate::formula_ir::{AggregateIr, AggregateKey, Prepared};
use crate::formula_support::{Counters, Join, Support};
use crate::{FormulaFailure, FormulaLimits};

use super::{
    LiteralIr, ObjectiveIr, RuleIr, dependency_closure, refusal, relevant_head, signature,
};

/// Only a generated position used as an objective weight seeds this analysis.
/// Reachability is deliberately conservative: an intervening numeric reduction
/// may remove the value-class concern but does not yet carry that certificate.
pub(super) fn required(
    rules: &[RuleIr],
    objectives: &[ObjectiveIr],
    graph: &DependencyGraph,
    generated: &BTreeMap<Signature, BTreeSet<usize>>,
) -> BTreeSet<usize> {
    let mut observed = BTreeSet::new();
    for objective in objectives {
        let Term::Variable(weight) = objective.template.weight() else {
            continue;
        };
        for atom in objective.template.positive() {
            let predicate = signature(atom.predicate());
            if generated.get(&predicate).is_some_and(|positions| {
                positions
                    .iter()
                    .any(|&position| atom.terms()[position] == Term::Variable(*weight))
            }) {
                observed.insert(predicate);
            }
        }
    }
    let relevant = dependency_closure(graph, observed);
    rules
        .iter()
        .filter(|rule| relevant_head(&rule.head, &relevant))
        .flat_map(|rule| &rule.body)
        .filter_map(|literal| match literal {
            LiteralIr::Aggregate(aggregate)
                if matches!(
                    aggregate.function,
                    AggregateFunction::Min | AggregateFunction::Max
                ) =>
            {
                Some(aggregate.id)
            }
            _ => None,
        })
        .collect()
}

pub(crate) fn check(
    prepared: &Prepared,
    support: &Support,
    limits: FormulaLimits,
    budget: &mut Budget,
    counters: &mut Counters,
) -> Result<(), FormulaFailure> {
    if prepared.objective_extrema.is_empty() {
        return Ok(());
    }
    for rule in &prepared.rules {
        for literal in &rule.body {
            let LiteralIr::Aggregate(aggregate) = literal else {
                continue;
            };
            if !prepared.objective_extrema.contains(&aggregate.id) {
                continue;
            }
            let mut outer = Join::rule(rule, support, budget)?;
            while let Some(binding) = outer.next(limits, budget, counters, rule.location)? {
                homogeneous(
                    aggregate,
                    &binding,
                    support,
                    limits,
                    budget,
                    counters,
                    rule.location,
                )?;
            }
        }
    }
    Ok(())
}

/// Distinct raw alternatives with one full key have the same first-value class,
/// so class inspection needs no tuple store or alternate coalescing algorithm.
/// Each completed rule binding is inspected independently; all repeated join
/// work and copied values remain charged through the existing finite budgets.
fn homogeneous(
    aggregate: &AggregateIr,
    binding: &[Value],
    support: &Support,
    limits: FormulaLimits,
    budget: &mut Budget,
    counters: &mut Counters,
    location: themelios_base::span::Location,
) -> Result<(), FormulaFailure> {
    let mut numeric = false;
    let mut nonnumeric = false;
    for element in &aggregate.elements {
        let AggregateKey::Tuple(tuple) = &element.key else {
            unreachable!("extrema assignments use complete tuples")
        };
        let first = tuple.first().expect("admitted nonempty extrema tuple");
        let mut local = Join::new(
            &element.condition,
            binding,
            element.variables,
            support,
            budget,
            location,
        )?;
        while let Some(row) = local.next(limits, budget, counters, location)? {
            counters.work(limits, location)?;
            if matches!(
                first.resolve(&row).expect("safe bound tuple"),
                Value::Number(_)
            ) {
                numeric = true;
            } else {
                nonnumeric = true;
            }
            if numeric && nonnumeric {
                return Err(refusal(location));
            }
        }
    }
    Ok(())
}
