//! Total acyclic relation renamings preserve generated argument positions.

use super::{Context, HeadIr, LiteralIr, RuleIr, heads};
use crate::FormulaFailure;
use std::collections::{BTreeMap, BTreeSet};
use themelios_analysis::depend::DependencyGraph;
use themelios_program::program::DefaultNegation;
use themelios_program::symbol::Signature;
use zetesis_core::{PatternRef, TemplateTerm};

pub(super) fn certify(
    rules: &[RuleIr],
    graph: &DependencyGraph,
    relevant: &BTreeSet<Signature>,
    generated: &mut BTreeMap<Signature, BTreeSet<usize>>,
    context: &mut Context<'_, '_>,
) -> Result<BTreeSet<Signature>, FormulaFailure> {
    let mut forwarded = BTreeSet::new();
    if generated.is_empty() {
        return Ok(forwarded);
    }
    let definitions = definitions(rules, context)?;
    for component in graph.components().filter(|group| !group.is_recursive()) {
        for producer in component.members() {
            if !relevant.contains(producer) || generated.contains_key(producer) {
                continue;
            }
            let Some(Some(rule)) = definitions.get(producer) else {
                continue;
            };
            context.location = rule.location;
            let (HeadIr::Normal(Some(head)), [LiteralIr::Atom(DefaultNegation::None, body)]) =
                (&rule.head, rule.body.as_slice())
            else {
                continue;
            };
            let Some(positions) = generated.get(&context.signature(*body)?) else {
                continue;
            };
            let head = context.pattern(*head)?;
            let body = context.pattern(*body)?;
            let Some(transported) = permutation(head, body, positions, context)? else {
                continue;
            };
            generated.insert(producer.clone(), transported);
            forwarded.insert(producer.clone());
        }
    }
    Ok(forwarded)
}

/// Missing means no definition; None means competing producer occurrences.
/// Default-negated choice operands never supply positive producer permission.
fn definitions<'rules>(
    rules: &'rules [RuleIr],
    context: &mut Context<'_, '_>,
) -> Result<BTreeMap<Signature, Option<&'rules RuleIr>>, FormulaFailure> {
    let mut definitions = BTreeMap::new();
    for rule in rules {
        context.location = rule.location;
        for atom in heads(&rule.head, true) {
            definitions
                .entry(context.signature(atom)?)
                .and_modify(|definition| *definition = None)
                .or_insert(Some(rule));
        }
    }
    Ok(definitions)
}

fn variables(
    atom: PatternRef<'_>,
    context: &mut Context<'_, '_>,
) -> Result<Option<BTreeSet<usize>>, FormulaFailure> {
    let mut variables = BTreeSet::new();
    for column in 0..atom.terms().len() {
        context.work()?;
        match atom.terms().at(column).expect("column within pattern") {
            TemplateTerm::Variable(variable) => {
                variables.insert(variable);
            }
            TemplateTerm::Constant(_) => return Ok(None),
        }
    }
    Ok(Some(variables))
}

fn permutation<'source>(
    head: PatternRef<'source>,
    body: PatternRef<'source>,
    generated: &BTreeSet<usize>,
    context: &mut Context<'_, 'source>,
) -> Result<Option<BTreeSet<usize>>, FormulaFailure> {
    let (Some(inputs), Some(outputs)) = (variables(body, context)?, variables(head, context)?)
    else {
        return Ok(None);
    };
    if inputs.len() != body.terms().len()
        || outputs.len() != head.terms().len()
        || inputs != outputs
    {
        return Ok(None);
    }
    let mut transported = BTreeSet::new();
    for position in 0..head.terms().len() {
        let term = context.term(head, position)?;
        for &input in generated {
            if context.term(body, input)? == term {
                transported.insert(position);
                break;
            }
        }
    }
    Ok(Some(transported))
}
