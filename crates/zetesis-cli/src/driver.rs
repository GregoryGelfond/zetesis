use crate::failure::Progress;
use crate::phase_timing::{Recorder, SolvePhase};
use crate::presentation::Diagnostics;
use crate::{Backend, Grounder, Options, RunFailure};
use std::fmt;
use std::io::{self, Write};
use zetesis_core::{Model, Program};
use zetesis_cpu::{BatchError, Control, Stop};
use zetesis_themelios::{
    AdmissionFailure, BundleAdmissionFailure, BundleError, ExpansionFailure, OutputSelection,
    SourceBundle,
};

/// Why a successful driver invocation stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Completion {
    /// Every relevant candidate was checked. With an objective, candidates
    /// worse than a verified incumbent may be omitted; all optimal ties remain
    /// covered. This is search coverage, independently of output delivery.
    /// UNSAT additionally requires no verified stable model.
    Exhausted,
    /// The requested number of models was returned; coverage remains partial.
    RequestedModels,
    /// Search or one oracle stopped with explicit incomplete coverage.
    Interrupted,
}

/// A typed reason why model enumeration could not establish complete coverage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interruption {
    /// Incremental candidate enumeration or a closure oracle stopped.
    Oracle(Stop),
    /// Reduct countermodel encoding, search or independent witness checking stopped.
    Countermodel(zetesis_sat::Incomplete),
    /// An objective could not be completely evaluated for a verified model.
    Objective(zetesis_objective::Error),
    /// Retaining complete incumbent models exceeded an explicit bound.
    Incumbent(crate::OptimizationStop),
}
impl fmt::Display for Interruption {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Oracle(error) => error.fmt(f),
            Self::Countermodel(error) => error.fmt(f),
            Self::Objective(error) => error.fmt(f),
            Self::Incumbent(error) => error.fmt(f),
        }
    }
}

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
    /// A static oracle returned an invalid dense closure representation.
    Words(zetesis_core::WordError),
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
                | Self::BundleLoad(_)
                | Self::BundleAdmission(_)
                | Self::FormulaAdmission(_)
                | Self::FormulaBundleAdmission(_)
        ) {
            f.write_str("source admission: ")?;
        }
        match self {
            Self::Input(error) => error.fmt(f),
            Self::MixedStandardInput => f.write_str(
                "standard input ('-') must be the only input; mixed or repeated stdin roots are unsupported",
            ),
            Self::Admission(error) => error.fmt(f),
            Self::Batch(error) => error.fmt(f),
            Self::CompletionPool(error) => write!(f, "completion worker pool: {error}"),
            Self::BundleLoad(error) => error.fmt(f),
            Self::BundleAdmission(error) => error.fmt(f),
            Self::Expansion(error) => error.fmt(f),
            Self::BackendUnavailable => f.write_str(
                "GPU support was not compiled; install the default build or enable --features gpu",
            ),
            Self::UnsupportedCombination { backend, grounder } => write!(
                f,
                "unsupported backend/grounding profile: {backend:?} with {}",
                grounder.label()
            ),
            Self::UnsupportedOracle { backend, grounder } => write!(
                f,
                "the countermodel oracle requires --grounder eager or auto; requested {backend:?} with {}",
                grounder.label()
            ),
            Self::PreparedInput { profile, oracle, grounder } => write!(f,
                "prepared {profile:?} cannot honor oracle {oracle:?} with grounder {grounder:?}"),
            Self::UnsupportedSourceBatching => f.write_str("shared source batching requires the relational closure route with lazy/auto grounding and cpu/auto backend"),
            Self::SharedCpu(cause) => cause.fmt(f),
            Self::Formula(error) => error.fmt(f),
            Self::FormulaAdmission(error) => error.fmt(f),
            Self::FormulaBundleAdmission(error) => error.fmt(f),
            Self::Output(error) => write!(f, "output: {error}"),
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
            Self::Words(error) => error.fmt(f),
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
            Self::PublicationStopped(error) => Some(error),
            Self::Observation(error) => Some(error),
            Self::JsonRecord(error) => Some(error),
            Self::ObservationOutputLimit { .. } => None,
            Self::MixedStandardInput
            | Self::BackendUnavailable
            | Self::UnsupportedCombination { .. }
            | Self::UnsupportedOracle { .. }
            | Self::UnsupportedSourceBatching
            | Self::LazyStatisticsOverflow
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
        .map(crate::SolveReport::into_report)
        .map_err(crate::SolveFailure::into_legacy)
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
) -> Result<crate::SolveReport, crate::SolveFailure> {
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
) -> Result<crate::SolveReport, crate::SolveFailure> {
    let mut diagnostics = Diagnostics::new(diagnostics, options.color.human(options.json));
    run_source_with_writer(source, options, output, &mut diagnostics, control)
}

