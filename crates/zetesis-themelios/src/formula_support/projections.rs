//! Successful unary expression projections for source joins.
//!
//! A descriptor borrows its exact immutable expression, never its transient
//! address as an unowned key. Reached checks and optional totality preparation
//! populate the same successful canonical results. Equal inputs reuse those
//! results; failed computations are never retained. Selection requires its own
//! certificate, while source-family evidence retains complete traversal. Each
//! lookup and retained pair consumes work and leased support storage.
//! Ordinary cursors own their entries; prepared rules can retain a completed
//! finite-domain map for immutable borrowing by later cursors.

mod domains;
mod totality;

use std::{cmp::Ordering, ops::Deref};

use themelios_program::program::Relation;
use zetesis_core::catalog::TermKey;

use super::term_table::TermTable;

use super::{Computation, Context, Evaluation, Failures, StorageLease, comparison};
use crate::FormulaFailure;
use crate::formula_binding::Binding;
use crate::formula_ir::{Expression, LiteralIr};

/// A cursor either owns its successful memo entries or borrows the immutable
/// complete-column preparation of its exact rule. Borrowed maps never grow.
pub(super) enum Projections<'a> {
    Local(ProjectionValues<'a>),
    Prepared(&'a ProjectionValues<'a>),
}

pub(super) struct ProjectionValues<'a> {
    values: Vec<Projection<'a>>,
    prepared: bool,
    lease: StorageLease,
}

struct Projection<'a> {
    literal: usize,
    side: usize,
    expression: &'a Expression,
    slot: usize,
    inputs: TermTable,
    results: Binding<'static>,
    /// A prefix covering every possible complete binding of this expression's
    /// input. Only completed source-domain totality publishes this witness;
    /// ordinary successful memo entries establish no coverage.
    covered: Option<usize>,
}

impl<'a> ProjectionValues<'a> {
    pub(super) fn new(context: &Context<'_, &Computation<'_, '_>>) -> Result<Self, FormulaFailure> {
        let mut result = Self {
            values: Vec::new(),
            prepared: false,
            lease: context.computation.lease(),
        };
        result
            .lease
            .observe(result.bytes(), context.work.location)?;
        context.computation.storage_observed(
            &result.lease,
            0,
            size_of::<Self>(),
            context.work.limits,
            context.work.counters,
            context.work.location,
        )?;
        Ok(result)
    }

    /// The retained rule slot owns this inline header after publication. Only
    /// immutable borrows may read this map afterward; its buffers keep their
    /// original leases and cannot be populated by a later cursor.
    pub(super) fn retain_in_rule(
        mut self,
        location: crate::ProgramSite,
    ) -> Result<Self, FormulaFailure> {
        self.lease
            .observe(self.bytes() - size_of::<Self>(), location)?;
        Ok(self)
    }

