//! Ordered results bound to a slate's identity and to a submission's place in it.

use std::time::Duration;

use zetesis_ferraris::{SlateId, Theory};

use crate::{GpuError, GpuErrorKind};

/// The decoded results of one submission of a slate: the slate's theory
/// instance and identity, the submission's offset in the slate and its count.
/// Only a successful check of a prepared submission creates this receipt.
///
/// It binds results as [`crate::CheckedBatch`] does, with the slate's identity
/// and the submission's offset in place of a borrowed candidate slice. So it
/// can outlive the borrow of the slate's candidates, which the next slate's
/// production needs, and still names the place its results belong to: two
/// submissions of one slate with equal counts are not interchangeable.
///
/// Each non-`NotModel` formula or tight result establishes original
/// satisfaction, as in [`crate::CheckedBatch`]. This receipt certifies the
/// operation and its subject, not mechanical verification of shaders.
#[derive(Debug)]
pub struct CheckedSlate<T> {
    theory: Theory,
    slate: SlateId,
    offset: usize,
    count: usize,
    checks: Vec<T>,
    finish_wait: Option<Duration>,
}

impl<T> CheckedSlate<T> {
    pub(crate) fn new(
        theory: Theory,
        slate: SlateId,
        offset: usize,
        count: usize,
        checks: Vec<T>,
    ) -> Self {
        Self {
            theory,
            slate,
            offset,
            count,
            checks,
            finish_wait: None,
        }
    }

    /// The receipt of an overlapping check, with its finish's wait.
    pub(crate) const fn with_finish_wait(mut self, wait: Option<Duration>) -> Self {
        self.finish_wait = wait;
        self
    }

    /// The finish's wait, for a submission checked while other work ran beside
    /// it: from the end of that work to the observed completion of the
    /// submission, before its readback was mapped and decoded. It is the
    /// device's remaining exposure once the work beside it is done. Absent for
    /// a check with nothing run beside it.
    #[must_use]
    pub const fn finish_wait(&self) -> Option<Duration> {
        self.finish_wait
    }

    /// The theory instance the submission checked.
    #[must_use]
    pub const fn theory(&self) -> &Theory {
        &self.theory
    }

    /// The identity of the slate the submission belongs to.
    #[must_use]
    pub const fn slate(&self) -> SlateId {
        self.slate
    }

    /// The position of the submission's first candidate in its slate.
    #[must_use]
    pub const fn offset(&self) -> usize {
        self.offset
    }

    /// The candidates the submission checked.
    #[must_use]
    pub const fn count(&self) -> usize {
        self.count
    }

    /// Ordered results, without detaching them from this receipt.
    #[must_use]
    pub fn checks(&self) -> &[T] {
        &self.checks
    }

    /// Consume the results only for their theory instance, slate and place.
    /// `offset` is where the caller expects this submission's results to
    /// start: the count of the slate's results it has already accepted.
    ///
    /// # Errors
    /// Refuses another theory instance, another slate's identity, another
    /// offset, or results whose count differs from the submission's.
    pub fn into_checks(
        self,
        theory: &Theory,
        slate: SlateId,
        offset: usize,
    ) -> Result<Vec<T>, GpuError> {
        if !self.theory.same_instance(theory)
            || self.slate != slate
            || self.offset != offset
            || self.checks.len() != self.count
        {
            return Err(GpuError::new(
                GpuErrorKind::Readback,
                "device receipt does not belong to this place in this candidate slate",
            ));
        }
        Ok(self.checks)
    }
}

#[cfg(test)]
mod tests {
    use super::CheckedSlate;
    use zetesis_ferraris::{AdmissionLimits, FormulaParts, SlateId, Theory};

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
    fn a_receipt_yields_its_results_at_its_own_place() {
        let owner = theory();
        let slate = SlateId::first(&owner).unwrap();
        let receipt = CheckedSlate::new(owner.clone(), slate, 3, 2, vec![1, 2]);
        assert_eq!(receipt.into_checks(&owner, slate, 3).unwrap(), [1, 2]);
    }

    #[test]
    fn a_receipt_refuses_another_slates_identity() {
        let owner = theory();
        let slate = SlateId::first(&owner).unwrap();
        let next = slate.successor().unwrap();
        let receipt = CheckedSlate::new(owner.clone(), slate, 0, 1, vec![1]);
        assert!(receipt.into_checks(&owner, next, 0).is_err());
    }

    #[test]
    fn a_receipt_refuses_another_offset() {
        let owner = theory();
        let slate = SlateId::first(&owner).unwrap();
        let receipt = CheckedSlate::new(owner.clone(), slate, 2, 1, vec![1]);
        assert!(receipt.into_checks(&owner, slate, 0).is_err());
    }

    #[test]
    fn a_receipt_refuses_an_equal_but_foreign_theory() {
        let (owner, foreign) = (theory(), theory());
        let slate = SlateId::first(&owner).unwrap();
        let receipt = CheckedSlate::new(owner, slate, 0, 1, vec![1]);
        assert!(receipt.into_checks(&foreign, slate, 0).is_err());
    }

    #[test]
    fn a_receipt_refuses_results_short_of_its_count() {
        let owner = theory();
        let slate = SlateId::first(&owner).unwrap();
        let receipt = CheckedSlate::new(owner.clone(), slate, 0, 2, vec![1]);
        assert!(receipt.into_checks(&owner, slate, 0).is_err());
    }

    #[test]
    fn equal_count_receipts_of_one_slate_are_refused_out_of_order() {
        let owner = theory();
        let slate = SlateId::first(&owner).unwrap();
        let first = || CheckedSlate::new(owner.clone(), slate, 0, 2, vec![1, 2]);
        let second = || CheckedSlate::new(owner.clone(), slate, 2, 2, vec![3, 4]);
        // Accepting the second submission's results where the first's belong
        // is refused, and so is the first's where the second's belong.
        assert!(second().into_checks(&owner, slate, 0).is_err());
        assert!(first().into_checks(&owner, slate, 2).is_err());
        assert_eq!(first().into_checks(&owner, slate, 0).unwrap(), [1, 2]);
        assert_eq!(second().into_checks(&owner, slate, 2).unwrap(), [3, 4]);
    }
}
