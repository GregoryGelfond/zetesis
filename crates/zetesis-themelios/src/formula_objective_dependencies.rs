//! Structural objective observers over total scalar aggregate assignments.

use std::collections::{BTreeMap, BTreeSet};

use themelios_analysis::{Analysis, depend::DependencyKind};
use themelios_base::span::Location;
use themelios_program::program::{DefaultNegation, Program, Statement};
use themelios_program::symbol::{Name, Signature};
use zetesis_core::{Filter, Predicate, Term};

use crate::diagnostic::unsupported;
use crate::formula_ir::{HeadIr, LiteralIr, ObjectiveIr, RuleIr};
use crate::{FormulaFailure, ProfileFeature, extended};

pub(crate) fn check(
    rules: &[RuleIr],
    objectives: &[ObjectiveIr],
    analysis: &Analysis,
    source: &Program,
    fallback: Location,
) -> Result<(), FormulaFailure> {
    let graph = analysis.dependencies();
    let mut relevant = BTreeSet::new();
    let mut pending = Vec::new();
    for atom in objectives
        .iter()
        .flat_map(|objective| objective.template.positive())
    {
        let predicate = signature(atom.predicate());
        if relevant.insert(predicate.clone()) {
            pending.push(predicate);
        }
    }
    // Each predicate and edge is visited at most once after bounded analysis.
    // Only producers that can feed an objective affect its slot-presence policy.
    while let Some(predicate) = pending.pop() {
        for (_, dependency) in graph.edges_from(&predicate) {
            if relevant.insert(dependency.clone()) {
                pending.push(dependency.clone());
            }
        }
    }
    let mut generated = BTreeMap::<Signature, BTreeSet<usize>>::new();
    for rule in rules {
        if !relevant_head(&rule.head, &relevant) {
            continue;
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
            // A total assignment may consume another total assignment through
            // an unfiltered aggregate tuple source. Ordinary producer joins and
            // comparisons over generated values still require a broader contract.
            if generated.contains_key(dependency)
                && !total_dependency(rules, producer, dependency, &generated)
            {
                return Err(refusal(origin(source, producer, dependency, fallback)));
            }
        }
    }
    for objective in objectives {
        observer(objective, &generated)?;
    }
    Ok(())
}
fn head_profile(rule: &RuleIr) -> Result<(), FormulaFailure> {
    if matches!(rule.head, HeadIr::Disjunction(_)) {
        return Err(unsupported(
            ProfileFeature::ObjectiveDisjunctionDependency,
            rule.location,
        )
        .into());
    }
    if let HeadIr::Choice { elements, .. } = &rule.head
        && elements.iter().any(|element| element.count_tuple.is_some())
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
            .any(|head| relevant.contains(&signature(head.atom.predicate()))),
        HeadIr::Choice { elements, .. } => elements
            .iter()
            .any(|element| relevant.contains(&signature(element.head.predicate()))),
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
                .any(|head| signature(head.atom.predicate()) == *producer)
        {
            return false;
        }
        if let HeadIr::Choice { elements, .. } = &rule.head
            && elements
                .iter()
                .any(|element| signature(element.head.predicate()) == *producer)
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
                LiteralIr::ProjectedAtom(_, projection)
                    if signature(&projection.predicate) == *dependency =>
                {
                    return false;
                }
                LiteralIr::Aggregate(aggregate) => {
                    for element in &aggregate.elements {
                        if element.condition.iter().any(|literal| {
                            matches!(literal,
                            LiteralIr::ProjectedAtom(_, projection)
                                if signature(&projection.predicate) == *dependency)
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
    for atom in objective.template.positive() {
        for term in atom.terms() {
            if let Term::Variable(variable) = term {
                *occurrences.entry(*variable).or_default() += 1;
            }
        }
    }
    for atom in objective.template.positive() {
        let Some(positions) = generated.get(&signature(atom.predicate())) else {
            continue;
        };
        for &position in positions {
            let Term::Variable(variable) = atom.terms()[position] else {
                return Err(refusal(objective.location));
            };
            if occurrences.get(&variable) != Some(&1)
                || objective.template.filters().iter().any(|filter| {
                    let (left, right) = match filter {
                        Filter::Eq(left, right) | Filter::Neq(left, right) => (left, right),
                    };
                    *left == Term::Variable(variable) || *right == Term::Variable(variable)
                })
            {
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
