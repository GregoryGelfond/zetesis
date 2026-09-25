//! Live source workspace ownership, independent of cumulative work charges.
//!
//! Frames can move with a retained checker. Their leases therefore share a
//! Send-capable counter rather than borrowing a query-local Cell. Updates happen
//! only when named storage changes or its owner drops, never on term access.

use crate::formula_support::{Context, GroundingWork};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use themelios_base::span::Location;
use zetesis_core::catalog::{AssignmentError, AssignmentFailure, Error};

use crate::{FormulaFailure, FormulaResource};

#[derive(Clone, Debug, Default)]
pub(crate) struct Workspace(Arc<AtomicUsize>);

#[derive(Debug)]
pub(crate) struct StorageLease {
    workspace: Workspace,
    bytes: usize,
}

impl Workspace {
    pub(crate) fn bytes(&self) -> usize {
        self.0.load(Ordering::Relaxed)
    }

    pub(crate) fn lease(&self) -> StorageLease {
        StorageLease {
            workspace: self.clone(),
            bytes: 0,
        }
    }

    pub(crate) fn owns(&self, lease: &StorageLease) -> bool {
        Arc::ptr_eq(&self.0, &lease.workspace.0)
    }
}

impl StorageLease {
    pub(crate) fn bytes(&self) -> usize {
        self.bytes
    }

    /// Move an existing receipt between owners in the same workspace. No
    /// allocation occurs and the shared total never changes during the transfer.
    pub(crate) fn transfer_to(
        mut self,
        recipient: &mut Self,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        if !recipient.workspace.owns(&self) {
            return Err(crate::formula_binding::assignment(
                zetesis_core::catalog::AssignmentError::Read(
                    zetesis_core::catalog::ReadError::ForeignCatalog,
                ),
                location,
            ));
        }
        let bytes = recipient
            .bytes
            .checked_add(self.bytes)
            .ok_or(FormulaFailure::Limit {
                resource: FormulaResource::SupportBytes,
                observed: recipient.bytes as u128 + self.bytes as u128,
                limit: usize::MAX as u128,
                location,
            })?;
        recipient.bytes = bytes;
        self.bytes = 0;
        Ok(())
    }

    /// Account actual retained capacity, including capacity left by a refused
    /// operation. Admission precedes allocation in the caller. This receipt is
    /// recorded even when actual allocator capacity subsequently exceeds it.
    pub(crate) fn observe(
        &mut self,
        bytes: usize,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        if bytes == self.bytes {
            return Ok(());
        }
        self.workspace
            .0
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |used| {
                used.checked_sub(self.bytes)?.checked_add(bytes)
            })
            .map_err(|used| FormulaFailure::Limit {
                resource: FormulaResource::SupportBytes,
                observed: used as u128 - self.bytes as u128 + bytes as u128,
                limit: usize::MAX as u128,
                location,
            })?;
        self.bytes = bytes;
        Ok(())
    }
}

impl Drop for StorageLease {
    fn drop(&mut self) {
        let previous = self.workspace.0.fetch_sub(self.bytes, Ordering::Relaxed);
        debug_assert!(previous >= self.bytes, "each capacity has one live owner");
    }
}

/// Reserve one buffer inside a leased workspace. The header argument includes
/// every other buffer owned by this lease, so replacement overlap counts only
/// this buffer's old capacity. Geometric spare capacity is requested only when
/// its full replacement peak fits; otherwise the exact requirement is used.
/// The caller admits element initialization work.
pub(crate) fn reserve<T>(
    values: &mut Vec<T>,
    additional: usize,
    lease: &mut StorageLease,
    header_and_other_bytes: usize,
    context: Context<'_, &super::Computation<'_, '_>>,
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
    reserve_with_growth(
        values,
        additional,
        lease,
        header_and_other_bytes,
        true,
        Context::new(computation, limits, counters, location),
    )
}

/// Reserve a caller-planned capacity without adding spare cells. This uses the
/// same work, replacement-peak and actual-capacity checks as [`reserve`].
pub(crate) fn reserve_exact<T>(
    values: &mut Vec<T>,
    additional: usize,
    lease: &mut StorageLease,
    header_and_other_bytes: usize,
    context: Context<'_, &super::Computation<'_, '_>>,
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
    reserve_with_growth(
        values,
        additional,
        lease,
        header_and_other_bytes,
        false,
        Context::new(computation, limits, counters, location),
    )
}

