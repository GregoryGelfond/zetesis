use crate::failure::Progress;
use crate::phase_timing::Recorder;
use crate::presentation::Diagnostics;
use crate::{Backend, Completion, Grounder, Interruption, Options, RunFailure};
use std::fmt;
use std::io::{self, Write};
use zetesis_core::Model;
use zetesis_cpu::{BatchError, Control, Stop};
use zetesis_themelios::{
    AdmissionFailure, BundleAdmissionFailure, BundleError, ExpansionFailure, OutputSelection,
    SourceBundle,
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
    /// Necessary closure-candidate restrictions, including interrupted work.
    pub candidate_statistics: Option<zetesis_cpu::CandidateStatistics>,
    /// Cumulative countermodel accounting when available; absent for closure
    /// and for an initialization failure that returns no statistics.
    pub countermodel_statistics: Option<zetesis_sat::Statistics>,
    /// Actual batched formula execution and pending-result accounting; absent
    /// when the scalar CPU route was used or initialization did not finish.
    pub formula_execution: Option<crate::FormulaExecutionStatistics>,
    /// Actual lazy device execution, including shared source work and failed
    /// batch progress. Absent when no lazy device executor was initialized.
    pub lazy_execution: Option<crate::LazyExecutionStatistics>,
    /// Shared CPU source and per-world work, including failed-batch prefixes.
    pub shared_execution: Option<crate::SharedExecutionStatistics>,
    /// Best retained objective score and tied models found so far.
    /// Only exhausted coverage establishes that this incumbent is optimal.
    pub optimization: Option<crate::Optimization>,
    /// Opt-in attempted host timings; unavailable phases are absent.
    /// A timing snapshot does not establish semantic or output completion.
    pub phase_timings: Option<crate::PhaseTimings>,
}

