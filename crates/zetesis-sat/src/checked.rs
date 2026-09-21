//! Subject-bound native membership records, separate from public verdict data.

use zetesis_ferraris::{Interpretation, Theory};

use crate::{Cancellation, Check, Incomplete, Limits, StableModels};

/// A native membership attempt retaining its exact candidate and theory.
/// Constructed only by [`check_interpretation`]; an inconclusive attempt remains
/// distinguishable through [`Self::verdict`]. This runtime record is not a formal
/// proof object and makes no assertion about enumeration coverage.
///
/// A caller-supplied verdict cannot be attached to a candidate:
/// ```compile_fail
/// use zetesis_ferraris::Interpretation;
/// use zetesis_sat::{Check, CheckedInterpretation};
/// fn attach(candidate: Interpretation) -> CheckedInterpretation {
///     CheckedInterpretation { candidate, check: Check::Stable }
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

    /// The native result, including the reason for any inconclusive attempt.
    #[must_use]
    pub const fn verdict(&self) -> &Check {
        &self.check
    }

    /// Whether the native operation accepted this exact candidate.
    #[must_use]
    pub fn accepted(&self) -> bool {
        self.check.accepted()
    }

    /// Retain the accepted candidate as a stable interpretation. No checking,
    /// copying of packed words, or allocation is repeated.
    ///
    /// # Errors
    /// Returns this entire decision for a rejected or inconclusive candidate.
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

/// An interpretation accepted by native membership checking or scalar search.
/// The immutable interpretation retains the exact theory instance. Membership
/// makes no assertion about enumeration coverage or an objective optimum.
/// Cloning copies the packed words and shares the theory handle.
///
/// Public verdict data cannot construct this record:
/// ```compile_fail
/// use zetesis_ferraris::Interpretation;
/// use zetesis_sat::StableInterpretation;
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
/// Uses exactly [`crate::check`]'s original satisfaction and proper-subset
/// search. Search can be exponential; all native bounds and stops apply. The
/// candidate words are moved, not cloned. Caller-supplied verdicts do not enter
/// this operation. An inconclusive result retains the candidate and stop reason.
#[must_use]
pub fn check_interpretation(
    candidate: Interpretation,
    limits: Limits,
    cancellation: &Cancellation,
) -> CheckedInterpretation {
    let check = crate::check(candidate.theory(), &candidate, limits, cancellation);
    CheckedInterpretation { candidate, check }
}

impl StableModels {
    /// Advance scalar enumeration and retain each native accepted interpretation
    /// with its theory identity. This runs exactly one ordinary iterator step,
    /// including its native checking, work, and error behavior; no second check or
    /// word copy is performed. It never converts a supplied batch verdict into
    /// an accepted record. Pending batch proposals cause the existing scalar
    /// [`Incomplete::PendingBatch`] refusal.
    ///
    /// A returned interpretation establishes membership only. Consult
    /// [`Self::exhausted`] for completion, whose scope includes any accumulated
    /// candidate restrictions and relies on the contract of any earlier trusted
    /// batch calls. An error is emitted once and subsequent calls return `None`.
    pub fn next_verified(&mut self) -> Option<Result<StableInterpretation, Incomplete>> {
        self.next()
            .map(|result| result.map(|interpretation| StableInterpretation { interpretation }))
    }
}
