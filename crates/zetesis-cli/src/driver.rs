use crate::failure::Progress;
use crate::phase_timing::Recorder;
use crate::presentation::Diagnostics;
use crate::{Backend, Completion, Grounder, Interruption, Options, RunFailure};
use std::fmt;
use std::io::{self, Write};
use zetesis_cpu::{BatchError, Cancellation, Stop};
use zetesis_themelios::{
    AdmissionFailure, BundleAdmissionFailure, BundleError, ExpansionFailure, SourceBundle,
};

/// Publication counts and search coverage from a completed driver invocation.
/// Without objectives, stable models are written as found. Optimization retains
/// incumbent ties and publishes the requested optimal ties only after exhaustion
/// establishes the optimum.
/// Interrupted optimization may publish retained incumbents without an optimum claim.
#[derive(Clone, Debug)]
pub struct Report {
    /// Complete model records accepted by the output sink, including objective costs.
    /// Partial writes are excluded; sink acceptance does not establish durability.
    /// This count can be smaller than the number of verified stable models.
    pub models: usize,
    /// Closure result/control records examined, or classical formula candidates
    /// proposed (including explicitly retained pending batch work). A whole-batch
    /// closure interruption is one control record; shared statistics separately
    /// count every submitted occurrence without a complete check.
    pub checked: u64,
    /// Coverage classification independent of satisfiability.
    pub completion: Completion,
    /// Interruption reason, present exactly for interrupted reports.
    pub interruption: Option<Interruption>,
    /// Gate tuples discovered by the candidate generator.
    pub discovered_gate_atoms: usize,
    /// Expansion charges relational admission accepted, each under its
    /// ceiling; absent for the formula route.
    pub expansion: Option<zetesis_themelios::ExpansionUsage>,
    /// Necessary closure-candidate restrictions, including interrupted work.
    pub candidate_statistics: Option<zetesis_cpu::CandidateStatistics>,
    /// Cumulative countermodel accounting when available; absent for closure
    /// and for an initialization failure that returns no statistics.
    pub countermodel_statistics: Option<zetesis_sat::Statistics>,
    /// Actual batched formula execution and pending-result accounting; absent
    /// when the scalar CPU route was used or initialization did not finish.
    pub formula_execution: Option<crate::FormulaExecutionStatistics>,
    /// Retained-core answers and their source-constraint decisions. Only accepted
    /// checks establish membership in the original hybrid program.
    pub hybrid_execution: Option<zetesis_solve::HybridExecutionStatistics>,
    /// Base answers and completed original-answer reconstruction.
    pub terminal_execution: Option<zetesis_solve::TerminalExecutionStatistics>,
    /// Actual lazy device execution, including shared source work and failed
    /// batch progress. Absent when no lazy device executor was initialized.
    pub lazy_execution: Option<crate::LazyExecutionStatistics>,
    /// Shared CPU source and per-world work, including failed-batch prefixes.
    pub shared_execution: Option<crate::SharedExecutionStatistics>,
    /// Independent CPU closure counters summed over completed checks; absent
    /// for the shared, device and formula routes.
    pub closure_execution: Option<crate::ClosureExecutionStatistics>,
    /// Prepared independent CPU ownership receipts and any snapshot fault.
    pub query_execution: Option<crate::QueryExecutionObservation>,
    /// Whether semantic search established an optimum, independently of delivery.
    pub optimum_proved: bool,
    /// Best retained objective score and tied models found so far.
    /// A retained score is optimal only when `optimum_proved` is true.
    pub optimization: Option<crate::Optimization>,
    /// Opt-in attempted host timings; unavailable phases are absent.
    /// A timing snapshot does not establish semantic or output completion.
    pub phase_timings: Option<crate::PhaseTimings>,
}

