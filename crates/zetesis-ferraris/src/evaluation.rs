//! Subject-bound original truth with reusable, bounded workspace.

use std::{fmt, mem::size_of_val};

use crate::numeric_evaluation::NumericRowScratch;
use crate::oracle::{Work, evaluate, failed_root};
use crate::{Interpretation, Limits, Theory};
use zetesis_cpu::{Cancellation, Meter, Refusal, Stop};

/// Per-evaluation bounds, including storage retained from earlier evaluations.
#[derive(Clone, Copy, Debug)]
pub struct EvaluationLimits {
    /// One unit per formula node, read operand and tested asserted root, plus
    /// numeric-cache initialization and validity-word resets. Each authoritative
    /// numeric row is read once per pass, even when several cuts share it.
    pub max_work: u64,
    /// Workspace header plus actual truth and numeric-cache capacities, in bytes.
    /// The borrowed interpretation and theory are excluded.
    pub max_bytes: usize,
}

impl Default for EvaluationLimits {
    fn default() -> Self {
        Self {
            max_work: Limits::default().max_work,
            max_bytes: 64 * 1024 * 1024,
        }
    }
}

/// Reusable storage for evaluating classical formula satisfaction.
///
/// Each evaluation visits every node in topological order, then asserted roots
/// through the first false root. Work is linear in those visits; retained space
/// is linear in the largest admitted node and numeric-row counts. Reuse preserves
/// capacity, not truth: every node is recomputed for the supplied interpretation. Numeric
/// measures are reused only within that pass, indexed by authoritative row IDs.
/// Scratch includes dense measures and packed validity for all numeric rows
/// when live numeric nodes exist, including unused rows of either domain. A returned
/// [`FormulaEvaluation`] borrows both inputs and prevents workspace mutation
/// while its truth values remain observable.
#[derive(Debug, Default)]
pub struct EvaluationWorkspace {
    values: Vec<bool>,
    numeric: NumericRowScratch,
}

impl EvaluationWorkspace {
    /// Conservative work allowance for one complete original evaluation.
    ///
    /// This allocation-free, constant-time bound covers every node, every logical
    /// operand occurrence and every asserted root, plus fresh numeric-cache cell
    /// initialization and validity-word resets. Shared numeric rows can consume
    /// less than their repeated logical occurrences, and reused cells require no
    /// initialization. The bound therefore covers either a fresh or reused
    /// workspace, independently of the interpretation and earlier subjects.
    ///
    /// Numeric rows contribute cache work only when the owner has live numeric
    /// nodes; then all rows count, including unused rows of either domain.
    /// A theory without numeric nodes retains the node/occurrence/root bound.
    /// This is a work allowance, not a storage, cancellation or membership guarantee.
    #[must_use]
    pub fn work_bound(theory: &Theory) -> u128 {
        theory.nodes().len() as u128
            + theory.parts().occurrences() as u128
            + theory.roots().len() as u128
            + NumericRowScratch::reset_work_bound(theory)
    }

    /// Requested vector payload for a fresh evaluation workspace.
    ///
    /// This allocation-free, constant-time count includes one truth cell per node
    /// and the dense shared measure and validity columns whenever numeric nodes
    /// exist. Then all authoritative numeric rows count, including unused rows of
    /// either domain. With no numeric nodes this is the truth column alone.
    ///
    /// Workspace headers, retained capacity from earlier subjects and allocator
    /// excess are excluded. Reservations may return larger capacities; consumers
    /// must observe [`Self::retained_bytes`] before admitting actual workspace use.
    #[must_use]
    pub fn requested_payload_bytes(theory: &Theory) -> u128 {
        theory.nodes().len() as u128 * size_of::<bool>() as u128
            + NumericRowScratch::requested_bytes(theory)
    }

    /// Header and actual truth/cache capacities, excluding allocator bookkeeping.
    #[must_use]
    pub fn retained_bytes(&self) -> u128 {
        self.bytes(self.values.capacity(), self.numeric.retained_bytes())
    }

