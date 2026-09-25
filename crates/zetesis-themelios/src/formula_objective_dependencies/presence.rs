//! Source certificates for numeric presence and generated priority carriers.
//!
//! An extrema proposal can be numeric although a mandatory symbolic tuple
//! prevents every realized extremum from being numeric. Possible support alone
//! then cannot certify that a numeric objective priority survives grounding.
//! A flat fact/choice certificate excludes that numeric witness without pruning
//! proposals. A separate certificate completes the count, sum or extremum values
//! of every key set between the required and possible complete keys. Shared
//! optional conditions do not erase values from this source abstraction.
//! Objective preparation filters priority rows against the completed carrier;
//! the original equalities and model-relative conditions remain authoritative.
//! If a flat refinement does not apply, the complete possible carrier remains
//! eligible. A failed applied refinement still propagates its resource or value
//! error. The implicit empty endpoint is not a contributing tuple.

use std::collections::{BTreeMap, BTreeSet};

use crate::formula_support::components::Term;
use themelios_analysis::depend::DependencyGraph;
use themelios_program::program::AggregateFunction;
use themelios_program::symbol::Signature;
use zetesis_core::catalog::PredicateRef;
use zetesis_core::{TemplateTerm, ValueNodeRef};

use crate::expansion::Budget;
use crate::formula::ceiling;
use crate::formula_binding::Binding;
use crate::formula_ir::{AggregateIr, AggregateKey, Prepared};
use crate::formula_support::{Computation, Counters, Join, Support};
use crate::{FormulaFailure, FormulaLimits, FormulaResource};

use super::{Context, LiteralIr, ObjectiveIr, RuleIr, dependency_closure, relevant_head};

mod flat;

/// Completed exclusions and source carriers over borrowed predicates.
/// Carrier IDs are selected once per producer/forwarding family. Membership
/// searches these bounded families; it never tests answer-set realizability.
/// Nonnumeric exclusions apply only to the same unary generated weight. Neither
/// certificate replaces original equalities or asserts candidate activity.
#[derive(Default)]
pub(crate) struct Presence<'a> {
    nonnumeric: Predicates<'a>,
    carriers: Vec<flat::Carrier<'a>>,
    carrier_entries: usize,
}

