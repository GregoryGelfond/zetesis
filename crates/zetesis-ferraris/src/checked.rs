//! Owned native decisions bound to the candidate and theory actually checked.

use zetesis_cpu::{Cancellation, Stop};

use crate::{Check, Interpretation, Limits, Statistics, Theory, Verdict};

/// A completed native decision retaining its exact candidate and theory.
/// Constructed only by [`check_interpretation`], including when membership is
/// rejected. This is a runtime record of that operation, not a formal proof
/// object or evidence of enumeration coverage.
///
/// An unrelated native decision cannot be attached to another candidate:
/// ```compile_fail
/// use zetesis_ferraris::{Check, CheckedInterpretation, Interpretation};
/// fn attach(candidate: Interpretation, check: Check) -> CheckedInterpretation {
///     CheckedInterpretation { candidate, check }
/// }
/// ```
#[derive(Clone, Debug)]
pub struct CheckedInterpretation {
    candidate: Interpretation,
    check: Check,
}

impl CheckedInterpretation {
    /// The checked subject, borrowed without allocation or evaluation.
    #[must_use]
    pub const fn candidate(&self) -> &Interpretation {
        &self.candidate
    }

    /// The completed native verdict, borrowed without evaluation.
    #[must_use]
    pub fn verdict(&self) -> &Verdict {
        self.check.verdict()
    }

    /// Whether the native operation accepted this exact candidate.
    #[must_use]
    pub fn accepted(&self) -> bool {
        self.check.accepted()
    }

    /// Logical work used by the completed native check.
    #[must_use]
    pub fn statistics(&self) -> Statistics {
        self.check.statistics()
    }

    /// Retain the accepted candidate as a stable interpretation. No checking,
    /// copying of packed words, or allocation is repeated.
    ///
    /// # Errors
    /// Returns this entire decision when the candidate was rejected.
    pub fn into_stable_interpretation(self) -> Result<StableInterpretation, Self> {
        if self.accepted() {
            Ok(StableInterpretation {
                interpretation: self.candidate,
            })
        } else {
            Err(self)
        }
    }
}

/// An interpretation accepted by this crate's native membership operation.
/// The immutable interpretation retains the exact theory instance. Membership
/// makes no assertion about enumeration coverage or an objective optimum.
/// Cloning copies the packed words and shares the theory handle.
///
/// Public verdict data cannot construct this record:
/// ```compile_fail
/// use zetesis_ferraris::{Interpretation, StableInterpretation};
/// fn forge(interpretation: Interpretation) -> StableInterpretation {
///     StableInterpretation { interpretation }
/// }
/// ```
#[derive(Clone, Debug)]
pub struct StableInterpretation {
    interpretation: Interpretation,
}

impl StableInterpretation {
    /// Exact checked theory, borrowed in constant time.
    #[must_use]
    pub fn theory(&self) -> &Theory {
        self.interpretation.theory()
    }

    /// Accepted subject, borrowed without allocation or evaluation.
    #[must_use]
    pub const fn interpretation(&self) -> &Interpretation {
        &self.interpretation
    }

    /// Transfer the raw interpretation, preserving its theory identity and words.
    #[must_use]
    pub fn into_interpretation(self) -> Interpretation {
        self.interpretation
    }
}

/// Check and retain an owned candidate against its own immutable theory.
/// Uses exactly [`crate::check`]'s exhaustive reduct operation and bounds; the
/// candidate words are moved, not cloned. The search remains exponential in the
/// number of true atoms. No verdict supplied by a caller is treated as evidence.
///
/// # Errors
/// Propagates native cancellation, deadline, allocation, and work/subset stops.
/// An interrupted check produces no completed decision.
pub fn check_interpretation(
    candidate: Interpretation,
    limits: Limits,
    cancellation: &Cancellation,
) -> Result<CheckedInterpretation, Stop> {
    let check = crate::check(candidate.theory(), &candidate, limits, cancellation)?;
    Ok(CheckedInterpretation { candidate, check })
}