    /// Reserve workspace for a theory without evaluating an interpretation.
    /// Clears previous truth values, reserves numeric columns without initializing
    /// them, keeps existing capacity and establishes no subject or truth capability. A batch coordinator can use this to
    /// inspect actual worker capacity before admitting candidate work.
    ///
    /// # Errors
    /// Cancellation and deadlines precede storage checks. Byte ceilings include
    /// existing capacity; allocation failure or excess actual capacity is typed.
    /// A failure may retain increased capacity, reported by [`Self::retained_bytes`].
    pub fn reserve(
        &mut self,
        theory: &Theory,
        max_bytes: usize,
        cancellation: &Cancellation,
    ) -> Result<(), EvaluationError> {
        cancellation.poll()?;
        self.check_storage(
            theory.nodes().len().max(self.values.capacity()),
            self.numeric.required_bytes(theory),
            max_bytes,
        )?;
        self.values.clear();
        let reservation = self
            .values
            .try_reserve_exact(theory.nodes().len())
            .map_err(|_| EvaluationError::Stopped(Stop::Allocation));
        let observed = self.check_storage(
            self.values.capacity(),
            self.numeric.retained_bytes(),
            max_bytes,
        );
        reservation?;
        observed?;
        let base = self.bytes(self.values.capacity(), 0);
        self.numeric
            .reserve_observed(theory, |actual| Self::check_bytes(base + actual, max_bytes))
    }

    /// Evaluate original satisfaction and retain the truth of every node.
    ///
    /// A successful result authenticates node truth for this exact candidate;
    /// it does not establish answer-set membership. A stopped node or root scan
    /// exposes no truth capability. Cancellation is checked before storage
    /// admission, including for an empty theory. Storage refuses a lowered
    /// ceiling below existing capacity; this operation does not shrink it.
    /// Work and retained bytes are reported on every outcome. A failed attempt
    /// may have grown capacity or evaluated a prefix, which the next call resets.
    pub fn evaluate<'a>(
        &'a mut self,
        interpretation: &'a Interpretation,
        limits: EvaluationLimits,
        cancellation: &Cancellation,
    ) -> EvaluationAttempt<'a> {
        let metered = cancellation.metered(limits.max_work, |meter| {
            self.evaluate_into(
                interpretation,
                limits.max_bytes,
                cancellation,
                &mut Work::new(0, meter),
            )
        });
        self.attempt(interpretation, metered.result, metered.charged)
    }

    /// Evaluate as [`Self::evaluate`] does, as part of an enclosing metered
    /// operation: the evaluation charges that operation's meter, so its work
    /// counts toward that operation's limit, the meter's cadence continues
    /// through it, and the enclosing operation's exit settles its outcome.
    /// The attempt reports the work charged here.
    pub(crate) fn evaluate_within<'a>(
        &'a mut self,
        interpretation: &'a Interpretation,
        max_bytes: usize,
        cancellation: &Cancellation,
        meter: &mut Meter<'_>,
    ) -> EvaluationAttempt<'a> {
        let before = meter.charged();
        let result = self.evaluate_into(
            interpretation,
            max_bytes,
            cancellation,
            &mut Work::new(0, meter),
        );
        let work = meter.charged() - before;
        self.attempt(interpretation, result, work)
    }

    fn attempt<'a>(
        &'a self,
        interpretation: &'a Interpretation,
        result: Result<Option<usize>, EvaluationError>,
        work: u64,
    ) -> EvaluationAttempt<'a> {
        EvaluationAttempt {
            work,
            retained_bytes: self.retained_bytes(),
            result: result.map(|failed_root| FormulaEvaluation {
                interpretation,
                values: &self.values,
                failed_root,
            }),
        }
    }

    fn evaluate_into(
        &mut self,
        interpretation: &Interpretation,
        max_bytes: usize,
        cancellation: &Cancellation,
        work: &mut Work<'_, '_>,
    ) -> Result<Option<usize>, EvaluationError> {
        let theory = interpretation.theory();
        self.reserve(theory, max_bytes, cancellation)?;
        evaluate(
            theory,
            interpretation,
            None,
            &mut self.values,
            &mut self.numeric,
            work,
        )?;
        Ok(failed_root(theory, &self.values, work)?)
    }

    fn check_storage(
        &self,
        capacity: usize,
        numeric_bytes: u128,
        limit: usize,
    ) -> Result<(), EvaluationError> {
        Self::check_bytes(self.bytes(capacity, numeric_bytes), limit)
    }

    fn check_bytes(required: u128, limit: usize) -> Result<(), EvaluationError> {
        if required > limit as u128 {
            Err(EvaluationError::Storage { required, limit })
        } else {
            Ok(())
        }
    }

    fn bytes(&self, capacity: usize, numeric_bytes: u128) -> u128 {
        size_of_val(self) as u128 + capacity as u128 * size_of::<bool>() as u128 + numeric_bytes
    }
}

