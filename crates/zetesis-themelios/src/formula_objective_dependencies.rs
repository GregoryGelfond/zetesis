//! Select source-eligibility precision without changing the original query.
//!
//! A structural positive observer plan enables optional completed carrier
//! refinements. Nonqualification selects ordinary/possible-support completion;
//! it is not a failure of an applied certificate. Resource and evaluation errors
//! arise during materialization and always propagate.

mod forwarding;
mod presence;
pub(crate) mod completion;

pub(crate) use presence::{Presence, check as check_presence};

use std::collections::{BTreeMap, BTreeSet};

use themelios_analysis::{
    Analysis,
    depend::{DependencyGraph, DependencyKind},
};
use themelios_program::program::DefaultNegation;
use themelios_program::symbol::{Name, Signature};
use zetesis_core::{Filter, Predicate, Term};

use crate::formula_ir::{HeadIr, HeadLiteral, LiteralIr, ObjectiveIr, RuleIr};

pub(crate) fn check(
    rules: &[RuleIr],
    objectives: &mut [ObjectiveIr],
    analysis: &Analysis,
) -> BTreeSet<usize> {
    let mut extrema = BTreeSet::new();
    for objective in objectives {
        let graph = analysis.dependencies();
        let relevant = dependency_closure(
            graph,
            objective
                .condition
                .literals()
                .iter()
                .filter_map(|literal| match literal {
                    LiteralIr::Atom(_, atom) => Some(signature(atom.predicate())),
                    LiteralIr::PatternAtom(pattern) => Some(signature(pattern.atom.predicate())),
                    _ => None,
                }),
        );
        let precision =
            if completed_profile(rules, std::slice::from_ref(objective), graph, &relevant) {
                None
            } else {
                positive_precision(rules, std::slice::from_mut(objective), graph, &relevant)
            };
        if let Some(required) = precision {
            extrema.extend(required);
        } else {
            objective.source_completion = true;
        }
    }
    extrema
}

/// This is an applicability decision over admitted source structure. `None`
/// requires the conservative carrier, never a guessed exact source priority.
fn positive_precision(
    rules: &[RuleIr],
    objectives: &mut [ObjectiveIr],
    graph: &DependencyGraph,
    relevant: &BTreeSet<Signature>,
) -> Option<BTreeSet<usize>> {
    let mut generated = BTreeMap::<Signature, BTreeSet<usize>>::new();
    for rule in rules {
        if !relevant_head(&rule.head, relevant) {
            continue;
        }
        // The existing total-observer certificate assumes no filter or value
        // consumer of its aggregate output. A scheduled proposal does not justify
        // the more precise flat carrier; completed support remains applicable.
        if rule.bindings.as_ref().is_some_and(|plan| plan.consumers) {
            return None;
        }
        if rule
            .body
            .iter()
            .any(|literal| matches!(literal, LiteralIr::Conditional(_)))
        {
            return None;
        }
        if !head_profile(rule) {
            return None;
        }
        let aggregates: Vec<_> = rule
            .body
            .iter()
            .filter_map(|literal| {
                if let LiteralIr::Aggregate(value) = literal {
                    Some(value)
                } else {
                    None
                }
            })
            .collect();
        if aggregates.is_empty() {
            continue;
        }
        let [aggregate] = aggregates.as_slice() else {
            return None;
        };
        let (Some(target), HeadIr::Normal(Some(head))) = (aggregate.binding, &rule.head) else {
            return None;
        };
        let positions: BTreeSet<_> = head
            .terms()
            .iter()
            .enumerate()
            .filter_map(|(index, term)| (*term == Term::Variable(target)).then_some(index))
            .collect();
        if positions.is_empty() {
            return None;
        }
        generated
            .entry(signature(head.predicate()))
            .or_default()
            .extend(positions);
    }
    let forwarded = forwarding::certify(rules, graph, relevant, &mut generated);
    for producer in graph.predicates() {
        if !relevant.contains(producer) {
            continue;
        }
        for (kind, dependency) in graph.edges_from(producer) {
            if kind == DependencyKind::Negative {
                return None;
            }
            // Generated positions follow certified argument permutations before
            // downstream consumers are checked. The original rules and aggregate
            // equalities remain responsible for realization in each model.
            if generated.contains_key(dependency)
                && !forwarded.contains(producer)
                && !total_dependency(rules, producer, dependency, &generated)
            {
                return None;
            }
        }
    }
    for objective in objectives.iter_mut() {
        objective.priority_sources = observer(objective, &generated);
    }
    Some(presence::required(rules, objectives, graph, &generated))
}

fn completed_profile(
    rules: &[RuleIr],
    objectives: &[ObjectiveIr],
    graph: &DependencyGraph,
    relevant: &BTreeSet<Signature>,
) -> bool {
    let queried = objectives.iter().any(|objective| {
        objective.condition.literals().iter().any(|literal| {
            !matches!(
                literal,
                LiteralIr::Atom(DefaultNegation::None, _) | LiteralIr::Compare(..)
            )
        })
    });
    let expanded = relevant.iter().any(|predicate| {
        graph
            .edges_from(predicate)
            .any(|(kind, _)| kind == DependencyKind::Negative)
    }) || rules.iter().any(|rule| {
        relevant_head(&rule.head, relevant)
            && match &rule.head {
                HeadIr::Disjunction(_) => true,
                HeadIr::Choice(group) => group
                    .elements
                    .iter()
                    .any(|element| element.key.tuple().is_some()),
                HeadIr::Normal(_) => false,
            }
    });
    let ordinary = rules
        .iter()
        .filter(|rule| relevant_head(&rule.head, relevant))
        .all(|rule| {
            rule.body.iter().all(|literal| {
                matches!(
                    literal,
                    LiteralIr::Atom(..)
                        | LiteralIr::Compare(..)
                        | LiteralIr::Guard(_)
                        | LiteralIr::Bind { .. }
                        | LiteralIr::Range { .. }
                )
            })
        });
    queried || (expanded && ordinary)
}