/// Predicate ordering is not used by these certificates. Membership uses the
/// checked comparator; entries own only borrowed prefix views, not names.
#[derive(Default)]
pub(super) struct Predicates<'source>(Vec<PredicateRef<'source>>);
impl<'source> Predicates<'source> {
    fn len(&self) -> usize {
        self.0.len()
    }
    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    fn contains(
        &self,
        predicate: PredicateRef<'_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: themelios_base::span::Location,
    ) -> Result<bool, FormulaFailure> {
        for current in &self.0 {
            if current
                .compare_ref_with(predicate, || counters.work(limits, location))?
                .is_eq()
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
    fn insert(
        &mut self,
        predicate: PredicateRef<'source>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: themelios_base::span::Location,
    ) -> Result<bool, FormulaFailure> {
        if self.contains(predicate, limits, counters, location)? {
            return Ok(false);
        }
        counters.charge_work(self.0.len() as u128 + 1, limits, location)?;
        self.0
            .try_reserve(1)
            .map_err(|_| FormulaFailure::Objective {
                error: zetesis_objective::AdmissionError::Allocation,
                location,
            })?;
        self.0.push(predicate);
        Ok(true)
    }
}

impl Presence<'_> {
    pub(crate) fn retained_entries(&self) -> usize {
        self.nonnumeric.len().saturating_add(self.carrier_entries)
    }

    pub(crate) fn eligible(
        &self,
        objective: &ObjectiveIr,
        binding: &Binding,
        computation: &mut Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
    ) -> Result<bool, FormulaFailure> {
        if objective.priority_sources.is_empty() {
            return Ok(true);
        }
        for atom in &objective.positive {
            let atom = computation.static_pattern(*atom, limits, counters, objective.location)?;
            counters.work(limits, objective.location)?;
            let predicate = atom.predicate();
            for carrier in &self.carriers {
                if carrier
                    .predicates
                    .contains(predicate, limits, counters, objective.location)?
                {
                    counters.work(limits, objective.location)?;
                    let term = atom.terms().at(0).expect("carrier certificates are unary");
                    let key = match term {
                        TemplateTerm::Constant(value) => {
                            counters.work(limits, objective.location)?;
                            computation.read().term_key(value).map_err(|error| {
                                crate::formula_binding::assignment(error.into(), objective.location)
                            })?
                        }
                        TemplateTerm::Variable(variable) => {
                            binding.key(variable, objective.location)?
                        }
                    };
                    if !carrier
                        .values
                        .contains(&key, limits, counters, objective.location)?
                    {
                        return Ok(false);
                    }
                }
            }
        }
        Ok(true)
    }

    pub(crate) fn may_have_numeric_weight(
        &self,
        objective: &ObjectiveIr,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
    ) -> Result<bool, FormulaFailure> {
        if objective.needs_eligibility_query || self.nonnumeric.is_empty() {
            return Ok(true);
        }
        counters.work(limits, objective.location)?;
        let Some(Term::Variable(weight)) = objective.weight.term() else {
            return Ok(true);
        };
        for atom in &objective.positive {
            let atom = computation.static_pattern(*atom, limits, counters, objective.location)?;
            counters.work(limits, objective.location)?;
            if self
                .nonnumeric
                .contains(atom.predicate(), limits, counters, objective.location)?
            {
                counters.work(limits, objective.location)?;
                if atom.terms().len() == 1
                    && atom.terms().at(0) == Some(TemplateTerm::Variable(*weight))
                {
                    return Ok(false);
                }
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
    context: &mut Context<'_, '_>,
) -> Result<BTreeSet<usize>, FormulaFailure> {
    let mut observed = BTreeSet::new();
    for objective in objectives {
        context.location = objective.location;
        let Some(Term::Variable(weight)) = objective.weight.term() else {
            continue;
        };
        for atom in &objective.positive {
            let predicate = context.signature(*atom)?;
            let Some(positions) = generated.get(&predicate) else {
                continue;
            };
            let atom = context.pattern(*atom)?;
            for &position in positions {
                if context.term(atom, position)? == TemplateTerm::Variable(*weight) {
                    observed.insert(predicate);
                    break;
                }
            }
        }
    }
    let relevant = dependency_closure(graph, observed);
    let mut required = BTreeSet::new();
    for rule in rules {
        context.location = rule.location;
        if !relevant_head(&rule.head, &relevant, context)? {
            continue;
        }
        for literal in &rule.body {
            if let LiteralIr::Aggregate(aggregate) = literal
                && matches!(
                    aggregate.function,
                    AggregateFunction::Min | AggregateFunction::Max
                )
            {
                required.insert(aggregate.id);
            }
        }
    }
    Ok(required)
}

pub(crate) fn check<'source>(
    prepared: &Prepared,
    support: &Support,
    computation: &mut Computation<'_, 'source>,
    limits: &FormulaLimits,
    budget: &mut Budget,
    counters: &mut Counters,
) -> Result<Presence<'source>, FormulaFailure> {
    let mut presence = Presence::default();
    certify_priorities(prepared, &mut presence, computation, limits, counters)?;
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
            let mut outer = Join::rule(rule, support, computation, limits, budget, counters)?;
            while let Some(binding) =
                outer.next(computation, limits, budget, counters, rule.location)?
            {
                if mixed(
                    aggregate,
                    &rule.body_binding(&binding),
                    support,
                    budget,
                    crate::formula_support::Context::new(
                        &mut *computation,
                        limits,
                        counters,
                        rule.location,
                    ),
                )? {
                    let Some(excluded) = flat::certify(
                        prepared,
                        rule,
                        aggregate,
                        presence.nonnumeric.len() + presence.carrier_entries,
                        computation,
                        limits,
                        counters,
                    )?
                    else {
                        continue;
                    };
                    for predicate in excluded.0 {
                        counters.work(limits, rule.location)?;
                        // Consuming one temporary entry releases its slot before
                        // it is transferred into the completed certificate.
                        presence
                            .nonnumeric
                            .insert(predicate, limits, counters, rule.location)?;
                    }
                }
            }
        }
    }
    Ok(presence)
}