/// A failed input, transport, or backend operation; never a claim of UNSAT.
#[derive(Debug)]
pub enum RunError {
    /// A verified base answer could not be completely reconstructed.
    Reconstruction(zetesis_themelios::ReconstructionError),
    /// Consumed base-answer accounting cannot represent another answer.
    TerminalStatisticsOverflow,
    /// A streamed source constraint could not be completely evaluated.
    Constraint(zetesis_themelios::ConstraintCheckFailure),
    /// A source region refusal lost its required typed cause.
    ConstraintFailureMissing,
    /// Consumed core-answer accounting cannot represent another answer.
    HybridStatisticsOverflow,
    /// Streamed formula constraints do not have a device executor yet.
    HybridBackend {
        /// Explicitly requested execution hardware.
        backend: Backend,
    },
    /// Fixed-domain projected enumeration failed with full membership retained.
    Projection(zetesis_solve::ProjectionError),
    /// Reading a bounded standard-input source failed before semantic admission.
    Input(io::Error),
    /// The requested process deadline cannot be represented by the host clock.
    TimeLimitRange {
        /// Requested whole seconds, retained without an I/O attribution.
        seconds: u64,
    },
    /// The host refused the thread that would observe the process deadline.
    DeadlineTimer(io::Error),
    /// A complete observation could not be evaluated or rendered.
    Observation(zetesis_themelios::observation::Error),
    /// A bounded JSON model or terminal record could not be constructed.
    JsonRecord(zetesis_themelios::observation::ViewError),
    /// A complete buffered Answer would exceed its byte ceiling.
    ObservationOutputLimit {
        /// Attempted prefix bytes: a lower bound on the required complete record.
        observed: u128,
        /// Inclusive configured ceiling.
        limit: usize,
    },
    /// This input profile requires standard input to be the only source root.
    MixedStandardInput,
    /// Source was refused by the located themelios boundary.
    Admission(AdmissionFailure),
    /// Finite source expansion was refused, with original locations.
    Expansion(ExpansionFailure),
    /// Loading the original source graph failed before semantic admission.
    BundleLoad(BundleError),
    /// Extended bundle admission was refused, retaining all original sources.
    BundleAdmission(BundleAdmissionFailure),
    /// Owned CPU pool or batch could not be admitted.
    Batch(BatchError),
    /// Reading prepared CPU ownership receipts failed after candidate execution.
    QueryObservation(std::sync::Arc<BatchError>),
    /// Dedicated exact-completion pool construction failed.
    CompletionPool(rayon::ThreadPoolBuildError),
    /// Requested backend was not compiled into this binary.
    BackendUnavailable,
    /// A supplied library executor refused or failed its batch operation.
    Executor(zetesis_solve::ExecutorError),
    /// The requested materialization strategy is unavailable on that backend.
    UnsupportedCombination {
        /// Explicit execution hardware request.
        backend: Backend,
        /// Explicit materialization request.
        grounder: Grounder,
    },
    /// The countermodel oracle requires eager finite formula admission.
    UnsupportedOracle {
        /// Explicit execution hardware request.
        backend: Backend,
        /// Explicit materialization request.
        grounder: Grounder,
    },
    /// The requested shared source policy needs relational lazy CPU execution.
    UnsupportedSourceBatching,
    /// The workers' closure allowances together exceed the collective ceiling.
    ClosureReservation {
        /// Requested closure workers.
        workers: usize,
        /// Per-closure allowance, given or derived.
        max_closure_bytes: usize,
        /// Collective ceiling.
        max_closure_batch_bytes: usize,
    },
    /// An already prepared representation cannot honor the requested strategy.
    PreparedInput {
        /// Representation supplied by the caller.
        profile: crate::PreparedProfile,
        /// Explicit membership strategy.
        oracle: crate::Oracle,
        /// Explicit materialization strategy.
        grounder: Grounder,
    },
    /// Static normal-rule to formula translation exceeded its admission contract.
    Formula(zetesis_ferraris::AdmissionError),
    /// Finite source-to-formula admission was refused with original locations.
    FormulaAdmission(zetesis_themelios::FormulaFailure),
    /// Formula admission retained the original bundle's source diagnostics.
    FormulaBundleAdmission(zetesis_themelios::FormulaBundleFailure),
    /// Output could not be written.
    Output(io::Error),
    /// An external execution observer failed; no device fallback may consume it.
    ExecutionObservation(Box<dyn std::error::Error + Send + Sync>),
    /// Cancellation or a deadline stopped record preparation or prepublication.
    PublicationStopped(Stop),
    /// Explicit static lowering was refused.
    Static(zetesis_core::StaticError),
    /// GPU capability, submission, or result transport failed.
    #[cfg(feature = "gpu")]
    Gpu(zetesis_wgpu::GpuError),
    /// Lazy device or protocol failure with retained shared source progress.
    #[cfg(feature = "gpu")]
    LazyGpu(zetesis_cpu::lazy::Failure<zetesis_wgpu::GpuError>),
    /// Cumulative lazy execution counters could not represent another batch.
    LazyStatisticsOverflow,
    /// Cumulative independent closure counters could not represent another check.
    ClosureStatisticsOverflow,
    /// Concrete shared CPU evaluation violated its round protocol.
    SharedCpu(zetesis_cpu::lazy::shared::Cause),
    /// Static closure decoding refused its words or selected-position storage.
    Words(zetesis_core::WordError),
    /// A verified interpretation could not retain its selected catalog atoms.
    Model(zetesis_core::ModelError),
    /// A driver requested a successful legacy report before search classified its stop.
    /// This protocol failure establishes neither interruption nor unsatisfiability.
    CompletionUnavailable,
    /// Membership execution ended without exhausted candidate coverage.
    CandidateStreamNotExhausted,
    /// An injected batch checker violated its ordered result-count contract.
    FormulaBatchShape {
        /// Number of original candidates supplied.
        expected: usize,
        /// Number of verdicts returned.
        actual: usize,
    },
}
impl fmt::Display for RunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if matches!(
            self,
            Self::Admission(_)
                | Self::Expansion(_)
                | Self::BundleAdmission(_)
                | Self::FormulaAdmission(_)
                | Self::FormulaBundleAdmission(_)
        ) {
            f.write_str("source admission: ")?;
        }
        match self {
            Self::Reconstruction(error) => error.fmt(f),
            Self::TerminalStatisticsOverflow => f.write_str("terminal answer accounting overflow"),
            Self::Constraint(error) => error.fmt(f),
            Self::ConstraintFailureMissing => f.write_str("source region check stopped without its failure receipt"),
            Self::HybridStatisticsOverflow => f.write_str("hybrid answer accounting overflow"),
            Self::HybridBackend { backend } => write!(f,
                "streamed formula constraints support cpu or auto execution; requested {}", backend.label()),
            Self::Executor(error) => error.fmt(f),
            Self::Projection(error) => error.fmt(f),
            Self::Input(error) => write!(f, "standard input ('-'): {error}"),
            Self::TimeLimitRange { seconds } => write!(f, "--time-limit {seconds}: time limit exceeds the platform clock range"),
            Self::DeadlineTimer(error) => write!(f, "--time-limit: the deadline timer could not be started: {error}"),
            Self::MixedStandardInput => f.write_str(
                "standard input ('-') must be the only input; mixed or repeated stdin roots are unsupported",
            ),
            Self::Admission(error) => error.fmt(f),
            Self::Batch(error) => error.fmt(f),
            Self::QueryObservation(error) => write!(f, "query observation: {error}"),
            Self::CompletionPool(error) => write!(f, "completion worker pool: {error}"),
            Self::BundleLoad(error) => write!(f, "source loading: {error}"),
            Self::BundleAdmission(error) => error.fmt(f),
            Self::Expansion(error) => error.fmt(f),
            Self::BackendUnavailable => f.write_str(
                "GPU support was not compiled; install the default build or enable --features gpu",
            ),
            Self::UnsupportedCombination { backend, grounder } => write!(
                f,
                "unsupported backend/grounding profile: {} with {}",
                backend.label(),
                grounder.label()
            ),
            Self::UnsupportedOracle { backend, grounder } => write!(
                f,
                "the countermodel oracle requires --grounder eager or auto; requested {} with {}",
                backend.label(),
                grounder.label()
            ),
            Self::PreparedInput { profile, oracle, grounder } => write!(f,
                "prepared {profile:?} cannot honor oracle {} with grounder {}", oracle.label(), grounder.label()),
            Self::UnsupportedSourceBatching => f.write_str("shared source batching requires the relational closure route with lazy/auto grounding and cpu/auto backend"),
            Self::ClosureReservation {
                workers,
                max_closure_bytes,
                max_closure_batch_bytes,
            } => write!(
                f,
                "--threads {workers} at --max-closure-bytes {max_closure_bytes} need {} bytes, above --max-closure-batch-bytes {max_closure_batch_bytes}; use fewer threads, a smaller allowance, or a larger collective ceiling",
                (*workers as u128) * (*max_closure_bytes as u128)
            ),
            Self::SharedCpu(cause) => cause.fmt(f),
            Self::Formula(error) => error.fmt(f),
            Self::FormulaAdmission(error) => error.fmt(f),
            Self::FormulaBundleAdmission(error) => error.fmt(f),
            Self::Output(error) => write!(f, "output: {error}"),
            Self::ExecutionObservation(error) => write!(f, "execution observer: {error}"),
            Self::PublicationStopped(error) => write!(f, "publication stopped: {error}"),
            Self::Observation(error) => error.fmt(f),
            Self::JsonRecord(error) => error.fmt(f),
            Self::ObservationOutputLimit { observed, limit } => write!(f, "human Answer requires at least {observed} bytes; limit {limit}"),
            Self::Static(error) => error.fmt(f),
            #[cfg(feature = "gpu")]
            Self::Gpu(error) => error.fmt(f),
            #[cfg(feature = "gpu")]
            Self::LazyGpu(error) => error.fmt(f),
            Self::LazyStatisticsOverflow => f.write_str("lazy execution statistics overflow"),
            Self::ClosureStatisticsOverflow => {
                f.write_str("closure execution statistics overflow")
            }
            Self::CompletionUnavailable => f.write_str("driver report requires established search completion"),
            Self::Words(error) => error.fmt(f),
            Self::Model(error) => error.fmt(f),
            Self::CandidateStreamNotExhausted => f.write_str("membership execution ended before candidate exhaustion"),
            Self::FormulaBatchShape { expected, actual } => write!(f, "formula checker returned {actual} results for {expected} candidates"),
        }?;
        self.write_diagnostics(f)
    }
}
impl RunError {
    fn write_diagnostics(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (bundle, diagnostics) = match self {
            // Syntax refusals already render the retained source through the
            // canonical themelios view. Appending byte-only labels duplicates it.
            Self::Admission(AdmissionFailure::Syntax(_))
            | Self::Expansion(ExpansionFailure::Admission(AdmissionFailure::Syntax(_)))
            | Self::FormulaAdmission(zetesis_themelios::FormulaFailure::Expansion(
                ExpansionFailure::Admission(AdmissionFailure::Syntax(_)),
            )) => return Ok(()),
            Self::Admission(error) => (None, error.diagnostics()),
            Self::Expansion(error) => (None, error.diagnostics()),
            Self::FormulaAdmission(error) => (None, error.diagnostics()),
            Self::BundleAdmission(error) => (Some(error.bundle()), error.diagnostics()),
            Self::FormulaBundleAdmission(error) => (Some(error.bundle()), error.diagnostics()),
            _ => return Ok(()),
        };
        for diagnostic in diagnostics {
            let location = diagnostic.primary().location;
            write!(f, "\n  ")?;
            if let Some(bundle) = bundle {
                if let Some(source) = bundle.get(location.source) {
                    write!(f, "{}: ", source.path().display())?;
                } else {
                    write!(f, "source {}: ", location.source.get())?;
                }
            }
            write!(
                f,
                "bytes {}..{}: {}",
                location.span.start().get(),
                location.span.end().get(),
                diagnostic.message()
            )?;
        }
        Ok(())
    }
}
impl std::error::Error for RunError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Reconstruction(error) => Some(error),
            Self::Constraint(error) => Some(error),
            Self::Projection(error) => Some(error),
            Self::Input(error) => Some(error),
            Self::Admission(error) => Some(error),
            Self::Expansion(error) => Some(error),
            Self::BundleLoad(error) => Some(error),
            Self::BundleAdmission(error) => Some(error),
            Self::Batch(error) => Some(error),
            Self::QueryObservation(error) => Some(error.as_ref()),
            Self::CompletionPool(error) => Some(error),
            Self::Output(error) => Some(error),
            Self::DeadlineTimer(error) => Some(error),
            Self::ExecutionObservation(error) => Some(error.as_ref()),
            Self::Executor(error) => Some(error),
            Self::PublicationStopped(error) => Some(error),
            Self::Observation(error) => Some(error),
            Self::JsonRecord(error) => Some(error),
            Self::ObservationOutputLimit { .. } | Self::TimeLimitRange { .. } => None,
            Self::MixedStandardInput
            | Self::ConstraintFailureMissing
            | Self::TerminalStatisticsOverflow
            | Self::HybridStatisticsOverflow
            | Self::HybridBackend { .. }
            | Self::BackendUnavailable
            | Self::UnsupportedCombination { .. }
            | Self::UnsupportedOracle { .. }
            | Self::UnsupportedSourceBatching
            | Self::ClosureReservation { .. }
            | Self::LazyStatisticsOverflow
            | Self::ClosureStatisticsOverflow
            | Self::CompletionUnavailable
            | Self::FormulaBatchShape { .. }
            | Self::CandidateStreamNotExhausted => None,
            Self::Formula(error) => Some(error),
            Self::FormulaAdmission(error) => Some(error),
            Self::FormulaBundleAdmission(error) => Some(error),
            Self::Static(error) => Some(error),
            #[cfg(feature = "gpu")]
            Self::Gpu(error) => Some(error),
            #[cfg(feature = "gpu")]
            Self::LazyGpu(error) => Some(error),
            Self::Words(error) => Some(error),
            Self::Model(error) => Some(error),
            Self::SharedCpu(error) => Some(error),
            Self::PreparedInput { .. } => None,
        }
    }
}
impl From<io::Error> for RunError {
    fn from(error: io::Error) -> Self {
        Self::Output(error)
    }
}

