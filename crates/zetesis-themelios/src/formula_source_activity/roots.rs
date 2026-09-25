//! Complete scoped dependency discovery uses a bounded borrowed-frame stack.
//!
//! Logical expressions are visited independently of truth. Every frame is
//! charged and fallibly reserved beside the existing discovery sets; no second
//! signature collection or recursive descent of source-owned nesting is used.

use crate::formula_support::components::{Pattern, Predicate};
use std::collections::BTreeSet;
use themelios_program::symbol::Signature;

use super::{Context, signature};
use crate::formula_conditional_ir::{Alternative, Consequent, ConsequentOperand};
use crate::formula_ir::{AggregateElementIr, AggregateKey, LiteralIr, Projection};
use crate::{ExpansionResource, FormulaFailure};

enum Frame<'a> {
    Literals(&'a [LiteralIr]),
    Elements(&'a [AggregateElementIr]),
    Alternatives(&'a [Alternative]),
    Projection(&'a Projection),
    Atom(Pattern),
    Predicate(Predicate),
}

impl Context<'_, '_, '_> {
    fn root(
        &mut self,
        predicate: zetesis_core::catalog::PredicateRef<'_>,
        retained: usize,
        relevant: &mut BTreeSet<Signature>,
        pending: &mut Vec<Signature>,
    ) -> Result<(), FormulaFailure> {
        self.work()?;
        let bytes = predicate.name().len() as u128;
        self.counters
            .charge_work(bytes, self.limits, self.location)?;
        self.budget
            .charge(ExpansionResource::ScalarBytes, bytes, self.location)?;
        // Signature is the existing upstream dependency-graph key. It is not a
        // second admitted predicate and never supplies atom identity.
        self.discover(signature(predicate), retained, relevant, pending)
    }

    pub(super) fn source_roots(
        &mut self,
        literals: &[LiteralIr],
        retained: usize,
        relevant: &mut BTreeSet<Signature>,
        pending: &mut Vec<Signature>,
    ) -> Result<(), FormulaFailure> {
        let mut frames = Vec::new();
        push(
            &mut frames,
            Frame::Literals(literals),
            retained,
            relevant.len(),
            self,
        )?;
        while let Some(frame) = frames.pop() {
            self.work()?;
            let mut add = |frame| push(&mut frames, frame, retained, relevant.len(), self);
            match frame {
                Frame::Literals(literals) => literal_dependencies(literals, &mut add)?,
                Frame::Elements(elements) => {
                    let Some((element, rest)) = elements.split_first() else {
                        continue;
                    };
                    if !rest.is_empty() {
                        add(Frame::Elements(rest))?;
                    }
                    if let AggregateKey::Atom(atom) = &element.key {
                        add(Frame::Atom(*atom))?;
                    }
                    add(Frame::Literals(&element.condition))?;
                }
                Frame::Alternatives(alternatives) => {
                    let Some((alternative, rest)) = alternatives.split_first() else {
                        continue;
                    };
                    if !rest.is_empty() {
                        add(Frame::Alternatives(rest))?;
                    }
                    match &alternative.operand {
                        ConsequentOperand::Atom(atom) => add(Frame::Atom(*atom))?,
                        ConsequentOperand::Projection(projection) => {
                            add(Frame::Projection(projection))?;
                        }
                    }
                    add(Frame::Literals(&alternative.bindings))?;
                }
                Frame::Projection(projection) => {
                    if let Projection::Witnesses { bindings, .. } = projection {
                        add(Frame::Literals(bindings))?;
                    }
                    match projection {
                        Projection::Arguments { predicate, .. } => {
                            add(Frame::Predicate(*predicate))?;
                        }
                        Projection::Witnesses { atom, .. } => add(Frame::Atom(*atom))?,
                    }
                }
                Frame::Atom(pattern) => {
                    let pattern = self.computation.static_pattern(
                        pattern,
                        self.limits,
                        self.counters,
                        self.location,
                    )?;
                    self.root(
                        pattern.predicate(),
                        retained.saturating_add(frames.capacity()),
                        relevant,
                        pending,
                    )?;
                }
                Frame::Predicate(predicate) => {
                    let predicate = self.computation.static_predicate(
                        predicate,
                        self.limits,
                        self.counters,
                        self.location,
                    )?;
                    self.root(
                        predicate,
                        retained.saturating_add(frames.capacity()),
                        relevant,
                        pending,
                    )?;
                }
            }
        }
        Ok(())
    }
}

/// Schedule one literal and its remaining siblings in the existing stack order.
fn literal_dependencies<'a>(
    literals: &'a [LiteralIr],
    add: &mut impl FnMut(Frame<'a>) -> Result<(), FormulaFailure>,
) -> Result<(), FormulaFailure> {
    let Some((literal, rest)) = literals.split_first() else {
        return Ok(());
    };
    if !rest.is_empty() {
        add(Frame::Literals(rest))?;
    }
    match literal {
        LiteralIr::Atom(_, atom) => add(Frame::Atom(*atom))?,
        LiteralIr::PatternAtom(pattern) => {
            add(Frame::Atom(pattern.atom))?;
        }
        LiteralIr::ProjectedAtom(_, projection) => {
            add(Frame::Projection(projection))?;
        }
        LiteralIr::Aggregate(aggregate) => {
            add(Frame::Elements(&aggregate.elements))?;
        }
        LiteralIr::Conditional(conditional) => {
            if let Consequent::Atoms(_, alternatives) = &conditional.consequent {
                add(Frame::Alternatives(alternatives))?;
            }
            add(Frame::Literals(&conditional.condition))?;
        }
        LiteralIr::Compare(..)
        | LiteralIr::ArgumentCheck { .. }
        | LiteralIr::TupleCompare(..)
        | LiteralIr::Guard(_)
        | LiteralIr::Bind { .. }
        | LiteralIr::Range { .. } => {}
    }
    Ok(())
}

fn push<'a>(
    frames: &mut Vec<Frame<'a>>,
    frame: Frame<'a>,
    retained: usize,
    predicates: usize,
    context: &mut Context<'_, '_, '_>,
) -> Result<(), FormulaFailure> {
    let count = frames.capacity().max(frames.len().saturating_add(1));
    context.entries(
        retained
            .saturating_add(predicates.saturating_mul(3))
            .saturating_add(count),
    )?;
    if frames.len() == frames.capacity() {
        context.budget.charge(
            ExpansionResource::ScalarBytes,
            std::mem::size_of::<Frame<'_>>() as u128,
            context.location,
        )?;
        frames
            .try_reserve_exact(1)
            .map_err(|_| context.allocation())?;
        context.entries(
            retained
                .saturating_add(predicates.saturating_mul(3))
                .saturating_add(frames.capacity()),
        )?;
    }
    frames.push(frame);
    Ok(())
}
