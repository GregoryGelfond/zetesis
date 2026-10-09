//! Borrowed execution facts, distinct from answer-set evidence and presentation.

use crate::{Grounder, Oracle, SearchMethod, SolveError, SourceBatching};
use std::{error::Error, num::NonZeroUsize};
use zetesis_themelios::objective_bound::ObjectiveBoundError;

/// The integrity constraints a hybrid base streams.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StreamedConstraints {
    /// Lowered constraint templates, not a count of original declarations.
    pub templates: usize,
    /// Possible instances visited during complete source admission.
    pub instances: u64,
}

/// One observation at an execution boundary of the current solve.
///
/// Values borrow the active call; the solver retains no observation queue. These
/// facts describe attempted preparation or execution choices, not completed
/// membership, publication or coverage. An observer may copy what it needs under
/// its own storage policy. Its work and storage are outside solver resource limits.
#[derive(Debug)]
pub enum ExecutionObservation<'a> {
    /// The original source is represented by a checked base theory and terminal
    /// definitions. Formula populations and membership events that follow refer
    /// to that base; host reconstruction precedes every original answer.
    TerminalDefinitions {
        /// Requested schedule: automatic for an eager base, lazy for a hybrid one.
        requested: Grounder,
        /// How the base was grounded.
        base: zetesis_themelios::BaseKind,
        /// Admitted deferred rule occurrences, not a count of ground answers.
        deferred_templates: usize,
        /// A hybrid base's streamed constraints; absent for an eager base.
        streamed: Option<StreamedConstraints>,
    },
    /// The complete producer core is retained; eligible source constraints are
    /// checked on the host before any original-program answer is accepted.
    HybridGrounding {
        /// Requested schedule; effective execution combines eager and lazy work.
        requested: Grounder,
        /// Lowered constraint templates, not a count of original declarations.
        streamed_templates: usize,
        /// Possible instances visited during complete source admission.
        streamed_instances: u64,
    },
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
        /// How candidates are proposed and the reduct queried.
        search: SearchMethod,
    },
    /// Several workers walk the region tree at once and decide its leaves.
    ParallelRegions {
        /// Requested worker population.
        workers: NonZeroUsize,
    },
    /// Bounded workers propose formula leaves, then join before an independent
    /// membership executor checks the batch. This is not CPU membership.
    ParallelProposals {
        /// Requested candidate-production population.
        workers: NonZeroUsize,
    },
    /// Bounded parallel residual completion is requested.
    ExactCompletion {
        /// Requested worker population.
        workers: NonZeroUsize,
        /// Logical scratch ceiling, not process RSS.
        max_scratch_bytes: u64,
    },
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
        /// How candidates are proposed and the reduct queried.
        search: SearchMethod,
        /// Candidate batch ceiling.
        batch_size: NonZeroUsize,
        /// Requested CPU completion population.
        completion_workers: NonZeroUsize,
    },
    /// A complete tight certificate selects original truth and producer support
    /// checking on this device; no general propagator is constructed.
    #[cfg(feature = "gpu")]
    DeviceTight {
        /// Observed adapter of the actual tight-support executor.
        adapter: zetesis_wgpu::AdapterMetadata<'a>,
        /// Requested materialization policy.
        grounder: Grounder,
        /// Candidate proposal method; membership uses the tight certificate.
        search: SearchMethod,
        /// Candidate batch ceiling.
        batch_size: NonZeroUsize,
    },
    /// Formula search is about to be prepared over this admitted population.
    Formula {
        /// Dense atom count.
        atoms: usize,
        /// DAG node count.
        nodes: usize,
        /// Logical child occurrences, including repeated children. Inline pairs
        /// and implications each contribute two; this is not arena length.
        operands: usize,
        /// Asserted roots of this membership subproblem. Under a preceding
        /// `TerminalDefinitions` event these are base roots, not the full source.
        roots: usize,
        /// Written constraints over a keyed value asked as the one atom their
        /// key admits before grounding; zero when no constraint had the form.
        keyed_constraints: usize,
    },
    /// The key analysis behind the asked constraints stopped at its work
    /// ceiling; every constraint not yet asked was grounded as written.
    KeyAnalysisStopped(zetesis_themelios::DomainStop),
    /// A checked tight certificate enables specialized membership checking.
    TightMembership,
    /// Complete positive atomic-head classification and least consequences enable
    /// the unique-answer restriction; original constraints remain authoritative.
    PositiveMembership,
    /// Complete original stratified normal theory determines at most one answer.
    StratifiedMembership,
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