/// A failed input, transport, or backend operation; never a claim of UNSAT.
#[derive(Debug)]
pub enum RunError {
    /// Reading a bounded standard-input source failed before semantic admission.
    Input(io::Error),
    /// The requested process deadline cannot be represented by the host clock.
    TimeLimitRange {
        /// Requested whole seconds, retained without an I/O attribution.
        seconds: u64,
    },
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
    /// Dedicated exact-completion pool construction failed.
    CompletionPool(rayon::ThreadPoolBuildError),
    /// Requested backend was not compiled into this binary.
    BackendUnavailable,
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
    /// Concrete shared CPU evaluation violated its round protocol.
    SharedCpu(zetesis_cpu::lazy::shared::Cause),
    /// Static closure decoding refused its words or selected-position storage.
    Words(zetesis_core::WordError),
    /// A verified interpretation could not retain its selected catalog atoms.
    Model(zetesis_core::ModelError),
    /// A driver requested a successful legacy report before search classified its stop.
    /// This protocol failure establishes neither interruption nor unsatisfiability.
    CompletionUnavailable,
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
            Self::Input(error) => write!(f, "standard input ('-'): {error}"),
            Self::TimeLimitRange { seconds } => write!(f, "--time-limit {seconds}: time limit exceeds the platform clock range"),
            Self::MixedStandardInput => f.write_str(
                "standard input ('-') must be the only input; mixed or repeated stdin roots are unsupported",
            ),
            Self::Admission(error) => error.fmt(f),
            Self::Batch(error) => error.fmt(f),
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
            Self::CompletionUnavailable => f.write_str("driver report requires established search completion"),
            Self::Words(error) => error.fmt(f),
            Self::Model(error) => error.fmt(f),
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
            Self::Input(error) => Some(error),
            Self::Admission(error) => Some(error),
            Self::Expansion(error) => Some(error),
            Self::BundleLoad(error) => Some(error),
            Self::BundleAdmission(error) => Some(error),
            Self::Batch(error) => Some(error),
            Self::CompletionPool(error) => Some(error),
            Self::Output(error) => Some(error),
            Self::ExecutionObservation(error) => Some(error.as_ref()),
            Self::PublicationStopped(error) => Some(error),
            Self::Observation(error) => Some(error),
            Self::JsonRecord(error) => Some(error),
            Self::ObservationOutputLimit { .. } | Self::TimeLimitRange { .. } => None,
            Self::MixedStandardInput
            | Self::BackendUnavailable
            | Self::UnsupportedCombination { .. }
            | Self::UnsupportedOracle { .. }
            | Self::UnsupportedSourceBatching
            | Self::LazyStatisticsOverflow
            | Self::CompletionUnavailable
            | Self::FormulaBatchShape { .. } => None,
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
/// [`Report`] unless a later reporting operation fails. Cancellation during
/// observation evaluation or JSON encoding returns a failure instead.
pub fn run(
    source: String,
    options: &Options,
    output: &mut impl Write,
    control: &Control,
) -> Result<Report, RunError> {
    run_with_diagnostics(source, options, output, &mut io::sink(), control)
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
/// Cancellation during observation evaluation or JSON encoding returns a failure.
pub fn run_with_diagnostics(
    source: String,
    options: &Options,
    output: &mut impl Write,
    diagnostics: &mut impl Write,
    control: &Control,
) -> Result<Report, RunError> {
    run_detailed_with_diagnostics(source, options, output, diagnostics, control)
        .map_err(RunFailure::into_cause)
}

/// Admit and stream models, retaining typed failures and partial execution evidence.
/// Like [`run`], this discards backend diagnostics but never failure metadata.
///
/// # Errors
/// Returns the original cause plus any available progress and attempted timings.
/// Search, objective, and incumbent-retention stops return an interrupted report;
/// observation/view or output failures, including their control refusals, return
/// a failure with the evidence retained before that operation failed.
pub fn run_detailed(
    source: String,
    options: &Options,
    output: &mut impl Write,
    control: &Control,
) -> Result<Report, RunFailure> {
    run_detailed_with_diagnostics(source, options, output, &mut io::sink(), control)
}

/// Stream models with separate diagnostics and retain structured failure evidence.
/// A failed Answer is excluded from the published count even if its prefix escaped.
/// Search, objective, and incumbent-retention stops return an interrupted [`Report`]
/// unless a later observation/view or reporting operation fails.
///
/// # Errors
/// Returns [`RunFailure`] for source, backend, observation/view, or output failures.
/// Cancellation during observation evaluation or JSON encoding is such a failure,
/// with any preceding search evidence retained independently.
/// A secondary statistics-output error does not replace the original cause.
pub fn run_detailed_with_diagnostics(
    source: String,
    options: &Options,
    output: &mut impl Write,
    diagnostics: &mut impl Write,
    control: &Control,
) -> Result<Report, RunFailure> {
    run_finalized_with_diagnostics(source, options, output, diagnostics, control)
        .map(crate::PublicationReport::into_report)
        .map_err(crate::PublicationFailure::into_legacy)
}

/// Admit source and publish models, retaining semantic evidence independently.
/// Diagnostics are discarded; [`run_finalized_with_diagnostics`] retains an
/// explicit diagnostics sink. Existing [`run_detailed`] remains compatible.
///
/// # Errors
/// Returns the typed cause, finalized available semantics and delivery evidence.
pub fn run_finalized(
    source: String,
    options: &Options,
    output: &mut impl Write,
    control: &Control,
) -> Result<crate::PublicationReport, crate::PublicationFailure> {
    run_finalized_with_diagnostics(source, options, output, &mut io::sink(), control)
}

/// Run with finalized semantic evidence independent of publication.
///
/// # Errors
/// Retains the original cause, semantic outcome, and separate reporting failures.
pub fn run_finalized_with_diagnostics(
    source: String,
    options: &Options,
    output: &mut impl Write,
    diagnostics: &mut impl Write,
    control: &Control,
) -> Result<crate::PublicationReport, crate::PublicationFailure> {
    let mut diagnostics = Diagnostics::new(diagnostics, options.color.human(options.json));
    run_source_with_writer(source, options, output, &mut diagnostics, control)
}

pub(crate) fn run_source_with_writer(
    source: String,
    options: &Options,
    output: &mut impl Write,
    diagnostics: &mut Diagnostics<impl Write>,
    control: &Control,
) -> Result<crate::PublicationReport, crate::PublicationFailure> {
    let mut document = crate::output::Document::new(output, options.json)?;
    let phases = Recorder::new(options.stats);
    let result = crate::admission::source(
        source,
        options,
        &mut document,
        diagnostics,
        control,
        &phases,
    );
    let result = report_progress_statistics(result, diagnostics, options, &phases);
    document.finish(result, options)
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
/// Cancellation during observation evaluation or JSON encoding returns a failure.
pub fn run_bundle_with_diagnostics(
    bundle: SourceBundle,
    options: &Options,
    output: &mut impl Write,
    diagnostics: &mut impl Write,
    control: &Control,
) -> Result<Report, RunError> {
    run_bundle_detailed_with_diagnostics(bundle, options, output, diagnostics, control)
        .map_err(RunFailure::into_cause)
}

/// Original-bundle counterpart of [`run_detailed_with_diagnostics`].
/// Source identities, global constants and existing loading limits are preserved.
///
/// # Errors
/// Returns the original typed failure with any trustworthy execution evidence.
/// Search, objective, and incumbent-retention stops return an interrupted report;
/// observation/view or output failures, including their control refusals, return
/// a failure with the evidence retained before that operation failed.
pub fn run_bundle_detailed_with_diagnostics(
    bundle: SourceBundle,
    options: &Options,
    output: &mut impl Write,
    diagnostics: &mut impl Write,
    control: &Control,
) -> Result<Report, RunFailure> {
    run_bundle_finalized_with_diagnostics(bundle, options, output, diagnostics, control)
        .map(crate::PublicationReport::into_report)
        .map_err(crate::PublicationFailure::into_legacy)
}

/// Run with finalized semantic evidence independent of publication.
///
/// # Errors
/// Retains the original cause, semantic outcome, and separate reporting failures.
pub fn run_bundle_finalized_with_diagnostics(
    bundle: SourceBundle,
    options: &Options,
    output: &mut impl Write,
    diagnostics: &mut impl Write,
    control: &Control,
) -> Result<crate::PublicationReport, crate::PublicationFailure> {
    let mut diagnostics = Diagnostics::new(diagnostics, options.color.human(options.json));
    run_bundle_with_writer(bundle, options, output, &mut diagnostics, control)
}

pub(crate) fn run_bundle_with_writer(
    bundle: SourceBundle,
    options: &Options,
    output: &mut impl Write,
    diagnostics: &mut Diagnostics<impl Write>,
    control: &Control,
) -> Result<crate::PublicationReport, crate::PublicationFailure> {
    let mut document = crate::output::Document::new(output, options.json)?;
    let phases = Recorder::new(options.stats);
    let result = crate::admission::bundle(
        bundle,
        options,
        &mut document,
        diagnostics,
        control,
        &phases,
    );
    let result = report_progress_statistics(result, diagnostics, options, &phases);
    document.finish(result, options)
}

fn report_progress_statistics(
    mut result: Result<Progress, crate::PublicationFailure>,
    diagnostics: &mut impl Write,
    options: &Options,
    phases: &Recorder,
) -> Result<Progress, crate::PublicationFailure> {
    if let Some(timings) = phases.snapshot() {
        match &mut result {
            Ok(progress) => progress.phase_timings = Some(timings),
            Err(failure) => failure.phase_timings = Some(Box::new(timings)),
        }
        let reported = result.and_then(|progress| match progress.report() {
            Ok(report) => Ok((progress, report)),
            Err(cause) => Err(progress.fail(cause)),
        });
        let emitted = crate::statistics::write_detailed(
            diagnostics,
            options,
            reported.as_ref().map(|(_, report)| report),
            timings.driver_elapsed,
        )
        .and_then(|()| crate::stage_timing::write(diagnostics, &timings.stages))
        .and_then(|()| crate::phase_timing::write(diagnostics, &timings))
        .and_then(|()| crate::grounding_timing::write(diagnostics, &timings.grounding));
        if let Err(error) = emitted {
            return Err(match reported {
                Ok((progress, _)) => progress.fail(RunError::Output(error)),
                Err(mut failure) => {
                    failure.record_diagnostics(error);
                    failure
                }
            });
        }
        result = reported.map(|(progress, _)| progress);
    }
    result
}

pub(crate) fn write_atoms(
    output: &mut impl Write,
    model: &Model,
    selection: &OutputSelection,
) -> io::Result<()> {
    for (index, atom) in model
        .atoms()
        .iter()
        .filter(|atom| selection.includes(atom))
        .enumerate()
    {
        if index != 0 {
            write!(output, " ")?;
        }
        if atom.predicate().sign() == zetesis_core::Sign::Negative {
            write!(output, "-")?;
        }
        write!(output, "{}", atom.predicate().name())?;
        if !atom.values().is_empty() {
            write!(output, "(")?;
            for (position, value) in atom.values().iter().enumerate() {
                if position != 0 {
                    write!(output, ",")?;
                }
                match value {
                    zetesis_core::Value::Infimum => write!(output, "#inf")?,
                    zetesis_core::Value::Supremum => write!(output, "#sup")?,
                    zetesis_core::Value::Number(number) => write!(output, "{number}")?,
                    zetesis_core::Value::Symbol(symbol) => write!(output, "{symbol}")?,
                    zetesis_core::Value::String(string) => write_string(output, string)?,
                    zetesis_core::Value::Structured(value) => write!(output, "{value}")?,
                }
            }
            write!(output, ")")?;
        }
    }
    writeln!(output)
}

fn write_string(output: &mut impl Write, value: &str) -> io::Result<()> {
    // The admitted clingo string dialect has exactly these three escapes.
    // Other admitted characters, including literal tabs, retain their bytes;
    // Rust Debug's \t and \u{...} spellings are not clingo string escapes.
    write!(output, "\"")?;
    for character in value.chars() {
        match character {
            '"' => write!(output, "\\\"")?,
            '\\' => write!(output, "\\\\")?,
            '\n' => write!(output, "\\n")?,
            other => write!(output, "{other}")?,
        }
    }
    write!(output, "\"")
}

pub(crate) fn finish(
    output: &mut impl Write,
    progress: &Progress,
    json: bool,
    color: crate::ColorMode,
) -> Result<(), RunError> {
    let semantic = progress.semantic().ok_or(RunError::CompletionUnavailable)?;
    let completion = progress.completion()?;
    // JSON emits one final outcome after statistics and failure accounting.
    if json {
        return Ok(());
    }
    match completion {
        Completion::Exhausted => {
            if semantic.unsatisfiable() {
                color.status(output, "UNSATISFIABLE")?;
            } else if semantic.optimum_proved() {
                writeln!(output, "OPTIMUM FOUND")?;
            } else {
                color.status(output, "SATISFIABLE")?;
            }
            writeln!(output, "Coverage: exhausted")?;
        }
        Completion::RequestedModels => {
            color.status(output, "SATISFIABLE")?;
            writeln!(output, "Coverage: partial (requested model count reached)")?;
        }
        Completion::Interrupted => {
            writeln!(
                output,
                "INCOMPLETE: {}",
                semantic
                    .interruption()
                    .expect("interrupted outcome carries a reason")
            )?;
            writeln!(output, "Coverage: partial")?;
        }
    }
    if semantic.countermodel_statistics().is_some()
        || matches!(semantic.interruption(), Some(Interruption::Countermodel(_)))
    {
        writeln!(
            output,
            "Models: {}; candidates examined: {}; gate tuples discovered: n/a (formula search)",
            progress.publication.models,
            semantic.candidate_progress()
        )?;
    } else {
        let examined = if semantic.shared_execution().is_some() {
            "closure result/control records examined"
        } else {
            "candidates examined"
        };
        writeln!(
            output,
            "Models: {}; {examined}: {}; gate tuples discovered: {}",
            progress.publication.models,
            semantic.candidate_progress(),
            semantic.discovered_gate_atoms()
        )?;
    }
    Ok(())
}

impl From<zetesis_solve::SolveError> for RunError {
    fn from(error: zetesis_solve::SolveError) -> Self {
        use zetesis_solve::SolveError;
        match error {
            SolveError::Batch(error) => Self::Batch(error),
            SolveError::CompletionPool(error) => Self::CompletionPool(error),
            SolveError::BackendUnavailable => Self::BackendUnavailable,
            SolveError::UnsupportedSourceBatching => Self::UnsupportedSourceBatching,
            SolveError::Static(error) => Self::Static(error),
            SolveError::LazyStatisticsOverflow => Self::LazyStatisticsOverflow,
            SolveError::SharedCpu(error) => Self::SharedCpu(error),
            SolveError::Words(error) => Self::Words(error),
            SolveError::Model(error) => Self::Model(error),
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

#[cfg(test)]
mod value_output_tests {
    use super::write_atoms;
    use zetesis_core::{Atom, Model, Predicate, Value};
    use zetesis_themelios::OutputSelection;

    #[test]
    fn extrema_and_their_quoted_spellings_print_as_distinct_terms() {
        let values = [
            Value::Infimum,
            Value::String("#inf".into()),
            Value::String("#sup".into()),
            Value::Supremum,
        ];
        let model = Model::new(
            values
                .into_iter()
                .map(|value| Atom::new(Predicate::new("p", 1).unwrap(), vec![value]).unwrap()),
        );
        let mut output = Vec::new();
        write_atoms(&mut output, &model, &OutputSelection::default()).unwrap();
        assert_eq!(
            String::from_utf8(output).unwrap(),
            "p(#inf) p(\"#inf\") p(\"#sup\") p(#sup)\n"
        );
    }
}
