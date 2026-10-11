//! A formula reduct bound to the interpretation that determines its falsum mask.

use zetesis_cpu::{Cancellation, Stop};

use crate::numeric_evaluation::NumericRowScratch;
use crate::oracle::{Work, evaluate, failed_root, identities, reserve};
use crate::{Interpretation, Limits, Statistics, Theory};

/// The Ferraris reduct of an immutable theory in one fixed interpretation.
///
/// This value borrows the exact candidate, whose theory handle fixes the original
/// formula DAG. It owns one Boolean per DAG node: that node's classical truth in
/// the candidate. Testing the reduct masks every candidate-false subformula to
/// falsum, without constructing another DAG or reevaluating the candidate.
///
/// Freezing does not assert that the candidate models the original theory or is
/// an answer set. A satisfaction query does not establish subset minimality.
/// The candidate and mask remain immutable, so independent callers can share the
/// reduct; each query reserves its own temporary evaluation workspace.
///
/// The mask cannot be supplied separately from the checked subject:
/// ```compile_fail
/// use zetesis_ferraris::{FrozenReduct, Interpretation};
/// fn attach(candidate: &Interpretation, truth: Vec<bool>) -> FrozenReduct<'_> {
///     FrozenReduct { candidate, truth }
/// }
/// ```
#[derive(Debug)]
pub struct FrozenReduct<'a> {
    candidate: &'a Interpretation,
    truth: Vec<bool>,
}

impl<'a> FrozenReduct<'a> {
    /// Freeze the candidate's own theory with one topological evaluation.
    ///
    /// For N DAG nodes, this retains N Boolean values. Each Boolean
    /// child occurrence is read; each referenced numeric row is read once.
    /// Repeated positions within that row still count separately. If numeric nodes exist,
    /// temporary dense measures cover R numeric rows with ceil(R/64) validity
    /// words; initialization and each validity-word reset are charged. The
    /// temporary cache is released after freezing; it is not part of the mask. The candidate's packed words
    /// and theory are borrowed, not cloned. No root or proper subset is tested;
    /// `limits.max_subsets` has no effect. Even an empty DAG polls control.
    ///
    /// # Errors
    /// Returns cancellation, deadline, allocation, or work-limit stops. A stopped
    /// freeze returns no reduct. Storage reservation is fallible and precedes
    /// node evaluation, after the initial control poll.
    pub fn new(
        candidate: &'a Interpretation,
        limits: Limits,
        cancellation: &Cancellation,
    ) -> Result<Self, Stop> {
        cancellation.poll()?;
        let mut work = Work {
            limits,
            cancellation,
            statistics: Statistics::default(),
        };
        let mut numeric = NumericRowScratch::default();
        numeric.reserve(candidate.theory())?;
        Self::freeze(candidate, &mut numeric, &mut work)
    }

    /// The exact interpretation determining this reduct, without allocation.
    #[must_use]
    pub const fn candidate(&self) -> &'a Interpretation {
        self.candidate
    }

    /// The immutable original theory, without allocation or evaluation.
    #[must_use]
    pub fn theory(&self) -> &'a Theory {
        self.candidate.theory()
    }

    /// Whether `tested` satisfies every asserted root of this frozen reduct.
    ///
    /// `tested` may be any interpretation of the same admitted theory instance;
    /// it need not be a subset of the candidate. An independently admitted equal
    /// theory has a different identity and is refused before control is polled.
    ///
    /// A query charges every node, Boolean child occurrence, and each
    /// referenced numeric row once, then roots through the first failure. It
    /// also charges fresh dense-measure initialization and packed-validity reset
    /// when numeric nodes exist, as freezing does. Temporary truth and row-cache storage
    /// are released on completion or stop. The
    /// freeze is reused unchanged. This is a fresh per-query budget, excluding
    /// construction and previous queries; `limits.max_subsets` has no effect.
    ///
    /// # Errors
    /// Returns wrong-theory, cancellation, deadline, allocation, or work-limit
    /// stops. A stop is not a logical rejection and leaves this reduct reusable.
    pub fn is_satisfied_by(
        &self,
        tested: &Interpretation,
        limits: Limits,
        cancellation: &Cancellation,
    ) -> Result<bool, Stop> {
        identities(self.theory(), tested)?;
        cancellation.poll()?;
        let mut work = Work {
            limits,
            cancellation,
            statistics: Statistics::default(),
        };
        let mut values = reserve(self.truth.len())?;
        let mut numeric = NumericRowScratch::default();
        numeric.reserve(self.theory())?;
        self.satisfied_by(tested, &mut values, &mut numeric, &mut work)
    }

    // The one-shot wrapper reserves an equally sized tested workspace first,
    // keeping both fallible reservations before candidate evaluation. Sharing
    // Work preserves its combined freeze/query ceiling and stop precedence.
    pub(super) fn freeze(
        candidate: &'a Interpretation,
        numeric: &mut NumericRowScratch,
        work: &mut Work<'_>,
    ) -> Result<Self, Stop> {
        let mut truth = reserve(candidate.theory().nodes().len())?;
        evaluate(
            candidate.theory(),
            candidate,
            None,
            &mut truth,
            numeric,
            work,
        )?;
        Ok(Self { candidate, truth })
    }

    // Callers establish theory identity and reserve N values before this scan.
    // The immutable truth mask and tested workspace refer to the same DAG order.
    pub(super) fn satisfied_by(
        &self,
        tested: &Interpretation,
        values: &mut Vec<bool>,
        numeric: &mut NumericRowScratch,
        work: &mut Work<'_>,
    ) -> Result<bool, Stop> {
        evaluate(
            self.theory(),
            tested,
            Some(&self.truth),
            values,
            numeric,
            work,
        )?;
        Ok(failed_root(self.theory(), values, work)?.is_none())
    }
}
