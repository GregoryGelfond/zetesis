//! Finite source eligibility over an acyclic producer cone.
//!
//! Required/possible activity is a source grounding abstraction, not answer-set
//! realization. Optional means retained as a grounding possibility; it does not
//! assert that several optional literals can hold together. For example,
//! `a,not a` can retain a priority while its original-model query is always false.
//! Choices stay optional even when constraints force an answer.
//! The original theory is never read or changed by this certificate.

use std::collections::{BTreeMap, BTreeSet};

use themelios_base::span::Location;
use themelios_program::program::DefaultNegation;
use themelios_program::symbol::Signature;
use zetesis_core::{Atom, AtomPattern, Value};
mod query;
mod cyclic;

pub(crate) use query::condition as model_condition;

use super::signature;
use crate::expansion::Budget;
use crate::formula::ceiling;
use crate::formula_ir::{HeadIr, LiteralIr, Prepared, RuleIr};
use crate::formula_support::{self, Counters, Join, Support};
use crate::{ExpansionResource, FormulaFailure, FormulaLimits, FormulaResource};

fn refusal(location: Location) -> FormulaFailure {
    crate::diagnostic::unsupported(crate::ProfileFeature::ObjectiveSourceEligibility, location)
        .into()
}

/// A completed source atom can be absent, possible or required. This order
/// makes finite conjunction the minimum and alternative producers the maximum.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Activity {
    Absent,
    Optional,
    Required,
}

impl Activity {
    fn negate(self) -> Self {
        match self {
            Self::Absent => Self::Required,
            Self::Optional => Self::Optional,
            Self::Required => Self::Absent,
        }
    }
}

#[derive(Default)]
pub(crate) struct Completion {
    atoms: BTreeMap<Atom, Activity>,
}

pub(crate) struct Context<'a> {
    pub limits: &'a FormulaLimits,
    pub budget: &'a mut Budget,
    pub counters: &'a mut Counters,
    pub location: Location,
}

impl Context<'_> {
    fn work(&mut self) -> Result<(), FormulaFailure> {
        self.counters.work(self.limits, self.location)
    }

    fn atom(&mut self, pattern: &AtomPattern, binding: &[Value]) -> Result<Atom, FormulaFailure> {
        self.work()?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(pattern.terms().len())
            .map_err(|_| self.allocation())?;
        for term in pattern.terms() {
            self.work()?;
            values.push(formula_support::copy(
                term.resolve(binding)
                    .expect("admitted complete source binding"),
                self.budget,
                self.location,
            )?);
        }
        self.budget.charge(
            ExpansionResource::ScalarBytes,
            pattern.predicate().name().len() as u128,
            self.location,
        )?;
        Ok(Atom::new(pattern.predicate().clone(), values).expect("unchanged source arity"))
    }

    fn allocation(&self) -> FormulaFailure {
        FormulaFailure::Objective {
            error: zetesis_objective::AdmissionError::Allocation,
            location: self.location,
        }
    }

    fn scalar(&mut self, literal: &LiteralIr, binding: &[Value]) -> Result<bool, FormulaFailure> {
        match literal {
            LiteralIr::Compare(left, relation, right) => {
                let left = formula_support::expression(
                    left,
                    binding,
                    self.limits,
                    self.budget,
                    self.counters,
                    self.location,
                )?;
                let right = formula_support::expression(
                    right,
                    binding,
                    self.limits,
                    self.budget,
                    self.counters,
                    self.location,
                )?;
                Ok(formula_support::compare(&left, *relation, &right))
            }
            LiteralIr::Guard(guard) => guard.evaluate(
                binding,
                self.limits,
                self.budget,
                self.counters,
                self.location,
            ),
            LiteralIr::Bind { .. } | LiteralIr::Range { .. } => Ok(true),
            _ => Err(refusal(self.location)),
        }
    }

    /// Reserve the coexisting closure, pending traversal and remaining set
    /// before inserting any new predicate. Legacy certificate storage remains
    /// included throughout this independent certificate's construction.
    fn closure(
        &mut self,
        graph: &themelios_analysis::depend::DependencyGraph,
        retained: usize,
        roots: impl IntoIterator<Item = Signature>,
    ) -> Result<BTreeSet<Signature>, FormulaFailure> {
        let mut relevant = BTreeSet::new();
        let mut pending = Vec::new();
        for root in roots {
            self.work()?;
            self.discover(root, retained, &mut relevant, &mut pending)?;
        }
        while let Some(predicate) = pending.pop() {
            self.work()?;
            for (_, dependency) in graph.edges_from(&predicate) {
                self.work()?;
                if !relevant.contains(dependency) {
                    self.discover(dependency.clone(), retained, &mut relevant, &mut pending)?;
                }
            }
        }
        Ok(relevant)
    }

    fn discover(
        &mut self,
        predicate: Signature,
        retained: usize,
        relevant: &mut BTreeSet<Signature>,
        pending: &mut Vec<Signature>,
    ) -> Result<(), FormulaFailure> {
        if relevant.contains(&predicate) {
            return Ok(());
        }
        self.entries(retained.saturating_add(relevant.len().saturating_add(1).saturating_mul(3)))?;
        pending.try_reserve(1).map_err(|_| self.allocation())?;
        relevant.insert(predicate.clone());
        pending.push(predicate);
        Ok(())
    }

    fn entries(&self, count: usize) -> Result<(), FormulaFailure> {
        ceiling(
            FormulaResource::ObjectivePresenceEntries,
            count as u128,
            self.limits.max_objective_presence_entries as u128,
            self.location,
        )
    }
}

