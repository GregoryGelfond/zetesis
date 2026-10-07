//! Necessary whole-argument equalities for fully bound structural patterns.
//!
//! Reverse preorder resolves exact existing constructor identities. It admits
//! no term and writes no incoming slot. A missing identity cannot occur in this
//! support snapshot; partial or anonymous shapes keep the ordinary matcher.

use zetesis_core::catalog::TermKey;
use zetesis_core::{TemplateTerm, ValueNodeRef};

use super::{Buffer, Computation, Context, GroundingWork, Join, PositivePattern};
use crate::FormulaFailure;
use crate::formula_binding::Binding;
use crate::formula_pattern::{ArgumentPattern, Pattern, PatternNode};

mod prepared;

#[cfg(test)]
mod tests;

pub(super) enum Selection<'source> {
    Unchanged,
    Posting(Option<&'source [usize]>),
}

/// Only caller-owned ID metadata is retained between probes. Each component's
/// existing lease accounts its header, capacity and replacement overlap.
pub(super) struct Scratch<'a> {
    stack: Binding<'static>,
    children: Buffer<usize>,
    arguments: Binding<'static>,
    prepared: prepared::Prepared<'a>,
}

impl<'a> Scratch<'a> {
    pub(super) const fn leased_header() -> usize {
        2 * size_of::<Binding<'static>>()
            + size_of::<Buffer<usize>>()
            + prepared::Prepared::leased_header()
    }

