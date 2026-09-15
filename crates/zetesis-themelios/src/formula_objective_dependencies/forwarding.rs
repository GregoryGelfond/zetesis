//! Total relation renamings on acyclic paths from aggregate assignments.
//!
//! A single positive body atom with a variable permutation in the head neither
//! filters nor invents rows. Transport its generated positions, rather than
//! treating its output as an ordinary unrestricted objective domain. Each
//! predicate is visited once in the frontend's dependency order; retained state
//! is bounded by the analyzed heads and their argument positions. This is a
//! structural certificate, not a transformation of the source or its theory.

use std::collections::{BTreeMap, BTreeSet};

use themelios_analysis::depend::DependencyGraph;
use themelios_program::program::DefaultNegation;
use themelios_program::symbol::Signature;
use zetesis_core::{AtomPattern, Term};

use super::{HeadIr, LiteralIr, RuleIr, signature};

pub(super) fn certify(
    rules: &[RuleIr],
    graph: &DependencyGraph,
    relevant: &BTreeSet<Signature>,
    generated: &mut BTreeMap<Signature, BTreeSet<usize>>,
) -> BTreeSet<Signature> {
    let mut forwarded = BTreeSet::new();
    if generated.is_empty() {
        return forwarded;
    }
    let definitions = definitions(rules);
    // Dependency components are in producer-before-consumer order. A recursive
    // component cannot acquire a forwarding certificate, even if its assignment
    // seed already belongs to the previously supported total-observer profile.
    for component in graph.components().filter(|group| !group.is_recursive()) {
        for producer in component.members() {
            if !relevant.contains(producer) || generated.contains_key(producer) {
                continue;
            }
            let Some(Some(rule)) = definitions.get(producer) else {
                continue;
            };
            let (HeadIr::Normal(Some(head)), [LiteralIr::Atom(DefaultNegation::None, body)]) =
                (&rule.head, rule.body.as_slice())
            else {
                continue;
            };
            let Some(positions) = generated.get(&signature(body.predicate())) else {
                continue;
            };
            let Some(transported) = permutation(head, body, positions) else {
                continue;
            };
            generated.insert(producer.clone(), transported);
            forwarded.insert(producer.clone());
        }
    }
    forwarded
}

/// `None` records multiple definitions; a missing entry records no definition.
/// Unsigned choice occurrences count, so an ordinary rule cannot certify a
/// predicate that also has an independent producer. Default-negated choice
/// operands impose no producer permission; their active bounds remain in the
/// original theory. Disjunctive objective dependencies retain their refusal.
fn definitions(rules: &[RuleIr]) -> BTreeMap<Signature, Option<&RuleIr>> {
    let mut definitions = BTreeMap::new();
    for rule in rules {
        let mut record = |atom: &AtomPattern| {
            definitions
                .entry(signature(atom.predicate()))
                .and_modify(|definition| *definition = None)
                .or_insert(Some(rule));
        };
        match &rule.head {
            HeadIr::Normal(Some(atom)) => record(atom),
            HeadIr::Normal(None) => {}
            HeadIr::Disjunction(_) | HeadIr::ConditionalDisjunction { .. } => {
                for head in rule.head.disjuncts() {
                    if let Some(atom) = head.atom() {
                        record(atom);
                    }
                }
            }
            HeadIr::Choice(group) => {
                for element in &group.elements {
                    if let Some(head) = element.head.positive_atom() {
                        record(head);
                    }
                }
            }
        }
    }
    definitions
}

fn permutation(
    head: &AtomPattern,
    body: &AtomPattern,
    generated: &BTreeSet<usize>,
) -> Option<BTreeSet<usize>> {
    let variables = |atom: &AtomPattern| {
        atom.terms()
            .iter()
            .map(|term| match term {
                Term::Variable(variable) => Some(*variable),
                Term::Constant(_) => None,
            })
            .collect::<Option<BTreeSet<_>>>()
    };
    let inputs = variables(body)?;
    let outputs = variables(head)?;
    if inputs.len() != body.terms().len()
        || outputs.len() != head.terms().len()
        || inputs != outputs
    {
        return None;
    }
    Some(
        head.terms()
            .iter()
            .enumerate()
            .filter_map(|(position, term)| {
                generated
                    .iter()
                    .any(|&input| body.terms()[input] == *term)
                    .then_some(position)
            })
            .collect(),
    )
}