/// Admit and stream exact stable models, with separate coverage reporting.
/// Enumeration checks the empty seed alone before requesting more gate tuples.
/// The countermodel oracle checks and blocks complete semantic interpretations.
/// Both routes preserve distinct full models independently of display selection.
/// With objectives, requested optimal ties are published only after exhaustion
/// establishes the optimum.
/// Interrupted optimization may publish retained incumbents without an optimum claim.
/// A completed exhaustive run with no models is the only UNSAT outcome.
///
/// # Errors
/// Returns [`RunError`] for source, backend, observation/view, or output failures.
/// Search, objective, and incumbent-retention stops produce an interrupted
/// [`Report`] unless a later reporting operation fails. Cooperative publication
/// stops use the legacy `PublicationStopped` adapter; the finalized API exposes
/// them directly as `PublicationOutcome::Stopped`.
pub fn run(
    source: String,
    options: &Options,
    output: &mut impl Write,
    cancellation: &Cancellation,
) -> Result<Report, RunError> {
    run_with_diagnostics(source, options, output, &mut io::sink(), cancellation)
}

/// Admit and stream models with a separate injected backend-diagnostics sink.
/// This is the process adapter's entry point: model output goes to stdout and
/// backend selection/fallback reasons go to stderr. [`run`] keeps its original
/// signature and discards backend diagnostics for callers that do not need them.
///
/// # Errors
/// Returns [`RunError`] for source, backend, observation/view, model-output, or
/// diagnostics failures. Search, objective, and incumbent-retention stops produce
/// an interrupted [`Report`] unless a later reporting operation fails.
/// Cooperative publication stops use the legacy `PublicationStopped` error adapter;
/// the finalized API retains them as `PublicationOutcome::Stopped`.
pub fn run_with_diagnostics(
    source: String,
    options: &Options,
    output: &mut impl Write,
    diagnostics: &mut impl Write,
    cancellation: &Cancellation,
) -> Result<Report, RunError> {
    run_detailed_with_diagnostics(source, options, output, diagnostics, cancellation)
        .map_err(RunFailure::into_cause)
}

