//! The actual successful producer trace, over the one family's term owner.
//!
//! Distinct tuples are ordered once by the original producer. Occurrences retain
//! that order's coordinates, not previous full bindings. Each final local join
//! still computes its own current eligibility frame.

use super::Workspace;
use crate::FormulaFailure;
use crate::expansion::Budget;
use crate::formula_assignment::{Collected, TupleRows};
use crate::formula_binding::Binding;
use crate::formula_ground::cache::CoordinateMap;
use crate::formula_ir::AggregateIr;
use crate::formula_support::{
    Buffer, Computation, Context, GroundingWork, LocalPlanCertificate, StorageLease, Support,
};
use zetesis_core::catalog::TermKey;

#[derive(Clone, Copy)]
pub(in crate::formula_ground) struct Tuple {
    pub(in crate::formula_ground) coordinate: usize,
    pub(in crate::formula_ground) required: bool,
}

pub(in crate::formula_ground) struct Segment<'source> {
    pub(in crate::formula_ground) end: usize,
    prefix_len: usize,
    empty: bool,
    plan: Option<LocalPlanCertificate<'source>>,
}

pub(in crate::formula_ground) struct Replay<'source> {
    pub(in crate::formula_ground) tuples: Buffer<Tuple>,
    pub(in crate::formula_ground) occurrences: Buffer<usize>,
    pub(in crate::formula_ground) segments: Buffer<Segment<'source>>,
    lease: StorageLease,
}

struct Draft<'source> {
    occurrences: Buffer<usize>,
    segments: Buffer<Segment<'source>>,
}

pub(super) struct Trace<'owner, 'query, 'source> {
    owner: &'owner Workspace<'query, 'source>,
    draft: Option<Draft<'source>>,
    lease: StorageLease,
}

impl<'owner, 'query, 'source> Trace<'owner, 'query, 'source> {
    pub(super) fn new(
        owner: &'owner Workspace<'query, 'source>,
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<Self, FormulaFailure> {
        let Context { computation, work } = context;
        let GroundingWork {
            limits,
            counters,
            location,
        } = work;
        let occurrences = Buffer::new(computation, limits, counters, location)?;
        let segments = Buffer::new(computation, limits, counters, location)?;
        let mut lease = computation.lease();
        let header =
            size_of::<Self>() - size_of::<Buffer<usize>>() - size_of::<Buffer<Segment<'source>>>();
        lease.observe(header, location)?;
        computation.storage_observed(&lease, 0, header, limits, counters, location)?;
        Ok(Self {
            owner,
            draft: Some(Draft {
                occurrences,
                segments,
            }),
            lease,
        })
    }

    /// Remap the actual occurrence stream to the producer's already established
    /// ASP tuple order. The temporary coordinate index dies before publication;
    /// no second tuple table or full binding is retained.
    pub(super) fn finish(
        mut self,
        collected: &Collected,
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<Option<Replay<'source>>, FormulaFailure> {
        let Context { computation, work } = context;
        let GroundingWork {
            limits,
            counters,
            location,
        } = work;
        counters.work(limits, location)?;
        let Some(mut draft) = self.draft.take() else {
            return Ok(None);
        };
        let mut tuples = Buffer::new(computation, limits, counters, location)?;
        tuples.reserve(
            collected.tuples.len(),
            computation,
            limits,
            counters,
            location,
        )?;
        let mut order = CoordinateMap::new(computation, limits, counters, location)?;
        order.reserve(
            collected.tuples.len(),
            computation,
            limits,
            counters,
            location,
        )?;
        for at in 0..collected.tuples.len() {
            counters.work(limits, location)?;
            let key = collected.tuples.key(at, location)?;
            let coordinate = self
                .owner
                .terms(GroundingWork::new(limits, counters, location))?
                .find(&key, computation, limits, counters, location)?
                .expect("every collected tuple had an observed occurrence");
            let required = collected
                .required
                .contains(&key, limits, counters, location)?;
            tuples.push(
                Tuple {
                    coordinate,
                    required,
                },
                computation,
                limits,
                counters,
                location,
            )?;
            order.insert(
                coordinate,
                at,
                None,
                Context::new(computation, limits, counters, location),
            )?;
        }
        for occurrence in draft.occurrences.slice_mut() {
            counters.work(limits, location)?;
            let at = order
                .find(occurrence, computation, limits, counters, location)?
                .expect("every observed tuple belongs to the complete collection");
            counters.work(limits, location)?;
            *occurrence = at;
        }
        drop(order);
        let previous = self.lease.bytes();
        let header = size_of::<Replay<'source>>()
            - size_of::<Buffer<Tuple>>()
            - size_of::<Buffer<usize>>()
            - size_of::<Buffer<Segment<'source>>>();
        self.lease.observe(header, location)?;
        computation.storage_observed(&self.lease, previous, header, limits, counters, location)?;
        Ok(Some(Replay {
            tuples,
            occurrences: draft.occurrences,
            segments: draft.segments,
            lease: self.lease,
        }))
    }
}

impl<'source> TupleRows<'source> for Trace<'_, '_, 'source> {
    const RETAIN: bool = true;
    fn retaining(&self) -> bool {
        self.draft.is_some()
    }