impl Completion {
    pub(crate) fn build(
        prepared: &Prepared,
        support: &Support<'_>,
        retained: usize,
        context: &mut Context<'_>,
    ) -> Result<Self, FormulaFailure> {
        let graph = prepared.analysis.dependencies();
        let relevant = context.closure(
            graph,
            retained,
            prepared
                .objectives
                .iter()
                .filter(|objective| objective.source_completion)
                .flat_map(|objective| {
                    objective
                        .condition
                        .iter()
                        .filter_map(|literal| match literal {
                            LiteralIr::Atom(_, atom) => Some(signature(atom.predicate())),
                            _ => None,
                        })
                }),
        )?;
        // The dependency sets coexist with the completed atom table. These
        // logical planning slots are independent of allocator representation.
        let temporary = retained.saturating_add(relevant.len().saturating_mul(3));
        context.entries(temporary)?;
        let mut remaining = relevant.clone();
        let mut completed = BTreeSet::<Signature>::new();
        let mut result = Self::default();
        while !remaining.is_empty() {
            let mut ready = None;
            for predicate in &remaining {
                context.work()?;
                let mut available = true;
                for (_, dependency) in graph.edges_from(predicate) {
                    context.work()?;
                    available &= !relevant.contains(dependency) || completed.contains(dependency);
                }
                if available {
                    ready = Some(predicate.clone());
                    break;
                }
            }
            let Some(predicate) = ready else {
                result.cyclic(prepared, support, &remaining, temporary, context)?;
                break;
            };
            for rule in &prepared.rules {
                context.work()?;
                let produces = match &rule.head {
                    HeadIr::Normal(Some(head)) => signature(head.predicate()) == predicate,
                    HeadIr::Choice(group) => group
                        .elements
                        .iter()
                        .filter_map(|element| element.head.positive_atom())
                        .any(|head| signature(head.predicate()) == predicate),
                    HeadIr::Disjunction(heads) => heads
                        .iter()
                        .filter_map(|head| head.positive_atom())
                        .any(|head| signature(head.predicate()) == predicate),
                    HeadIr::Normal(None) => false,
                };
                if !produces {
                    continue;
                }
                result.rule(rule, &predicate, support, temporary, context)?;
            }
            remaining.remove(&predicate);
            completed.insert(predicate);
        }
        Ok(result)
    }

    fn rule(
        &mut self,
        rule: &RuleIr,
        predicate: &Signature,
        support: &Support<'_>,
        temporary: usize,
        context: &mut Context<'_>,
    ) -> Result<(), FormulaFailure> {
        context.location = rule.location;
        let mut outer = Join::rule(rule, support, context.budget)?;
        while let Some(binding) = outer.next(
            context.limits,
            context.budget,
            context.counters,
            rule.location,
        )? {
            let body = self.activity(&rule.body, &binding, context)?;
            match &rule.head {
                HeadIr::Normal(Some(head)) => {
                    self.retain(head, &binding, body, temporary, context)?;
                }
                HeadIr::Disjunction(heads) => {
                    if heads.iter().any(|head| head.positive_atom().is_none()) {
                        return Err(refusal(rule.location));
                    }
                    for head in heads.iter().filter_map(|head| head.positive_atom()) {
                        if signature(head.predicate()) == *predicate {
                            self.retain(
                                head,
                                &binding,
                                body.min(Activity::Optional),
                                temporary,
                                context,
                            )?;
                        }
                    }
                }
                HeadIr::Choice(group) => {
                    for element in &group.elements {
                        let Some(head) = element.head.positive_atom() else {
                            continue;
                        };
                        if signature(head.predicate()) != *predicate {
                            continue;
                        }
                        let mut local = Join::new(
                            &element.condition,
                            &binding,
                            element.variables,
                            support,
                            context.budget,
                            rule.location,
                        )?;
                        while let Some(row) = local.next(
                            context.limits,
                            context.budget,
                            context.counters,
                            rule.location,
                        )? {
                            let eligible = self.activity(&element.condition, &row, context)?;
                            self.retain(
                                head,
                                &row,
                                body.min(eligible).min(Activity::Optional),
                                temporary,
                                context,
                            )?;
                        }
                    }
                }
                HeadIr::Normal(None) => unreachable!("only producer heads selected"),
            }
        }
        Ok(())
    }

    fn retain(
        &mut self,
        pattern: &AtomPattern,
        binding: &[Value],
        activity: Activity,
        temporary: usize,
        context: &mut Context<'_>,
    ) -> Result<(), FormulaFailure> {
        // Resolve the complete row before selecting its source activity.
        let atom = context.atom(pattern, binding)?;
        if activity == Activity::Absent {
            return Ok(());
        }
        if let Some(previous) = self.atoms.get_mut(&atom) {
            *previous = (*previous).max(activity);
        } else {
            context.entries(temporary.saturating_add(self.atoms.len()).saturating_add(1))?;
            self.atoms.insert(atom, activity);
        }
        Ok(())
    }

    /// Source truth coverage is evaluated without constructing or charging a
    /// retained model query. Every literal is visited even after known absence.
    pub(crate) fn activity(
        &self,
        literals: &[LiteralIr],
        binding: &[Value],
        context: &mut Context<'_>,
    ) -> Result<Activity, FormulaFailure> {
        let mut result = Activity::Required;
        for literal in literals {
            context.work()?;
            let activity = match literal {
                LiteralIr::Atom(negation, pattern) => {
                    let atom = context.atom(pattern, binding)?;
                    let activity = self.atoms.get(&atom).copied().unwrap_or(Activity::Absent);
                    if *negation == DefaultNegation::Not {
                        activity.negate()
                    } else {
                        activity
                    }
                }
                _ => {
                    if context.scalar(literal, binding)? {
                        Activity::Required
                    } else {
                        Activity::Absent
                    }
                }
            };
            result = result.min(activity);
        }
        Ok(result)
    }
}
