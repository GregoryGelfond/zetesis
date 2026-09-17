//! Checked source occurrences for possible-head production.
//!
//! The normalized source and the original IR occurrence array have one owner.
//! The existing themelios graph supplies signed signatures, positive dependency
//! edges and dependency-before-dependent SCC order. These qualify support
//! scheduling only; they do not certify an interpretation's membership.

#[cfg(test)]
mod tests;

mod wake;
pub(super) use wake::Schedule;
use wake::Wake;

use std::cmp::Ordering;
use std::mem::size_of;
use std::ops::Range;

use themelios_analysis::depend::DependencyKind;
use themelios_base::span::Location;
use themelios_program::symbol::{Sign, Signature};
use zetesis_core::{Predicate, relation::Failure};

use super::delta::{self, Variants};
use super::relations::Memory;
use super::{Counters, Support};
use crate::formula_domains::PositiveSource;
use crate::formula_ir::{HeadIr, LiteralIr, Prepared, RuleIr};
use crate::{FormulaFailure, FormulaLimits};

/// Original-rule slots remain distinct even when normalized source rules
/// coalesce. Constraints have no possible-head producer. Every other admitted
/// rule owns exactly its original positive body occurrence interval.
pub(super) struct ProducerPlan<'source> {
    source: PositiveSource<'source>,
    rules: Vec<Option<Range<usize>>>,
    inputs: Vec<usize>,
    wake: Wake<'source>,
    bytes: usize,
}

struct Node<'source> {
    signature: &'source Signature,
    component: Option<usize>,
}

impl<'source> ProducerPlan<'source> {
    pub(super) fn prepare(
        prepared: &'source Prepared,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<Option<Self>, FormulaFailure> {
        let Some(source) = PositiveSource::check(prepared, limits, counters, location)? else {
            return Ok(None);
        };
        let mut memory = Memory::new(size_of::<Self>(), limits, counters, location);
        memory.add(0)?;
        let mut rules = Vec::new();
        let mut inputs = Vec::new();
        let mut input_predicates = Vec::new();
        memory.add(size_of::<Vec<usize>>())?;
        memory.reserve(&mut rules, prepared.rules.len())?;
        let mut input_count = 0_usize;
        for rule in &prepared.rules {
            counters.work(limits, rule.location)?;
            if matches!(rule.head, HeadIr::Normal(Some(_))) {
                input_count = input_count
                    .checked_add(rule.body.len())
                    .ok_or_else(|| invalid(Failure::Overflow, rule.location))?;
            }
        }
        memory.reserve(&mut inputs, input_count)?;
        memory.reserve(&mut input_predicates, input_count)?;
        let nodes = graph_nodes(prepared, &mut memory, limits, counters, location)?;
        for rule in &prepared.rules {
            counters.work(limits, rule.location)?;
            let HeadIr::Normal(Some(head)) = &rule.head else {
                rules.push(None);
                continue;
            };
            let head = find(
                nodes.len(),
                |index| nodes[index].signature,
                head.predicate(),
                limits,
                counters,
                rule.location,
            )?;
            let start = inputs.len();
            for (occurrence, literal) in rule.body.iter().enumerate() {
                counters.work(limits, rule.location)?;
                let LiteralIr::Atom(_, atom) = literal else {
                    return Err(invalid(Failure::Owner, rule.location));
                };
                let body = find(
                    nodes.len(),
                    |index| nodes[index].signature,
                    atom.predicate(),
                    limits,
                    counters,
                    rule.location,
                )?;
                validate_dependency(
                    prepared,
                    &nodes,
                    (head, body),
                    limits,
                    counters,
                    rule.location,
                )?;
                inputs.push(occurrence);
                counters.work(limits, rule.location)?;
                input_predicates.push(body);
            }
            rules.push(Some(start..inputs.len()));
        }
        let wake = Wake::prepare(
            &nodes,
            &rules,
            &input_predicates,
            &mut memory,
            limits,
            counters,
            location,
        )?;
        let scratch_bytes = size_of::<Vec<Node<'_>>>()
            + nodes.capacity() * size_of::<Node<'_>>()
            + size_of::<Vec<usize>>()
            + input_predicates.capacity() * size_of::<usize>();
        drop(input_predicates);
        drop(nodes);
        memory.release(scratch_bytes);
        Ok(Some(Self {
            source,
            rules,
            inputs,
            wake,
            bytes: memory.bytes,
        }))
    }