    fn begin(
        &mut self,
        plan: Option<LocalPlanCertificate<'source>>,
        prefix_len: usize,
        empty: bool,
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<(), FormulaFailure> {
        let Context { computation, work } = context;
        let GroundingWork {
            limits,
            counters,
            location,
        } = work;
        counters.work(limits, location)?;
        if self.draft.is_none() {
            return Ok(());
        }
        if !empty && plan.is_none() {
            // Absence of a certificate is an ordinary applicability result.
            // Typed constructor failures never reach this point. Return all
            // private trace capacities; already performed work/peak stays real.
            let previous = self.lease.bytes();
            drop(self.draft.take());
            self.lease.observe(size_of::<Self>(), location)?;
            computation.storage_observed(
                &self.lease,
                previous,
                size_of::<Self>(),
                limits,
                counters,
                location,
            )?;
            return Ok(());
        }
        let draft = self.draft.as_mut().expect("active trace");
        draft.segments.push(
            Segment {
                end: draft.occurrences.len(),
                prefix_len,
                empty,
                plan,
            },
            computation,
            limits,
            counters,
            location,
        )
    }

    fn row(
        &mut self,
        tuple: &TermKey,
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<(), FormulaFailure> {
        let Context { computation, work } = context;
        let GroundingWork {
            limits,
            counters,
            location,
        } = work;
        counters.work(limits, location)?;
        let Some(draft) = self.draft.as_mut() else {
            return Ok(());
        };
        let coordinate = self
            .owner
            .terms_mut(GroundingWork::new(limits, counters, location))?
            .insert(tuple, None, computation, limits, counters, location)?;
        draft
            .occurrences
            .push(coordinate, computation, limits, counters, location)
    }

    fn end(&mut self, context: Context<'_, &Computation<'_, '_>>) -> Result<(), FormulaFailure> {
        let Context { computation, work } = context;
        let GroundingWork {
            limits,
            counters,
            location,
        } = work;
        computation.allowance(&self.lease, limits, location)?;
        counters.work(limits, location)?;
        let Some(draft) = self.draft.as_mut() else {
            return Ok(());
        };
        draft
            .segments
            .slice_mut()
            .last_mut()
            .expect("an element begins before it ends")
            .end = draft.occurrences.len();
        Ok(())
    }
}

impl<'source> Replay<'source> {
    /// Carrier validity has already been authenticated by the single family
    /// key. Ordered replay has this additional all-element precondition. This
    /// checks only existing immutable plans, never prepares or executes a join.
    pub(in crate::formula_ground) fn certifies(
        &self,
        aggregate: &'source AggregateIr,
        assignment: &Binding,
        support: &Support<'source>,
        budget: &mut Budget,
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<bool, FormulaFailure> {
        let Context { computation, work } = context;
        let GroundingWork {
            limits,
            counters,
            location,
        } = work;
        computation.allowance(&self.lease, limits, location)?;
        counters.work(limits, location)?;
        assert_eq!(
            self.segments.len(),
            aggregate.elements.len(),
            "same original aggregate"
        );
        for (segment, element) in self.segments.iter().zip(&aggregate.elements) {
            counters.work(limits, location)?;
            if segment.prefix_len != assignment.len() {
                return Ok(false);
            }
            if segment.empty {
                assert!(element.condition.is_empty(), "same original element");
                continue;
            }
            let Some(plan) = &segment.plan else {
                return Ok(false);
            };
            if !support.certifies_local_plan(
                plan,
                &element.condition,
                assignment,
                element.variables,
                budget,
                Context::new(computation, limits, counters, location),
            )? {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

#[cfg(test)]
mod tests;
