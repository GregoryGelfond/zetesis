//! One last successful constructor resolution per actual source argument.
//!
//! Input and result IDs have one scoped binding owner. The join keeps its fixed
//! support snapshot and append-only vocabulary; ordinary `arguments.view` still
//! authenticates the selected result's readable prefix. No absent lookup, row,
//! atom activation or incoming binding is retained here.

use super::{ArgumentPattern, Binding, Buffer, Computation, Context, Pattern, PatternNode};
use crate::FormulaFailure;
use crate::formula_support::{GroundingWork, PatternOccurrence, PositivePattern};
use zetesis_core::TemplateTerm;
use zetesis_core::catalog::TermKey;

#[derive(Clone, Copy)]
struct Argument<'a> {
    source: &'a ArgumentPattern,
    start: usize,
    result: usize,
    valid: bool,
}

#[derive(Clone, Copy)]
enum Entry<'a> {
    Empty,
    // Reused capture coordinates in manually assembled IR, or a wildcard.
    Unavailable,
    Argument(Argument<'a>),
}

pub(super) struct Prepared<'a> {
    entries: Buffer<Entry<'a>>,
    values: Binding<'static>,
}

impl<'a> Prepared<'a> {
    pub(super) const fn leased_header() -> usize {
        size_of::<Buffer<Entry<'a>>>() + size_of::<Binding<'static>>()
    }

    pub(super) fn new(
        patterns: &[PatternOccurrence<'a>],
        context: &mut Context<'_, &Computation<'_, '_>>,
    ) -> Result<Self, FormulaFailure> {
        let mut prepared = Self {
            entries: Buffer::new(
                context.computation,
                context.work.limits,
                context.work.counters,
                context.work.location,
            )?,
            values: Binding::new(
                context.computation,
                context.work.limits,
                context.work.counters,
                context.work.location,
            )?,
        };
        #[cfg(test)]
        if !super::tests::reuse_enabled() {
            return Ok(prepared);
        }
        for occurrence in patterns {
            context
                .work
                .counters
                .work(context.work.limits, context.work.location)?;
            let PositivePattern::Structural(pattern) = occurrence.pattern else {
                continue;
            };
            for argument in pattern.arguments() {
                prepared.argument(pattern, argument, context)?;
            }
        }
        Ok(prepared)
    }

    fn argument(
        &mut self,
        pattern: Pattern<'a>,
        argument: &'a ArgumentPattern,
        context: &mut Context<'_, &Computation<'_, '_>>,
    ) -> Result<(), FormulaFailure> {
        let Context { computation, work } = context;
        work.counters.work(work.limits, work.location)?;
        let Some(slot) = capture(pattern, argument) else {
            return Ok(());
        };
        let end = next(slot, work.location)?;
        if end > self.entries.len() {
            self.entries.resize(
                end,
                Entry::Empty,
                computation,
                work.limits,
                work.counters,
                work.location,
            )?;
        }
        if !matches!(self.entries.slice()[slot], Entry::Empty) {
            self.entries.slice_mut()[slot] = Entry::Unavailable;
            return Ok(());
        }
        let start = self.values.len();
        let mut result = start;
        for node in &argument.nodes {
            work.counters.work(work.limits, work.location)?;
            match node {
                PatternNode::Slot(_) => result = next(result, work.location)?,
                PatternNode::Wildcard => {
                    self.entries.slice_mut()[slot] = Entry::Unavailable;
                    return Ok(());
                }
                _ => {}
            }
        }
        self.values.extend_scope(
            next(result, work.location)?,
            computation,
            work.limits,
            work.counters,
            work.location,
        )?;
        // The complete plan is published only when all fallible preparation
        // succeeds. A failed new plan drops both leases with its partial data.
        self.entries.slice_mut()[slot] = Entry::Argument(Argument {
            source: argument,
            start,
            result,
            valid: false,
        });
        Ok(())
    }

    fn argument_mut(
        &mut self,
        pattern: Pattern<'_>,
        source: &ArgumentPattern,
    ) -> Option<&mut Argument<'a>> {
        let slot = capture(pattern, source)?;
        match self.entries.slice_mut().get_mut(slot)? {
            Entry::Argument(argument) if std::ptr::eq(argument.source, source) => Some(argument),
            _ => None,
        }
    }

    pub(super) fn key(
        &mut self,
        pattern: Pattern<'_>,
        source: &ArgumentPattern,
        incoming: &Binding<'_>,
        work: &mut GroundingWork<'_>,
    ) -> Result<Option<TermKey>, FormulaFailure> {
        #[cfg(test)]
        if !super::tests::reuse_enabled() {
            return Ok(None);
        }
        work.counters.work(work.limits, work.location)?;
        let Some(argument) = self.argument_mut(pattern, source) else {
            return Ok(None);
        };
        if !argument.valid {
            return Ok(None);
        }
        argument.valid = false;
        let argument = *argument;
        let mut position = argument.start;
        for node in &source.nodes {
            work.counters.work(work.limits, work.location)?;
            if let PatternNode::Slot(slot) = node {
                if incoming
                    .slots()
                    .compare_slot(*slot, self.values.slots(), position)
                    .map_err(|error| crate::formula_binding::assignment(error, work.location))?
                    != std::cmp::Ordering::Equal
                {
                    return Ok(None);
                }
                position += 1;
            }
        }
        debug_assert_eq!(position, argument.result);
        work.counters.work(work.limits, work.location)?;
        let key = self.values.key(argument.result, work.location)?;
        self.argument_mut(pattern, source)
            .expect("same prepared argument")
            .valid = true;
        #[cfg(test)]
        super::tests::reuse_record();
        Ok(Some(key))
    }

    pub(super) fn remember(
        &mut self,
        pattern: Pattern<'_>,
        source: &ArgumentPattern,
        incoming: &Binding<'_>,
        key: &TermKey,
        work: &mut GroundingWork<'_>,
    ) -> Result<(), FormulaFailure> {
        #[cfg(test)]
        if !super::tests::reuse_enabled() {
            return Ok(());
        }
        work.counters.work(work.limits, work.location)?;
        let Some(argument) = self.argument_mut(pattern, source) else {
            return Ok(());
        };
        argument.valid = false;
        let argument = *argument;
        let mut position = argument.start;
        for node in &source.nodes {
            work.counters.work(work.limits, work.location)?;
            if let PatternNode::Slot(slot) = node {
                self.values.copy_slot(position, incoming, *slot, work)?;
                position += 1;
            }
        }
        debug_assert_eq!(position, argument.result);
        self.values.set(
            argument.result,
            key,
            work.limits,
            work.counters,
            work.location,
        )?;
        // Refusal anywhere above leaves the record invalid, including when
        // part of a replacement input tuple has already been copied.
        self.argument_mut(pattern, source)
            .expect("same prepared argument")
            .valid = true;
        Ok(())
    }
}

fn capture(pattern: Pattern<'_>, argument: &ArgumentPattern) -> Option<usize> {
    match pattern.atom().terms().at(argument.position)? {
        TemplateTerm::Variable(slot) => Some(slot),
        TemplateTerm::Constant(_) => None,
    }
}

fn next(value: usize, location: crate::ProgramSite) -> Result<usize, FormulaFailure> {
    value.checked_add(1).ok_or_else(|| {
        crate::formula_binding::assignment(
            zetesis_core::catalog::AssignmentError::Storage(zetesis_core::catalog::Error::Overflow),
            location,
        )
    })
}
