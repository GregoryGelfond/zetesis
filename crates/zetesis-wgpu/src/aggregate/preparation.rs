//! Numeric carrier checking before device ownership or mask packing.

use super::{
    AggregateGpuCapability as Capability, AggregateGpuError as Error, AggregateGpuPlanLimits,
    capacity, poll,
};
use std::sync::Arc;
use zetesis_core::ValueNodeRef;
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{
    AggregateComparison,
    native_aggregate::{Bound, Function, GroupRef},
};

/// Opaque numeric view of one exact retained group. Cloning shares wire storage
/// and keeps the same borrowed Group identity; independent preparations do not
/// silently alias device residency. Preparation does not acquire M/J truth or
/// establish source completeness or aggregate head permission.
#[derive(Clone, Debug)]
pub struct AggregateGpuPlan<'g> {
    pub(super) group: GroupRef<'g>,
    pub(super) numeric: Arc<Numeric>,
}

#[derive(Debug)]
pub(super) struct Numeric {
    pub(super) function: Function,
    pub(super) tuples: Vec<u32>,
    pub(super) guards: Vec<u32>,
    pub(super) tuple_count: u32,
    pub(super) guard_count: u32,
    pub(super) lower: i32,
    pub(super) upper: i32,
    pub(super) bytes: u64,
    pub(super) work: u64,
}

impl<'g> AggregateGpuPlan<'g> {
    /// Check numeric guards and complete contribution carriers, retaining whole
    /// tuple occurrence order. Count includes empty/nonnumeric keys; sums keep
    /// them as neutral contributions. Extrema accept numeric first components
    /// and ignore empty keys; other first terms are an explicit capability limit.
    ///
    /// Signed sums require the sum of all positive contributions ≤ `i32::MAX` and
    /// all negative contributions ≥ `i32::MIN`. Every disjoint parallel partial
    /// subset then fits, regardless of cancellation in the final selected sum.
    /// Work is linear in tuple/guard counts; wire storage is linear and shared.
    ///
    /// # Errors
    /// Returns typed capability, resource, control or fallible vector-allocation
    /// failures. These never change source admission; callers may use the native
    /// CPU reduction. No partially prepared plan escapes.
    pub fn new(
        group: impl Into<GroupRef<'g>>,
        limits: AggregateGpuPlanLimits,
        cancellation: &Cancellation,
    ) -> Result<Self, Error> {
        poll(cancellation)?;
        let group = group.into();
        let tuples = group.tuples().len();
        let guards = group.guards().len();
        let work = u64::try_from(tuples)
            .ok()
            .and_then(|count| count.checked_add(u64::try_from(guards).ok()?))
            .ok_or_else(|| capacity("aggregate preparation work overflow"))?;
        let tuple_words = tuples
            .max(1)
            .checked_mul(2)
            .ok_or_else(|| capacity("aggregate tuple dimensions overflow"))?;
        let guard_words = guards
            .max(1)
            .checked_mul(2)
            .ok_or_else(|| capacity("aggregate guard dimensions overflow"))?;
        let bytes = u64::try_from(tuple_words)
            .ok()
            .and_then(|count| count.checked_add(u64::try_from(guard_words).ok()?))
            .and_then(|count| count.checked_mul(4))
            .ok_or_else(|| capacity("aggregate wire bytes overflow"))?;
        if tuples > limits.max_tuples
            || guards > limits.max_guards
            || work > limits.max_work
            || bytes > limits.max_bytes
        {
            return Err(capacity("aggregate preparation ceiling exceeded").into());
        }
        let tuple_count =
            u32::try_from(tuples).map_err(|_| capacity("aggregate tuple count exceeds u32"))?;
        let guard_count =
            u32::try_from(guards).map_err(|_| capacity("aggregate guard count exceeds u32"))?;
        if group.function() == Function::Count && tuples > i32::MAX as usize {
            return Err(Error::Capability(Capability::CountRange { tuples }));
        }
        let mut numeric = Numeric {
            function: group.function(),
            tuples: reserve(tuple_words)?,
            guards: reserve(guard_words)?,
            tuple_count,
            guard_count,
            lower: 0,
            upper: 0,
            bytes,
            work,
        };
        numeric.pack(group, cancellation)?;
        poll(cancellation)?;
        Ok(Self {
            group,
            numeric: Arc::new(numeric),
        })
    }
    /// Complete retained operation supplying tuple order and eligibility identity.
    #[must_use]
    pub const fn group(&self) -> GroupRef<'g> {
        self.group
    }
    /// Requested retained numeric wire bytes, including empty-binding padding.
    #[must_use]
    pub fn bytes(&self) -> u64 {
        self.numeric.bytes
    }
    /// Completed tuple/guard preparation visits, excluding Group construction.
    #[must_use]
    pub fn work(&self) -> u64 {
        self.numeric.work
    }
}

