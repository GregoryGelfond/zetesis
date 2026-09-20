//! Extensible execution of bounded, original-theory candidate batches.

use std::{error::Error, fmt, sync::Arc};

use zetesis_cpu::Control;
use zetesis_ferraris::{Interpretation, Theory, TightPlan};
use zetesis_sat::{BatchVerdict, Incomplete};

/// Host accounting for an explicitly injected executor, including failed calls.
/// These are protocol receipts, not device work, instruction counts or RSS.
#[derive(Clone, Copy, Debug)]
pub struct BatchExecutionStatistics {
    /// Operations declared before preparation.
    pub capabilities: ExecutorCapabilities,
    /// Actual semantic operation selected from those capabilities.
    pub operation: MembershipOperation,
    /// Proposal, checker, residual and commit accounting from the semantic owner.
    pub batches: zetesis_sat::BatchStatistics,
    /// Exact host completion attempts, including interrupted work.
    pub completion: crate::CompletionAccounting,
    /// Verified interpretations waiting for objective scoring or delivery.
    pub queued_models: usize,
}

/// Membership operations an executor implements exactly, possibly leaving
/// individual candidates for native residual completion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecutorCapabilities {
    /// General finite formula reduct checking, without a class certificate.
    General,
    /// Producer support under a complete original-theory tight certificate.
    Tight,
    /// Both operations, preferring tight support when its certificate is available.
    GeneralAndTight,
}

/// Actual membership operation supplied to an executor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MembershipOperation {
    /// General formula reduct checking, with exact host residual completion.
    General,
    /// Support under the complete original-theory tight certificate.
    Tight,
}

impl ExecutorCapabilities {
    const fn general(self) -> bool {
        matches!(self, Self::General | Self::GeneralAndTight)
    }

    pub(crate) const fn tight(self) -> bool {
        matches!(self, Self::Tight | Self::GeneralAndTight)
    }

    pub(crate) fn select_plan<'a>(
        self,
        theory: &'a Theory,
        tight: Option<&'a Arc<TightPlan>>,
    ) -> Result<MembershipPlan<'a>, ExecutorError> {
        let selected = if self.tight() {
            tight.map(MembershipPlan::Tight)
        } else {
            None
        };
        selected
            .or_else(|| self.general().then_some(MembershipPlan::General(theory)))
            .ok_or(ExecutorError::Capability(self))
    }
}

/// The immutable semantic operation selected before executor preparation.
/// This view never contains an objective restriction or a partial certificate.
#[derive(Clone, Copy, Debug)]
pub enum MembershipPlan<'a> {
    /// Original satisfaction and proper-subset checking of its frozen reduct.
    General(&'a Theory),
    /// Original truth and producer support under this complete checked plan.
    Tight(&'a Arc<TightPlan>),
}

impl MembershipPlan<'_> {
    /// Selected operation, independently of hardware and eventual success.
    #[must_use]
    pub const fn operation(self) -> MembershipOperation {
        match self {
            Self::General(_) => MembershipOperation::General,
            Self::Tight(_) => MembershipOperation::Tight,
        }
    }

    /// Exact original immutable subject. Equal independent admissions differ.
    #[must_use]
    pub fn theory(&self) -> &Theory {
        match self {
            Self::General(theory) => theory,
            Self::Tight(plan) => plan.theory(),
        }
    }
}

/// Original models borrowed for one synchronous executor call, in proposal order.
///
/// The host already checked original satisfaction. Each interpretation belongs
/// to `theory()`, and remains the candidate fixing its reduct. The executor may
/// inspect or copy them under its own resource policy; it cannot alter the host's
/// candidates, coverage or restrictions. This view has no public constructor.
pub struct CandidateBatch<'a> {
    theory: &'a Theory,
    candidates: &'a [Interpretation],
}

impl<'a> CandidateBatch<'a> {
    pub(crate) const fn new(theory: &'a Theory, candidates: &'a [Interpretation]) -> Self {
        Self { theory, candidates }
    }

