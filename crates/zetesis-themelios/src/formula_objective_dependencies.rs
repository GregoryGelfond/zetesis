//! Structural objective observers over total scalar aggregate assignments.

mod forwarding;
mod presence;

pub(crate) use presence::check as check_presence;

use std::collections::{BTreeMap, BTreeSet};

use themelios_analysis::{
    Analysis,
    depend::{DependencyGraph, DependencyKind},
};
use themelios_base::span::Location;
use themelios_program::program::{DefaultNegation, Program, Statement};
use themelios_program::symbol::{Name, Signature};
use zetesis_core::{Filter, Predicate, Term};

use crate::diagnostic::unsupported;
use crate::formula_ir::{HeadIr, HeadLiteral, LiteralIr, ObjectiveIr, RuleIr};
use crate::{FormulaFailure, ProfileFeature, extended};

pub(crate) fn check(
    rules: &[RuleIr],
    objectives: &[ObjectiveIr],
    analysis: &Analysis,
    source: &Program,
    fallback: Location,
) -> Result<BTreeSet<usize>, FormulaFailure> {
    if objectives.is_empty() {
        return Ok(BTreeSet::new());
    }
    let graph = analysis.dependencies();
    let relevant = dependency_closure(
        graph,
        objectives
            .iter()
            .flat_map(|objective| &objective.positive)
            .map(|atom| signature(atom.predicate())),
    );
    let mut generated = BTreeMap::<Signature, BTreeSet<usize>>::new();
    for rule in rules {
        if !relevant_head(&rule.head, &relevant) {
            continue;
        }
        // The existing total-observer certificate assumes no filter or value
        // consumer of its aggregate output. A scheduled proposal does not prove
        // that the objective priority survives grounding simplification.
        if rule.bindings.as_ref().is_some_and(|plan| plan.consumers) {
            return Err(refusal(rule.location));
        }
        if rule
            .body
            .iter()
            .any(|literal| matches!(literal, LiteralIr::Conditional(_)))
        {
            return Err(unsupported(
                ProfileFeature::ObjectiveConditionalDependency,
                rule.location,
            )
            .into());
        }
        head_profile(rule)?;
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
            return Err(refusal(rule.location));
        };
        let (Some(target), HeadIr::Normal(Some(head))) = (aggregate.binding, &rule.head) else {
            return Err(refusal(rule.location));
        };
        let positions: BTreeSet<_> = head
            .terms()
            .iter()
            .enumerate()
            .filter_map(|(index, term)| (*term == Term::Variable(target)).then_some(index))
            .collect();
        if positions.is_empty() {
            return Err(refusal(rule.location));
        }
        generated
            .entry(signature(head.predicate()))
            .or_default()
            .extend(positions);
    }
    let forwarded = forwarding::certify(rules, graph, &relevant, &mut generated);
    for producer in graph.predicates() {
        if !relevant.contains(producer) {
            continue;
        }
        for (kind, dependency) in graph.edges_from(producer) {
            if kind == DependencyKind::Negative {
                return Err(unsupported(
                    ProfileFeature::ObjectiveNegativeDependency,
                    origin(source, producer, dependency, fallback),
                )
                .into());
            }
            // Generated positions follow certified argument permutations before
            // downstream consumers are checked. The original rules and aggregate
            // equalities remain responsible for realization in each model.
            if generated.contains_key(dependency)
                && !forwarded.contains(producer)
                && !total_dependency(rules, producer, dependency, &generated)
            {
                return Err(refusal(origin(source, producer, dependency, fallback)));
            }
        }
    }
    for objective in objectives {
        observer(objective, &generated)?;
    }
    Ok(presence::required(rules, objectives, graph, &generated))
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

fn head_profile(rule: &RuleIr) -> Result<(), FormulaFailure> {
    if matches!(rule.head, HeadIr::Disjunction(_)) {
        return Err(unsupported(
            ProfileFeature::ObjectiveDisjunctionDependency,
            rule.location,
        )
        .into());
    }
    if let HeadIr::Choice(group) = &rule.head
        && group
            .elements
            .iter()
            .any(|element| element.key.tuple().is_some())
    {
        return Err(refusal(rule.location));
    }
    Ok(())
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
) -> Result<(), FormulaFailure> {
    let mut occurrences = BTreeMap::<usize, usize>::new();
    for atom in &objective.positive {
        for term in atom.terms() {
            if let Term::Variable(variable) = term {
                *occurrences.entry(*variable).or_default() += 1;
            }
        }
    }
    for atom in &objective.positive {
        let Some(positions) = generated.get(&signature(atom.predicate())) else {
            continue;
        };
        for &position in positions {
            let Term::Variable(variable) = atom.terms()[position] else {
                return Err(refusal(objective.location));
            };
            if occurrences.get(&variable) != Some(&1)
                || objective.filters.iter().any(|filter| {
                    let (left, right) = match filter {
                        Filter::Eq(left, right) | Filter::Neq(left, right) => (left, right),
                    };
                    *left == Term::Variable(variable) || *right == Term::Variable(variable)
                })
            {
                return Err(refusal(objective.location));
            }
            // Proposal values in generated positions are not an exact priority
            // carrier. Until a correlated eligibility certificate is available,
            // a priority may only read the independently bound input columns.
            if objective.priority.inputs().any(|input| input == variable) {
                return Err(refusal(objective.location));
            }
        }
    }
    Ok(())
}
fn signature(predicate: &Predicate) -> Signature {
    Signature {
        sign: crate::coherence::source_sign(predicate.sign()),
        name: Name::new(predicate.name()).expect("validated source predicate"),
        arity: u32::try_from(predicate.arity()).expect("bounded source arity"),
    }
}
fn origin(
    source: &Program,
    producer: &Signature,
    dependency: &Signature,
    fallback: Location,
) -> Location {
    source
        .statements()
        .find_map(|statement| {
            if let Statement::Rule(rule) = statement.get()
                && rule.head_signatures().any(|head| head == *producer)
                && rule.body_signatures().any(|(_, body)| body == *dependency)
            {
                return Some(extended::origin(statement, fallback));
            }
            None
        })
        .unwrap_or(fallback)
}
fn refusal(location: Location) -> FormulaFailure {
    unsupported(ProfileFeature::ObjectiveAggregateDependency, location).into()
}
