//! Candidate-state copying through the one existing search work account.
//!
//! The old formula knowledge and bound generation retain their ordinary clone
//! policy. Each new arithmetic slot is copied only through its charged owner;
//! a failed destination is unavailable, while the parent stays untouched.

use zetesis_algebra::{Cancellation, Failure, Meter, Quota};

use super::CandidateKnowledge;

impl CandidateKnowledge {
    /// Copy once into recycled storage, invalidating that target on any refusal.
    /// The caller owns the encompassing refund and receipt. Existing formula
    /// cloning retains its allocation behavior; new arithmetic field reads,
    /// tags and initialized payload are admitted before they execute.
    pub(in crate::ferraris) fn copy_reserved<Q: Quota + ?Sized>(
        &self,
        destination: &mut Option<Self>,
        cancellation: &Cancellation,
        work: &mut Meter<'_, Q>,
    ) -> Result<(), Failure<Q::Error>> {
        let result = cancellation
            .poll()
            .map_err(Failure::Stopped)
            .and_then(|()| {
                if let Some(target) = destination.as_mut() {
                    target.clone_entries_from(self);
                } else {
                    // The entries and generation are the pre-existing copy. Only
                    // the two newly initialized absent arithmetic tags are added.
                    work.bounded(2, || {
                        *destination = Some(Self {
                            entries: self.entries.clone(),
                            bound_generation: self.bound_generation,
                            parity: None,
                            cardinality: None,
                        });
                        ((), 2)
                    })?;
                }
                let target = destination.as_mut().expect("candidate copy target exists");
                work.bounded(1, || ((), 1))?;
                if let Some(parity) = &self.parity {
                    parity.copy_reserved(&mut target.parity, cancellation, work)?;
                } else {
                    work.bounded(1, || {
                        target.parity = None;
                        ((), 1)
                    })?;
                }
                work.bounded(1, || ((), 1))?;
                if let Some(cardinality) = &self.cardinality {
                    cardinality.copy_reserved(&mut target.cardinality, cancellation, work)?;
                } else {
                    work.bounded(1, || {
                        target.cardinality = None;
                        ((), 1)
                    })?;
                }
                Ok(())
            });
        if result.is_err() {
            // Attempt cleanup is not another payload operation after refusal.
            *destination = None;
        }
        result
    }
}
