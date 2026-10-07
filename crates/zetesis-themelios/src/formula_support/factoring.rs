//! Consecutive positive witnesses over one checked continuation and support.
//!
//! The base cursor keeps one lent lookahead in its original frame. The proposal
//! cursor keeps the representative's inputs and comparison certificate. Neither
//! cursor may advance the other's state implicitly.

use super::{
    Advance, Computation, Context, Frame, GroundingWork, Join, Ownership, PositivePattern, Row,
    StorageLease, comparison,
};
use crate::FormulaFailure;
use crate::expansion::Budget;
use crate::formula_binding::Binding;
use crate::formula_ir::{HeadIr, Operation, RuleIr};
use crate::grounding_observer::Event;

pub(crate) struct Continuations<'join, 'a, 'source> {
    join: &'join mut Join<'a, 'source>,
    inputs: &'a [usize],
    suspended: bool,
    finished: bool,
    lease: StorageLease,
}

impl<'join, 'a, 'source> Continuations<'join, 'a, 'source> {
    /// A checked assignment plan supplies all non-positive and head reads.
    /// Lookahead is restricted to a total base stage, so it cannot reorder a
    /// later arithmetic failure ahead of the current continuation's failure.
    pub(crate) fn new(
        join: &'join mut Join<'a, 'source>,
        rule: &'a RuleIr,
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<Option<Self>, FormulaFailure> {
        let Context {
            computation,
            work:
                GroundingWork {
                    limits,
                    counters,
                    location,
                },
        } = context;
        if !std::ptr::eq(join.literals, rule.body.as_slice()) {
            return Err(FormulaFailure::SupportRelation {
                error: zetesis_core::relation::Failure::Owner,
                location,
            });
        }
        if !matches!(rule.head, HeadIr::Normal(Some(_))) || !join.generated {
            return Ok(None);
        }
        let Some(inputs) = rule
            .bindings
            .as_ref()
            .and_then(|plan| plan.continuation_inputs.as_deref())
        else {
            return Ok(None);
        };
        if join.plan.patterns.is_empty() {
            return Ok(None);
        }
        for occurrence in &join.plan.patterns {
            counters.work(limits, location)?;
            if !matches!(occurrence.pattern, PositivePattern::Flat(_)) {
                return Ok(None);
            }
        }
        for (index, literal) in join.literals.iter().enumerate() {
            counters.work(limits, location)?;
            if join.plan.decisions.decides(index) {
                let (left, _, right) = comparison(literal).expect("prefix comparison");
                for node in left.nodes.iter().chain(&right.nodes) {
                    counters.work(limits, location)?;
                    if !matches!(node, Operation::Constant(_) | Operation::Variable(_)) {
                        return Ok(None);
                    }
                }
            }
        }
        let mut lease = computation.lease();
        lease.observe(size_of::<Self>(), location)?;
        computation.storage_observed(&lease, 0, size_of::<Self>(), limits, counters, location)?;
        Ok(Some(Self {
            join,
            inputs,
            suspended: false,
            finished: false,
            lease,
        }))
    }

    /// Begin exactly one continuation. A differing lent row has already passed
    /// base admission; copy it once before resuming its suspended undo.
    pub(crate) fn start(
        &mut self,
        budget: &mut Budget,
        context: Context<'_, &mut Computation<'_, '_>>,
    ) -> Result<bool, FormulaFailure> {
        let Context {
            computation,
            work:
                GroundingWork {
                    limits,
                    counters,
                    location,
                },
        } = context;
        computation.allowance(&self.lease, limits, location)?;
        if self.finished {
            return Ok(false);
        }
        let values = if self.suspended {
            let values = self
                .join
                .values
                .copied(computation, limits, counters, location)?;
            counters.record(Event::BindingSnapshot);
            self.join.resume(limits, counters, location)?;
            self.suspended = false;
            values
        } else {
            let Some(frame) = self.join.next_base(
                Ownership::Own,
                None,
                budget,
                Context::new(computation, limits, counters, location),
            )?
            else {
                self.finished = true;
                return Ok(false);
            };
            frame.into_owned()
        };
        self.join.prepare_continuation(
            values,
            None,
            Context::new(computation, limits, counters, location),
        )?;
        Ok(true)
    }

    /// The ordinary staging loop supplies filtering, head evaluation and family
    /// evidence, but cannot advance the independently suspended base cursor.
    pub(crate) fn next(
        &mut self,
        budget: &mut Budget,
        context: Context<'_, &mut Computation<'_, '_>>,
    ) -> Result<Option<Row<'static>>, FormulaFailure> {
        let row = self
            .join
            .next_staged(Ownership::Own, None, Advance::Current, budget, context)?;
        Ok(row.map(|row| Row {
            values: row.frame.into_owned(),
            passes: row.passes,
        }))
    }

    /// Match every base row normally. Equal inputs lend the actual witness;
    /// a differing row remains in the base frame for the next continuation.
    pub(crate) fn witness(
        &mut self,
        budget: &mut Budget,
        context: Context<'_, &mut Computation<'_, '_>>,
    ) -> Result<Option<Binding<'_>>, FormulaFailure> {
        let Context {
            computation,
            work:
                GroundingWork {
                    limits,
                    counters,
                    location,
                },
        } = context;
        assert!(!self.suspended, "a different row belongs to the next run");
        if self.finished {
            return Ok(None);
        }
        let Some(frame) = self.join.next_base(
            Ownership::Lend,
            None,
            budget,
            Context::new(computation, limits, counters, location),
        )?
        else {
            self.finished = true;
            return Ok(None);
        };
        assert!(
            matches!(frame, Frame::Current),
            "witnesses lend the base frame"
        );
        let binding = Binding::borrowed(self.join.values.slots());
        let equal = self
            .join
            .pending
            .as_ref()
            .expect("started continuation")
            .matches_inputs(
                &binding,
                self.inputs,
                Context::new(computation, limits, counters, location),
            )?;
        if equal {
            Ok(Some(binding))
        } else {
            self.suspended = true;
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests;
