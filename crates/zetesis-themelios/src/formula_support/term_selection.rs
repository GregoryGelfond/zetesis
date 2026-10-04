//! A selected root set and semantic order over one canonical vocabulary.
use crate::ProgramSite;
use crate::formula::ceiling;
use crate::formula_binding::Binding;
use crate::formula_support::{Computation, Counters, StorageLease};
use crate::{FormulaFailure, FormulaLimits, FormulaResource};
use std::cmp::Ordering;
use zetesis_core::catalog::{AssignmentError, AssignmentFailure, Error, TermKey, TermRef, TermSet};

use crate::formula_support::{Context, GroundingWork};
pub(crate) struct TermSelection {
    values: Binding<'static>,
    selected: TermSet,
    lease: StorageLease,
}
impl TermSelection {
    pub(crate) fn new(
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Self, FormulaFailure> {
        let values = Binding::new(computation, limits, counters, location)?;
        let selected = computation.read().term_set();
        let mut lease = computation.lease();
        lease.observe(size_of::<Self>() - size_of::<Binding>(), location)?;
        computation.storage_observed(
            &lease,
            0,
            size_of::<Self>() - size_of::<Binding>(),
            limits,
            counters,
            location,
        )?;
        Ok(Self {
            values,
            selected,
            lease,
        })
    }
    pub(crate) fn insert(
        &mut self,
        value: &TermKey,
        resource: FormulaResource,
        maximum: usize,
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<(), FormulaFailure> {
        let Context {
            computation,
            work:
                GroundingWork {
                    limits,
                    counters,
                    location,
                },
        } = context;
        if self
            .selected
            .contains_with(value, || counters.work(limits, location))
            .map_err(|error| crate::formula_binding::failure(error, location))?
        {
            return Ok(());
        }
        ceiling(
            resource,
            self.values.len() as u128 + 1,
            maximum as u128,
            location,
        )?;
        let slot = self.values.len();
        let prepared = self
            .values
            .extend_scope(slot + 1, computation, limits, counters, location)
            .and_then(|()| self.values.set(slot, value, limits, counters, location));
        if let Err(error) = prepared {
            self.values.discard_suffix(slot);
            return Err(error);
        }
        let header = size_of::<Self>() - size_of::<Binding>();
        let extra = header - size_of::<TermSet>();
        // The frame may have grown, so recompute the set allowance. A refused
        // preflight rolls back only the provisional logical slot.
        let allowance = match computation.allowance(&self.lease, limits, location) {
            Ok(remaining) => remaining,
            Err(error) => {
                self.values.discard_suffix(slot);
                return Err(error);
            }
        };
        let previous = self.lease.bytes();
        let result = self
            .selected
            .insert_with(value, allowance.saturating_sub(extra), || {
                counters.work(limits, location)
            })
            .map_err(|error| match error {
                AssignmentFailure::Assignment(AssignmentError::Storage(Error::Storage {
                    required,
                    limit,
                })) => AssignmentFailure::Assignment(AssignmentError::Storage(Error::Storage {
                    required: required + extra as u128,
                    limit: limit + extra,
                })),
                error => error,
            });
        self.lease
            .observe(extra + self.selected.retained_bytes(), location)?;
        let observed =
            computation.storage_observed(&self.lease, previous, header, limits, counters, location);
        if result.is_err() {
            self.values.discard_suffix(slot);
        }
        computation.storage_result(result, &self.lease, limits, location)?;
        observed
    }
    pub(crate) fn len(&self) -> usize {
        self.values.len()
    }
    pub(crate) fn contains(
        &self,
        value: &TermKey,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<bool, FormulaFailure> {
        self.selected
            .contains_with(value, || counters.work(limits, location))
            .map_err(|error| crate::formula_binding::failure(error, location))
    }

    pub(crate) fn ordered(
        mut self,
        tuples: bool,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Binding<'static>, FormulaFailure> {
        let len = self.values.len();
        for root in (0..len / 2).rev() {
            self.sift(
                root,
                len,
                tuples,
                Context::new(computation, limits, counters, location),
            )?;
        }
        for end in (1..len).rev() {
            self.values.swap(0, end, limits, counters, location)?;
            self.sift(
                0,
                end,
                tuples,
                Context::new(computation, limits, counters, location),
            )?;
        }
        Ok(self.values)
    }
    fn sift(
        &mut self,
        mut root: usize,
        end: usize,
        tuples: bool,
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<(), FormulaFailure> {
        let Context {
            computation,
            work:
                GroundingWork {
                    limits,
                    counters,
                    location,
                },
        } = context;
        while root < end / 2 {
            let mut child = root * 2 + 1;
            if child + 1 < end
                && self
                    .compare(
                        child,
                        child + 1,
                        tuples,
                        Context::new(computation, limits, counters, location),
                    )?
                    .is_lt()
            {
                child += 1;
            }
            if !self
                .compare(
                    root,
                    child,
                    tuples,
                    Context::new(computation, limits, counters, location),
                )?
                .is_lt()
            {
                break;
            }
            self.values.swap(root, child, limits, counters, location)?;
            root = child;
        }
        Ok(())
    }
    fn compare(
        &self,
        left: usize,
        right: usize,
        tuples: bool,
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<Ordering, FormulaFailure> {
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
        let left = self.values.read(left, read, location)?;
        let right = self.values.read(right, read, location)?;
        if tuples {
            tuple_order(left, right, limits, counters, location)
        } else {
            left.compare_ref_with(right, || counters.work(limits, location))
        }
    }
}

/// Aggregate tuple keys preserve element-wise structural Value ordering and
/// then length. Canonical Tuple's own arity-first identity order is different.
fn tuple_order(
    left: TermRef<'_>,
    right: TermRef<'_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
) -> Result<Ordering, FormulaFailure> {
    let mut column = 0;
    loop {
        counters.work(limits, location)?;
        match (left.child(column), right.child(column)) {
            (Some(left), Some(right)) => {
                let order = left.compare_ref_with(right, || counters.work(limits, location))?;
                if !order.is_eq() {
                    return Ok(order);
                }
            }
            (None, None) => return Ok(Ordering::Equal),
            (None, Some(_)) => return Ok(Ordering::Less),
            (Some(_), None) => return Ok(Ordering::Greater),
        }
        column += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formula_support::{Support, SupportCatalog};
    use themelios_base::{
        source::SourceId,
        span::{ByteOffset, Span},
    };
    use zetesis_core::ValueNodeRef;

    fn location() -> ProgramSite {
        ProgramSite::source(themelios_base::span::Location {
            source: SourceId::new(71),
            span: Span::empty(ByteOffset::new(0)),
        })
    }

    fn with_computation<T>(run: impl FnOnce(&mut Computation<'_, '_>, &mut Counters) -> T) -> T {
        let limits = FormulaLimits::default();
        let mut counters = Counters::default();
        let mut owner = SupportCatalog::default();
        let (relations, mut append) = owner.split(&limits, &mut counters, location()).unwrap();
        let support = Support::indexed(&relations, &limits, &counters, location()).unwrap();
        let mut computation = Computation::new(&mut append, &support);
        run(&mut computation, &mut counters)
    }
    fn retry(
        selection: TermSelection,
        key: &TermKey,
        computation: &Computation<'_, '_>,
        counters: &mut Counters,
    ) {
        let limits = FormulaLimits::default();
        let mut selection = selection;
        selection
            .insert(
                key,
                FormulaResource::AssignmentValues,
                8,
                Context::new(computation, &limits, counters, location()),
            )
            .unwrap();
        assert!(
            selection
                .contains(key, &limits, counters, location())
                .unwrap()
        );
        assert_eq!(selection.len(), 1);
        let ordered = selection
            .ordered(false, computation, &limits, counters, location())
            .unwrap();
        assert_eq!(
            ordered
                .read(0, computation.read(), location())
                .unwrap()
                .descriptor(),
            ValueNodeRef::Number(7)
        );
    }
    #[test]
    fn each_work_stop_preserves_selected_root_on_retry() {
        let needed = with_computation(|computation, counters| {
            let limits = FormulaLimits::default();
            let key = computation
                .number(7, &limits, counters, location())
                .unwrap();
            let mut selection =
                TermSelection::new(computation, &limits, counters, location()).unwrap();
            let before = counters.accounting.work;
            selection
                .insert(
                    &key,
                    FormulaResource::AssignmentValues,
                    8,
                    Context::new(computation, &limits, counters, location()),
                )
                .unwrap();
            counters.accounting.work - before
        });
        for cutoff in 0..needed {
            with_computation(|computation, counters| {
                let limits = FormulaLimits::default();
                let key = computation
                    .number(7, &limits, counters, location())
                    .unwrap();
                let mut selection =
                    TermSelection::new(computation, &limits, counters, location()).unwrap();
                let bounded = FormulaLimits {
                    max_work: counters.accounting.work + cutoff,
                    ..limits
                };
                assert!(
                    selection
                        .insert(
                            &key,
                            FormulaResource::AssignmentValues,
                            8,
                            Context::new(computation, &bounded, counters, location())
                        )
                        .is_err()
                );
                retry(selection, &key, computation, counters);
            });
        }
    }
    #[test]
    fn storage_refusal_preserves_selected_root_on_retry() {
        let mut refused = 0;
        for extra in 0..128 {
            with_computation(|computation, counters| {
                let limits = FormulaLimits::default();
                let key = computation
                    .number(7, &limits, counters, location())
                    .unwrap();
                let mut selection =
                    TermSelection::new(computation, &limits, counters, location()).unwrap();
                let empty = computation.lease();
                let current = limits.max_support_bytes
                    - computation.allowance(&empty, &limits, location()).unwrap();
                let bounded = FormulaLimits {
                    max_support_bytes: current + extra,
                    ..limits
                };
                if selection
                    .insert(
                        &key,
                        FormulaResource::AssignmentValues,
                        8,
                        Context::new(computation, &bounded, counters, location()),
                    )
                    .is_err()
                {
                    refused += 1;
                    retry(selection, &key, computation, counters);
                }
            });
        }
        assert!(refused > 0);
    }
}
