//! Ordered results bound to the exact subject of a completed device operation.

use zetesis_ferraris::{Interpretation, Theory};

use crate::{GpuError, GpuErrorKind};

/// A completely decoded device batch, retaining its original theory and the
/// exact borrowed candidate slice. Only a successful oracle operation creates
/// this receipt; callers cannot reconstruct it from detached verdict values.
///
/// Each non-`NotModel` formula or tight result establishes original satisfaction,
/// including a residual that still needs exact reduct completion. This receipt
/// certifies the operation and subject, not mechanical verification of shaders.
#[derive(Debug)]
pub struct CheckedBatch<'a, T> {
    theory: &'a Theory,
    candidates: &'a [Interpretation],
    checks: Vec<T>,
}

impl<'a, T> CheckedBatch<'a, T> {
    pub(crate) fn new(
        theory: &'a Theory,
        candidates: &'a [Interpretation],
        checks: Vec<T>,
    ) -> Self {
        Self {
            theory,
            candidates,
            checks,
        }
    }

    /// Original theory checked by the operation.
    #[must_use]
    pub const fn theory(&self) -> &'a Theory {
        self.theory
    }

    /// Exact submitted candidates, in result order.
    #[must_use]
    pub const fn candidates(&self) -> &'a [Interpretation] {
        self.candidates
    }

    /// Ordered results, without detaching them from this receipt.
    #[must_use]
    pub fn checks(&self) -> &[T] {
        &self.checks
    }

    /// Consume the results only for their exact original subject and slice.
    ///
    /// # Errors
    /// Refuses a foreign theory, copied/reordered slice, or inconsistent count.
    pub fn into_checks(
        self,
        theory: &Theory,
        candidates: &[Interpretation],
    ) -> Result<Vec<T>, GpuError> {
        if !self.theory.same_instance(theory)
            || !std::ptr::eq(self.candidates, candidates)
            || self.checks.len() != candidates.len()
        {
            return Err(GpuError::new(
                GpuErrorKind::Readback,
                "device receipt does not belong to this exact candidate batch",
            ));
        }
        Ok(self.checks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zetesis_ferraris::{AdmissionLimits, FormulaParts};

    fn theory() -> Theory {
        Theory::new(
            1,
            FormulaParts::default(),
            vec![],
            AdmissionLimits::default(),
        )
        .unwrap()
    }

    #[test]
    fn receipt_refuses_equal_but_foreign_subjects() {
        let owner = theory();
        let foreign = theory();
        let candidates = [Interpretation::new(&owner, []).unwrap()];
        let copies = candidates.clone();
        for (subject, slice) in [(&foreign, &candidates[..]), (&owner, &copies[..])] {
            let receipt = CheckedBatch::new(&owner, &candidates, vec![true]);
            assert!(receipt.into_checks(subject, slice).is_err());
        }
    }

    #[test]
    fn receipt_preserves_exact_order_and_count() {
        let owner = theory();
        let candidates = [
            Interpretation::new(&owner, []).unwrap(),
            Interpretation::new(&owner, [0]).unwrap(),
        ];
        let receipt = CheckedBatch::new(&owner, &candidates, vec![1, 2]);
        assert_eq!(
            receipt.into_checks(&owner, &candidates).unwrap(),
            vec![1, 2]
        );
        let short = CheckedBatch::new(&owner, &candidates, vec![1]);
        assert!(short.into_checks(&owner, &candidates).is_err());
    }
}