    fn new(
        patterns: &[super::PatternOccurrence<'a>],
        context: &mut Context<'_, &Computation<'_, '_>>,
    ) -> Result<Self, FormulaFailure> {
        let prepared = prepared::Prepared::new(patterns, context)?;
        let Context { computation, work } = context;
        Ok(Self {
            stack: Binding::new(computation, work.limits, work.counters, work.location)?,
            children: Buffer::new(computation, work.limits, work.counters, work.location)?,
            arguments: Binding::new(computation, work.limits, work.counters, work.location)?,
            prepared,
        })
    }

    fn clear(&mut self) {
        // These provisional query IDs carry no binding or publication state.
        self.stack.discard_suffix(0);
        self.children.clear();
        self.arguments.discard_suffix(0);
    }

    fn prepare(
        &mut self,
        pattern: Pattern<'_>,
        incoming: &Binding<'_>,
        context: &mut Context<'_, &mut Computation<'_, '_>>,
    ) -> Result<bool, FormulaFailure> {
        self.clear();
        {
            let Context { computation, work } = context;
            let terms = pattern.atom().terms();
            self.arguments.extend_scope(
                terms.len(),
                computation,
                work.limits,
                work.counters,
                work.location,
            )?;
            for column in 0..terms.len() {
                work.counters.work(work.limits, work.location)?;
                let key = match terms.at(column).expect("admitted pattern column") {
                    TemplateTerm::Constant(value) => {
                        Some(computation.read().term_key(value).map_err(|error| {
                            crate::formula_binding::assignment(error.into(), work.location)
                        })?)
                    }
                    TemplateTerm::Variable(slot) if incoming.is_bound(slot, work.location)? => {
                        Some(incoming.key(slot, work.location)?)
                    }
                    TemplateTerm::Variable(_) => None,
                };
                if let Some(key) = key {
                    self.arguments
                        .set(column, &key, work.limits, work.counters, work.location)?;
                }
            }
        }
        for argument in pattern.arguments() {
            if !eligible(argument, pattern, incoming, &mut context.work)? {
                continue;
            }
            let key = if let Some(key) =
                self.prepared
                    .key(pattern, argument, incoming, &mut context.work)?
            {
                Some(key)
            } else {
                let key = self.resolve(argument, incoming, context)?;
                if let Some(key) = &key {
                    self.prepared
                        .remember(pattern, argument, incoming, key, &mut context.work)?;
                }
                key
            };
            let Some(key) = key else {
                return Ok(false);
            };
            self.arguments.set(
                argument.position,
                &key,
                context.work.limits,
                context.work.counters,
                context.work.location,
            )?;
        }
        Ok(true)
    }

    fn resolve(
        &mut self,
        argument: &ArgumentPattern,
        incoming: &Binding<'_>,
        context: &mut Context<'_, &mut Computation<'_, '_>>,
    ) -> Result<Option<TermKey>, FormulaFailure> {
        self.stack.discard_suffix(0);
        for node in argument.nodes.iter().rev() {
            let Context { computation, work } = context;
            work.counters.work(work.limits, work.location)?;
            let key = match node {
                PatternNode::Slot(slot) => incoming.key(*slot, work.location)?,
                PatternNode::Constant(scalar) => {
                    computation.static_key(*scalar, work.limits, work.counters, work.location)?
                }
                PatternNode::Constructor(constructor) => {
                    let descriptor = computation.static_constructor(
                        *constructor,
                        work.limits,
                        work.counters,
                        work.location,
                    )?;
                    let (ValueNodeRef::Function { arity, .. } | ValueNodeRef::Tuple { arity }) =
                        descriptor
                    else {
                        unreachable!("admitted structural constructor")
                    };
                    let start = self
                        .stack
                        .len()
                        .checked_sub(arity)
                        .expect("admitted preorder supplies constructor children");
                    self.children.clear();
                    for child in (start..self.stack.len()).rev() {
                        self.children.push(
                            child,
                            computation,
                            work.limits,
                            work.counters,
                            work.location,
                        )?;
                    }
                    let key = computation.find_constructed(
                        descriptor,
                        self.stack.slots(),
                        self.children.slice(),
                        GroundingWork::new(work.limits, work.counters, work.location),
                    )?;
                    let Some(key) = key else {
                        return Ok(None);
                    };
                    self.stack
                        .truncate(start, work.limits, work.counters, work.location)?;
                    key
                }
                PatternNode::Wildcard => unreachable!("only determined arguments are resolved"),
            };
            let next = self.stack.len();
            let end = next.checked_add(1).ok_or_else(|| {
                crate::formula_binding::assignment(
                    zetesis_core::catalog::AssignmentError::Storage(
                        zetesis_core::catalog::Error::Overflow,
                    ),
                    work.location,
                )
            })?;
            self.stack
                .extend_scope(end, computation, work.limits, work.counters, work.location)?;
            self.stack
                .set(next, &key, work.limits, work.counters, work.location)?;
        }
        debug_assert_eq!(self.stack.len(), 1);
        context
            .work
            .counters
            .work(context.work.limits, context.work.location)?;
        self.stack.key(0, context.work.location).map(Some)
    }
}

fn eligible(
    argument: &ArgumentPattern,
    pattern: Pattern<'_>,
    incoming: &Binding<'_>,
    work: &mut GroundingWork<'_>,
) -> Result<bool, FormulaFailure> {
    work.counters.work(work.limits, work.location)?;
    match pattern
        .atom()
        .terms()
        .at(argument.position)
        .expect("admitted argument column")
    {
        TemplateTerm::Constant(_) => return Ok(false),
        TemplateTerm::Variable(slot) if incoming.is_bound(slot, work.location)? => {
            return Ok(false);
        }
        TemplateTerm::Variable(_) => {}
    }
    for node in &argument.nodes {
        work.counters.work(work.limits, work.location)?;
        match node {
            PatternNode::Wildcard => return Ok(false),
            PatternNode::Slot(slot) if !incoming.is_bound(*slot, work.location)? => {
                return Ok(false);
            }
            _ => {}
        }
    }
    Ok(true)
}

impl<'source> Join<'_, 'source> {
    pub(super) fn structural_selection(
        &mut self,
        pattern: PositivePattern<'_>,
        source: Option<&'source super::relations::RelationRows<'source>>,
        mut context: Context<'_, &mut Computation<'_, '_>>,
    ) -> Result<Selection<'source>, FormulaFailure> {
        #[cfg(test)]
        if !tests::enabled() {
            return Ok(Selection::Unchanged);
        }
        let PositivePattern::Structural(pattern) = pattern else {
            return Ok(Selection::Unchanged);
        };
        // No constructor query is needed to prove an absent relation empty.
        if source.is_none() {
            return Ok(Selection::Unchanged);
        }
        let mut determined = false;
        for argument in pattern.arguments() {
            if eligible(argument, pattern, &self.values, &mut context.work)? {
                determined = true;
                break;
            }
        }
        if !determined {
            return Ok(Selection::Unchanged);
        }
        #[cfg(test)]
        tests::record();
        if self.structural_query.is_none() {
            self.structural_query = Some(Scratch::new(
                &self.plan.patterns,
                &mut Context::new(
                    &*context.computation,
                    context.work.limits,
                    context.work.counters,
                    context.work.location,
                ),
            )?);
            self.lease
                .observe(self.storage_bytes(), context.work.location)?;
        }
        let scratch = self
            .structural_query
            .as_mut()
            .expect("prepared structural query");
        let result = (|| {
            if !scratch.prepare(pattern, &self.values, &mut context)? {
                return Ok(Selection::Posting(Some(&[])));
            }
            let arguments = scratch.arguments.view(
                context.computation.read(),
                context.work.limits,
                context.work.counters,
                context.work.location,
            )?;
            self.support
                .probe_arguments_at(
                    source,
                    pattern.atom(),
                    arguments,
                    context.work.limits,
                    context.work.counters,
                    context.work.location,
                )
                .map(Selection::Posting)
        })();
        scratch.clear();
        result
    }
}