    /// Exact original subject, shared with the prepared membership plan.
    #[must_use]
    pub const fn theory(&self) -> &'a Theory {
        self.theory
    }

    /// Immutable original candidates; result position must preserve this order.
    #[must_use]
    pub const fn candidates(&self) -> &'a [Interpretation] {
        self.candidates
    }

    /// Associate one ordered verdict with each candidate in this exact request.
    ///
    /// This checks shape and retains request identity; it does **not** verify
    /// semantic soundness or detect verdicts swapped within the same request.
    /// `NoProperSubset` requires complete membership evidence; `Refuted` requires
    /// a sound nonminimality argument, such as failed support under the complete
    /// tight plan. An unfinished check must return `Residual` or an interruption.
    ///
    /// Transfers the vector without copying. Its allocation belongs to the
    /// executor; the host's pending allowance already includes one verdict slot
    /// per candidate, but excludes executor-private storage and allocator overhead.
    ///
    /// # Errors
    /// Refuses any result count different from the request's candidate count.
    pub fn finish(self, verdicts: Vec<BatchVerdict>) -> Result<BatchResult<'a>, ExecutorError> {
        if verdicts.len() != self.candidates.len() {
            return Err(ExecutorError::Shape {
                expected: self.candidates.len(),
                actual: verdicts.len(),
            });
        }
        Ok(BatchResult {
            batch: self,
            verdicts,
        })
    }
}

/// Request-associated ordered verdict data, not independently verified answers.
/// The host validates the association, exactly completes `Residual` checks and
/// commits results under [`BatchExecutor`]'s soundness contract. Decisive verdicts
/// rely on that contract; the receipt does not independently verify them.
pub struct BatchResult<'a> {
    batch: CandidateBatch<'a>,
    verdicts: Vec<BatchVerdict>,
}

impl BatchResult<'_> {
    pub(crate) fn into_verdicts(
        self,
        theory: &Theory,
        candidates: &[Interpretation],
    ) -> Result<Vec<BatchVerdict>, ExecutorError> {
        if !self.batch.theory.same_instance(theory)
            || !std::ptr::eq(self.batch.candidates, candidates)
        {
            return Err(ExecutorError::ForeignBatch);
        }
        Ok(self.verdicts)
    }
}

/// A caller-supplied membership primitive used by an ordinary formula session.
///
/// Preparation happens once after bounded semantic planning. Checks are
/// synchronous on the pulling thread and receive bounded original-model batches.
/// The session owns this executor exclusively; handles inside it may share
/// caller-owned infrastructure. No executor method controls candidate coverage,
/// objective selection, exact residual completion or answer publication.
///
/// This is an explicit semantic trust boundary. A decision must be sound for
/// the exact selected plan and candidate's frozen reduct. The host validates
/// original satisfaction, receipt identity and result count, but does not prove
/// an arbitrary implementation's verdicts. Return `Residual` when no exact
/// decision is available. Resource exhaustion and lost device completion cannot
/// become a decisive verdict. Panics are not caught and effects cannot be undone.
///
/// Implementations must poll `Control` cooperatively and bound their own driver,
/// compilation and execution allocations. Solver quotas cover host orchestration,
/// not arbitrary callback work, device memory, or process RSS. Preparation and
/// check failures terminate this session without another executor or fallback.
pub trait BatchExecutor: Send {
    /// Original external fault retained by the solve failure.
    type Error: Error + Send + Sync + 'static;

    /// Operations this implementation supports; queried once before planning.
    fn capabilities(&self) -> ExecutorCapabilities;

    /// Prepare resources for the exact selected immutable plan.
    ///
    /// # Errors
    /// Refuse unsupported dimensions/capabilities, an incomplete operation, or
    /// an external fault explicitly. No failure establishes membership or coverage.
    fn prepare(
        &mut self,
        plan: MembershipPlan<'_>,
        control: &Control,
    ) -> Result<(), ExecutorFailure<Self::Error>>;