/// Try applicable flat refinements for requested generated fields. No returned
/// carrier means no extra exclusion, while an applied operation's errors remain
/// errors. This never equates unqualified precision with successful proof.
fn certify_priorities<'source>(
    prepared: &Prepared,
    presence: &mut Presence<'source>,
    computation: &mut Computation<'_, 'source>,
    limits: &FormulaLimits,
    counters: &mut Counters,
) -> Result<(), FormulaFailure> {
    let mut requested = Predicates::default();
    for objective in &prepared.objectives {
        for &index in &objective.priority_sources {
            let atom = computation.static_pattern(
                objective.positive[index],
                limits,
                counters,
                objective.location,
            )?;
            counters.work(limits, objective.location)?;
            let predicate = atom.predicate();
            counters.work(limits, objective.location)?;
            if !requested.contains(predicate, limits, counters, objective.location)? {
                ceiling(
                    FormulaResource::ObjectivePresenceEntries,
                    requested.len() as u128 + 1,
                    limits.max_objective_presence_entries as u128,
                    objective.location,
                )?;
                requested.insert(predicate, limits, counters, objective.location)?;
            }
        }
    }
    if requested.is_empty() {
        return Ok(());
    }
    for rule in &prepared.rules {
        counters.work(limits, rule.location)?;
        let Some(carrier) = flat::carrier(
            prepared,
            rule,
            &requested,
            requested.len() + presence.carrier_entries,
            computation,
            limits,
            counters,
        )?
        else {
            continue;
        };
        // Tuple/cone temporaries have been released. Transfer the completed
        // predicate/value slots without duplicating carrier values for aliases.
        let entries = carrier.predicates.len() + carrier.values.len() + 1;
        ceiling(
            FormulaResource::ObjectivePresenceEntries,
            requested.len() as u128 + presence.carrier_entries as u128 + entries as u128,
            limits.max_objective_presence_entries as u128,
            rule.location,
        )?;
        presence
            .carriers
            .try_reserve(1)
            .map_err(|_| FormulaFailure::Objective {
                error: zetesis_objective::AdmissionError::Allocation,
                location: rule.location,
            })?;
        presence.carrier_entries += entries;
        presence.carriers.push(carrier);
    }
    Ok(())
}

/// Distinct raw alternatives with one full key have the same first-value class,
/// so class inspection needs no tuple store or alternate coalescing algorithm.
/// Each completed rule binding is inspected independently; all repeated join
/// work and leased binding metadata remain charged through the finite budgets.
fn mixed(
    aggregate: &AggregateIr,
    binding: &Binding,
    support: &Support,
    budget: &mut Budget,
    context: crate::formula_support::Context<'_, &mut Computation<'_, '_>>,
) -> Result<bool, FormulaFailure> {
    let crate::formula_support::Context { computation, work } = context;
    let crate::formula_support::GroundingWork {
        limits,
        counters,
        location,
    } = work;
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
            crate::formula_support::Context::new(&*computation, limits, counters, location),
        )?;
        while let Some(row) = local.next(computation, limits, budget, counters, location)? {
            counters.work(limits, location)?;
            if matches!(
                row.resolve(
                    computation.static_term(*first, limits, counters, location)?,
                    computation.read(),
                    location
                )?
                .descriptor(),
                ValueNodeRef::Number(_)
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
