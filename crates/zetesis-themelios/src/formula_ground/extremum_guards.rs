//! Shared typed preparation for an assignment's complete proposal family.

use super::aggregate_guards::GuardFamily;
use super::{Builder, CoordinateMap, ExtremumElement, remap};
use crate::formula::ceiling;
use crate::formula_binding::Binding;
use crate::formula_support::{Buffer, Computation, Context, GroundingWork, TermTable};
use crate::{FormulaFailure, FormulaResource, ProgramSite};
use zetesis_core::catalog::{TermKey, TermRef};
use zetesis_ferraris::{
    AggregateComparison, AggregateExtremum, AggregateFamilyLimits, ValueExtremumElement,
    ValueExtremumFamilySpace, ValueExtremumGuard,
};

impl Builder<'_, '_, '_> {
    /// Choose the compilation before evaluating proposals or spending compiler
    /// work. Both routes have the same semantics. Families whose optional
    /// native preparation does not fit retain scalar Boolean compilation;
    /// in particular a zero state allowance preserves prior extrema admission.
    /// An error after choosing shared preparation is returned, never retried.
    pub(super) fn shares_extremum_family(
        &self,
        elements: usize,
        guards: usize,
        location: ProgramSite,
    ) -> Result<bool, FormulaFailure> {
        if guards == 0 {
            return Ok(false);
        }
        let Some(space) = ValueExtremumFamilySpace::new(elements, guards) else {
            return Ok(false);
        };
        if space.states() > self.limits.aggregate.max_states {
            return Ok(false);
        }
        let inputs = size_of::<Buffer<ValueExtremumElement<TermRef<'_>>>>() as u128
            + elements as u128 * size_of::<ValueExtremumElement<TermRef<'_>>>() as u128
            + size_of::<Buffer<ValueExtremumGuard<TermRef<'_>>>>() as u128
            + guards as u128 * size_of::<ValueExtremumGuard<TermRef<'_>>>() as u128;
        let empty = self.computation.lease();
        let available = self.computation.allowance(&empty, self.limits, location)?;
        Ok(inputs + space.storage_bytes() as u128 <= available as u128)
    }

    pub(super) fn extremum_assignment_roots(
        &mut self,
        elements: &[ExtremumElement],
        kind: AggregateExtremum,
        values: &Binding,
        roots: &mut CoordinateMap<usize, usize>,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        let first = self.nodes.view().len();
        let family = if self.shares_extremum_family(elements.len(), values.len(), location)? {
            Some(self.append_extremum_assignment_family(elements, kind, values, location)?)
        } else {
            None
        };
        let canonical = family
            .as_ref()
            .map(|family| self.intern_appended(&family.appended.suffix, location))
            .transpose()?;
        for slot in 0..values.len() {
            self.work(location)?;
            let key = values.key(slot, location)?;
            let value = self.term(&key, location)?;
            let root = if let Some(family) = &family {
                remap(
                    family.build.roots()[slot],
                    first,
                    canonical
                        .as_ref()
                        .expect("shared family was interned")
                        .slice(),
                )
            } else {
                self.extremum_root(elements, kind, AggregateComparison::Eq, &key, location)?
            };
            roots.insert(
                value,
                root,
                None,
                Context::new(
                    &*self.computation,
                    self.limits,
                    &mut self.counters,
                    location,
                ),
            )?;
        }
        Ok(())
    }

    /// Typed borrowed inputs are leased separately. The family owns one scratch
    /// allocation and its ordered roots; the root lease survives detachment and
    /// canonical remapping. No first-value grouping discards complete tuples.
    fn append_extremum_assignment_family(
        &mut self,
        elements: &[ExtremumElement],
        kind: AggregateExtremum,
        values: &Binding,
        location: ProgramSite,
    ) -> Result<GuardFamily, FormulaFailure> {
        self.append_extremum_family(elements, kind, GuardInput::Assignment(values), location)
    }

    pub(super) fn native_extremum_root(
        &mut self,
        elements: &[ExtremumElement],
        kind: AggregateExtremum,
        comparison: AggregateComparison,
        bound: &TermKey,
        location: ProgramSite,
    ) -> Result<usize, FormulaFailure> {
        let first = self.nodes.view().len();
        let family = self.append_extremum_family(
            elements,
            kind,
            GuardInput::Single(comparison, bound),
            location,
        )?;
        let canonical = self.intern_appended(&family.appended.suffix, location)?;
        Ok(remap(family.build.roots()[0], first, canonical.slice()))
    }

    fn append_extremum_family(
        &mut self,
        elements: &[ExtremumElement],
        kind: AggregateExtremum,
        values: GuardInput<'_>,
        location: ProgramSite,
    ) -> Result<GuardFamily, FormulaFailure> {
        let space = ValueExtremumFamilySpace::new(elements.len(), values.len())
            .expect("shared preparation passed its space preflight");
        let cancellation = self.counters.cancellation().cloned().unwrap_or_default();
        let mut storage = self.computation.lease();
        let (transaction, build) = {
            let inputs = family_inputs(
                elements,
                &self.terms,
                values,
                Context::new(
                    &*self.computation,
                    self.limits,
                    &mut self.counters,
                    location,
                ),
            )?;
            let available = self
                .computation
                .allowance(&storage, self.limits, location)?;
            ceiling(
                FormulaResource::SupportBytes,
                (self.limits.max_support_bytes - available) as u128 + space.storage_bytes() as u128,
                self.limits.max_support_bytes as u128,
                location,
            )?;
            // Input preparation consumed formula work. Snapshot the remaining
            // compiler ceiling only now, after those charged operations.
            let limits = AggregateFamilyLimits {
                aggregate: self.aggregate_limits(),
                max_guards: match values {
                    GuardInput::Assignment(_) => self.limits.max_assignment_values,
                    GuardInput::Single(..) => 1,
                },
            };
            let mut transaction = self.nodes.transaction();
            let attempt = transaction.append_value_extremum_family_refs(
                inputs.elements.slice(),
                kind,
                inputs.guards.slice(),
                limits,
                &cancellation,
            );
            let statistics = match &attempt.result {
                Ok(build) => build.statistics(),
                Err(error) => error.statistics(),
            };
            self.counters.accounting.work += statistics.work;
            let observed = self.computation.storage_peak(
                &storage,
                attempt.auxiliary_peak_bytes,
                self.limits,
                &self.counters,
                location,
            );
            // Observe capacity on either outcome. A resource observation must
            // not conceal the compiler's original typed refusal.
            let build = attempt
                .result
                .map_err(|error| FormulaFailure::Aggregate { error, location })?;
            observed?;
            storage.observe(build.root_storage_bytes(), location)?;
            self.computation.storage_observed(
                &storage,
                0,
                size_of::<Vec<usize>>(),
                self.limits,
                &self.counters,
                location,
            )?;
            (transaction, build)
        };
        let appended = super::appended::detach(
            transaction,
            self.computation,
            self.limits,
            &mut self.counters,
            location,
        )?;
        Ok(GuardFamily::new(build, appended, storage))
    }
}