/// Admit and stream models, retaining typed failures and partial execution evidence.
/// Like [`run`], this discards backend diagnostics but never failure metadata.
///
/// # Errors
/// Returns the original cause plus any available progress and attempted timings.
/// Search, objective, and incumbent-retention stops return an interrupted report;
/// observation/view or output failures return a failure with the retained evidence.
/// Cooperative publication stops use the legacy `PublicationStopped` adapter.
pub fn run_detailed(
    source: String,
    options: &Options,
    output: &mut impl Write,
    cancellation: &Cancellation,
) -> Result<Report, RunFailure> {
    run_detailed_with_diagnostics(source, options, output, &mut io::sink(), cancellation)
}

/// Stream models with separate diagnostics and retain structured failure evidence.
/// A failed Answer is excluded from the published count even if its prefix escaped.
/// Search, objective, and incumbent-retention stops return an interrupted [`Report`]
/// unless a later observation/view or reporting operation fails.
///
/// # Errors
/// Returns [`RunFailure`] for source, backend, observation/view, or output failures.
/// Cooperative publication stops use the legacy `PublicationStopped` error adapter,
/// with the preceding search evidence retained independently.
/// A secondary statistics-output error does not replace the original cause.
pub fn run_detailed_with_diagnostics(
    source: String,
    options: &Options,
    output: &mut impl Write,
    diagnostics: &mut impl Write,
    cancellation: &Cancellation,
) -> Result<Report, RunFailure> {
    run_finalized_with_diagnostics(source, options, output, diagnostics, cancellation)
        .and_then(crate::PublicationOutcome::into_legacy)
        .map(crate::PublicationReport::into_report)
        .map_err(crate::PublicationFailure::into_legacy)
}

