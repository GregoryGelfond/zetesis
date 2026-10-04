//! Compile same-contribution nonnegative guards through one checked family.

#[cfg(test)]
mod tests;

use super::{
    Builder, NumericComparison, VERUM, aggregate_comparison, boolean, numeric_comparison, remap,
};
use crate::ProgramSite;
use crate::formula::ceiling;
use crate::formula_binding::Binding;
use crate::formula_ir::AggregateGuard;
use crate::formula_support::{self, Buffer, StorageLease};
use crate::{FormulaFailure, FormulaResource};
use zetesis_ferraris::{
    AggregateElement, AggregateFamilyBuild, AggregateFamilyLimits, AggregateGuard as NumericGuard,
};

/// Keep returned roots charged while canonical remapping and consumers overlap.
pub(super) struct GuardFamily {
    pub(super) build: AggregateFamilyBuild,
    _storage: StorageLease,
}

impl Builder<'_, '_, '_> {
    /// Signed contributions retain scalar compilation: its subset ceiling is
    /// per guard, whereas the family primitive's subset ceiling is cumulative.
    pub(super) fn nonnegative_elements(
        &mut self,
        elements: &[AggregateElement],
        location: ProgramSite,
    ) -> Result<bool, FormulaFailure> {
        for element in elements {
            self.work(location)?;
            if element.weight < 0 {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub(super) fn numeric_guard_family(
        &mut self,
        elements: &[AggregateElement],
        guards: &[AggregateGuard],
        assignment: &Binding,
        location: ProgramSite,
        mut capture: Option<&mut crate::formula_count_plan::Bounds>,
    ) -> Result<usize, FormulaFailure> {
        let (evaluated, numeric) = self.evaluate_numeric_guards(guards, assignment, location)?;
        // An entirely logical comparison list needs no aggregate validation or
        // threshold state, just as on the scalar route.
        let first = self.nodes.len();
        let family = if numeric.len() == 0 {
            None
        } else {
            Some(self.append_guard_family(elements, numeric.slice(), numeric.len(), location)?)
        };
        let canonical = self.intern_appended(first, location)?;
        let mut roots = family.as_ref().map(|family| family.build.roots().iter());
        let mut result = VERUM;
        for (guard, bound) in guards.iter().zip(evaluated.iter()) {
            self.work(location)?;
            let root = match *bound {
                NumericComparison::Threshold(bound) => {
                    let root = roots
                        .as_mut()
                        .and_then(Iterator::next)
                        .expect("one family root per numeric guard");
                    if let Some(capture) = capture.as_deref_mut() {
                        capture.guard(aggregate_comparison(guard.relation), bound);
                    }
                    remap(*root, first, &canonical)
                }
                NumericComparison::Constant(truth) => {
                    if let Some(capture) = capture.as_deref_mut() {
                        capture.exclude_logical_guard();
                    }
                    boolean(truth)
                }
            };
            result = self.and(result, root, location)?;
        }
        Ok(result)
    }

    /// Retain source order separately from the contiguous numeric slice required
    /// by the family API. No false logical guard suppresses a later evaluation.
    fn evaluate_numeric_guards(
        &mut self,
        guards: &[AggregateGuard],
        assignment: &Binding,
        location: ProgramSite,
    ) -> Result<(Buffer<NumericComparison>, Buffer<NumericGuard>), FormulaFailure> {
        let mut evaluated =
            Buffer::new(self.computation, self.limits, &mut self.counters, location)?;
        let mut numeric = Buffer::new(self.computation, self.limits, &mut self.counters, location)?;
        for guard in guards {
            let key = formula_support::expression(
                &guard.bound,
                assignment,
                self.computation,
                self.limits,
                &mut self.counters,
                location,
            )?;
            let value = self.computation.read().term(&key).map_err(|error| {
                crate::formula_binding::assignment(
                    zetesis_core::catalog::AssignmentError::Read(error),
                    location,
                )
            })?;
            let bound = numeric_comparison(
                guard.relation,
                value,
                self.limits,
                &mut self.counters,
                location,
            )?;
            if let NumericComparison::Threshold(bound) = bound {
                numeric.push(
                    NumericGuard {
                        comparison: aggregate_comparison(guard.relation),
                        bound: i64::from(bound),
                    },
                    self.computation,
                    self.limits,
                    &mut self.counters,
                    location,
                )?;
            }
            evaluated.push(
                bound,
                self.computation,
                self.limits,
                &mut self.counters,
                location,
            )?;
        }
        Ok((evaluated, numeric))
    }

    /// Both assignment proposals and written guard groups use the same checked
    /// transaction and receipt. Work includes failed compiler prefixes; returned
    /// root capacity stays live until its consumer finishes canonical remapping.
    pub(super) fn append_guard_family(
        &mut self,
        elements: &[AggregateElement],
        guards: &[NumericGuard],
        max_guards: usize,
        location: ProgramSite,
    ) -> Result<GuardFamily, FormulaFailure> {
        let mut storage = self.computation.lease();
        let header = size_of::<Vec<usize>>();
        let requested = header as u128 + guards.len() as u128 * size_of::<usize>() as u128;
        let allowance = self
            .computation
            .allowance(&storage, self.limits, location)?;
        ceiling(
            FormulaResource::SupportBytes,
            (self.limits.max_support_bytes - allowance) as u128 + requested,
            self.limits.max_support_bytes as u128,
            location,
        )?;
        let limits = AggregateFamilyLimits {
            aggregate: self.aggregate_limits(),
            max_guards,
        };
        let result = self.nodes.append_aggregate_family(
            elements,
            guards,
            limits,
            &zetesis_cpu::Cancellation::default(),
        );
        let work = match &result {
            Ok(build) => build.statistics().work,
            Err(error) => error.statistics().work,
        };
        // The transaction's work ceiling is capped by remaining formula work.
        self.counters.accounting.work += work;
        let build = result.map_err(|error| FormulaFailure::Aggregate { error, location })?;
        storage.observe(build.root_storage_bytes(), location)?;
        self.computation.storage_observed(
            &storage,
            0,
            header,
            self.limits,
            &self.counters,
            location,
        )?;
        Ok(GuardFamily {
            build,
            _storage: storage,
        })
    }
}