    /// Check every candidate, preserving request order and exact subject identity.
    ///
    /// # Errors
    /// Return an interruption or fault without claiming a completed partial
    /// batch. Host-owned pending candidates and previous answers remain accounted.
    fn check<'a>(
        &mut self,
        batch: CandidateBatch<'a>,
        control: &Control,
    ) -> Result<BatchResult<'a>, ExecutorFailure<Self::Error>>;
}

/// An incomplete primitive, explicit refusal, or original external fault.
#[derive(Debug)]
pub enum ExecutorFailure<E> {
    /// Resource or cooperative-control stop; never a completed negative answer.
    Interrupted(Incomplete),
    /// This implementation cannot execute the supplied plan or dimensions.
    Unsupported,
    /// Request/result protocol failure.
    Protocol(ExecutorError),
    /// Original implementation-specific fault, without a replacement executor.
    Failed(E),
}

impl<E: fmt::Display> fmt::Display for ExecutorFailure<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Interrupted(error) => error.fmt(f),
            Self::Unsupported => f.write_str("executor cannot perform the supplied operation"),
            Self::Protocol(error) => error.fmt(f),
            Self::Failed(error) => error.fmt(f),
        }
    }
}

impl<E: Error + 'static> Error for ExecutorFailure<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Interrupted(error) => Some(error),
            Self::Unsupported => None,
            Self::Protocol(error) => Some(error),
            Self::Failed(error) => Some(error),
        }
    }
}

impl<E> From<ExecutorError> for ExecutorFailure<E> {
    fn from(error: ExecutorError) -> Self {
        Self::Protocol(error)
    }
}

impl<E> From<Incomplete> for ExecutorFailure<E> {
    fn from(error: Incomplete) -> Self {
        Self::Interrupted(error)
    }
}

impl<E> From<zetesis_cpu::Stop> for ExecutorFailure<E> {
    fn from(error: zetesis_cpu::Stop) -> Self {
        Self::Interrupted(error.into())
    }
}

/// Refusal or malformed output at the ordinary session's executor boundary.
#[derive(Debug)]
pub enum ExecutorError {
    /// The supplied prepared representation is not the formula profile.
    Input(crate::PreparedProfile),
    /// An explicit builtin hardware request conflicts with explicit injection.
    Backend(crate::Backend),
    /// No available plan matches the executor's declared operations.
    Capability(ExecutorCapabilities),
    /// Executor preparation or checking explicitly refused the operation.
    Unsupported,
    /// A receipt belongs to a different exact subject or pending batch.
    ForeignBatch,
    /// The executor returned a different number of verdicts than candidates.
    Shape {
        /// Original candidate count.
        expected: usize,
        /// Returned verdict count.
        actual: usize,
    },
    /// Original implementation-specific fault; available for downcasting.
    External(Box<dyn Error + Send + Sync>),
}

impl fmt::Display for ExecutorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Input(profile) => {
                write!(f, "batch executor requires formula input, got {profile:?}")
            }
            Self::Backend(backend) => write!(
                f,
                "explicit batch executor conflicts with {} backend",
                backend.label()
            ),
            Self::Capability(capabilities) => {
                write!(f, "no membership plan supports {capabilities:?} executor")
            }
            Self::Unsupported => f.write_str("batch executor refused the selected operation"),
            Self::ForeignBatch => {
                f.write_str("executor receipt belongs to another candidate batch")
            }
            Self::Shape { expected, actual } => write!(
                f,
                "batch executor returned {actual} verdicts for {expected} candidates"
            ),
            Self::External(error) => write!(f, "batch executor: {error}"),
        }
    }
}

impl Error for ExecutorError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::External(error) => Some(error.as_ref()),
            _ => None,
        }
    }
}

