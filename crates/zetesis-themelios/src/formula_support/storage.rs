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
use zetesis_core::catalog::{AssignmentError, AssignmentFailure, Error, ReadError};

use crate::formula::ceiling;
use crate::grounding_observer::Event;
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

    /// Authenticate the one buffer whose allocation is being measured. External
    /// bytes describe the caller's named storage outside this whole workspace;
    /// they must not contain any of its leases. This view owns no storage and
    /// adds neither a new allowance nor a second receipt for shared payload.
    pub(crate) fn scope<'a>(
        &'a self,
        lease: &'a StorageLease,
        external_bytes: usize,
    ) -> Result<Scope<'a>, ReadError> {
        if !self.owns(lease) {
            return Err(ReadError::ForeignCatalog);
        }
        Ok(Scope {
            workspace: self,
            lease,
            external_bytes,
        })
    }
}

/// One authenticated lease within the current workspace and its external owner.
/// Resource proposals and actual peaks stay distinct. Each owner translates an
/// authentication error at its existing semantic boundary before using this
/// common arithmetic; the workspace itself knows nothing about source truth.
pub(crate) struct Scope<'a> {
    workspace: &'a Workspace,
    lease: &'a StorageLease,
    external_bytes: usize,
}

impl Scope<'_> {
    pub(crate) fn live_bytes(&self) -> u128 {
        self.external_bytes as u128 + self.workspace.bytes() as u128
    }

    fn outside(&self) -> u128 {
        // Authentication and the lease's sole capacity receipt establish this
        // inclusion. Wide addition retains an over-limit requirement exactly.
        self.live_bytes() - self.lease.bytes() as u128
    }

    /// The component receives all remaining allowance after other live owners.
    /// A caller requiring its current total to fit checks that separately; this
    /// operation also supports reporting a component's refused actual capacity.
    pub(crate) fn allowance(
        &self,
        limits: &crate::FormulaLimits,
        location: Location,
    ) -> Result<usize, FormulaFailure> {
        let outside = self.outside();
        ceiling(
            FormulaResource::SupportBytes,
            outside,
            limits.max_support_bytes as u128,
            location,
        )?;
        Ok(limits.max_support_bytes - usize::try_from(outside).expect("admitted usize bound"))
    }

    /// A replacement retains its prior buffer during allocation; shared headers
    /// are counted once. A refused proposal is never recorded as an actual peak.
    pub(crate) fn observed(
        &self,
        previous: usize,
        header: usize,
        limits: &crate::FormulaLimits,
        counters: &super::Counters,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        let overlap = if self.lease.bytes() > previous {
            previous.saturating_sub(header)
        } else {
            0
        };
        self.peak(
            self.lease.bytes() as u128 + overlap as u128,
            limits,
            counters,
            location,
        )
    }

    pub(crate) fn peak(
        &self,
        component_peak: u128,
        limits: &crate::FormulaLimits,
        counters: &super::Counters,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        let peak = self.outside().checked_add(component_peak).ok_or_else(|| {
            crate::formula_binding::assignment(AssignmentError::Storage(Error::Overflow), location)
        })?;
        counters.record(Event::SupportPeakBytes(peak));
        ceiling(
            FormulaResource::SupportBytes,
            peak,
            limits.max_support_bytes as u128,
            location,
        )
    }

    pub(crate) fn result<T>(
        &self,
        result: Result<T, AssignmentFailure<FormulaFailure>>,
        limits: &crate::FormulaLimits,
        location: Location,
    ) -> Result<T, FormulaFailure> {
        result.map_err(|error| match error {
            AssignmentFailure::Assignment(AssignmentError::Storage(Error::Storage {
                required,
                ..
            })) => self.outside().checked_add(required).map_or_else(
                || {
                    crate::formula_binding::assignment(
                        AssignmentError::Storage(Error::Overflow),
                        location,
                    )
                },
                |observed| FormulaFailure::Limit {
                    resource: FormulaResource::SupportBytes,
                    observed,
                    limit: limits.max_support_bytes as u128,
                    location,
                },
            ),
            error => crate::formula_binding::failure(error, location),
        })
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
    let external = computation.external_bytes(lease, location)?;
    reserve_scoped(
        values,
        additional,
        lease,
        header_and_other_bytes,
        external as u128,
        GroundingWork::new(limits, counters, location),
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
    let external = computation.external_bytes(lease, location)?;
    reserve_exact_scoped(
        values,
        additional,
        lease,
        header_and_other_bytes,
        external as u128,
        GroundingWork::new(limits, counters, location),
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

/// Reserve against a genuine external owner without requiring support queries.
/// External bytes exclude every lease in the supplied account's workspace.
/// The baseline remains fixed during this synchronous metadata reservation;
/// callers recompute it after canonical owner growth. No new ledger is created.
pub(crate) fn reserve_scoped<T>(
    values: &mut Vec<T>,
    additional: usize,
    lease: &mut StorageLease,
    header_and_other_bytes: usize,
    external_bytes: u128,
    work: GroundingWork<'_>,
) -> Result<(), FormulaFailure> {
    reserve_with_growth(
        values,
        additional,
        lease,
        header_and_other_bytes,
        external_bytes,
        true,
        work,
    )
}

/// Exact-capacity variant of [`reserve_scoped`], sharing its complete engine.
pub(crate) fn reserve_exact_scoped<T>(
    values: &mut Vec<T>,
    additional: usize,
    lease: &mut StorageLease,
    header_and_other_bytes: usize,
    external_bytes: u128,
    work: GroundingWork<'_>,
) -> Result<(), FormulaFailure> {
    reserve_with_growth(
        values,
        additional,
        lease,
        header_and_other_bytes,
        external_bytes,
        false,
        work,
    )
}

fn reserve_with_growth<T>(
    values: &mut Vec<T>,
    additional: usize,
    lease: &mut StorageLease,
    header_and_other_bytes: usize,
    external_bytes: u128,
    geometric: bool,
    work: GroundingWork<'_>,
) -> Result<(), FormulaFailure> {
    let GroundingWork {
        limits,
        counters,
        location,
    } = work;
    let workspace = &counters.accounting.workspace;
    // Authenticate before arithmetic, including an unrepresentable external
    // proposal. The current total must fit before granting a new allowance.
    if !workspace.owns(lease) {
        return Err(crate::formula_binding::assignment(
            AssignmentError::Read(ReadError::ForeignCatalog),
            location,
        ));
    }
    let live = external_bytes
        .checked_add(workspace.bytes() as u128)
        .ok_or_else(|| {
            crate::formula_binding::assignment(AssignmentError::Storage(Error::Overflow), location)
        })?;
    ceiling(
        FormulaResource::SupportBytes,
        live,
        limits.max_support_bytes as u128,
        location,
    )?;
    let external = usize::try_from(external_bytes).expect("admitted usize ceiling");
    let maximum = workspace
        .scope(lease, external)
        .map_err(|error| {
            crate::formula_binding::assignment(AssignmentError::Read(error), location)
        })?
        .allowance(limits, location)?;
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
    let scope = counters
        .accounting
        .workspace
        .scope(lease, external)
        .map_err(|error| {
            crate::formula_binding::assignment(AssignmentError::Read(error), location)
        })?;
    let observed = scope.observed(previous, header_and_other_bytes, limits, counters, location);
    scope.result(result, limits, location)?;
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
    fn scope_rejects_a_foreign_workspace_lease() {
        let workspace = Workspace::default();
        let foreign = Workspace::default().lease();
        assert!(matches!(
            workspace.scope(&foreign, 0),
            Err(ReadError::ForeignCatalog)
        ));
    }

    #[test]
    fn scope_allowance_keeps_other_owners_charged() {
        let workspace = Workspace::default();
        let mut selected = workspace.lease();
        let mut sibling = workspace.lease();
        selected.observe(24, location()).unwrap();
        sibling.observe(40, location()).unwrap();
        let external = 128;
        let limits = FormulaLimits {
            max_support_bytes: external + 40 + 64,
            ..FormulaLimits::default()
        };
        let scope = workspace.scope(&selected, external).unwrap();
        assert_eq!(scope.live_bytes(), 192);
        assert_eq!(scope.allowance(&limits, location()).unwrap(), 64);
    }

    #[test]
    fn replacement_peak_counts_the_previous_buffer() {
        let workspace = Workspace::default();
        let mut lease = workspace.lease();
        let header = 24;
        let old_buffer = 16;
        let replacement = 32;
        let external = 128;
        lease.observe(header + replacement, location()).unwrap();
        let peak = external + header + old_buffer + replacement;
        let limits = FormulaLimits {
            max_support_bytes: peak - 1,
            ..FormulaLimits::default()
        };
        let counters = super::super::Counters::default();
        let scope = workspace.scope(&lease, external).unwrap();
        assert!(matches!(
            scope.observed(header + old_buffer, header, &limits, &counters, location()),
            Err(FormulaFailure::Limit {
                resource: FormulaResource::SupportBytes, observed, limit, ..
            }) if observed == peak as u128 && limit == peak as u128 - 1
        ));
        assert_eq!(
            scope.live_bytes(),
            (external + header + replacement) as u128
        );
    }

    #[test]
    fn refused_component_requirement_includes_external_storage() {
        let workspace = Workspace::default();
        let mut lease = workspace.lease();
        lease.observe(24, location()).unwrap();
        let external = 128;
        let limits = FormulaLimits {
            max_support_bytes: external + 24,
            ..FormulaLimits::default()
        };
        let scope = workspace.scope(&lease, external).unwrap();
        let attempt: Result<(), _> = Err(AssignmentFailure::Assignment(AssignmentError::Storage(
            Error::Storage {
                required: 80,
                limit: 24,
            },
        )));
        assert!(matches!(
            scope.result(attempt, &limits, location()),
            Err(FormulaFailure::Limit {
                resource: FormulaResource::SupportBytes,
                observed: 208,
                limit: 152,
                ..
            })
        ));
        assert_eq!(workspace.bytes(), 24, "a refused proposal owns no capacity");
    }

    #[test]
    fn unrepresentable_peak_is_a_typed_refusal() {
        let workspace = Workspace::default();
        let lease = workspace.lease();
        let scope = workspace.scope(&lease, 1).unwrap();
        assert!(matches!(
            scope.peak(
                u128::MAX,
                &FormulaLimits::default(),
                &super::super::Counters::default(),
                location()
            ),
            Err(FormulaFailure::TermAssignment {
                error: AssignmentError::Storage(Error::Overflow),
                ..
            })
        ));
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

    #[test]
    fn scoped_reservation_charges_external_and_sibling_storage() {
        let owner = zetesis_core::atom_interner::AtomInterner::new();
        let external = owner.storage_bytes();
        let mut counters = super::super::Counters::default();
        let mut lease = counters.lease();
        let mut sibling = counters.lease();
        let header = size_of::<Vec<usize>>();
        lease.observe(header, location()).unwrap();
        sibling.observe(41, location()).unwrap();
        let mut values = Vec::<usize>::new();
        let total = external + (header + 41 + size_of::<usize>()) as u128;
        let limits = FormulaLimits {
            max_support_bytes: usize::try_from(total - 1).unwrap(),
            ..FormulaLimits::default()
        };
        assert!(matches!(
            reserve_scoped(&mut values, 1, &mut lease, header, external,
                GroundingWork::new(&limits, &mut counters, location())),
            Err(FormulaFailure::Limit { resource: FormulaResource::SupportBytes, observed, .. })
                if observed == total
        ));
        assert_eq!(values.capacity(), 0);
        assert_eq!(counters.workspace_bytes(), header + 41);
    }

    #[test]
    fn scoped_reservation_authenticates_before_external_overflow() {
        let mut counters = super::super::Counters::default();
        let mut foreign = Workspace::default().lease();
        let mut values = Vec::<usize>::new();
        assert!(matches!(
            reserve_scoped(
                &mut values,
                1,
                &mut foreign,
                size_of::<Vec<usize>>(),
                u128::MAX,
                GroundingWork::new(&FormulaLimits::default(), &mut counters, location())
            ),
            Err(FormulaFailure::TermAssignment {
                error: AssignmentError::Read(ReadError::ForeignCatalog),
                ..
            })
        ));
        assert_eq!(values.capacity(), 0);
    }

    #[test]
    fn scoped_reservation_keeps_work_cutoffs_retryable() {
        for cutoff in 0..3 {
            let mut counters = super::super::Counters::default();
            let mut lease = counters.lease();
            let mut values = Vec::with_capacity(2);
            values.extend([11_usize, 29]);
            let header = size_of::<Vec<usize>>();
            let previous = header + values.capacity() * size_of::<usize>();
            lease.observe(previous, location()).unwrap();
            let limits = FormulaLimits {
                max_work: cutoff,
                ..FormulaLimits::default()
            };
            assert!(matches!(
                reserve_scoped(
                    &mut values,
                    1,
                    &mut lease,
                    header,
                    0,
                    GroundingWork::new(&limits, &mut counters, location())
                ),
                Err(FormulaFailure::Limit {
                    resource: FormulaResource::Work,
                    ..
                })
            ));
            assert_eq!(values, [11, 29]);
            assert_eq!(lease.bytes(), previous);
            reserve_scoped(
                &mut values,
                1,
                &mut lease,
                header,
                0,
                GroundingWork::new(&FormulaLimits::default(), &mut counters, location()),
            )
            .unwrap();
            assert!(values.capacity() >= 3);
            assert_eq!(
                lease.bytes(),
                header + values.capacity() * size_of::<usize>()
            );
        }
    }
}
