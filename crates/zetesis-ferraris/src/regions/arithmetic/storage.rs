//! Named simultaneous capacities during optional row preparation.
//!
//! There is one allowance, owned by `Budget`. This ledger only tracks which
//! buffers are live. A move reserves both buffers, charges the moved logical
//! fields, and releases the old capacity. Allocator internals are excluded.

use zetesis_algebra::Quota;

use std::mem::size_of;

use zetesis_algebra::Stop;

use super::budget::{Budget, BudgetError};

#[derive(Default)]
pub(super) struct Storage {
    bytes: u128,
}

impl Storage {
    /// Account for caller-owned live data alongside this scope's buffers.
    /// The caller admits this baseline before constructing the ledger; buffers
    /// reserved here are released here, while this borrowed baseline remains.
    pub(super) fn with_live_bytes(bytes: u128) -> Self {
        Self { bytes }
    }

    pub(super) fn bytes(&self) -> u128 {
        self.bytes
    }

    /// Admit named non-buffer metadata or fixed scratch while it is live.
    /// Contained vector headers already tracked by `reserve` are excluded.
    pub(super) fn admit_record<Q: Quota + ?Sized>(
        &mut self,
        bytes: u128,
        budget: &mut Budget<'_, '_, Q>,
    ) -> Result<(), BudgetError<Q::Error>> {
        let next = self
            .bytes
            .checked_add(bytes)
            .ok_or(BudgetError::Stopped(Stop::Allocation))?;
        budget.admit_bytes(next)?;
        self.bytes = next;
        Ok(())
    }

    /// End the lifetime of fixed scratch; it is absent from later peaks.
    pub(super) fn release_record(&mut self, bytes: u128) -> Result<(), Stop> {
        self.bytes = self.bytes.checked_sub(bytes).ok_or(Stop::InvalidProgram)?;
        Ok(())
    }

    /// Borrow fixed workspace for one operation, including its failure prefix.
    pub(super) fn with_record<T, Q: Quota + ?Sized>(
        &mut self,
        bytes: u128,
        budget: &mut Budget<'_, '_, Q>,
        operation: impl FnOnce(&mut Self, &mut Budget<'_, '_, Q>) -> Result<T, BudgetError<Q::Error>>,
    ) -> Result<T, BudgetError<Q::Error>> {
        self.admit_record(bytes, budget)?;
        let result = operation(self, budget);
        let released = self.release_record(bytes);
        result.and_then(|value| released.map(|()| value).map_err(BudgetError::Stopped))
    }

    /// Transfer a vector header into an already-accounted outer allocation.
    /// Its payload stays live; the outer element already includes the header.
    pub(super) fn embed<T>(&mut self) -> Result<(), Stop> {
        self.bytes = self
            .bytes
            .checked_sub(size_of::<Vec<T>>() as u128)
            .ok_or(Stop::InvalidProgram)?;
        Ok(())
    }

    /// Release a payload whose header belongs to its enclosing allocation.
    /// Moving that header through a call is ABI transport, not another owner.
    pub(super) fn release_embedded<T>(&mut self, values: Vec<T>) -> Result<(), Stop> {
        self.bytes = self
            .bytes
            .checked_sub(values.capacity() as u128 * size_of::<T>() as u128)
            .ok_or(Stop::InvalidProgram)?;
        drop(values);
        Ok(())
    }

    pub(super) fn reserve<T, Q: Quota + ?Sized>(
        &mut self,
        count: usize,
        budget: &mut Budget<'_, '_, Q>,
    ) -> Result<Vec<T>, BudgetError<Q::Error>> {
        budget.charge(3)?; // initialize the vector's three-word header
        self.reserve_bytes(count, budget)
    }