impl Numeric {
    fn pack(&mut self, group: GroupRef<'_>, cancellation: &Cancellation) -> Result<(), Error> {
        for (index, tuple) in group.tuples().iter().enumerate() {
            poll(cancellation)?;
            let first = tuple
                .key
                .first()
                .map(zetesis_core::catalog::TermRef::descriptor);
            let (value, present) = match self.function {
                Function::Count => (1, true),
                Function::Sum | Function::SumPlus => match first {
                    Some(ValueNodeRef::Number(value))
                        if self.function == Function::Sum || value > 0 =>
                    {
                        (value, true)
                    }
                    _ => (0, true),
                },
                Function::Min | Function::Max => match first {
                    Some(ValueNodeRef::Number(value)) => (value, true),
                    None => (0, false),
                    _ => {
                        return Err(Error::Capability(Capability::NonNumericExtremum {
                            tuple: index,
                        }));
                    }
                },
            };
            self.carrier(value)?;
            self.tuples.extend([bits(value), u32::from(present)]);
        }
        for index in 0..group.guards().len() {
            poll(cancellation)?;
            let guard = group.guard(index).expect("admitted guard extent");
            let value = match guard.bound {
                Bound::Integer(value) => i32::try_from(value)
                    .map_err(|_| Error::Capability(Capability::GuardRange { guard: index }))?,
                Bound::Term(value) => match value.descriptor() {
                    ValueNodeRef::Number(value) => value,
                    _ => {
                        return Err(Error::Capability(Capability::NonNumericGuard {
                            guard: index,
                        }));
                    }
                },
            };
            self.guards
                .extend([comparison(guard.comparison), bits(value)]);
        }
        if self.tuples.is_empty() {
            self.tuples.extend([0, 0]);
        }
        if self.guards.is_empty() {
            self.guards.extend([0, 0]);
        }
        Ok(())
    }

    fn carrier(&mut self, value: i32) -> Result<(), Error> {
        if matches!(self.function, Function::Min | Function::Max) {
            self.lower = self.lower.min(value);
            self.upper = self.upper.max(value);
        } else if value > 0 {
            let total = i64::from(self.upper) + i64::from(value);
            self.upper = i32::try_from(total)
                .map_err(|_| Error::Capability(Capability::PositiveSum { total }))?;
        } else {
            let total = i64::from(self.lower) + i64::from(value);
            self.lower = i32::try_from(total)
                .map_err(|_| Error::Capability(Capability::NegativeSum { total }))?;
        }
        Ok(())
    }
}

pub(super) fn bits(value: i32) -> u32 {
    u32::from_ne_bytes(value.to_ne_bytes())
}
pub(super) fn signed(value: u32) -> i32 {
    i32::from_ne_bytes(value.to_ne_bytes())
}

fn comparison(value: AggregateComparison) -> u32 {
    match value {
        AggregateComparison::Eq => 0,
        AggregateComparison::Ne => 1,
        AggregateComparison::Lt => 2,
        AggregateComparison::Le => 3,
        AggregateComparison::Gt => 4,
        AggregateComparison::Ge => 5,
    }
}

pub(super) fn reserve<T>(count: usize) -> Result<Vec<T>, crate::GpuError> {
    let mut values = Vec::new();
    values.try_reserve_exact(count).map_err(|error| {
        crate::GpuError::new(crate::GpuErrorKind::Allocation, error.to_string())
    })?;
    Ok(values)
}
