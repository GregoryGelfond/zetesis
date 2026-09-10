//! Completed numeric priority presence for qualified extrema carriers.
//!
//! An extrema proposal can be numeric although a mandatory symbolic tuple
//! prevents every realized extremum from being numeric. Possible support alone
//! then cannot certify that a numeric objective priority survives grounding.
//! A flat fact/choice certificate excludes that numeric witness without pruning
//! proposals. Other mixed carriers retain an explicit refusal. The implicit
//! empty endpoint is not a contributing tuple.

use std::collections::{BTreeMap, BTreeSet};

use themelios_analysis::depend::DependencyGraph;
use themelios_program::program::AggregateFunction;
use themelios_program::symbol::Signature;
use zetesis_core::{Predicate, Term, Value};

use crate::expansion::Budget;
use crate::formula::ceiling;
use crate::formula_ir::{AggregateIr, AggregateKey, Prepared};
use crate::formula_support::{Counters, Join, Support};
use crate::{FormulaFailure, FormulaLimits, FormulaResource};

use super::{
    LiteralIr, ObjectiveIr, RuleIr, dependency_closure, refusal, relevant_head, signature,
};

mod flat;

/// Completed exclusion and fixed-value certificates over borrowed predicates.
/// Fixed logical values are owned; their payload and retained slots are bounded.
/// Nonnumeric exclusions apply only to the same unary generated weight. Neither
/// certificate replaces original equalities or asserts candidate activity.
#[derive(Default)]
pub(crate) struct Presence<'a> {
    nonnumeric: BTreeSet<&'a Predicate>,
    fixed: BTreeMap<&'a Predicate, Value>,
}

impl Presence<'_> {
    /// A completed proposal row is eligible only when every certified observer
    /// in it carries its proved fixed value. The original equalities remain in
    /// the theory; this filter affects objective specialization only.
    pub(crate) fn eligible(
        &self,
        objective: &ObjectiveIr,
        binding: &[Value],
        limits: &FormulaLimits,
        counters: &mut Counters,
    ) -> Result<bool, FormulaFailure> {
        if objective.priority_sources.is_empty() {
            return Ok(true);
        }
        for atom in &objective.positive {
            counters.work(limits, objective.location)?;
            if let Some(value) = self.fixed.get(atom.predicate()) {
                let [term] = atom.terms() else {
                    unreachable!("fixed certificates are unary")
                };
                if term.resolve(binding).expect("safe objective row") != value {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }
    /// False proves exclusion; true still requires completed-support activation.
    pub(crate) fn may_have_numeric_weight(
        &self,
        objective: &ObjectiveIr,
        limits: &FormulaLimits,
        counters: &mut Counters,
    ) -> Result<bool, FormulaFailure> {
        if self.nonnumeric.is_empty() {
            return Ok(true);
        }
        counters.work(limits, objective.location)?;
        let Term::Variable(weight) = &objective.weight else {
            return Ok(true);
        };
        for atom in &objective.positive {
            counters.work(limits, objective.location)?;
            if self.nonnumeric.contains(atom.predicate())
                && atom.terms() == [Term::Variable(*weight)]
            {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

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
        let Term::Variable(weight) = &objective.weight else {
            continue;
        };
        for atom in &objective.positive {
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

pub(crate) fn check<'a>(
    prepared: &'a Prepared,
    support: &Support,
    limits: &FormulaLimits,
    budget: &mut Budget,
    counters: &mut Counters,
) -> Result<Presence<'a>, FormulaFailure> {
    let mut presence = Presence::default();
    certify_priorities(prepared, &mut presence, limits, budget, counters)?;
    if prepared.objective_extrema.is_empty() {
        return Ok(presence);
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
                if mixed(
                    aggregate,
                    &binding,
                    support,
                    limits,
                    budget,
                    counters,
                    rule.location,
                )? {
                    let Some(excluded) = flat::certify(
                        prepared,
                        rule,
                        aggregate,
                        presence.nonnumeric.len() + presence.fixed.len(),
                        limits,
                        counters,
                    )?
                    else {
                        return Err(refusal(rule.location));
                    };
                    for predicate in excluded {
                        counters.work(limits, rule.location)?;
                        // Consuming one temporary entry releases its slot before
                        // it is transferred into the completed certificate.
                        presence.nonnumeric.insert(predicate);
                    }
                }
            }
        }
    }
    Ok(presence)
}

/// Source dependency analysis names every generated priority input. Only a
/// complete fixed-value certificate can discharge that obligation here.
fn certify_priorities<'a>(
    prepared: &'a Prepared,
    presence: &mut Presence<'a>,
    limits: &FormulaLimits,
    budget: &mut Budget,
    counters: &mut Counters,
) -> Result<(), FormulaFailure> {
    let mut requested = BTreeSet::new();
    for objective in &prepared.objectives {
        for &index in &objective.priority_sources {
            let predicate = objective.positive[index].predicate();
            counters.work(limits, objective.location)?;
            if !requested.contains(predicate) {
                ceiling(
                    FormulaResource::ObjectivePresenceEntries,
                    requested.len() as u128 + 1,
                    limits.max_objective_presence_entries as u128,
                    objective.location,
                )?;
                requested.insert(predicate);
            }
        }
    }
    if requested.is_empty() {
        return Ok(());
    }
    for rule in &prepared.rules {
        counters.work(limits, rule.location)?;
        let [LiteralIr::Aggregate(aggregate)] = rule.body.as_slice() else {
            continue;
        };
        let Some(fixed) = flat::fixed(
            prepared,
            rule,
            aggregate,
            requested.len() + presence.fixed.len(),
            limits,
            budget,
            counters,
        )?
        else {
            continue;
        };
        for predicate in fixed.predicates {
            counters.work(limits, rule.location)?;
            if requested.contains(predicate) {
                let value = crate::formula_support::copy(&fixed.value, budget, rule.location)?;
                // The returned borrowed predicate slot is transferred into the
                // retained certificate. Its value payload is charged separately.
                presence.fixed.insert(predicate, value);
            }
        }
    }
    for objective in &prepared.objectives {
        for &index in &objective.priority_sources {
            counters.work(limits, objective.location)?;
            if !presence
                .fixed
                .contains_key(objective.positive[index].predicate())
            {
                return Err(refusal(objective.location));
            }
        }
    }
    Ok(())
}

/// Distinct raw alternatives with one full key have the same first-value class,
/// so class inspection needs no tuple store or alternate coalescing algorithm.
/// Each completed rule binding is inspected independently; all repeated join
/// work and copied values remain charged through the existing finite budgets.
fn mixed(
    aggregate: &AggregateIr,
    binding: &[Value],
    support: &Support,
    limits: &FormulaLimits,
    budget: &mut Budget,
    counters: &mut Counters,
    location: themelios_base::span::Location,
) -> Result<bool, FormulaFailure> {
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
                return Ok(true);
            }
        }
    }
    Ok(false)
}