/// Admit source and publish models, retaining semantic evidence independently.
/// Diagnostics are discarded; [`run_finalized_with_diagnostics`] retains an
/// explicit diagnostics sink. Existing [`run_detailed`] remains compatible.
///
/// # Errors
/// Returns actual execution, resource, encoding or writer failures. Cooperative
/// publication stops are successful `PublicationOutcome::Stopped` values.
pub fn run_finalized(
    source: String,
    options: &Options,
    output: &mut impl Write,
    cancellation: &Cancellation,
) -> Result<crate::PublicationOutcome, crate::PublicationFailure> {
    run_finalized_with_diagnostics(source, options, output, &mut io::sink(), cancellation)
}

/// Run with finalized semantic evidence independent of publication.
///
/// # Errors
/// Retains the original cause, semantic outcome, and separate reporting failures.
/// Cooperative publication stops are returned as `PublicationOutcome::Stopped`.
pub fn run_finalized_with_diagnostics(
    source: String,
    options: &Options,
    output: &mut impl Write,
    diagnostics: &mut impl Write,
    cancellation: &Cancellation,
) -> Result<crate::PublicationOutcome, crate::PublicationFailure> {
    let mut diagnostics = Diagnostics::new(diagnostics, options.color.human(options.json));
    run_source_with_writer(source, options, output, &mut diagnostics, cancellation)
}