/// Erases the associated error so ordinary sessions need not be generic over an
/// executor. Request identity and the public executor contract remain unchanged.
pub(crate) trait ErasedExecutor: Send {
    fn capabilities(&self) -> ExecutorCapabilities;
    fn prepare(
        &mut self,
        plan: MembershipPlan<'_>,
        control: &Control,
    ) -> Result<(), crate::formula_execution::Failure>;
    fn check<'a>(
        &mut self,
        batch: CandidateBatch<'a>,
        control: &Control,
    ) -> Result<BatchResult<'a>, crate::formula_execution::Failure>;
}

pub(crate) struct Adapter<E>(pub(crate) E);

impl<E: BatchExecutor> ErasedExecutor for Adapter<E> {
    fn capabilities(&self) -> ExecutorCapabilities {
        self.0.capabilities()
    }

    fn prepare(
        &mut self,
        plan: MembershipPlan<'_>,
        control: &Control,
    ) -> Result<(), crate::formula_execution::Failure> {
        self.0.prepare(plan, control).map_err(failure)
    }

    fn check<'a>(
        &mut self,
        batch: CandidateBatch<'a>,
        control: &Control,
    ) -> Result<BatchResult<'a>, crate::formula_execution::Failure> {
        self.0.check(batch, control).map_err(failure)
    }
}

fn failure<E: Error + Send + Sync + 'static>(
    error: ExecutorFailure<E>,
) -> crate::formula_execution::Failure {
    use crate::formula_execution::Failure;
    let error = match error {
        ExecutorFailure::Interrupted(stop) => return Failure::Search(stop),
        ExecutorFailure::Unsupported => ExecutorError::Unsupported,
        ExecutorFailure::Protocol(error) => error,
        ExecutorFailure::Failed(error) => ExecutorError::External(Box::new(error)),
    };
    Failure::from(error)
}

#[derive(Clone, Copy)]
pub(crate) enum Mode {
    Builtin,
    External(ExecutorCapabilities),
}

impl Mode {
    pub(crate) fn native_workers(
        self,
        config: &crate::SolveConfig,
    ) -> Option<std::num::NonZeroUsize> {
        match self {
            Self::Builtin => config.region_workers(),
            Self::External(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CandidateBatch, ExecutorError};
    use zetesis_ferraris::{AdmissionLimits, Interpretation, Theory};
    use zetesis_sat::BatchVerdict;

    fn theory() -> Theory {
        Theory::new(1, Vec::new(), Vec::new(), AdmissionLimits::default()).unwrap()
    }

    #[test]
    fn equal_foreign_theory_cannot_supply_a_receipt() {
        let original = theory();
        let foreign = theory();
        let candidates = [Interpretation::new(&original, []).unwrap()];
        let receipt = CandidateBatch::new(&foreign, &candidates)
            .finish(vec![BatchVerdict::Residual])
            .unwrap_or_else(|error| panic!("{error}"));
        assert!(matches!(
            receipt.into_verdicts(&original, &candidates),
            Err(ExecutorError::ForeignBatch)
        ));
    }

    #[test]
    fn equal_candidate_copies_are_not_the_pending_batch() {
        let original = theory();
        let candidates = [Interpretation::new(&original, []).unwrap()];
        let copied = candidates.clone();
        let receipt = CandidateBatch::new(&original, &copied)
            .finish(vec![BatchVerdict::Residual])
            .unwrap_or_else(|error| panic!("{error}"));
        assert!(matches!(
            receipt.into_verdicts(&original, &candidates),
            Err(ExecutorError::ForeignBatch)
        ));
    }

    #[test]
    fn exact_batch_receipt_preserves_ordered_verdicts() {
        let original = theory();
        let candidates = [
            Interpretation::new(&original, []).unwrap(),
            Interpretation::new(&original, [0]).unwrap(),
        ];
        let verdicts = vec![BatchVerdict::NoProperSubset, BatchVerdict::Residual];
        let receipt = CandidateBatch::new(&original, &candidates)
            .finish(verdicts.clone())
            .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(
            receipt.into_verdicts(&original, &candidates).unwrap(),
            verdicts
        );
    }
}
