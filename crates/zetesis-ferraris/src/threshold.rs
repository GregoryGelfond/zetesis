//! Numeric threshold reduction over already-computed predecessor truth.

use crate::Threshold;
use zetesis_cpu::Stop;

#[cfg(test)]
mod tests;

impl Threshold<'_> {
    /// Test this nonnegative weighted threshold from predecessor truth.
    ///
    /// The callback reads each condition occurrence exactly once, in row order,
    /// including repeated conditions and zero contributions. It owns truth
    /// acquisition and its work/cancellation charges; this operation performs
    /// no formula traversal or allocation and retains only a checked wide sum.
    /// The caller applies the original-node truth gate for a frozen reduct.
    ///
    /// # Errors
    /// Propagates callback errors. A mismatched row or an unrepresentable sum
    /// returns [`Stop::InvalidProgram`]; admitted formula rows exclude both.
    // Expose the callback to callers' hot loops so work and cancellation state
    // need not spill across a separate reduction call.
    #[inline]
    pub fn evaluate<E: From<Stop>>(
        self,
        read: impl FnMut(usize) -> Result<bool, E>,
    ) -> Result<bool, E> {
        Ok(self.selected_sum(read)? >= self.bound)
    }

    /// Reduce every occurrence to a checked selected weight, independent of a
    /// guard bound. The public threshold operation retains the same callback
    /// order and error behavior; a pass-local row owner may reuse this result.
    #[inline]
    pub(crate) fn selected_sum<E: From<Stop>>(
        self,
        mut read: impl FnMut(usize) -> Result<bool, E>,
    ) -> Result<i128, E> {
        if self.conditions.len() != self.weights.len() {
            return Err(Stop::InvalidProgram.into());
        }
        let mut total = 0_i128;
        for (&condition, &weight) in self.conditions.iter().zip(self.weights) {
            if read(condition)? {
                total = total
                    .checked_add(i128::from(weight))
                    .ok_or(Stop::InvalidProgram)?;
            }
        }
        Ok(total)
    }
}