    /// Prepare once for a reached complete row or a finite totality attempt.
    /// Without that attempt, prefix-only joins and empty relational extensions
    /// retain no expression-index metadata.
    pub(super) fn prepare(
        &mut self,
        literals: &'a [LiteralIr],
        mut context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<(), FormulaFailure> {
        if self.prepared {
            return Ok(());
        }
        for (literal, value) in literals.iter().enumerate() {
            tick(&mut context)?;
            let Some((left, _, right)) = comparison(value) else {
                continue;
            };
            for (side, expression) in [left, right].into_iter().enumerate() {
                context.work.counters.charge_work(
                    expression.nodes.len() as u128,
                    context.work.limits,
                    context.work.location,
                )?;
                let mut inputs = expression.inputs();
                let Some(slot) = inputs.next() else {
                    continue;
                };
                if expression.nodes.len() == 1 || !inputs.all(|input| input == slot) {
                    continue;
                }
                self.reserve_projection(&mut context)?;
                self.values.push(Projection {
                    literal,
                    side,
                    expression,
                    slot,
                    inputs: TermTable::new(
                        context.computation,
                        context.work.limits,
                        context.work.counters,
                        context.work.location,
                    )?,
                    covered: None,
                    results: Binding::new(
                        context.computation,
                        context.work.limits,
                        context.work.counters,
                        context.work.location,
                    )?,
                });
                self.lease.observe(self.bytes(), context.work.location)?;
            }
        }
        self.prepared = true;
        Ok(())
    }

    fn reserve_projection(
        &mut self,
        context: &mut Context<'_, &Computation<'_, '_>>,
    ) -> Result<(), FormulaFailure> {
        tick(context)?;
        let allowance = context.computation.allowance(
            &self.lease,
            context.work.limits,
            context.work.location,
        )?;
        if self.values.len() < self.values.capacity() {
            return Ok(());
        }
        let required = self.values.len().checked_add(1).ok_or_else(|| {
            crate::formula_binding::assignment(
                zetesis_core::catalog::AssignmentError::Storage(
                    zetesis_core::catalog::Error::Overflow,
                ),
                context.work.location,
            )
        })?;
        let capacity = super::storage::growth_capacity(self.values.capacity(), required);
        let old_buffer = self.values.capacity() * size_of::<Projection>();
        // Child leases already own the occupied TermTable/Binding headers.
        // Reallocation additionally holds one entire replacement vector; its
        // empty cells do not yet have separately owned embedded containers.
        let peak = self.bytes() as u128 + capacity as u128 * size_of::<Projection>() as u128;
        crate::formula::ceiling(
            crate::FormulaResource::SupportBytes,
            (context.work.limits.max_support_bytes - allowance) as u128 + peak,
            context.work.limits.max_support_bytes as u128,
            context.work.location,
        )?;
        self.values
            .try_reserve_exact(capacity - self.values.len())
            .map_err(|_| FormulaFailure::SupportRelation {
                error: zetesis_core::relation::Failure::Allocation,
                location: context.work.location,
            })?;
        self.lease.observe(self.bytes(), context.work.location)?;
        context.computation.storage_peak(
            &self.lease,
            self.bytes() as u128 + old_buffer as u128,
            context.work.limits,
            context.work.counters,
            context.work.location,
        )
    }

    fn bytes(&self) -> usize {
        // Occupied descriptors lend these two headers to their existing leases.
        // Spare vector cells remain wholly charged to this container.
        size_of::<Self>() + self.values.capacity() * size_of::<Projection>()
            - self.values.len() * (size_of::<TermTable>() + size_of::<Binding>())
    }

    fn find<C>(
        &self,
        source: (usize, usize),
        context: &mut Context<'_, C>,
    ) -> Result<Option<usize>, FormulaFailure> {
        let mut low = 0;
        let mut high = self.values.len();
        while low < high {
            tick(context)?;
            let middle = low + (high - low) / 2;
            let item = &self.values[middle];
            match (item.literal, item.side).cmp(&source) {
                Ordering::Less => low = middle + 1,
                Ordering::Greater => high = middle,
                Ordering::Equal => return Ok(Some(middle)),
            }
        }
        Ok(None)
    }
}

impl<'a> Deref for Projections<'a> {
    type Target = ProjectionValues<'a>;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Local(values) => values,
            Self::Prepared(values) => values,
        }
    }
}

impl<'a> Projections<'a> {
    pub(super) fn new(context: &Context<'_, &Computation<'_, '_>>) -> Result<Self, FormulaFailure> {
        ProjectionValues::new(context).map(Self::Local)
    }

    /// Borrow only within the workspace that owns the retained map's leases.
    /// Completion identity separately authenticates its rule and term carrier.
    pub(super) fn borrowed(
        values: &'a ProjectionValues<'a>,
        context: &Context<'_, &Computation<'_, '_>>,
    ) -> Result<Self, FormulaFailure> {
        context
            .computation
            .allowance(&values.lease, context.work.limits, context.work.location)?;
        Ok(Self::Prepared(values))
    }

