//! Select source-eligibility precision without changing the original query.
//!
//! Nonqualification selects ordinary possible-support eligibility. A failed
//! component read or work permit is an error, never a precision certificate.

mod forwarding;
mod presence;
pub(crate) use presence::{Presence, check as check_presence};

use crate::formula_ir::{HeadIr, LiteralIr, ObjectiveIr, RuleIr};
use crate::formula_source_activity::signature;
use crate::formula_support::{Counters, components::Pattern};
use crate::{FormulaFailure, FormulaLimits};
use std::collections::{BTreeMap, BTreeSet};
use themelios_analysis::{
    Analysis,
    depend::{DependencyGraph, DependencyKind},
};
use themelios_base::span::Location;
use themelios_program::program::DefaultNegation;
use themelios_program::symbol::Signature;
use zetesis_core::{PatternRef, TemplateComponentsRef, TemplateTerm};

struct Context<'a, 'source> {
    components: TemplateComponentsRef<'source>,
    limits: &'a FormulaLimits,
    counters: &'a mut Counters,
    location: Location,
}
impl<'source> Context<'_, 'source> {
    fn work(&mut self) -> Result<(), FormulaFailure> {
        self.counters.work(self.limits, self.location)
    }
    fn pattern(&mut self, pattern: Pattern) -> Result<PatternRef<'source>, FormulaFailure> {
        pattern.get(self.components, self.limits, self.counters, self.location)
    }
    fn term(
        &mut self,
        pattern: PatternRef<'source>,
        column: usize,
    ) -> Result<TemplateTerm<'source>, FormulaFailure> {
        self.work()?;
        pattern
            .terms()
            .at(column)
            .ok_or_else(|| crate::formula_support::components::missing(self.location))
    }
    fn signature(&mut self, pattern: Pattern) -> Result<Signature, FormulaFailure> {
        let pattern = self.pattern(pattern)?;
        self.work()?;
        let predicate = pattern.predicate();
        self.counters
            .charge_work(predicate.name().len() as u128, self.limits, self.location)?;
        Ok(signature(predicate))
    }
    fn literal_signature(
        &mut self,
        literal: &LiteralIr,
    ) -> Result<Option<Signature>, FormulaFailure> {
        match literal {
            LiteralIr::Atom(_, atom) => self.signature(*atom).map(Some),
            LiteralIr::PatternAtom(pattern) => self.signature(pattern.atom).map(Some),
            LiteralIr::ProjectedAtom(_, projection) => {
                let predicate = projection.predicate(
                    self.components,
                    self.limits,
                    self.counters,
                    self.location,
                )?;
                self.counters.charge_work(
                    predicate.name().len() as u128,
                    self.limits,
                    self.location,
                )?;
                Ok(Some(signature(predicate)))
            }
            _ => Ok(None),
        }
    }
}

pub(crate) fn check(
    rules: &[RuleIr],
    objectives: &mut [ObjectiveIr],
    analysis: &Analysis,
    components: TemplateComponentsRef<'_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
) -> Result<BTreeSet<usize>, FormulaFailure> {
    let mut extrema = BTreeSet::new();
    for objective in objectives {
        let mut context = Context {
            components,
            limits,
            counters,
            location: objective.location,
        };
        let graph = analysis.dependencies();
        let mut roots = Vec::new();
        for literal in objective.condition.literals() {
            // Preserve the original observer roots: projected/compound bodies
            // force the conservative path rather than broadening this proof.
            let atom = match literal {
                LiteralIr::Atom(_, atom) => Some(*atom),
                LiteralIr::PatternAtom(pattern) => Some(pattern.atom),
                _ => None,
            };
            if let Some(atom) = atom {
                roots.push(context.signature(atom)?);
            }
        }
        let relevant = dependency_closure(graph, roots);
        let precision = if needs_eligibility_query(
            rules,
            std::slice::from_ref(objective),
            graph,
            &relevant,
            &mut context,
        )? {
            None
        } else {
            positive_precision(
                rules,
                std::slice::from_mut(objective),
                graph,
                &relevant,
                &mut context,
            )?
        };
        if let Some(required) = precision {
            extrema.extend(required);
        } else {
            objective.needs_eligibility_query = true;
        }
    }
    Ok(extrema)
}