/// Existing spare capacity is retained. Overflow in the optional doubling does
/// not refuse an otherwise representable exact requirement.
pub(crate) fn growth_capacity(current: usize, required: usize) -> usize {
    if required <= current {
        current
    } else {
        current.checked_mul(2).unwrap_or(required).max(required)
    }
}

fn reserve_with_growth<T>(
    values: &mut Vec<T>,
    additional: usize,
    lease: &mut StorageLease,
    header_and_other_bytes: usize,
    geometric: bool,
    context: Context<'_, &super::Computation<'_, '_>>,
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

    let maximum = computation.allowance(lease, limits, location)?;
    let previous = lease.bytes();
    let result = (|| {
        let count = values
            .len()
            .checked_add(additional)
            .ok_or(AssignmentFailure::Assignment(AssignmentError::Storage(
                Error::Overflow,
            )))?;
        if count <= values.capacity() {
            return Ok(());
        }
        let required = header_and_other_bytes as u128
            + values.capacity() as u128 * size_of::<T>() as u128
            + count as u128 * size_of::<T>() as u128;
        if required > maximum as u128 {
            return Err(AssignmentFailure::Assignment(AssignmentError::Storage(
                Error::Storage {
                    required,
                    limit: maximum,
                },
            )));
        }
        let preferred = if geometric {
            growth_capacity(values.capacity(), count)
        } else {
            count
        };
        let preferred_peak = header_and_other_bytes as u128
            + values.capacity() as u128 * size_of::<T>() as u128
            + preferred as u128 * size_of::<T>() as u128;
        let target = if preferred_peak <= maximum as u128 {
            preferred
        } else {
            count
        };
        for _ in 0..values.len() {
            counters
                .work(limits, location)
                .map_err(AssignmentFailure::Stopped)?;
        }
        counters
            .work(limits, location)
            .map_err(AssignmentFailure::Stopped)?;
        values
            .try_reserve_exact(target - values.len())
            .map_err(|_| AssignmentFailure::Assignment(AssignmentError::Storage(Error::Allocation)))
    })();
    let actual = values
        .capacity()
        .checked_mul(size_of::<T>())
        .and_then(|bytes| bytes.checked_add(header_and_other_bytes))
        .ok_or_else(|| {
            crate::formula_binding::assignment(AssignmentError::Storage(Error::Overflow), location)
        })?;
    lease.observe(actual, location)?;
    let observed = computation.storage_observed(
        lease,
        previous,
        header_and_other_bytes,
        limits,
        counters,
        location,
    );
    computation.storage_result(result, lease, limits, location)?;
    observed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FormulaLimits;
    use crate::formula_support::testing::Fixture;
    use themelios_base::source::SourceId;
    use themelios_base::span::{ByteOffset, Span};

    fn location() -> Location {
        Location {
            source: SourceId::new(0),
            span: Span::empty(ByteOffset::new(0)),
        }
    }

    #[test]
    fn lease_transfer_preserves_the_live_total() {
        let workspace = Workspace::default();
        let mut donor = workspace.lease();
        let mut recipient = workspace.lease();
        donor.observe(13, location()).unwrap();
        recipient.observe(7, location()).unwrap();
        donor.transfer_to(&mut recipient, location()).unwrap();
        assert_eq!(recipient.bytes(), 20);
        assert_eq!(workspace.bytes(), 20);
        drop(recipient);
        assert_eq!(workspace.bytes(), 0);
    }

    #[test]
    fn foreign_lease_transfer_is_refused() {
        let first = Workspace::default();
        let second = Workspace::default();
        let mut donor = first.lease();
        let mut recipient = second.lease();
        donor.observe(13, location()).unwrap();
        recipient.observe(7, location()).unwrap();
        assert!(matches!(
            donor.transfer_to(&mut recipient, location()),
            Err(FormulaFailure::TermAssignment { .. })
        ));
        assert_eq!(first.bytes(), 0);
        assert_eq!(second.bytes(), 7);
        assert_eq!(recipient.bytes(), 7);
    }

    #[test]
    fn incremental_reservation_has_linear_copy_work() {
        Fixture::default().with(location(), |_, computation, counters| {
            let limits = FormulaLimits::default();
            let header = size_of::<Vec<usize>>();
            let mut lease = computation.lease();
            lease.observe(header, location()).unwrap();
            let mut values = Vec::<usize>::new();
            let before = counters.accounting.work;
            let mut growths = 0;
            for value in 0..257 {
                let old = values.capacity();
                reserve(
                    &mut values,
                    1,
                    &mut lease,
                    header,
                    Context::new(computation, &limits, counters, location()),
                )
                .unwrap();
                growths += usize::from(values.capacity() > old);
                values.push(value);
            }
            assert!(growths <= 10);
            assert!(counters.accounting.work - before <= 2 * 257 + 10);
            assert_eq!(
                lease.bytes(),
                header + values.capacity() * size_of::<usize>()
            );
            assert!(values.iter().copied().eq(0..257));
        });
    }

    #[test]
    fn reservation_uses_exact_capacity_near_the_peak_limit() {
        Fixture::default().with(location(), |_, computation, counters| {
            let limits = FormulaLimits::default();
            let header = size_of::<Vec<usize>>();
            let mut lease = computation.lease();
            lease.observe(header, location()).unwrap();
            let mut values = Vec::new();
            reserve_exact(
                &mut values,
                3,
                &mut lease,
                header,
                Context::new(computation, &limits, counters, location()),
            )
            .unwrap();
            values.resize(values.capacity(), 7_usize);
            let old = values.capacity();
            let required = old + 1;
            let outer = limits.max_support_bytes
                - computation.allowance(&lease, &limits, location()).unwrap();
            let bounded = FormulaLimits {
                max_support_bytes: outer + header + (old + required) * size_of::<usize>(),
                ..limits
            };
            let before = counters.accounting.work;
            reserve(
                &mut values,
                1,
                &mut lease,
                header,
                Context::new(computation, &bounded, counters, location()),
            )
            .unwrap();
            assert_eq!(values.capacity(), required);
            assert_eq!(values.len(), old);
            assert_eq!(
                counters.accounting.work - before,
                u64::try_from(old + 1).unwrap()
            );
            assert_eq!(lease.bytes(), header + required * size_of::<usize>());
        });
    }

    #[test]
    fn refused_reservation_keeps_only_actual_capacity() {
        Fixture::default().with(location(), |_, computation, counters| {
            let limits = FormulaLimits::default();
            let header = size_of::<Vec<usize>>();
            let mut lease = computation.lease();
            lease.observe(header, location()).unwrap();
            let mut values = Vec::new();
            reserve_exact(
                &mut values,
                3,
                &mut lease,
                header,
                Context::new(computation, &limits, counters, location()),
            )
            .unwrap();
            values.resize(values.capacity(), 7_usize);
            let capacity = values.capacity();
            let retained = lease.bytes();
            let outer = limits.max_support_bytes
                - computation.allowance(&lease, &limits, location()).unwrap();
            let bounded = FormulaLimits {
                max_support_bytes: outer + header + (2 * capacity + 1) * size_of::<usize>() - 1,
                ..limits
            };
            let before = counters.accounting.work;
            assert!(matches!(
                reserve(
                    &mut values,
                    1,
                    &mut lease,
                    header,
                    Context::new(computation, &bounded, counters, location())
                ),
                Err(FormulaFailure::Limit {
                    resource: FormulaResource::SupportBytes,
                    ..
                })
            ));
            assert_eq!(values.capacity(), capacity);
            assert_eq!(values.len(), capacity);
            assert_eq!(lease.bytes(), retained);
            assert_eq!(counters.accounting.work, before);
        });
    }

    #[test]
    fn zero_sized_reservation_keeps_only_the_header() {
        Fixture::default().with(location(), |_, computation, counters| {
            let limits = FormulaLimits::default();
            let header = size_of::<Vec<()>>();
            let mut lease = computation.lease();
            lease.observe(header, location()).unwrap();
            let mut values = Vec::<()>::new();
            let before = counters.accounting.work;
            reserve(
                &mut values,
                1024,
                &mut lease,
                header,
                Context::new(computation, &limits, counters, location()),
            )
            .unwrap();
            assert_eq!(lease.bytes(), header);
            assert_eq!(counters.accounting.work, before);
            assert!(values.is_empty());
        });
    }

    #[test]
    fn doubling_overflow_preserves_the_exact_requirement() {
        assert_eq!(
            growth_capacity(usize::MAX / 2 + 1, usize::MAX / 2 + 2),
            usize::MAX / 2 + 2
        );
    }
}
