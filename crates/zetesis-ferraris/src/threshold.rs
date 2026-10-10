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
    pub fn evaluate<E: From<Stop>>(
        self,
        mut read: impl FnMut(usize) -> Result<bool, E>,
    ) -> Result<bool, E> {
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
        Ok(total >= self.bound)
    }
}