pub(crate) fn run_source_with_writer(
    source: String,
    options: &Options,
    output: &mut impl Write,
    diagnostics: &mut Diagnostics<impl Write>,
    cancellation: &Cancellation,
) -> Result<crate::PublicationOutcome, crate::PublicationFailure> {
    let mut renderer = crate::view::builtin::Builtin::new(output, options);
    run_source_with_renderer(source, options, &mut renderer, diagnostics, cancellation)
}

/// Admit an original include graph through the extended source profile, then
/// stream models through the same solver as [`run_with_diagnostics`]. Global
/// constants and rule origins retain their original file identities. Loading
/// and its shared root/byte/file/depth limits belong to [`SourceBundle::load_many`].
///
/// Oracle selection follows the supported source constructs. String entry
/// points never resolve includes from implicit paths.
///
/// # Errors
/// Returns [`RunError`] for source, backend, observation/view, model-output, or
/// diagnostics failures. Search, objective, and incumbent-retention stops produce
/// an interrupted [`Report`] unless a later reporting operation fails.
/// Cooperative publication stops use the legacy `PublicationStopped` error adapter;
/// the finalized API retains them as `PublicationOutcome::Stopped`.
pub fn run_bundle_with_diagnostics(
    bundle: SourceBundle,
    options: &Options,
    output: &mut impl Write,
    diagnostics: &mut impl Write,
    cancellation: &Cancellation,
) -> Result<Report, RunError> {
    run_bundle_detailed_with_diagnostics(bundle, options, output, diagnostics, cancellation)
        .map_err(RunFailure::into_cause)
}

/// Original-bundle counterpart of [`run_detailed_with_diagnostics`].
/// Source identities, global constants and existing loading limits are preserved.
///
/// # Errors
/// Returns the original typed failure with any trustworthy execution evidence.
/// Search, objective, and incumbent-retention stops return an interrupted report;
/// observation/view or output failures return a failure with the retained evidence.
/// Cooperative publication stops use the legacy `PublicationStopped` adapter.
pub fn run_bundle_detailed_with_diagnostics(
    bundle: SourceBundle,
    options: &Options,
    output: &mut impl Write,
    diagnostics: &mut impl Write,
    cancellation: &Cancellation,
) -> Result<Report, RunFailure> {
    run_bundle_finalized_with_diagnostics(bundle, options, output, diagnostics, cancellation)
        .and_then(crate::PublicationOutcome::into_legacy)
        .map(crate::PublicationReport::into_report)
        .map_err(crate::PublicationFailure::into_legacy)
}

/// Run with finalized semantic evidence independent of publication.
///
/// # Errors
/// Retains the original cause, semantic outcome, and separate reporting failures.
/// Cooperative publication stops are returned as `PublicationOutcome::Stopped`.
pub fn run_bundle_finalized_with_diagnostics(
    bundle: SourceBundle,
    options: &Options,
    output: &mut impl Write,
    diagnostics: &mut impl Write,
    cancellation: &Cancellation,
) -> Result<crate::PublicationOutcome, crate::PublicationFailure> {
    let mut diagnostics = Diagnostics::new(diagnostics, options.color.human(options.json));
    run_bundle_with_writer(bundle, options, output, &mut diagnostics, cancellation)
}

