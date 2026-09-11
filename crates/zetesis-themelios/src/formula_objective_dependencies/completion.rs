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
use zetesis_objective::{Condition, ConditionNode};

use super::{dependency_closure, signature};
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

pub(crate) struct CompletedCondition {
    pub activity: Activity,
    pub query: Condition,
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
        let relevant = dependency_closure(
            graph,
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
        );
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
                return Err(refusal(context.location));
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
            let body = self.condition(&rule.body, &binding, context)?.activity;
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
                        if element.key.tuple().is_some() {
                            return Err(refusal(rule.location));
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
                            let eligible =
                                self.condition(&element.condition, &row, context)?.activity;
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

    pub(crate) fn condition(
        &self,
        literals: &[LiteralIr],
        binding: &[Value],
        context: &mut Context<'_>,
    ) -> Result<CompletedCondition, FormulaFailure> {
        let mut query = Query {
            nodes: Vec::new(),
            activity: Vec::new(),
        };
        let mut root = query.node(ConditionNode::Boolean(true), Activity::Required, context)?;
        for literal in literals {
            context.work()?;
            let node = match literal {
                LiteralIr::Atom(negation, pattern) => {
                    let atom = context.atom(pattern, binding)?;
                    let activity = self.atoms.get(&atom).copied().unwrap_or(Activity::Absent);
                    let mut node = query.node(ConditionNode::Atom(atom), activity, context)?;
                    if *negation != DefaultNegation::None {
                        node = query.negate(node, context)?;
                        if *negation == DefaultNegation::NotNot {
                            node = query.negate(node, context)?;
                        }
                    }
                    node
                }
                LiteralIr::Compare(left, relation, right) => {
                    let left = formula_support::expression(
                        left,
                        binding,
                        context.limits,
                        context.budget,
                        context.counters,
                        context.location,
                    )?;
                    let right = formula_support::expression(
                        right,
                        binding,
                        context.limits,
                        context.budget,
                        context.counters,
                        context.location,
                    )?;
                    query.boolean(formula_support::compare(&left, *relation, &right), context)?
                }
                LiteralIr::Guard(guard) => {
                    let value = guard.evaluate(
                        binding,
                        context.limits,
                        context.budget,
                        context.counters,
                        context.location,
                    )?;
                    query.boolean(value, context)?
                }
                LiteralIr::Bind { .. } | LiteralIr::Range { .. } => query.boolean(true, context)?,
                _ => return Err(refusal(context.location)),
            };
            root = query.node(
                ConditionNode::And(root, node),
                query.activity[root].min(query.activity[node]),
                context,
            )?;
        }
        Ok(CompletedCondition {
            activity: query.activity[root],
            query: Condition::new(query.nodes),
        })
    }
}

struct Query {
    nodes: Vec<ConditionNode>,
    activity: Vec<Activity>,
}

impl Query {
    fn node(
        &mut self,
        node: ConditionNode,
        activity: Activity,
        context: &mut Context<'_>,
    ) -> Result<usize, FormulaFailure> {
        context.work()?;
        if self.nodes.len() >= context.limits.objective.max_condition_nodes {
            return Err(FormulaFailure::Objective {
                error: zetesis_objective::AdmissionError::Limit {
                    resource: zetesis_objective::AdmissionResource::ConditionNodes,
                    template: None,
                    actual: self.nodes.len().saturating_add(1),
                    limit: context.limits.objective.max_condition_nodes,
                },
                location: context.location,
            });
        }
        self.nodes
            .try_reserve(1)
            .map_err(|_| context.allocation())?;
        self.activity
            .try_reserve(1)
            .map_err(|_| context.allocation())?;
        let index = self.nodes.len();
        self.nodes.push(node);
        self.activity.push(activity);
        Ok(index)
    }

    fn boolean(&mut self, value: bool, context: &mut Context<'_>) -> Result<usize, FormulaFailure> {
        self.node(
            ConditionNode::Boolean(value),
            if value {
                Activity::Required
            } else {
                Activity::Absent
            },
            context,
        )
    }

    fn negate(&mut self, node: usize, context: &mut Context<'_>) -> Result<usize, FormulaFailure> {
        self.node(
            ConditionNode::Not(node),
            self.activity[node].negate(),
            context,
        )
    }
}