    pub(super) fn prepare(
        &mut self,
        literals: &'a [LiteralIr],
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<(), FormulaFailure> {
        match self {
            Self::Local(values) => values.prepare(literals, context),
            Self::Prepared(_) => Ok(()),
        }
    }

    /// The map's lease owns its inline header only when the cursor owns it.
    /// A borrowed view contributes no second receipt for retained payload.
    pub(super) fn leased_header(&self) -> usize {
        match self {
            Self::Local(_) => size_of::<ProjectionValues<'_>>(),
            Self::Prepared(_) => 0,
        }
    }

    pub(super) fn compare(
        &mut self,
        literal: usize,
        (expressions, relation): ([&Expression; 2], Relation),
        binding: &Binding<'_>,
        evaluation: &mut Evaluation,
        mut context: Context<'_, &mut Computation<'_, '_>>,
    ) -> Result<bool, FormulaFailure> {
        let mut failures = Failures::default();
        let mut values = [None, None];
        for (side, expression) in expressions.into_iter().enumerate() {
            let result = self.value(
                (literal, side),
                expression,
                binding,
                evaluation,
                &mut context,
            );
            values[side] = failures.value(result, evaluation.zero_divisor())?;
        }
        failures.finish(evaluation)?;
        let [left, right] = values.map(|value| value.expect("both components are defined"));
        let read = context.computation.read();
        let mut resolve = |key: &TermKey| {
            context
                .work
                .counters
                .work(context.work.limits, context.work.location)?;
            read.term(key).map_err(|error| {
                crate::formula_binding::assignment(error.into(), context.work.location)
            })
        };
        let left = resolve(&left)?;
        let right = resolve(&right)?;
        super::compare(
            left,
            relation,
            right,
            context.work.limits,
            context.work.counters,
            context.work.location,
        )
    }

    fn value(
        &mut self,
        source: (usize, usize),
        expression: &Expression,
        binding: &Binding<'_>,
        evaluation: &mut Evaluation,
        context: &mut Context<'_, &mut Computation<'_, '_>>,
    ) -> Result<TermKey, FormulaFailure> {
        evaluation.clear_zero_divisor();
        let projected = if self.values.is_empty() || expression.nodes.len() == 1 {
            None
        } else {
            tick(context)?;
            context.computation.allowance(
                &self.lease,
                context.work.limits,
                context.work.location,
            )?;
            self.find(source, context)?
        };
        let input = if let Some(index) = projected {
            tick(context)?;
            let projection = &self.values[index];
            let input = binding.key(projection.slot, context.work.location)?;
            if let Some(position) = projection.inputs.find(
                &input,
                context.computation,
                context.work.limits,
                context.work.counters,
                context.work.location,
            )? {
                tick(context)?;
                let result = projection.results.key(position, context.work.location)?;
                context.computation.read().term(&result).map_err(|error| {
                    crate::formula_binding::assignment(error.into(), context.work.location)
                })?;
                return Ok(result);
            }
            Some((index, input))
        } else {
            None
        };
        // A prefix can read inputs outside the covering column of a later
        // occurrence. Its checked evaluation remains authoritative; a borrowed
        // certificate neither grows nor claims that this input is covered.
        let expression = projected.map_or(expression, |index| self.values[index].expression);
        let result = evaluation.source_expression(
            expression,
            |slot| binding.key(slot, context.work.location),
            context.computation,
            context.work.limits,
            context.work.counters,
            context.work.location,
        )?;
        if let (Self::Local(values), Some((index, input))) = (self, input) {
            values.values[index].insert(&input, &result, context)?;
        }
        Ok(result)
    }
}

impl Projection<'_> {
    fn insert(
        &mut self,
        input: &TermKey,
        result: &TermKey,
        context: &mut Context<'_, &mut Computation<'_, '_>>,
    ) -> Result<(), FormulaFailure> {
        let position = self.results.len();
        let end = position.checked_add(1).ok_or_else(|| {
            crate::formula_binding::assignment(
                zetesis_core::catalog::AssignmentError::Storage(
                    zetesis_core::catalog::Error::Overflow,
                ),
                context.work.location,
            )
        })?;
        // Prepare the result before publishing the input coordinate. A refusal
        // rolls back its logical slot; all retained capacity stays accounted.
        let prepared = self
            .results
            .extend_scope(
                end,
                context.computation,
                context.work.limits,
                context.work.counters,
                context.work.location,
            )
            .and_then(|()| {
                self.results.set(
                    position,
                    result,
                    context.work.limits,
                    context.work.counters,
                    context.work.location,
                )
            })
            .and_then(|()| {
                self.inputs.insert(
                    input,
                    None,
                    context.computation,
                    context.work.limits,
                    context.work.counters,
                    context.work.location,
                )
            });
        match prepared {
            Ok(inserted) => {
                debug_assert_eq!(inserted, position);
                Ok(())
            }
            Err(error) => {
                self.results.discard_suffix(position);
                Err(error)
            }
        }
    }
}

fn tick<C>(context: &mut Context<'_, C>) -> Result<(), FormulaFailure> {
    context
        .work
        .counters
        .work(context.work.limits, context.work.location)
}

#[cfg(test)]
mod tests;
