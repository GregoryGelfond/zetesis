//! The byte dimension of the shared meter, independent of work charges.

use std::mem::size_of;

/// Named simultaneous capacity, with its peak published into the caller's
/// existing receipt. This dimension owns only the current byte count; it
/// neither duplicates the peak nor owns the caller's configured ceiling.
///
/// Control observations and work charges remain at the caller's established
/// boundaries. Reservations include actual vector capacity, not allocator
/// metadata or RSS. Headers and overlapping owners must be observed explicitly.
#[derive(Debug)]
#[repr(transparent)]
pub struct MeterBytes {
    current: u128,
}

/// A byte observation or reservation could not complete under its ceiling.
/// Callers retain their named resource and typed failure when translating it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeterBytesError {
    /// The complete requested or observed capacity exceeds its inclusive limit.
    Limit {
        /// Complete capacity, rather than only the latest increment.
        observed: u128,
        /// The caller's unchanged configured ceiling.
        limit: u128,
    },
    /// Capacity addition or release subtraction cannot be represented.
    Overflow,
    /// Fallible vector reservation failed.
    Allocation,
}

impl MeterBytes {
    /// Start at zero capacity without polling, charging or allocating.
    #[must_use]
    pub const fn new() -> Self {
        Self { current: 0 }
    }

    /// Check a caller-supplied byte requirement against its inclusive ceiling.
    ///
    /// This pure check records no current or peak capacity. The caller retains
    /// the distinction between a requested envelope and an actual observation,
    /// along with its allocation, publication and control boundaries. Zero fits
    /// every ceiling. No addition, conversion, polling or allocation occurs.
    ///
    /// # Errors
    /// Returns only [`MeterBytesError::Limit`], containing the exact requirement
    /// and ceiling, when the requirement exceeds that ceiling.
    pub fn check(total: u128, limit: u128) -> Result<(), MeterBytesError> {
        if total > limit {
            Err(MeterBytesError::Limit {
                observed: total,
                limit,
            })
        } else {
            Ok(())
        }
    }

    /// Record a caller-supplied byte requirement in its existing peak receipt,
    /// then check that requirement against the inclusive ceiling.
    ///
    /// The caller determines whether `total` describes a requested envelope or
    /// actual capacity. A refused observation remains in the peak. An earlier
    /// peak above a newly lowered ceiling does not itself refuse a smaller
    /// current requirement. This operation owns no current counter or other
    /// state, and performs no allocation, arithmetic addition or control poll.
    ///
    /// # Errors
    /// Returns only [`MeterBytesError::Limit`], containing the exact current
    /// requirement and ceiling. The peak is updated before returning it.
    pub fn observe_peak(total: u128, limit: u128, peak: &mut u128) -> Result<(), MeterBytesError> {
        observe_peak_as(total, limit, peak, std::convert::identity)
    }

    /// Observe a machine-sized capacity in its existing machine-sized peak.
    ///
    /// This has the same publication-before-refusal contract as
    /// [`Self::observe_peak`], without widening or copying the caller's receipt.
    /// Current capacity, allocation and control observations remain caller-owned.
    ///
    /// # Errors
    /// Returns only [`MeterBytesError::Limit`], with the exact current requirement
    /// and ceiling widened to `u128`. The peak is updated before returning it.
    pub fn observe_peak_usize(
        total: usize,
        limit: usize,
        peak: &mut usize,
    ) -> Result<(), MeterBytesError> {
        observe_peak_as(total, limit, peak, |value| value as u128)
    }

    /// Observe a u64 byte requirement in the caller's existing u64 peak.
    ///
    /// Like [`Self::observe_peak`], this publishes the maximum before checking
    /// the current requirement. It neither widens nor copies the stored peak;
    /// only the error's exact diagnostic values are widened to u128.
    ///
    /// # Errors
    /// Returns only [`MeterBytesError::Limit`]. A refused observation remains
    /// recorded; an older peak above the ceiling does not refuse a fitting total.
    pub fn observe_peak_u64(total: u64, limit: u64, peak: &mut u64) -> Result<(), MeterBytesError> {
        observe_peak_as(total, limit, peak, u128::from)
    }

    /// Record an actual total and its peak before checking the ceiling.
    /// A refused observation is retained. A lower total can represent storage
    /// already dropped; it never erases an earlier peak.
    ///
    /// # Errors
    /// Returns the complete observed capacity when it exceeds `limit`.
    pub fn observe(
        &mut self,
        total: u128,
        limit: u128,
        peak: &mut u128,
    ) -> Result<(), MeterBytesError> {
        self.current = total;
        Self::observe_peak(total, limit, peak)
    }

    /// Reserve one empty vector after admitting its requested payload.
    /// Requested capacity that is refused before allocation changes neither
    /// current bytes nor peak. Actual allocator capacity is observed before
    /// its ceiling check; on refusal the vector drops but that attempted
    /// observation remains recorded. No control poll or work charge is added.
    ///
    /// # Errors
    /// Reports unrepresentable addition, requested or observed excess, or a
    /// failed allocation. No vector is returned on any refusal.
    pub fn reserve<T>(
        &mut self,
        count: usize,
        limit: u128,
        peak: &mut u128,
    ) -> Result<Vec<T>, MeterBytesError> {
        let requested = self
            .current
            .checked_add(count as u128 * size_of::<T>() as u128)
            .ok_or(MeterBytesError::Overflow)?;
        Self::check(requested, limit)?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(count)
            .map_err(|_| MeterBytesError::Allocation)?;
        let actual = self
            .current
            .checked_add(values.capacity() as u128 * size_of::<T>() as u128)
            .ok_or(MeterBytesError::Overflow)?;
        self.observe(actual, limit, peak)?;
        Ok(values)
    }

    /// Drop a vector, then remove its actual payload from current capacity.
    /// The vector must belong to this accounted total. No peak, work or control
    /// observation is changed, and there is no automatic accounting on drop.
    ///
    /// # Errors
    /// Returns an arithmetic refusal after dropping the vector if its payload
    /// exceeds the currently accounted capacity; current bytes stay unchanged.
    pub fn release<T>(&mut self, value: Vec<T>) -> Result<(), MeterBytesError> {
        let bytes = value.capacity() as u128 * size_of::<T>() as u128;
        drop(value);
        self.current = self
            .current
            .checked_sub(bytes)
            .ok_or(MeterBytesError::Overflow)?;
        Ok(())
    }
}

impl Default for MeterBytes {
    /// The same zero-capacity, effect-free construction as [`Self::new`].
    fn default() -> Self {
        Self::new()
    }
}

// The adapters supply only lossless diagnostic widening. Publication mutates
// the actual receipt at its own width; no converted account is kept or copied.
fn observe_peak_as<T: Copy + Ord>(
    total: T,
    limit: T,
    peak: &mut T,
    widen: impl Fn(T) -> u128,
) -> Result<(), MeterBytesError> {
    *peak = (*peak).max(total);
    MeterBytes::check(widen(total), widen(limit))
}

#[cfg(test)]
mod tests;
