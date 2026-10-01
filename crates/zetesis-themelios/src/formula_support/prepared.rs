//! Reusable ordering for an exact rule over completed immutable support.

use super::{
    CompletedQueries, CompletedSupport, Completion, Computation, Counters, FilteredRows, Join,
    RowFilter, order,
};
use crate::expansion::Budget;
use crate::formula_binding::Binding;
use crate::formula_ir::RuleIr;
use crate::{FormulaFailure, FormulaLimits};

/// No candidate, binding, arithmetic result or traversal position is retained.
pub(crate) struct PreparedRule<'source> {
    rule: &'source RuleIr,
    completion: &'source Completion,
    plan: order::Plan<'source>,
}

impl<'source> PreparedRule<'source> {
    /// Prepare sparse slots only when the first rule survives its predicate
    /// gate. Each plan is then published independently after successful setup.
    pub(crate) fn slots(
        rules: &[RuleIr],
        support: &mut CompletedSupport<'_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
    ) -> Result<Vec<Option<Self>>, FormulaFailure> {
        let location = rules.first().expect("nonempty streamed source").location;
        let mut slots = Vec::new();
        let bytes = |capacity: usize| capacity as u128 * size_of::<Option<Self>>() as u128;
        support.admit_workspace(bytes(rules.len()), limits, counters, location)?;
        counters.work(limits, location)?;
        slots
            .try_reserve_exact(rules.len())
            .map_err(|_| FormulaFailure::SupportRelation {
                error: zetesis_core::relation::Failure::Allocation,
                location,
            })?;
        support.admit_workspace(bytes(slots.capacity()), limits, counters, location)?;
        for rule in rules {
            counters.work(limits, rule.location)?;
            slots.push(None);
        }
        support.retain_workspace(
            usize::try_from(bytes(slots.capacity())).expect("admitted support bytes fit usize"),
        );
        Ok(slots)
    }

    pub(crate) fn new(
        rule: &'source RuleIr,
        completed: &mut CompletedSupport<'source>,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
    ) -> Result<Self, FormulaFailure> {
        // Descriptor traversal and reservations consume checker work; the
        // shared planner retains its original source TermWork charges.
        for _ in &rule.body {
            counters.work(limits, rule.location)?;
        }
        counters.work(limits, rule.location)?;
        let queries = completed.queries(
            crate::JoinStrategy::Indexed,
            limits,
            counters,
            rule.location,
        )?;
        let computation = queries.computation(rule.location)?;
        let empty = Binding::new(&computation, limits, counters, rule.location)?;
        let plan = order::Plan::new(
            &rule.body,
            &empty,
            rule.variables,
            order::SourceRows {
                relations: &completed.relations,
                pivot: None,
            },
            budget,
            rule.location,
            Some(&mut |capacity| match capacity {
                order::Capacity::Requested(bytes) => {
                    counters.work(limits, rule.location)?;
                    computation.preparation_capacity(
                        order::Capacity::Requested(bytes),
                        limits,
                        counters,
                        rule.location,
                    )
                }
                order::Capacity::Allocated(bytes) => {
                    // Actual capacity remains evidence even when the next
                    // work permit refuses; retain that original refusal.
                    let observed = computation.preparation_capacity(
                        order::Capacity::Allocated(bytes),
                        limits,
                        counters,
                        rule.location,
                    );
                    counters.work(limits, rule.location)?;
                    observed
                }
            }),
        )?;
        completed.admit_workspace(plan.retained_bytes(), limits, counters, rule.location)?;
        completed.retain_workspace(
            usize::try_from(plan.retained_bytes()).expect("admitted support bytes fit usize"),
        );
        Ok(Self {
            rule,
            completion: completed.completion,
            plan,
        })
    }

    /// The exact rule is retained here; the query must authenticate the same
    /// completed catalog before the shared join borrows its immutable plan.
    pub(crate) fn rows<'a, 'queries>(
        &'a self,
        queries: &'a CompletedQueries<'queries>,
        filter: Option<&'a dyn RowFilter>,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
    ) -> Result<FilteredRows<'a, 'queries>, FormulaFailure> {
        if !self.completion.same(queries.completion) {
            return Err(FormulaFailure::SupportRelation {
                error: zetesis_core::relation::Failure::Owner,
                location: self.rule.location,
            });
        }
        Join::filtered_rule(
            self.rule,
            queries.support(),
            filter,
            Some(&self.plan),
            budget,
            crate::formula_support::Context::new(computation, limits, counters, self.rule.location),
        )
    }
}

#[cfg(test)]
mod tests;