pub(crate) fn run_source_with_writer(
    source: String,
    options: &Options,
    output: &mut impl Write,
    diagnostics: &mut Diagnostics<impl Write>,
    control: &Control,
) -> Result<crate::SolveReport, crate::SolveFailure> {
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
        .map(crate::SolveReport::into_report)
        .map_err(crate::SolveFailure::into_legacy)
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
) -> Result<crate::SolveReport, crate::SolveFailure> {
    let mut diagnostics = Diagnostics::new(diagnostics, options.color.human(options.json));
    run_bundle_with_writer(bundle, options, output, &mut diagnostics, control)
}

pub(crate) fn run_bundle_with_writer(
    bundle: SourceBundle,
    options: &Options,
    output: &mut impl Write,
    diagnostics: &mut Diagnostics<impl Write>,
    control: &Control,
) -> Result<crate::SolveReport, crate::SolveFailure> {
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

#[cfg(test)]
pub(crate) fn report_statistics(
    result: Result<Progress, crate::SolveFailure>,
    diagnostics: &mut impl Write,
    options: &Options,
    phases: &Recorder,
) -> Result<Report, RunFailure> {
    report_progress_statistics(result, diagnostics, options, phases)
        .map(|progress| progress.report)
        .map_err(crate::SolveFailure::into_legacy)
}

fn report_progress_statistics(
    mut result: Result<Progress, crate::SolveFailure>,
    diagnostics: &mut impl Write,
    options: &Options,
    phases: &Recorder,
) -> Result<Progress, crate::SolveFailure> {
    if let Some(timings) = phases.snapshot() {
        match &mut result {
            Ok(progress) => progress.report.phase_timings = Some(timings),
            Err(failure) => failure.phase_timings = Some(Box::new(timings)),
        }
        let emitted = crate::statistics::write_detailed(
            diagnostics,
            options,
            result.as_ref().map(|progress| &progress.report),
            timings.driver_elapsed,
        )
        .and_then(|()| crate::stage_timing::write(diagnostics, &timings.stages))
        .and_then(|()| crate::phase_timing::write(diagnostics, &timings))
        .and_then(|()| crate::grounding_timing::write(diagnostics, &timings.grounding));
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

pub(crate) fn solve_program(
    program: &Program,
    selection: &OutputSelection,
    options: &Options,
    output: &mut impl Write,
    diagnostics: &mut Diagnostics<impl Write>,
    control: &Control,
    phases: &Recorder,
) -> Result<Progress, crate::SolveFailure> {
    let _solving = phases.stage(crate::SolveStage::Solving);
    let config = options.into();
    let mut session = crate::closure_session::ClosureSession::new(
        program,
        None,
        &config,
        diagnostics,
        control,
        phases,
    )?;
    let mut progress = Progress::new(0);
    let observations = zetesis_themelios::observation::ObservationProgram::default();
    let display = crate::display::Display {
        selection,
        observations: &observations,
        options,
        control,
    };
    loop {
        let next = session.next(&config, diagnostics, control, phases);
        progress.apply(session.outcome());
        match next {
            Some(Ok(model)) => {
                let result = phases.measure(SolvePhase::ObservationOutput, || {
                    display.write(output, progress.report.models + 1, &model, None)
                });
                if let Err(error) = result {
                    return Err(progress.fail(error));
                }
                progress.report.models += 1;
            }
            Some(Err(error)) => return Err(progress.fail(error)),
            None => break,
        }
    }
    let finished = phases.measure(SolvePhase::ObservationOutput, || {
        finish(output, &progress.report, options.json, options.color)
    });
    match finished {
        Ok(()) => {
            progress.summary_published = !options.json;
            Ok(progress)
        }
        Err(error) => Err(progress.fail(error)),
    }
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
    report: &Report,
    json: bool,
    color: crate::ColorMode,
) -> Result<(), RunError> {
    // JSON emits one final outcome after statistics and failure accounting.
    if json {
        return Ok(());
    }
    match report.completion {
        Completion::Exhausted => {
            if report.models == 0 {
                color.status(output, "UNSATISFIABLE")?;
            } else if report.optimization.is_some() {
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
                report
                    .interruption
                    .as_ref()
                    .expect("interrupted report carries a reason")
            )?;
            writeln!(output, "Coverage: partial")?;
        }
    }
    if report.countermodel_statistics.is_some()
        || matches!(report.interruption, Some(Interruption::Countermodel(_)))
    {
        writeln!(
            output,
            "Models: {}; candidates examined: {}; gate tuples discovered: n/a (formula search)",
            report.models, report.checked
        )?;
    } else {
        let examined = if report.shared_execution.is_some() {
            "closure result/control records examined"
        } else {
            "candidates examined"
        };
        writeln!(
            output,
            "Models: {}; {examined}: {}; gate tuples discovered: {}",
            report.models, report.checked, report.discovered_gate_atoms
        )?;
    }
    Ok(())
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
