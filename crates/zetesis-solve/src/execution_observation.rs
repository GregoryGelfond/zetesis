//! Borrowed execution facts, distinct from answer-set evidence and presentation.

use crate::{CandidateSearch, Grounder, Oracle, SolveError, SourceBatching};
use std::{error::Error, num::NonZeroUsize};
use zetesis_themelios::objective_bound::ObjectiveBoundError;

/// One observation at an execution boundary of the current solve.
///
/// Values borrow the active call; the solver retains no observation queue. These
/// facts describe attempted preparation or execution choices, not completed
/// membership, publication or coverage. An observer may copy what it needs under
/// its own storage policy. Its work and storage are outside solver resource limits.
#[derive(Debug)]
pub enum ExecutionObservation<'a> {
    /// An admitted static relational representation is available.
    StaticGrounding {
        /// Requested materialization policy.
        requested: Grounder,
        /// Admitted atom count.
        atoms: usize,
        /// Admitted ground rule count.
        rules: usize,
        /// Configured lowering limits; a device can impose a smaller atom ceiling.
        limits: zetesis_core::StaticLimits,
    },
    /// Host source joins will supply CPU reduct closure.
    LazyGrounding {
        /// Requested materialization policy.
        requested: Grounder,
    },
    /// CPU closure executor was prepared.
    CpuClosure {
        /// Effective materialization; always Eager or Lazy.
        grounder: Grounder,
        /// Source round policy.
        batching: SourceBatching,
        /// Requested worker population.
        workers: NonZeroUsize,
    },
    /// CPU formula membership execution was selected.
    CpuFormula {
        /// Requested membership policy.
        oracle: Oracle,
        /// Requested materialization policy.
        grounder: Grounder,
        /// How classical candidates are proposed to the reduct.
        candidates: CandidateSearch,
    },
    /// Bounded parallel residual completion is requested.
    ExactCompletion {
        /// Requested worker population.
        workers: NonZeroUsize,
        /// Logical scratch ceiling, not process RSS.
        max_scratch_bytes: u64,
    },
    /// Automatic execution retains CPU because no measured device crossover
    /// has been established for the current policy.
    AutomaticCpu,
    /// Explicit shared source rounds select CPU execution.
    SharedCpu,
    /// This binary has no compiled device support.
    DeviceNotCompiled,
    /// Host source joins will supply device consequence rounds.
    #[cfg(feature = "gpu")]
    LazyDeviceGrounding {
        /// Requested materialization policy.
        requested: Grounder,
    },
    /// Device closure execution was prepared.
    #[cfg(feature = "gpu")]
    DeviceClosure {
        /// Observed adapter, borrowed from this executor.
        adapter: zetesis_wgpu::AdapterMetadata<'a>,
        /// Static atom/rule counts, or absence for lazy source rounds.
        static_counts: Option<(usize, usize)>,
    },
    /// A device propagator is ready; exact CPU residual completion is requested.
    #[cfg(feature = "gpu")]
    DeviceFormula {
        /// Observed adapter, borrowed from this executor.
        adapter: zetesis_wgpu::AdapterMetadata<'a>,
        /// Exact gate evaluator compiled into this executor. This records the
        /// selected implementation, not a device submission or completed check.
        projection: zetesis_wgpu::GateProjection,
        /// Requested materialization policy.
        grounder: Grounder,
        /// Candidate batch ceiling.
        batch_size: NonZeroUsize,
        /// Requested CPU completion population.
        completion_workers: NonZeroUsize,
    },
    /// Formula search is about to be prepared over this admitted population.
    Formula {
        /// Dense atom count.
        atoms: usize,
        /// DAG node count.
        nodes: usize,
        /// Original asserted root count.
        roots: usize,
        /// Written constraints over a keyed value asked as the one atom their
        /// key admits before grounding; zero when no constraint had the form.
        keyed_constraints: usize,
    },
    /// A checked tight certificate enables specialized membership checking.
    TightMembership,
    /// Complete positive atomic-head classification and least consequences enable
    /// the unique-answer restriction; original constraints remain authoritative.
    PositiveMembership,
    /// General reduct checking remains after an optional certificate refusal.
    GeneralMembership(zetesis_sat::CertificateError),
    /// Optional objective-plan preparation was refused; exact search remains.
    ObjectiveUnavailable(ObjectiveBoundError),
    /// Optional objective-bound construction was refused; exact search remains.
    ObjectiveBoundStopped(ObjectiveBoundError),
    /// A bound failed the original-theory ownership check.
    ObjectiveTheoryMismatch,
    /// Candidate restriction was refused; prior restrictions remain sound.
    ObjectiveRestrictionStopped(zetesis_sat::Incomplete),
    /// A dominance bound was installed after retaining a verified incumbent.
    ObjectiveBound {
        /// Cumulative installed restriction count.
        restrictions: u64,
        /// Incumbent priority/cost pairs.
        costs: &'a [(i32, i64)],
        /// Cumulative charged construction work.
        work: u64,
    },
}

/// Synchronous observations of one session's execution boundaries.
///
/// Each call is scoped to the session being initialized or pulled. The solver
/// borrows this observer only for that operation, does not retain events and
/// never treats an observer's error as a device failure or semantic evidence.
/// Failure stops the operation before its next execution step; a failed pull
/// leaves the session terminal and preserves its already checked prefix. The
/// callback controls its own effects, cancellation and resource use. Panics are
/// not caught. Returning an error cannot undo effects the callback already made.
pub trait ExecutionObserver {
    /// External observation failure, retained as the source of the solve fault.
    type Error: Error + Send + Sync + 'static;

    /// Consume an observation before the next execution step.
    ///
    /// # Errors
    /// Return an external failure to stop the current operation.
    fn observe(&mut self, observation: ExecutionObservation<'_>) -> Result<(), Self::Error>;
}

pub(crate) trait ExecutionSink {
    fn record(&mut self, observation: ExecutionObservation<'_>) -> Result<(), SolveError>;
}

pub(crate) struct Ignore;
impl ExecutionSink for Ignore {
    fn record(&mut self, _: ExecutionObservation<'_>) -> Result<(), SolveError> {
        Ok(())
    }
}

pub(crate) struct Observer<'a, O>(pub(crate) &'a mut O);
impl<O: ExecutionObserver> ExecutionSink for Observer<'_, O> {
    fn record(&mut self, observation: ExecutionObservation<'_>) -> Result<(), SolveError> {
        self.0
            .observe(observation)
            .map_err(|error| SolveError::ExecutionObservation(Box::new(error)))
    }
}
