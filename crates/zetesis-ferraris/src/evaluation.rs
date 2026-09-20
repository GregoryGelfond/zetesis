//! Subject-bound original truth with reusable, bounded workspace.

use std::fmt;

use crate::oracle::{Work, evaluate, failed_root};
use crate::{Interpretation, Limits, Statistics, Theory};
use zetesis_cpu::{Control, Stop};

/// Per-evaluation bounds, including storage retained from earlier evaluations.
#[derive(Clone, Copy, Debug)]
pub struct EvaluationLimits {
    /// One unit per formula node and tested asserted root.
    pub max_work: u64,
    /// Workspace header plus actual truth-vector capacity, in bytes.
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
/// is linear in the largest admitted node count. Reuse preserves capacity, not
/// truth: every node is recomputed for the supplied interpretation. A returned
/// [`FormulaEvaluation`] borrows both inputs and prevents workspace mutation
/// while its truth values remain observable.
#[derive(Debug, Default)]
pub struct EvaluationWorkspace {
    values: Vec<bool>,
}

impl EvaluationWorkspace {
    /// Header and actual truth capacity, excluding allocator bookkeeping.
    #[must_use]
    pub fn retained_bytes(&self) -> u128 {
        Self::bytes(self.values.capacity())
    }

    /// Reserve workspace for a theory without evaluating an interpretation.
    /// Clears previous scratch values, keeps existing capacity and establishes
    /// no subject or truth capability. A batch coordinator can use this to
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
        control: &Control,
    ) -> Result<(), EvaluationError> {
        control.poll()?;
        Self::check_storage(theory.nodes().len().max(self.values.capacity()), max_bytes)?;
        self.values.clear();
        self.values
            .try_reserve_exact(theory.nodes().len())
            .map_err(|_| Stop::Allocation)?;
        Self::check_storage(self.values.capacity(), max_bytes)
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
        control: &Control,
    ) -> EvaluationAttempt<'a> {
        let mut work = Work {
            limits: Limits {
                max_work: limits.max_work,
                max_subsets: 0,
            },
            control,
            statistics: Statistics::default(),
        };
        let result = self.evaluate_into(interpretation, limits.max_bytes, &mut work);
        EvaluationAttempt {
            work: work.statistics.work,
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
        work: &mut Work<'_>,
    ) -> Result<Option<usize>, EvaluationError> {
        let theory = interpretation.theory();
        self.reserve(theory, max_bytes, work.control)?;
        evaluate(theory, interpretation, None, &mut self.values, work)?;
        Ok(failed_root(theory, &self.values, work)?)
    }

    fn check_storage(capacity: usize, limit: usize) -> Result<(), EvaluationError> {
        let required = Self::bytes(capacity);
        if required > limit as u128 {
            Err(EvaluationError::Storage { required, limit })
        } else {
            Ok(())
        }
    }

    fn bytes(capacity: usize) -> u128 {
        size_of::<Self>() as u128 + capacity as u128 * size_of::<bool>() as u128
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
/// # use zetesis_cpu::Control;
/// # fn stale(candidate: &Interpretation) {
/// let mut workspace = EvaluationWorkspace::default();
/// let control = Control::default();
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
    /// Charged node/root visits, including on interruption.
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