fn positive_precision(
    rules: &[RuleIr],
    objectives: &mut [ObjectiveIr],
    graph: &DependencyGraph,
    relevant: &BTreeSet<Signature>,
    context: &mut Context<'_, '_>,
) -> Result<Option<BTreeSet<usize>>, FormulaFailure> {
    let mut generated = BTreeMap::<Signature, BTreeSet<usize>>::new();
    for rule in rules {
        context.location = rule.location;
        if !relevant_head(&rule.head, relevant, context)? {
            continue;
        }
        if rule.bindings.as_ref().is_some_and(|plan| plan.consumers)
            || rule
                .body
                .iter()
                .any(|literal| matches!(literal, LiteralIr::Conditional(_)))
            || !head_profile(rule)
        {
            return Ok(None);
        }
        let mut aggregates = rule.body.iter().filter_map(|literal| match literal {
            LiteralIr::Aggregate(value) => Some(value),
            _ => None,
        });
        let Some(aggregate) = aggregates.next() else {
            continue;
        };
        if aggregates.next().is_some() {
            return Ok(None);
        }
        let (Some(target), HeadIr::Normal(Some(head))) = (aggregate.binding, &rule.head) else {
            return Ok(None);
        };
        let view = context.pattern(*head)?;
        let mut positions = BTreeSet::new();
        for index in 0..view.terms().len() {
            if context.term(view, index)? == TemplateTerm::Variable(target) {
                positions.insert(index);
            }
        }
        if positions.is_empty() {
            return Ok(None);
        }
        generated
            .entry(context.signature(*head)?)
            .or_default()
            .extend(positions);
    }
    let forwarded = forwarding::certify(rules, graph, relevant, &mut generated, context)?;
    for producer in graph.predicates() {
        if !relevant.contains(producer) {
            continue;
        }
        for (kind, dependency) in graph.edges_from(producer) {
            if kind == DependencyKind::Negative {
                return Ok(None);
            }
            if generated.contains_key(dependency)
                && !forwarded.contains(producer)
                && !total_dependency(rules, producer, dependency, &generated, context)?
            {
                return Ok(None);
            }
        }
    }
    for objective in objectives.iter_mut() {
        context.location = objective.location;
        objective.priority_sources = observer(objective, &generated, context)?;
    }
    presence::required(rules, objectives, graph, &generated, context).map(Some)
}

fn needs_eligibility_query(
    rules: &[RuleIr],
    objectives: &[ObjectiveIr],
    graph: &DependencyGraph,
    relevant: &BTreeSet<Signature>,
    context: &mut Context<'_, '_>,
) -> Result<bool, FormulaFailure> {
    let queried = objectives.iter().any(|objective| {
        objective.condition.literals().iter().any(|literal| {
            !matches!(
                literal,
                LiteralIr::Atom(DefaultNegation::None, _) | LiteralIr::Compare(..)
            )
        })
    });
    let mut expanded = relevant.iter().any(|predicate| {
        graph
            .edges_from(predicate)
            .any(|(kind, _)| kind == DependencyKind::Negative)
    });
    let mut ordinary = true;
    for rule in rules {
        context.location = rule.location;
        if !relevant_head(&rule.head, relevant, context)? {
            continue;
        }
        expanded |= match &rule.head {
            HeadIr::Disjunction(_) | HeadIr::ConditionalDisjunction { .. } => true,
            HeadIr::Choice(group) => group
                .elements
                .iter()
                .any(|element| element.key.tuple().is_some()),
            HeadIr::Normal(_) => false,
        };
        ordinary &= rule.body.iter().all(|literal| {
            matches!(
                literal,
                LiteralIr::Atom(..)
                    | LiteralIr::Compare(..)
                    | LiteralIr::Guard(_)
                    | LiteralIr::HeadGuard(_)
                    | LiteralIr::Bind { .. }
                    | LiteralIr::Range { .. }
            )
        });
    }
    Ok(queried || (expanded && ordinary))
}