/// Guard identity is kept until borrowed source values are available. Both
/// assignment families and single comparisons use the same charged preparation.
#[derive(Clone, Copy)]
enum GuardInput<'a> {
    Assignment(&'a Binding<'a>),
    Single(AggregateComparison, &'a TermKey),
}

impl GuardInput<'_> {
    fn len(self) -> usize {
        match self {
            Self::Assignment(values) => values.len(),
            Self::Single(..) => 1,
        }
    }
}

struct Inputs<'a> {
    elements: Buffer<ValueExtremumElement<TermRef<'a>>>,
    guards: Buffer<ValueExtremumGuard<TermRef<'a>>>,
}

fn family_inputs<'a>(
    elements: &[ExtremumElement],
    terms: &TermTable,
    values: GuardInput<'_>,
    context: Context<'_, &'a Computation<'_, '_>>,
) -> Result<Inputs<'a>, FormulaFailure> {
    let Context {
        computation,
        work:
            GroundingWork {
                limits,
                counters,
                location,
            },
    } = context;
    let read = computation.read();
    let mut guards = Buffer::new(computation, limits, counters, location)?;
    guards.reserve(values.len(), computation, limits, counters, location)?;
    // Proposal order and source endpoint validation are retained even though
    // the primitive later sorts borrowed boundary indexes.
    for slot in 0..values.len() {
        let (comparison, bound) = match values {
            GuardInput::Assignment(values) => {
                (AggregateComparison::Eq, values.read(slot, read, location)?)
            }
            GuardInput::Single(comparison, bound) => {
                let bound = read.term(bound).map_err(|error| {
                    crate::formula_binding::assignment(
                        zetesis_core::catalog::AssignmentError::Read(error),
                        location,
                    )
                })?;
                (comparison, bound)
            }
        };
        crate::formula_assignment::extremum_value(bound, location)?;
        guards.push(
            ValueExtremumGuard { comparison, bound },
            computation,
            limits,
            counters,
            location,
        )?;
    }
    let mut borrowed = Buffer::new(computation, limits, counters, location)?;
    borrowed.reserve(elements.len(), computation, limits, counters, location)?;
    for element in elements {
        borrowed.push(
            ValueExtremumElement {
                value: terms.value(element.value, read, location)?,
                condition: element.condition,
            },
            computation,
            limits,
            counters,
            location,
        )?;
    }
    Ok(Inputs {
        elements: borrowed,
        guards,
    })
}

#[cfg(test)]
mod tests;
