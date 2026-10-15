//! One bounded original-truth workspace for the model-only batch boundary.

use crate::timing::{self, Phase};
use crate::{Cancellation, Incomplete, SearchPhaseTimings, Statistics};
use zetesis_ferraris::{
    EvaluationError, EvaluationLimits, EvaluationWorkspace, Interpretation, Theory,
};

/// Only capacity survives calls. Evaluation replaces every node's truth and
/// resets numeric validity before the next interpretation is admitted.
#[derive(Debug, Default)]
pub(super) struct Validation {
    workspace: EvaluationWorkspace,
    work: u64,
}

impl Validation {
    pub(super) const fn work(&self) -> u64 {
        self.work
    }

    pub(super) fn retained_bytes(&self) -> u128 {
        self.workspace.retained_bytes()
    }

    /// Scratch is optional: a smaller caller allowance need not retain capacity
    /// from an earlier attempt. Pending interpretations retain their independent
    /// validation; dropping this scratch does not require checking them again.
    pub(super) fn release_excess(&mut self, available: usize) {
        if self.retained_bytes() > available as u128 {
            self.workspace = EvaluationWorkspace::default();
        }
    }

    /// Count each owned proposal once. The independent protocol establishes
    /// original truth here; a joint checker establishes it before commit.
    pub(super) fn admit(
        &mut self,
        theory: &Theory,
        candidate: &Interpretation,
        limits: EvaluationLimits,
        cancellation: &Cancellation,
        statistics: &mut Statistics,
        original: super::OriginalCheck,
    ) -> Result<(), Incomplete> {
        if !theory.same_instance(candidate.theory()) {
            return Err(Incomplete::WrongTheory);
        }
        if original == super::OriginalCheck::Independent {
            self.check(
                theory,
                candidate,
                limits,
                cancellation,
                &mut statistics.phase_timings,
            )?;
        } else {
            cancellation.poll().map_err(Incomplete::from)?;
        }
        crate::search::increment(&mut statistics.candidates)
    }

    pub(super) fn check(
        &mut self,
        theory: &Theory,
        candidate: &Interpretation,
        limits: EvaluationLimits,
        cancellation: &Cancellation,
        timings: &mut Option<SearchPhaseTimings>,
    ) -> Result<(), Incomplete> {
        let started = timing::start(timings.as_ref());
        let result = (|| {
            // Preserve the convenience checker's exact-owner refusal before
            // evaluating the candidate or consuming its work allowance.
            if !theory.same_instance(candidate.theory()) {
                return Err(Incomplete::WrongTheory);
            }
            let attempt = self.workspace.evaluate(candidate, limits, cancellation);
            self.work = self
                .work
                .checked_add(attempt.work)
                .ok_or(Incomplete::CounterOverflow)?;
            let original = attempt.result.map_err(|error| match error {
                EvaluationError::Stopped(stop) => Incomplete::from(stop),
                EvaluationError::Storage { .. } => Incomplete::PendingBytes,
            })?;
            if !original.is_model() {
                return Err(Incomplete::InvalidWitness);
            }
            Ok(())
        })();
        timing::finish(timings, Phase::OriginalValidation, started);
        result
    }
}

#[cfg(test)]
mod tests;