/// Truth of all nodes and the completed classical satisfaction decision.
///
/// Only [`EvaluationWorkspace::evaluate`] constructs this view. Its candidate
/// carries the exact theory owner; equal-looking foreign theories cannot be
/// substituted. Consumers using it with another prepared object must check that
/// object's theory identity. No public constructor accepts caller-supplied bits.
///
/// The borrow prevents stale truth from surviving workspace reuse:
///
/// ```compile_fail
/// # use zetesis_ferraris::{EvaluationLimits, EvaluationWorkspace, Interpretation};
/// # use zetesis_cpu::Cancellation;
/// # fn stale(candidate: &Interpretation) {
/// let mut workspace = EvaluationWorkspace::default();
/// let control = Cancellation::default();
/// let truth = workspace.evaluate(candidate, EvaluationLimits::default(), &control)
///     .result.unwrap();
/// let next = workspace.evaluate(candidate, EvaluationLimits::default(), &control);
/// assert!(truth.is_model());
/// # }
/// ```
#[derive(Debug)]
pub struct FormulaEvaluation<'a> {
    interpretation: &'a Interpretation,
    values: &'a [bool],
    failed_root: Option<usize>,
}

impl FormulaEvaluation<'_> {
    /// Exact interpretation whose original truth was evaluated.
    #[must_use]
    pub fn interpretation(&self) -> &Interpretation {
        self.interpretation
    }

    /// Exact admitted theory of the evaluated interpretation.
    #[must_use]
    pub fn theory(&self) -> &Theory {
        self.interpretation.theory()
    }

    /// Whether every asserted root is classically satisfied.
    #[must_use]
    pub fn is_model(&self) -> bool {
        self.failed_root.is_none()
    }

    /// First false asserted root, or absence after a complete successful scan.
    #[must_use]
    pub fn failed_root(&self) -> Option<usize> {
        self.failed_root
    }

    /// Original truth at a valid node index; absence denotes an invalid index.
    #[must_use]
    pub fn node_truth(&self, node: usize) -> Option<bool> {
        self.values.get(node).copied()
    }

    /// The truth of every node in index order: the frozen mask of the
    /// interpretation's reduct, a node false here being falsum there.
    #[must_use]
    pub fn truth(&self) -> &[bool] {
        self.values
    }
}

/// Completed truth or a typed failure, with its exact consumed work prefix.
#[derive(Debug)]
#[must_use]
pub struct EvaluationAttempt<'a> {
    /// A truth view exists only after node evaluation and satisfaction complete.
    pub result: Result<FormulaEvaluation<'a>, EvaluationError>,
    /// Charged node, operand and root visits, including on interruption.
    pub work: u64,
    /// Workspace header plus actual capacity after this attempt.
    pub retained_bytes: u128,
}

/// Failure to complete evaluation; never evidence of formula falsity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EvaluationError {
    /// Cancellation, deadline, allocation or charged-work exhaustion.
    Stopped(Stop),
    /// Existing or requested workspace capacity exceeds its ceiling.
    Storage {
        /// Complete workspace charge, including its header.
        required: u128,
        /// Requested ceiling; zero is a real limit.
        limit: usize,
    },
}

impl From<Stop> for EvaluationError {
    fn from(stop: Stop) -> Self {
        Self::Stopped(stop)
    }
}

impl Refusal for EvaluationError {
    fn is_resource_refusal(&self) -> bool {
        match self {
            Self::Stopped(stop) => stop.is_resource_refusal(),
            Self::Storage { .. } => true,
        }
    }

    fn from_stop(stop: Stop) -> Self {
        Self::Stopped(stop)
    }
}

impl fmt::Display for EvaluationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stopped(stop) => stop.fmt(formatter),
            Self::Storage { required, limit } => write!(
                formatter,
                "formula evaluation needs {required} workspace bytes; limit is {limit}"
            ),
        }
    }
}

impl std::error::Error for EvaluationError {}

#[cfg(test)]
mod tests {
    use super::EvaluationError;
    use zetesis_cpu::{Refusal, Stop};

    #[test]
    fn evaluation_errors_classify_resource_refusals() {
        for refusal in [
            EvaluationError::Storage {
                required: 9,
                limit: 8,
            },
            EvaluationError::Stopped(Stop::WorkLimit),
            EvaluationError::Stopped(Stop::Allocation),
        ] {
            assert!(refusal.is_resource_refusal(), "{refusal:?}");
        }
        for other in [
            EvaluationError::Stopped(Stop::Cancelled),
            EvaluationError::Stopped(Stop::WrongProgram),
        ] {
            assert!(!other.is_resource_refusal(), "{other:?}");
        }
    }
}