    /// Reserve and account for a vector when its producer meters the logical
    /// work itself. Both the request and the actual capacity must fit.
    pub(super) fn reserve_bytes<T, Q: Quota + ?Sized>(
        &mut self,
        count: usize,
        budget: &mut Budget<'_, '_, Q>,
    ) -> Result<Vec<T>, BudgetError<Q::Error>> {
        let mut values = Vec::new();
        let header = size_of::<Vec<T>>() as u128;
        let requested = count as u128 * size_of::<T>() as u128;
        let request = self
            .bytes
            .checked_add(header)
            .and_then(|bytes| bytes.checked_add(requested))
            .ok_or(BudgetError::Stopped(Stop::Allocation))?;
        budget.admit_bytes(request)?;
        values
            .try_reserve_exact(count)
            .map_err(|_| BudgetError::Stopped(Stop::Allocation))?;
        let retained = header + values.capacity() as u128 * size_of::<T>() as u128;
        let next = self
            .bytes
            .checked_add(retained)
            .ok_or(BudgetError::Stopped(Stop::Allocation))?;
        budget.admit_bytes(next)?;
        self.bytes = next;
        Ok(values)
    }

    /// Append an already-metered value, admitting the simultaneous old and
    /// new buffers before a move.
    pub(super) fn push_bytes<T, Q: Quota + ?Sized>(
        &mut self,
        values: &mut Vec<T>,
        value: T,
        budget: &mut Budget<'_, '_, Q>,
    ) -> Result<(), BudgetError<Q::Error>> {
        if values.len() == values.capacity() {
            let capacity = values.capacity().saturating_mul(2).max(1);
            let mut next = self.reserve_bytes(capacity, budget)?;
            next.append(values);
            self.release(std::mem::replace(values, next))?;
        }
        values.push(value);
        Ok(())
    }

    /// Grow by doubling the current capacity, with one cell as the first step.
    /// Actual capacities govern bytes; every moved logical field is charged.
    /// The finite geometric schedule moves fewer than twice the final capacity
    /// cells when the allocator returns the requested exact capacities.
    pub(super) fn push<T, Q: Quota + ?Sized>(
        &mut self,
        values: &mut Vec<T>,
        value: T,
        words: u64,
        budget: &mut Budget<'_, '_, Q>,
    ) -> Result<(), BudgetError<Q::Error>> {
        budget.charge(1)?;
        if values.len() == values.capacity() {
            let capacity = values.capacity().saturating_mul(2).max(1);
            let mut next = self.reserve(capacity, budget)?;
            let transfer = u64::try_from(values.len())
                .ok()
                .and_then(|count| count.checked_mul(words + 1))
                .and_then(|work| work.checked_add(1)); // clear source length
            let admission = transfer
                .ok_or(BudgetError::Stopped(Stop::WorkLimit))
                .and_then(|work| budget.charge(work));
            if let Err(error) = admission {
                self.release(next)?;
                return Err(error);
            }
            // Capacity is already available and the whole bounded transfer was
            // admitted before append changes the source's header or moves a cell.
            next.append(values);
            if let Err(error) = budget.charge(6) {
                self.release(next)?;
                return Err(error);
            }
            std::mem::swap(values, &mut next);
            self.release(next)?;
        }
        budget.charge(words + 1)?;
        values.push(value);
        Ok(())
    }

    pub(super) fn filled<T: Copy, Q: Quota + ?Sized>(
        &mut self,
        count: usize,
        value: T,
        words: u64,
        budget: &mut Budget<'_, '_, Q>,
    ) -> Result<Vec<T>, BudgetError<Q::Error>> {
        let mut values = self.reserve(count, budget)?;
        for _ in 0..count {
            budget.charge(words + 1)?;
            values.push(value);
        }
        Ok(values)
    }

    pub(super) fn release<T>(&mut self, values: Vec<T>) -> Result<(), Stop> {
        let capacity =
            size_of::<Vec<T>>() as u128 + values.capacity() as u128 * size_of::<T>() as u128;
        self.bytes = self
            .bytes
            .checked_sub(capacity)
            .ok_or(Stop::InvalidProgram)?;
        drop(values);
        Ok(())
    }
}