pub(crate) fn run_bundle_with_writer(
    bundle: SourceBundle,
    options: &Options,
    output: &mut impl Write,
    diagnostics: &mut Diagnostics<impl Write>,
    cancellation: &Cancellation,
) -> Result<crate::PublicationOutcome, crate::PublicationFailure> {
    let mut renderer = crate::view::builtin::Builtin::new(output, options);
    run_bundle_renderer_inner(bundle, options, &mut renderer, diagnostics, cancellation)
}

/// Admit source and stream typed views into a caller-supplied renderer.
/// Renderer selection is independent of `options.json`; that legacy field only
/// selects the default renderer in the writer convenience APIs. Observation
/// limits, source/execution configuration and diagnostics remain explicit.
///
/// # Errors
/// Preserves source, solver, view and external publication failures, including
/// semantic progress independent of accepted record counts. Cooperative
/// publication stops return `PublicationOutcome::Stopped`.
pub fn run_with_renderer(
    source: String,
    options: &Options,
    renderer: &mut impl crate::AnswerRenderer,
    diagnostics: &mut impl Write,
    cancellation: &Cancellation,
) -> Result<crate::PublicationOutcome, crate::PublicationFailure> {
    let mut diagnostics = Diagnostics::new(diagnostics, options.color.human(options.json));
    run_source_with_renderer(source, options, renderer, &mut diagnostics, cancellation)
}

fn run_source_with_renderer(
    source: String,
    options: &Options,
    renderer: &mut impl crate::AnswerRenderer,
    diagnostics: &mut Diagnostics<impl Write>,
    cancellation: &Cancellation,
) -> Result<crate::PublicationOutcome, crate::PublicationFailure> {
    let mut invocation = crate::view::session::Session::start(renderer)?;
    let renderer = &mut invocation;
    let phases = Recorder::new(options.stats);
    let result = crate::admission::source(
        source,
        options,
        renderer,
        diagnostics,
        cancellation,
        &phases,
    );
    let result = report_progress_statistics(result, diagnostics, options, &phases);
    crate::publication::finalize(renderer, result)
}

/// Admit an original source bundle using the same replaceable typed view stream.
///
/// # Errors
/// Returns the same retained failures and cooperative publication outcomes as
/// [`run_with_renderer`], with original bundle locations retained.
pub fn run_bundle_with_renderer(
    bundle: SourceBundle,
    options: &Options,
    renderer: &mut impl crate::AnswerRenderer,
    diagnostics: &mut impl Write,
    cancellation: &Cancellation,
) -> Result<crate::PublicationOutcome, crate::PublicationFailure> {
    let mut diagnostics = Diagnostics::new(diagnostics, options.color.human(options.json));
    run_bundle_renderer_inner(bundle, options, renderer, &mut diagnostics, cancellation)
}

fn run_bundle_renderer_inner(
    bundle: SourceBundle,
    options: &Options,
    renderer: &mut impl crate::AnswerRenderer,
    diagnostics: &mut Diagnostics<impl Write>,
    cancellation: &Cancellation,
) -> Result<crate::PublicationOutcome, crate::PublicationFailure> {
    let mut invocation = crate::view::session::Session::start(renderer)?;
    let renderer = &mut invocation;
    let phases = Recorder::new(options.stats);
    let result = crate::admission::bundle(
        bundle,
        options,
        renderer,
        diagnostics,
        cancellation,
        &phases,
    );
    let result = report_progress_statistics(result, diagnostics, options, &phases);
    crate::publication::finalize(renderer, result)
}

/// Publish an already admitted input without reparsing or collecting its family.
/// The same session controller and renderer lifecycle serve source and prepared
/// inputs; semantic membership stays in `zetesis-solve`.
///
/// # Errors
/// Retains execution and publication evidence under the same contract as
/// [`run_with_renderer`]. No source admission is performed here.
pub fn publish_prepared(
    input: crate::PreparedInput<'_>,
    config: &crate::PublicationConfig,
    renderer: &mut impl crate::AnswerRenderer,
    diagnostics: &mut impl Write,
    cancellation: &Cancellation,
) -> Result<crate::PublicationOutcome, crate::PublicationFailure> {
    let mut diagnostics = Diagnostics::new(diagnostics, crate::ColorMode::Never);
    let mut invocation = crate::view::session::Session::start(renderer)?;
    let renderer = &mut invocation;
    let phases = Recorder::new(config.solve.stats);
    let mut result = crate::publication::solve(
        input,
        None,
        config,
        renderer,
        &mut diagnostics,
        cancellation,
        &phases,
    );
    if let Some(timings) = phases.snapshot() {
        match &mut result {
            Ok(progress) => progress.phase_timings = Some(timings),
            Err(failure) => failure.phase_timings = Some(Box::new(timings)),
        }
    }
    crate::publication::finalize(renderer, result)
}