/// Source analysis names remain the independent analysis library's vocabulary.
fn dependency_closure(
    graph: &DependencyGraph,
    roots: impl IntoIterator<Item = Signature>,
) -> BTreeSet<Signature> {
    let mut relevant = BTreeSet::new();
    let mut pending = Vec::new();
    for predicate in roots {
        if relevant.insert(predicate.clone()) {
            pending.push(predicate);
        }
    }
    while let Some(predicate) = pending.pop() {
        for (_, dependency) in graph.edges_from(&predicate) {
            if relevant.insert(dependency.clone()) {
                pending.push(dependency.clone());
            }
        }
    }
    relevant
}
fn head_profile(rule: &RuleIr) -> bool {
    match &rule.head {
        HeadIr::Disjunction(_) | HeadIr::ConditionalDisjunction { .. } => false,
        HeadIr::Choice(group) => group
            .elements
            .iter()
            .all(|element| element.key.tuple().is_none()),
        HeadIr::Normal(_) => true,
    }
}
fn heads(head: &HeadIr, positive: bool) -> impl Iterator<Item = Pattern> + '_ {
    let normal = match head {
        HeadIr::Normal(value) => *value,
        _ => None,
    };
    let choices = match head {
        HeadIr::Choice(group) => Some(group.elements.as_slice()),
        _ => None,
    };
    normal
        .into_iter()
        .chain(head.disjuncts().filter_map(|head| head.atom().copied()))
        .chain(choices.into_iter().flatten().filter_map(move |element| {
            if positive {
                element.head.positive_atom().copied()
            } else {
                element.head.atom().copied()
            }
        }))
}
fn relevant_head(
    head: &HeadIr,
    relevant: &BTreeSet<Signature>,
    context: &mut Context<'_, '_>,
) -> Result<bool, FormulaFailure> {
    for atom in heads(head, false) {
        if relevant.contains(&context.signature(atom)?) {
            return Ok(true);
        }
    }
    Ok(false)
}
fn total_dependency(
    rules: &[RuleIr],
    producer: &Signature,
    dependency: &Signature,
    generated: &BTreeMap<Signature, BTreeSet<usize>>,
    context: &mut Context<'_, '_>,
) -> Result<bool, FormulaFailure> {
    for rule in rules {
        context.location = rule.location;
        if !matches!(rule.head, HeadIr::Normal(_)) {
            for atom in heads(&rule.head, true) {
                if context.signature(atom)? == *producer {
                    return Ok(false);
                }
            }
            continue;
        }
        let HeadIr::Normal(Some(head)) = &rule.head else {
            continue;
        };
        if context.signature(*head)? != *producer {
            continue;
        }
        for literal in &rule.body {
            if context.literal_signature(literal)?.as_ref() == Some(dependency) {
                return Ok(false);
            }
            let LiteralIr::Aggregate(aggregate) = literal else {
                continue;
            };
            for element in &aggregate.elements {
                let mut ordinary_dependency = false;
                for literal in &element.condition {
                    if context.literal_signature(literal)?.as_ref() != Some(dependency) {
                        continue;
                    }
                    if matches!(literal, LiteralIr::Atom(..)) {
                        ordinary_dependency = true;
                    } else {
                        return Ok(false);
                    }
                }
                if !ordinary_dependency {
                    continue;
                }
                let [LiteralIr::Atom(DefaultNegation::None, atom)] = element.condition.as_slice()
                else {
                    return Ok(false);
                };
                if aggregate.binding.is_none() {
                    return Ok(false);
                }
                let atom = context.pattern(*atom)?;
                for &position in &generated[dependency] {
                    let TemplateTerm::Variable(variable) = context.term(atom, position)? else {
                        return Ok(false);
                    };
                    let mut occurrences = 0;
                    for column in 0..atom.terms().len() {
                        if context.term(atom, column)? == TemplateTerm::Variable(variable) {
                            occurrences += 1;
                        }
                    }
                    if occurrences != 1 {
                        return Ok(false);
                    }
                }
            }
        }
    }
    Ok(true)
}
fn observer(
    objective: &ObjectiveIr,
    generated: &BTreeMap<Signature, BTreeSet<usize>>,
    context: &mut Context<'_, '_>,
) -> Result<BTreeSet<usize>, FormulaFailure> {
    let mut priorities = BTreeSet::new();
    let mut occurrences = BTreeMap::<usize, usize>::new();
    for atom in &objective.positive {
        let atom = context.pattern(*atom)?;
        for column in 0..atom.terms().len() {
            if let TemplateTerm::Variable(variable) = context.term(atom, column)? {
                *occurrences.entry(variable).or_default() += 1;
            }
        }
    }
    for (index, coordinate) in objective.positive.iter().enumerate() {
        let Some(positions) = generated.get(&context.signature(*coordinate)?) else {
            continue;
        };
        let atom = context.pattern(*coordinate)?;
        for &position in positions {
            let required = match context.term(atom, position)? {
                TemplateTerm::Constant(_) => true,
                TemplateTerm::Variable(variable) => {
                    let mut filtered = false;
                    for filter in &objective.filters {
                        let filter = filter.get(
                            context.components,
                            context.limits,
                            context.counters,
                            context.location,
                        )?;
                        context.work()?;
                        let (left, right) = filter.terms();
                        filtered |= left == TemplateTerm::Variable(variable)
                            || right == TemplateTerm::Variable(variable);
                    }
                    occurrences.get(&variable) != Some(&1)
                        || filtered
                        || objective.priority.inputs().any(|input| input == variable)
                        || (objective.weight.term().is_none() && objective.weight.uses(variable))
                        || objective
                            .tuple
                            .iter()
                            .any(|field| field.term().is_none() && field.uses(variable))
                }
            };
            if required {
                priorities.insert(index);
            }
        }
    }
    Ok(priorities)
}