/// Include the roots and every producer that can feed them. Each analyzed
/// predicate and edge is visited at most once; cycles need no recursive stack.
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
        HeadIr::Disjunction(_) => false,
        HeadIr::Choice(group) => group
            .elements
            .iter()
            .all(|element| element.key.tuple().is_none()),
        HeadIr::Normal(_) => true,
    }
}
fn relevant_head(head: &HeadIr, relevant: &BTreeSet<Signature>) -> bool {
    match head {
        HeadIr::Normal(None) => false,
        HeadIr::Normal(Some(atom)) => relevant.contains(&signature(atom.predicate())),
        HeadIr::Disjunction(heads) => heads
            .iter()
            .filter_map(HeadLiteral::atom)
            .any(|atom| relevant.contains(&signature(atom.predicate()))),
        HeadIr::Choice(group) => group
            .elements
            .iter()
            .filter_map(|element| element.head.atom())
            .any(|head| relevant.contains(&signature(head.predicate()))),
    }
}
fn total_dependency(
    rules: &[RuleIr],
    producer: &Signature,
    dependency: &Signature,
    generated: &BTreeMap<Signature, BTreeSet<usize>>,
) -> bool {
    for rule in rules {
        if let HeadIr::Disjunction(heads) = &rule.head
            && heads
                .iter()
                .filter_map(HeadLiteral::atom)
                .any(|atom| signature(atom.predicate()) == *producer)
        {
            return false;
        }
        // Only an unsigned choice occurrence competes as a value producer.
        // Signed activity and bounds remain constraints of the original theory.
        if let HeadIr::Choice(group) = &rule.head
            && group
                .elements
                .iter()
                .filter_map(|element| element.head.positive_atom())
                .any(|head| signature(head.predicate()) == *producer)
        {
            return false;
        }
        let HeadIr::Normal(Some(head)) = &rule.head else {
            continue;
        };
        if signature(head.predicate()) != *producer {
            continue;
        }
        for literal in &rule.body {
            match literal {
                LiteralIr::Atom(_, atom) if signature(atom.predicate()) == *dependency => {
                    return false;
                }
                LiteralIr::PatternAtom(pattern)
                    if signature(pattern.atom.predicate()) == *dependency =>
                {
                    return false;
                }
                LiteralIr::ProjectedAtom(_, projection)
                    if signature(projection.predicate()) == *dependency =>
                {
                    return false;
                }
                LiteralIr::Aggregate(aggregate) => {
                    for element in &aggregate.elements {
                        if element.condition.iter().any(|literal| match literal {
                            LiteralIr::PatternAtom(pattern) => {
                                signature(pattern.atom.predicate()) == *dependency
                            }
                            LiteralIr::ProjectedAtom(_, projection) => {
                                signature(projection.predicate()) == *dependency
                            }
                            _ => false,
                        }) {
                            return false;
                        }
                        if !element.condition.iter().any(|literal| {
                            matches!(literal,
                            LiteralIr::Atom(_, atom) if signature(atom.predicate()) == *dependency)
                        }) {
                            continue;
                        }
                        let [LiteralIr::Atom(DefaultNegation::None, atom)] =
                            element.condition.as_slice()
                        else {
                            return false;
                        };
                        if aggregate.binding.is_none()
                            || generated[dependency].iter().any(|&position| {
                                let Term::Variable(variable) = atom.terms()[position] else {
                                    return true;
                                };
                                atom.terms()
                                    .iter()
                                    .filter(|term| **term == Term::Variable(variable))
                                    .count()
                                    != 1
                            })
                        {
                            return false;
                        }
                    }
                }
                _ => {}
            }
        }
    }
    true
}
fn observer(
    objective: &ObjectiveIr,
    generated: &BTreeMap<Signature, BTreeSet<usize>>,
) -> BTreeSet<usize> {
    let mut priorities = BTreeSet::new();
    let mut occurrences = BTreeMap::<usize, usize>::new();
    for atom in &objective.positive {
        for term in atom.terms() {
            if let Term::Variable(variable) = term {
                *occurrences.entry(*variable).or_default() += 1;
            }
        }
    }
    for (index, atom) in objective.positive.iter().enumerate() {
        let Some(positions) = generated.get(&signature(atom.predicate())) else {
            continue;
        };
        for &position in positions {
            let required = match atom.terms()[position] {
                Term::Constant(_) => true,
                Term::Variable(variable) => {
                    occurrences.get(&variable) != Some(&1)
                        || objective.filters.iter().any(|filter| {
                            let (left, right) = match filter {
                                Filter::Eq(left, right) | Filter::Neq(left, right) => (left, right),
                            };
                            *left == Term::Variable(variable) || *right == Term::Variable(variable)
                        })
                        || objective.priority.inputs().any(|input| input == variable)
                        || (objective.weight.term().is_none() && objective.weight.uses(variable))
                        || objective
                            .tuple
                            .iter()
                            .any(|field| field.term().is_none() && field.uses(variable))
                }
            };
            // A literal, filter or join can distinguish a proposal from a
            // completed source value even when the priority itself is fixed.
            if required {
                priorities.insert(index);
            }
        }
    }
    priorities
}
fn signature(predicate: &Predicate) -> Signature {
    Signature {
        sign: crate::coherence::source_sign(predicate.sign()),
        name: Name::new(predicate.name()).expect("validated source predicate"),
        arity: u32::try_from(predicate.arity()).expect("bounded source arity"),
    }
}