fn report_progress_statistics(
    mut result: Result<Progress, crate::PublicationFailure>,
    diagnostics: &mut Diagnostics<impl Write>,
    options: &Options,
    phases: &Recorder,
) -> Result<Progress, crate::PublicationFailure> {
    if let Some(timings) = phases.snapshot() {
        match &mut result {
            Ok(progress) => progress.phase_timings = Some(timings),
            Err(failure) => failure.phase_timings = Some(Box::new(timings)),
        }
        let emitted = match options.statistics_view {
            crate::StatisticsView::Records => crate::statistics::write_progress(
                diagnostics,
                options,
                result.as_ref(),
                timings.driver_elapsed,
            )
            .and_then(|()| crate::stage_timing::write(diagnostics, &timings.stages))
            .and_then(|()| crate::phase_timing::write(diagnostics, &timings))
            .and_then(|()| crate::grounding_timing::write(diagnostics, &timings.grounding)),
            crate::StatisticsView::Human => {
                let view = crate::PublicationView {
                    result: result.as_ref(),
                };
                let config = crate::SolveConfig::from(options);
                let statistics = crate::statistics_view::Statistics {
                    requested: &config,
                    timings: &timings,
                    semantic: view.semantic(),
                    publication: view.publication(),
                    failed: view.failure().is_some(),
                };
                let layout = diagnostics.layout();
                statistics.write_human(diagnostics, layout)
            }
        };
        if let Err(error) = emitted {
            return Err(match result {
                Ok(progress) => progress.fail(RunError::Output(error)),
                Err(mut failure) => {
                    failure.record_diagnostics(error);
                    failure
                }
            });
        }
    }
    result
}

impl From<zetesis_solve::SolveError> for RunError {
    fn from(error: zetesis_solve::SolveError) -> Self {
        use zetesis_solve::SolveError;
        match error {
            SolveError::Reconstruction(error) => Self::Reconstruction(error),
            SolveError::TerminalStatisticsOverflow => Self::TerminalStatisticsOverflow,
            SolveError::Constraint(error) => Self::Constraint(error),
            SolveError::ConstraintFailureMissing => Self::ConstraintFailureMissing,
            SolveError::HybridStatisticsOverflow => Self::HybridStatisticsOverflow,
            SolveError::HybridBackend { backend } => Self::HybridBackend { backend },
            SolveError::Executor(error) => Self::Executor(error),
            SolveError::Projection(error) => Self::Projection(error),
            SolveError::Batch(error) => Self::Batch(error),
            SolveError::QueryObservation(error) => Self::QueryObservation(error),
            SolveError::CompletionPool(error) => Self::CompletionPool(error),
            SolveError::BackendUnavailable => Self::BackendUnavailable,
            SolveError::UnsupportedSourceBatching => Self::UnsupportedSourceBatching,
            SolveError::ClosureReservation {
                workers,
                max_closure_bytes,
                max_closure_batch_bytes,
            } => Self::ClosureReservation {
                workers,
                max_closure_bytes,
                max_closure_batch_bytes,
            },
            SolveError::Static(error) => Self::Static(error),
            SolveError::LazyStatisticsOverflow => Self::LazyStatisticsOverflow,
            SolveError::ClosureStatisticsOverflow => Self::ClosureStatisticsOverflow,
            SolveError::SharedCpu(error) => Self::SharedCpu(error),
            SolveError::Words(error) => Self::Words(error),
            SolveError::Model(error) => Self::Model(error),
            SolveError::CandidateStreamNotExhausted => Self::CandidateStreamNotExhausted,
            SolveError::UnsupportedOracle { backend, grounder } => {
                Self::UnsupportedOracle { backend, grounder }
            }
            SolveError::PreparedInput {
                profile,
                oracle,
                grounder,
            } => Self::PreparedInput {
                profile,
                oracle,
                grounder,
            },
            SolveError::FormulaBatchShape { expected, actual } => {
                Self::FormulaBatchShape { expected, actual }
            }
            #[cfg(feature = "gpu")]
            SolveError::Gpu(error) => Self::Gpu(error),
            #[cfg(feature = "gpu")]
            SolveError::LazyGpu(error) => Self::LazyGpu(error),
            SolveError::ExecutionObservation(error) => match error.downcast::<std::io::Error>() {
                Ok(error) => Self::Output(*error),
                Err(error) => Self::ExecutionObservation(error),
            },
        }
    }
}