    pub(super) fn schedule(&self) -> Schedule<'_> {
        self.wake.schedule()
    }

    /// Build the next wake set from this complete round's proposed new atoms.
    /// No consumer reads it until all corresponding catalog insertions succeed.
    pub(super) fn advance(
        &mut self,
        delta: &std::collections::BTreeSet<zetesis_core::Atom>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        self.wake.advance(delta, limits, counters, location)
    }

    pub(super) fn bytes(&self) -> usize {
        self.bytes
    }

    /// Check the original owner before using any occurrence IDs. The returned
    /// traversal still tests each previous-round frontier in source order.
    pub(super) fn variants<'a, 'rows>(
        &'a self,
        index: usize,
        rule: &'a RuleIr,
        support: &'a Support<'rows>,
        first: bool,
        limits: &FormulaLimits,
        counters: &mut Counters,
    ) -> Result<Variants<'a, 'rows>, FormulaFailure> {
        counters.work(limits, rule.location)?;
        if !self.source.contains(index, rule) {
            return Err(invalid(Failure::Owner, rule.location));
        }
        let Some(Some(inputs)) = self.rules.get(index) else {
            return Err(invalid(Failure::Owner, rule.location));
        };
        Ok(delta::prepared_variants(
            rule,
            support,
            &self.inputs[inputs.clone()],
            first,
        ))
    }
}

/// The graph already has canonical signature order. Store only borrowed keys
/// and SCC positions; neither graph edges nor logical payloads are copied.
fn graph_nodes<'source>(
    prepared: &'source Prepared,
    memory: &mut Memory<'_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: Location,
) -> Result<Vec<Node<'source>>, FormulaFailure> {
    let graph = prepared.analysis.dependencies();
    let mut count = 0_usize;
    for _ in graph.predicates() {
        counters.work(limits, location)?;
        count = count
            .checked_add(1)
            .ok_or_else(|| invalid(Failure::Overflow, location))?;
    }
    memory.add(size_of::<Vec<Node<'_>>>())?;
    let mut nodes = Vec::new();
    memory.reserve(&mut nodes, count)?;
    for signature in graph.predicates() {
        counters.work(limits, location)?;
        nodes.push(Node {
            signature,
            component: None,
        });
    }
    for (component, members) in graph.components().enumerate() {
        counters.work(limits, location)?;
        for signature in members.members() {
            let mut start = 0;
            let mut end = nodes.len();
            while start < end {
                let middle = start + (end - start) / 2;
                counters.charge_work(
                    1 + signature.name.as_str().len() as u128
                        + nodes[middle].signature.name.as_str().len() as u128,
                    limits,
                    location,
                )?;
                match nodes[middle].signature.cmp(signature) {
                    Ordering::Less => start = middle + 1,
                    Ordering::Greater => end = middle,
                    Ordering::Equal => {
                        nodes[middle].component = Some(component);
                        break;
                    }
                }
            }
        }
    }
    Ok(nodes)
}

fn find<'source>(
    count: usize,
    signature_at: impl Fn(usize) -> &'source Signature,
    predicate: &Predicate,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: Location,
) -> Result<usize, FormulaFailure> {
    let mut start = 0;
    let mut end = count;
    while start < end {
        let middle = start + (end - start) / 2;
        let signature = signature_at(middle);
        counters.charge_work(
            1 + signature.name.as_str().len() as u128 + predicate.name().len() as u128,
            limits,
            location,
        )?;
        let sign = match predicate.sign() {
            zetesis_core::Sign::Positive => Sign::Positive,
            zetesis_core::Sign::Negative => Sign::Negative,
        };
        let comparison = signature
            .sign
            .cmp(&sign)
            .then_with(|| signature.name.as_str().cmp(predicate.name()))
            .then_with(|| (u128::from(signature.arity)).cmp(&(predicate.arity() as u128)));
        match comparison {
            Ordering::Less => start = middle + 1,
            Ordering::Greater => end = middle,
            Ordering::Equal => return Ok(middle),
        }
    }
    Err(invalid(Failure::Owner, location))
}

fn validate_dependency(
    prepared: &Prepared,
    nodes: &[Node<'_>],
    edge: (usize, usize),
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: Location,
) -> Result<(), FormulaFailure> {
    let (head, body) = edge;
    let (Some(head_component), Some(body_component)) =
        (nodes[head].component, nodes[body].component)
    else {
        return Err(invalid(Failure::Owner, location));
    };
    if body_component > head_component {
        return Err(invalid(Failure::Owner, location));
    }
    let signature = nodes[head].signature;
    // The upstream graph finds the head's edges by one ordered lookup: at
    // most one comparison per level of a balanced tree over the graph's
    // nodes, each comparison bounded by the name's length.
    let levels = u128::from(usize::BITS - nodes.len().leading_zeros()) + 1;
    counters.charge_work(
        levels * (1 + signature.name.as_str().len() as u128),
        limits,
        location,
    )?;
    for (kind, target) in prepared.analysis.dependencies().edges_from(signature) {
        counters.charge_work(
            1 + target.name.as_str().len() as u128
                + nodes[body].signature.name.as_str().len() as u128,
            limits,
            location,
        )?;
        if kind == DependencyKind::Positive && target == nodes[body].signature {
            return Ok(());
        }
    }
    Err(invalid(Failure::Owner, location))
}

fn invalid(error: Failure, location: Location) -> FormulaFailure {
    FormulaFailure::SupportRelation { error, location }
}
