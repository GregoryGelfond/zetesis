//! Streaming JSON document adapter over typed model views and driver outcomes.

mod arithmetic;
mod handovers;

use std::io::{self, Write};

use zetesis_themelios::observation::ViewError;

use crate::{Completion, Interruption, Options, PhaseTimings, RunError, RunFailure, SolvePhase};

pub(crate) struct Document<W> {
    sink: W,
    failed: bool,
}
impl<W: Write> Document<W> {
    pub(crate) const fn new(sink: W) -> Self {
        Self {
            sink,
            failed: false,
        }
    }

    pub(crate) const fn failed(&self) -> bool {
        self.failed
    }

    pub(crate) fn start(&mut self) {
        self.failed = false;
    }

    pub(crate) fn into_inner(self) -> W {
        self.sink
    }
}
impl<W: Write> Write for Document<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let result = self.sink.write(bytes);
        if result
            .as_ref()
            .is_err_and(|error| error.kind() != io::ErrorKind::Interrupted)
            || matches!(result, Ok(0)) && !bytes.is_empty()
        {
            self.failed = true;
        }
        result
    }
    fn flush(&mut self) -> io::Result<()> {
        let result = self.sink.flush();
        self.failed |= result.is_err();
        result
    }
}

/// Preflight the bounded UTF-8 record, then write it to the supplied sink.
/// The record spells the atoms the document has not spelled and refers to
/// every atom by its index in `atoms`, the document's table.
pub(crate) fn write_model_record(
    output: &mut impl Write,
    number: usize,
    view: &zetesis_themelios::observation::ModelView<'_>,
    atoms: &mut zetesis_themelios::observation::json::AtomTable,
    max_record_bytes: usize,
    cancellation: &zetesis_algebra::Cancellation,
) -> Result<(), RunError> {
    let prefix = format!(
        "{}{{\"number\":{number},\"model\":",
        if number == 1 { "" } else { "," }
    );
    let overhead = prefix.len() + 2;
    let maximum = max_record_bytes
        .checked_sub(overhead)
        .ok_or(RunError::JsonRecord(ViewError::Bytes))?;
    let record = view
        .record(atoms, model_limits(maximum), cancellation)
        .map_err(RunError::JsonRecord)?;
    cancellation
        .poll()
        .map_err(|error| RunError::JsonRecord(ViewError::Stopped(error)))?;
    // All semantic/encoding failures precede publication. I/O can still fail
    // after a prefix; the caller increments its count only after all writes.
    output.write_all(prefix.as_bytes())?;
    output.write_all(record.as_bytes())?;
    output.write_all(b"}\n")?;
    Ok(())
}

// Record storage and traversal depth remain bounded. Ordinary encoding counts
// work up to its representation and observes the supplied cancellation token.
fn model_limits(max_bytes: usize) -> zetesis_themelios::observation::ViewLimits {
    zetesis_themelios::observation::ViewLimits {
        max_bytes,
        max_work: u64::MAX,
        ..Default::default()
    }
}

/// JSON for failures that precede the ordinary source/bundle driver entry.
pub(crate) fn input_failure(
    output: &mut impl Write,
    failure: RunFailure,
    options: &Options,
) -> RunFailure {
    use crate::AnswerRenderer;
    let mut renderer = crate::JsonRenderer::new(
        output,
        options.resources().json_record_bytes(),
        options.resources().formula_limits().theory.max_atoms,
    );
    match renderer.begin() {
        Ok(()) => crate::publication::finalize(&mut renderer, Err(failure.into()))
            .expect_err("input failure remains failed")
            .into_legacy(),
        Err(output_failure) => {
            let mut failure = failure;
            failure.secondary_output = Some(io::Error::other(output_failure));
            failure
        }
    }
}

fn completion(value: Completion) -> &'static str {
    match value {
        Completion::Exhausted => "exhausted",
        Completion::RequestedModels => "requested_models",
        Completion::Interrupted => "interrupted",
    }
}

pub(crate) fn summary(
    publication: crate::PublicationView<'_>,
    maximum: usize,
) -> Result<Vec<u8>, RunError> {
    let result = publication.result;
    let status = match result {
        Err(_) => "failed",
        Ok(progress) if progress.stop.is_some() => "incomplete",
        Ok(progress) => {
            let semantic = progress.semantic().ok_or(RunError::CompletionUnavailable)?;
            match progress.completion()? {
                Completion::Interrupted => "incomplete",
                Completion::Exhausted if semantic.unsatisfiable() => "unsatisfiable",
                _ => "satisfiable",
            }
        }
    };
    let mut out = Buffer::new(maximum);
    let view = SummaryView::new(publication);
    out.text("],\"outcome\":{\"status\":")?;
    out.string(status)?;
    out.text(",\"completion\":")?;
    out.optional_string(view.completion.map(completion))?;
    out.text(",\"coverage\":")?;
    out.string(match view.completion {
        Some(Completion::Exhausted) => "exhausted",
        Some(_) => "partial",
        None => "unavailable",
    })?;
    out.number_field("published_models", view.published_models)?;
    out.text(",\"verified_models\":")?;
    out.optional_number(view.verified_models)?;
    out.text(",\"checked\":")?;
    out.optional_number(view.checked)?;
    out.text(",\"projection\":")?;
    write_projection(&mut out, view.projection)?;
    out.text(",\"interruption\":")?;
    write_interruption(&mut out, view.interruption)?;
    out.text(",\"publication_stop\":")?;
    let stop = match result {
        Ok(progress) => progress.stop.as_ref(),
        Err(failure) => failure.publication_stop(),
    };
    if let Some(stop) = stop {
        out.text("{\"phase\":")?;
        out.string(match stop.phase() {
            crate::PublicationPhase::Observation => "observation",
            crate::PublicationPhase::Encoding => "encoding",
            crate::PublicationPhase::RecordPreparation => "record_preparation",
        })?;
        out.text(",\"code\":")?;
        out.string(stop_code(stop.reason()))?;
        out.text("}")?;
    } else {
        out.text("null")?;
    }
    out.text(",\"optimization\":")?;
    write_optimization(&mut out, view.optimization, view.optimum_proved)?;
    out.text(",\"error\":")?;
    if let Err(failure) = result {
        out.text("{\"kind\":")?;
        out.string(error_kind(&failure.cause))?;
        out.text(",\"detail\":")?;
        out.display_string(&failure.cause)?;
        out.text(",\"secondary_output_failure\":")?;
        out.text(if failure.secondary_output.is_some() {
            "true"
        } else {
            "false"
        })?;
        out.text("}")?;
    } else {
        out.text("null")?;
    }
    out.text("},\"statistics\":")?;
    statistics(&mut out, &view)?;
    out.text("}")?;
    Ok(out.bytes)
}

fn phases(out: &mut Buffer, timings: Option<&PhaseTimings>) -> Result<(), RunError> {
    let Some(timings) = timings else {
        return out.text("null");
    };
    out.text(&format!(
        "{{\"schema\":7,\"clock\":\"host_monotonic\",\"driver_elapsed_ns\":{},\"measurements\":{{",
        timings.driver_elapsed.as_nanos()
    ))?;
    for (index, phase) in SolvePhase::ALL.into_iter().enumerate() {
        if index != 0 {
            out.text(",")?;
        }
        out.string(phase.label())?;
        out.text(":")?;
        if let Some(value) = timings.get(phase) {
            out.text(&format!(
                "{{\"calls\":{},\"elapsed_ns\":{},\"complete\":{}}}",
                value.calls,
                value.elapsed.as_nanos(),
                !value.overflowed
            ))?;
        } else {
            out.text("null")?;
        }
    }
    out.text("},\"source_loading\":\"excluded\",\"statistics_output\":\"excluded\",\"json_envelope\":\"excluded\",\"kernel_time\":null}")
}

fn stages(out: &mut Buffer, timings: Option<&crate::StageTimings>) -> Result<(), RunError> {
    let Some(timings) = timings else {
        return out.text("null");
    };
    out.text("{\"schema\":1,\"clock\":\"host_monotonic\",\"scope\":\"driver\"")?;
    out.number_field("driver_elapsed_ns", timings.driver_elapsed.as_nanos())?;
    out.text(",\"grounding_mode\":")?;
    out.string(timings.grounding_mode.label())?;
    out.text(",\"measurements\":{")?;
    for (index, stage) in crate::SolveStage::ALL.into_iter().enumerate() {
        if index != 0 {
            out.text(",")?;
        }
        out.string(stage.label())?;
        out.text(":")?;
        if let Some(value) = timings.get(stage) {
            out.text("{\"calls\":")?;
            out.text(&value.calls.to_string())?;
            out.number_field("elapsed_ns", value.elapsed.as_nanos())?;
            out.text(",\"complete\":")?;
            out.text(if value.overflowed { "false" } else { "true" })?;
            out.text("}")?;
        } else {
            out.text("null")?;
        }
    }
    out.text("},\"unattributed_elapsed_ns\":")?;
    if let Some(value) = timings.unattributed {
        out.text(&value.as_nanos().to_string())?;
    } else {
        out.text("null")?;
    }
    out.text(",\"complete\":")?;
    out.text(if timings.is_complete() {
        "true"
    } else {
        "false"
    })?;
    out.text(",\"timer_overhead\":\"not_separated\",\"source_loading\":\"excluded\",\"statistics_output\":\"excluded\",\"json_envelope\":\"excluded\",\"kernel_time\":null}")
}

fn error_kind(error: &RunError) -> &'static str {
    match error {
        RunError::Reconstruction(error) => reconstruction_kind(error),
        RunError::TerminalStatisticsOverflow => "terminal_statistics_overflow",
        RunError::Constraint(_) => "constraint",
        RunError::ConstraintFailureMissing => "constraint_failure_missing",
        RunError::HybridStatisticsOverflow => "hybrid_statistics_overflow",
        RunError::Input(_) => "input",
        RunError::TimeLimitRange { .. } => "time_limit_range",
        RunError::DeadlineTimer(_) => "deadline_timer",
        RunError::Observation(_) => "observation",
        RunError::Projection(_) => "answer_projection",
        RunError::JsonRecord(_) => "json_record",
        RunError::ObservationOutputLimit { .. } => "observation_output_limit",
        RunError::MixedStandardInput => "mixed_standard_input",
        RunError::BundleLoad(_) => "bundle_load",
        RunError::CompletionPool(_) => "completion_pool",
        RunError::CompletionUnavailable => "completion_unavailable",
        RunError::BackendUnavailable => "backend_unavailable",
        RunError::GpuExecutableUnavailable { .. } => "gpu_executable_unavailable",
        RunError::Executor(_) => "executor",
        RunError::UnsupportedCombination { .. } => "unsupported_combination",
        RunError::UnsupportedOracle { .. } => "unsupported_oracle",
        RunError::PreparedInput { .. } => "prepared_input",
        RunError::FormulaAdmission(_) => "formula_admission",
        RunError::FormulaBundleAdmission(_) => "formula_bundle_admission",
        RunError::Output(_) => "output",
        RunError::ExecutionObservation(_) => "execution_observation",
        RunError::PublicationStopped(_) => "publication_stopped",
        RunError::Model(_) => "model",
        RunError::FormulaBatchShape { .. } => "formula_batch_shape",
        RunError::CandidateStreamNotExhausted => "candidate_stream_not_exhausted",
        #[cfg(feature = "gpu")]
        RunError::Gpu(_) => "gpu",
        RunError::ModelStatisticsOverflow => "model_statistics_overflow",
    }
}

fn reconstruction_kind(error: &zetesis_themelios::ReconstructionError) -> &'static str {
    use zetesis_core::{ModelError, ModelFailure};
    use zetesis_themelios::{FormulaFailure, ReconstructionError};
    match error {
        ReconstructionError::Source(error)
        | ReconstructionError::Model(ModelFailure::Stopped(error))
            if matches!(error.as_ref(), FormulaFailure::Limit { .. }) =>
        {
            "answer_reconstruction_limit"
        }
        ReconstructionError::Model(ModelFailure::Model(ModelError::Bytes { .. })) => {
            "answer_reconstruction_limit"
        }
        _ => "answer_reconstruction",
    }
}

fn reason_code(reason: Interruption) -> &'static str {
    match reason {
        Interruption::ModelConstruction(reason) => match reason {
            zetesis_solve::ModelConstructionStop::Control(stop) => stop_code(stop),
            zetesis_solve::ModelConstructionStop::Work { .. } => "work_limit",
            zetesis_solve::ModelConstructionStop::Bytes { .. } => "bytes_limit",
        },
        Interruption::Preparation(reason)
        | Interruption::Oracle(reason)
        | Interruption::Constraint(reason)
        | Interruption::Reconstruction(reason) => stop_code(reason),
        Interruption::Countermodel(reason) => {
            use zetesis_stable::Incomplete;
            match reason {
                Incomplete::Cancelled => "cancelled",
                Incomplete::Deadline => "deadline",
                Incomplete::WorkLimit => "work_limit",
                Incomplete::DecisionLimit => "decision_limit",
                Incomplete::CandidateLimit => "candidate_limit",
                Incomplete::PendingBytes => "pending_bytes",
                Incomplete::CompletionScratch => "completion_scratch",
                Incomplete::ReductStorage { .. } => "reduct_storage",
                Incomplete::NativeNarrowingStorage { .. } => "native_narrowing_storage",
                Incomplete::BatchCandidateLimit => "batch_candidate_limit",
                Incomplete::PendingBatch => "pending_batch",
                Incomplete::Allocation => "allocation",
                Incomplete::WorkerPanicked => "worker_panicked",
                Incomplete::WrongTheory => "wrong_theory",
                Incomplete::RestrictionUniverse { .. } => "restriction_universe",
                Incomplete::ClosedEnumerator => "closed_enumerator",
                Incomplete::LateCertificate => "late_certificate",
                Incomplete::RegionFilter => "region_filter",
                Incomplete::InvalidRegionConsequence => "invalid_region_consequence",
                Incomplete::LateRegionFilter => "late_region_filter",
                Incomplete::LateCandidateBounds => "late_candidate_bounds",
                Incomplete::CandidateBoundPlan => "candidate_bound_plan",
                Incomplete::CandidateBoundNotImproving => "candidate_bound_not_improving",
                Incomplete::RegionFilterAlreadySet => "region_filter_already_set",
                Incomplete::LateLearnedStore => "late_learned_store",
                Incomplete::LearnedStoreAlreadySet => "learned_store_already_set",
                Incomplete::Certificate(_) => "certificate",
                Incomplete::ArithmeticSource(_) => "arithmetic_source",
                Incomplete::Verification(_) => "verification",
                Incomplete::InvalidWitness => "invalid_witness",
                Incomplete::CounterOverflow => "counter_overflow",
            }
        }
        Interruption::Objective(error) => objective_code(error),
        Interruption::PreparedObjective(error) => {
            use zetesis_themelios::objective_bound::{
                ObjectiveBoundErrorKind, ObjectiveScoreErrorKind,
            };
            match error.kind() {
                ObjectiveScoreErrorKind::WrongTheory => "wrong_theory",
                ObjectiveScoreErrorKind::Reduction(error) => objective_code(error),
                ObjectiveScoreErrorKind::Eligibility(error) => match error.kind() {
                    ObjectiveBoundErrorKind::Control(stop) => stop_code(stop),
                    ObjectiveBoundErrorKind::Limit(_) => "work_limit",
                    ObjectiveBoundErrorKind::Allocation => "allocation",
                    ObjectiveBoundErrorKind::Overflow => "arithmetic_overflow",
                    _ => "objective_preparation",
                },
            }
        }
        Interruption::Incumbent(reason) => match reason {
            crate::OptimizationStop::Models => "models",
            crate::OptimizationStop::Atoms => "atoms",
            crate::OptimizationStop::Bytes => "bytes",
            crate::OptimizationStop::Overflow => "overflow",
            crate::OptimizationStop::Allocation => "allocation",
        },
    }
}

fn objective_code(error: zetesis_objective::Error) -> &'static str {
    use zetesis_objective::{ErrorKind, Stop};
    match error.kind() {
        ErrorKind::WeightNormalizationOverflow => "weight_normalization_overflow",
        ErrorKind::CostOverflow { .. } => "cost_overflow",
        ErrorKind::UnboundVariable { .. } => "unbound_variable",
        ErrorKind::Stopped(reason) => match reason {
            Stop::Cancelled => "cancelled",
            Stop::Deadline => "deadline",
            Stop::WorkLimit => "work_limit",
            Stop::BindingLimit => "binding_limit",
            Stop::KeyLimit => "key_limit",
            Stop::KeyBytesLimit => "key_bytes_limit",
            Stop::Allocation => "allocation",
            Stop::ArithmeticOverflow => "arithmetic_overflow",
            Stop::Control(_) => "control",
        },
    }
}

fn stop_code(reason: zetesis_algebra::Stop) -> &'static str {
    use zetesis_algebra::Stop;
    match reason {
        Stop::Cancelled => "cancelled",
        Stop::Deadline => "deadline",
        Stop::WorkLimit => "work_limit",
        Stop::RoundLimit => "round_limit",
        Stop::StorageLimit => "storage_limit",
        Stop::DerivedAtomLimit => "derived_atom_limit",
        Stop::CandidateLimit => "candidate_limit",
        Stop::CarrierLimit => "carrier_limit",
        Stop::Allocation => "allocation",
        Stop::WrongProgram => "wrong_program",
        Stop::InvalidProgram => "invalid_program",
    }
}

struct Buffer {
    bytes: Vec<u8>,
    maximum: usize,
    refusal: Option<ViewError>,
}
impl Buffer {
    fn new(maximum: usize) -> Self {
        Self {
            bytes: Vec::new(),
            maximum,
            refusal: None,
        }
    }
    fn text(&mut self, value: &str) -> Result<(), RunError> {
        self.write_all(value.as_bytes())
            .map_err(|error| self.error(error))
    }
    fn string(&mut self, value: &str) -> Result<(), RunError> {
        self.json(value)
    }
    fn json<T: serde::Serialize + ?Sized>(&mut self, value: &T) -> Result<(), RunError> {
        serde_json::to_writer(&mut *self, value)
            .map_err(|error| self.error(io::Error::other(error)))
    }
    /// Escape display fragments directly into the bounded record buffer.
    /// `collect_str` stops formatting as soon as this writer refuses a fragment.
    fn display_string(&mut self, value: &impl std::fmt::Display) -> Result<(), RunError> {
        use serde::Serializer as _;
        serde_json::Serializer::new(&mut *self)
            .collect_str(value)
            .map_err(|error| self.error(io::Error::other(error)))
    }
    fn number_field(&mut self, key: &str, value: impl std::fmt::Display) -> Result<(), RunError> {
        self.text(",")?;
        self.string(key)?;
        self.text(":")?;
        self.text(&value.to_string())
    }
    fn optional_string(&mut self, value: Option<&str>) -> Result<(), RunError> {
        if let Some(value) = value {
            self.string(value)
        } else {
            self.text("null")
        }
    }
    fn optional_number(&mut self, value: Option<u64>) -> Result<(), RunError> {
        if let Some(value) = value {
            self.text(&value.to_string())
        } else {
            self.text("null")
        }
    }
    fn error(&self, error: io::Error) -> RunError {
        self.refusal
            .map_or(RunError::Output(error), RunError::JsonRecord)
    }
}
impl Write for Buffer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let needed = self
            .bytes
            .len()
            .checked_add(bytes.len())
            .filter(|needed| *needed <= self.maximum);
        let Some(needed) = needed else {
            self.refusal = Some(ViewError::Bytes);
            return Err(io::Error::other("JSON record byte ceiling"));
        };
        if needed > self.bytes.capacity() {
            let capacity = needed
                .max(self.bytes.capacity().saturating_mul(2))
                .min(self.maximum);
            if self
                .bytes
                .try_reserve_exact(capacity - self.bytes.len())
                .is_err()
            {
                self.refusal = Some(ViewError::Allocation);
                return Err(io::Error::other("JSON record allocation"));
            }
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn write_interruption(
    out: &mut Buffer,
    interruption: Option<Interruption>,
) -> Result<(), RunError> {
    if let Some(reason) = interruption {
        let kind = match reason {
            Interruption::Preparation(_) => "preparation",
            Interruption::Oracle(_) => "oracle",
            Interruption::Countermodel(_) => "countermodel",
            Interruption::Constraint(_) => "constraint",
            Interruption::Reconstruction(_) => "answer_reconstruction",
            Interruption::ModelConstruction(_) => "model_construction",
            Interruption::Objective(_) => "objective",
            Interruption::PreparedObjective(_) => "prepared_objective",
            Interruption::Incumbent(_) => "incumbent",
        };
        out.text("{\"kind\":")?;
        out.string(kind)?;
        out.text(",\"code\":")?;
        out.string(reason_code(reason))?;
        out.text(",\"detail\":")?;
        out.display_string(&reason)?;
        if let Interruption::ModelConstruction(stop) = reason {
            match stop {
                zetesis_solve::ModelConstructionStop::Work { observed, limit } => {
                    out.text(",\"resource\":\"work\"")?;
                    out.number_field("required", observed)?;
                    out.number_field("limit", limit)?;
                }
                zetesis_solve::ModelConstructionStop::Bytes { required, limit } => {
                    out.text(",\"resource\":\"bytes\"")?;
                    out.number_field("required", required)?;
                    out.number_field("limit", limit)?;
                }
                zetesis_solve::ModelConstructionStop::Control(_) => {}
            }
        }
        out.text("}")?;
    } else {
        out.text("null")?;
    }
    Ok(())
}

fn statistics(out: &mut Buffer, view: &SummaryView<'_>) -> Result<(), RunError> {
    // Counts are typed and optional; this is a bounded fixed-shape view, not a
    // serialization of debug text or the human statistics protocol.
    if view.timings.is_some() {
        out.text("{\"executable\":")?;
        match crate::statistics::running_executable() {
            Some(path) => out.string(&path.to_string_lossy())?,
            None => out.text("null")?,
        }
        out.text(",\"search\":")?;
        search_statistics(
            out,
            view.search,
            if let Some(terminal) = view.terminal_execution {
                match terminal.base {
                    zetesis_themelios::BaseKind::Eager => "terminal_base",
                    // The search ran over the hybrid base's producer core.
                    zetesis_themelios::BaseKind::Hybrid => "terminal_retained_core",
                }
            } else if view.hybrid_execution.is_some() {
                "retained_core"
            } else {
                "original_theory"
            },
        )?;
        out.text(",\"hybrid_execution\":")?;
        hybrid_statistics(out, view.hybrid_execution)?;
        out.text(",\"terminal_execution\":")?;
        terminal_statistics(out, view.terminal_execution)?;
        out.text(",\"source_evaluation\":")?;
        source_evaluation_statistics(out, view.source_evaluation)?;
        out.text(",\"objective_work\":")?;
        out.optional_number(view.objective_work)?;
        out.text(",\"model_construction\":")?;
        model_construction_statistics(out, view.model_construction)?;
        out.text(",\"execution\":")?;
        execution_statistics(out, view.execution)?;
        out.text(",\"phase_timings\":")?;
        phases(out, view.timings)?;
        out.text(",\"stage_timings\":")?;
        stages(out, view.timings.map(|timing| &timing.stages))?;
        out.text(",\"grounding_attribution\":")?;
        grounding(out, view.timings.map(|timing| &timing.grounding))?;
        out.text("}")?;
    } else {
        out.text("null")?;
    }
    Ok(())
}

fn source_evaluation_statistics(
    out: &mut Buffer,
    statistics: Option<&zetesis_themelios::SourceEvaluationStatistics>,
) -> Result<(), RunError> {
    let Some(statistics) = statistics else {
        return out.text("null");
    };
    out.text("{\"scope\":\"source_admission\"")?;
    out.number_field("work", statistics.work)?;
    out.number_field("substitutions", statistics.substitutions)?;
    out.text("}")
}

fn model_construction_statistics(
    out: &mut Buffer,
    statistics: Option<&zetesis_solve::ModelConstructionStatistics>,
) -> Result<(), RunError> {
    let Some(statistics) = statistics else {
        return out.text("null");
    };
    out.text("{\"work\":")?;
    out.text(&statistics.work.to_string())?;
    out.number_field("prepared_bytes", statistics.prepared_bytes)?;
    out.number_field("peak_bytes", statistics.peak_bytes)?;
    out.number_field("constructed", statistics.constructed)?;
    out.text("}")
}

fn grounding(out: &mut Buffer, timings: Option<&crate::GroundingTimings>) -> Result<(), RunError> {
    let Some(timings) = timings else {
        return out.text("null");
    };
    out.text(
        "{\"schema\":1,\"clock\":\"host_monotonic\",\"scope\":\"eager_formula\",\"measurements\":{",
    )?;
    for (index, phase) in crate::GroundingPhase::ALL.into_iter().enumerate() {
        if index != 0 {
            out.text(",")?;
        }
        out.string(phase.label())?;
        out.text(":")?;
        if let Some(value) = timings.get(phase) {
            out.text("{\"elapsed_ns\":")?;
            optional_number(out, value.elapsed.map(|duration| duration.as_nanos()))?;
            out.text(",\"outcomes\":{")?;
            for (index, outcome) in crate::GroundingOutcome::ALL.into_iter().enumerate() {
                if index != 0 {
                    out.text(",")?;
                }
                out.string(outcome.label())?;
                out.text(":")?;
                optional_number(out, value.count(outcome).map(u128::from))?;
            }
            out.text("},\"work\":{")?;
            for (index, (name, count)) in crate::grounding_timing::work_fields(&value.work)
                .into_iter()
                .enumerate()
            {
                if index != 0 {
                    out.text(",")?;
                }
                out.string(name)?;
                out.text(":")?;
                optional_number(out, count.map(u128::from))?;
            }
            out.text("}}")?;
        } else {
            out.text("null")?;
        }
    }
    out.text("},\"overhead\":\"included\",\"relational_grounding\":null,\"kernel_time\":null}")
}

fn optional_number(out: &mut Buffer, value: Option<u128>) -> Result<(), RunError> {
    match value {
        Some(value) => out.text(&value.to_string()),
        None => out.text("null"),
    }
}

fn write_projection(
    out: &mut Buffer,
    projection: Option<&zetesis_solve::ProjectionStatistics>,
) -> Result<(), RunError> {
    let Some(projection) = projection else {
        return out.text("null");
    };
    out.text(&format!("{{\"complete\":{}", projection.complete))?;
    out.number_field("representatives", projection.representatives)?;
    out.number_field("duplicates", projection.duplicates)?;
    out.number_field("work", projection.work)?;
    out.number_field("retained_bytes", projection.retained_bytes)?;
    out.number_field("peak_bytes", projection.peak_bytes)?;
    out.text("}")
}

fn write_optimization(
    out: &mut Buffer,
    optimization: Option<&crate::Optimization>,
    optimum_proved: bool,
) -> Result<(), RunError> {
    if let Some(best) = optimization {
        // A strict bound at the optimum leaves the number of its ties unknown.
        let tied_models = if best.ties_pruned {
            "null".to_owned()
        } else {
            best.tied_models.to_string()
        };
        out.text(&format!(
            "{{\"optimal\":{},\"tied_models\":{},\"scored_models\":{},\"work\":{},\"costs\":[",
            optimum_proved, tied_models, best.scored_models, best.work
        ))?;
        for (index, (priority, value)) in best.score.costs().iter().enumerate() {
            if index != 0 {
                out.text(",")?;
            }
            out.text(&format!("{{\"priority\":{priority},\"value\":{value}}}"))?;
        }
        out.text("]}")?;
    } else {
        out.text("null")?;
    }
    Ok(())
}

// Borrow the retained semantic evidence; publication does not manufacture it.
struct SummaryView<'a> {
    projection: Option<&'a zetesis_solve::ProjectionStatistics>,
    completion: Option<Completion>,
    optimum_proved: bool,
    published_models: usize,
    verified_models: Option<u64>,
    checked: Option<u64>,
    interruption: Option<Interruption>,
    optimization: Option<&'a crate::Optimization>,
    objective_work: Option<u64>,
    search: Option<&'a zetesis_stable::Statistics>,

    execution: Option<&'a crate::FormulaExecutionStatistics>,
    hybrid_execution: Option<&'a zetesis_solve::HybridExecutionStatistics>,
    terminal_execution: Option<&'a zetesis_solve::TerminalExecutionStatistics>,
    source_evaluation: Option<&'a zetesis_themelios::SourceEvaluationStatistics>,
    model_construction: Option<&'a zetesis_solve::ModelConstructionStatistics>,

    timings: Option<&'a PhaseTimings>,
}
impl<'a> SummaryView<'a> {
    fn new(publication: crate::PublicationView<'a>) -> Self {
        match publication.result {
            Ok(progress) => {
                let semantic = progress.semantic();
                Self {
                    projection: semantic.and_then(crate::SemanticOutcome::projection),
                    completion: semantic.and_then(crate::SemanticOutcome::completion),
                    optimum_proved: semantic.is_some_and(crate::SemanticOutcome::optimum_proved),
                    published_models: progress.publication.models,
                    verified_models: semantic.map(crate::SemanticOutcome::verified_models),
                    checked: semantic.map(crate::SemanticOutcome::candidate_progress),
                    interruption: semantic.and_then(crate::SemanticOutcome::interruption),
                    optimization: semantic.and_then(crate::SemanticOutcome::incumbent),
                    objective_work: semantic.map(crate::SemanticOutcome::objective_work),
                    search: semantic.and_then(crate::SemanticOutcome::countermodel_statistics),
                    execution: semantic.and_then(crate::SemanticOutcome::formula_execution),
                    hybrid_execution: semantic.and_then(crate::SemanticOutcome::hybrid_execution),
                    terminal_execution: semantic
                        .and_then(crate::SemanticOutcome::terminal_execution),
                    source_evaluation: semantic.and_then(crate::SemanticOutcome::source_evaluation),
                    model_construction: semantic
                        .and_then(crate::SemanticOutcome::model_construction),
                    timings: progress.phase_timings.as_ref(),
                }
            }
            Err(failure) => {
                let partial = failure.partial_report.as_deref();
                Self {
                    projection: failure
                        .semantic()
                        .and_then(crate::SemanticOutcome::projection),
                    completion: partial.and_then(|p| p.completion),
                    optimum_proved: failure
                        .semantic()
                        .is_some_and(crate::SemanticOutcome::optimum_proved),
                    published_models: partial.map_or(0, |p| p.published_models),
                    verified_models: partial.map(|p| p.verified_models),
                    checked: partial.map(|p| p.checked),
                    interruption: partial.and_then(|p| p.interruption),
                    optimization: partial.and_then(|p| p.optimization.as_ref()),
                    objective_work: failure
                        .semantic()
                        .map(crate::SemanticOutcome::objective_work),
                    search: partial.and_then(|p| p.countermodel_statistics.as_ref()),
                    execution: partial.and_then(|p| p.formula_execution.as_ref()),
                    hybrid_execution: partial.and_then(|p| p.hybrid_execution.as_ref()),
                    terminal_execution: partial.and_then(|p| p.terminal_execution.as_ref()),
                    source_evaluation: partial.and_then(|p| p.source_evaluation.as_ref()),
                    model_construction: partial.and_then(|p| p.model_construction.as_ref()),
                    timings: failure.phase_timings.as_deref(),
                }
            }
        }
    }
}

fn search_statistics(
    out: &mut Buffer,
    statistics: Option<&zetesis_stable::Statistics>,
    scope: &str,
) -> Result<(), RunError> {
    let Some(stats) = statistics else {
        return out.text("null");
    };
    out.text("{\"work\":")?;
    out.text(&stats.search.work.to_string())?;
    out.text(",\"scope\":")?;
    out.string(scope)?;
    out.number_field("decisions", stats.search.decisions)?;
    out.number_field("candidate_restrictions", stats.candidate_restrictions)?;
    out.number_field("candidates", stats.candidates)?;
    out.number_field("countermodel_queries", stats.countermodel_queries)?;
    out.number_field("countermodels", stats.countermodels)?;
    out.number_field("stable_models", stats.stable_models)?;
    out.text(",\"parity\":")?;
    arithmetic::parity(out, stats.regions.as_ref().map(|regions| &regions.parity))?;
    out.text(",\"cardinality\":")?;
    arithmetic::cardinality(
        out,
        stats.regions.as_ref().map(|regions| &regions.cardinality),
    )?;
    out.text(",\"candidate_regions\":")?;
    region_statistics(out, stats.regions.as_ref())?;
    out.text(",\"region_filter\":")?;
    region_filter_statistics(out, stats.region_filter)?;
    out.text(",\"frozen_positive\":{\"attempts\":")?;
    out.text(&stats.reduct.frozen_positive_attempts.to_string())?;
    out.number_field("completed", stats.reduct.frozen_positive_completed)?;
    out.number_field(
        "preparation_work",
        stats.reduct.frozen_positive_preparation_work,
    )?;
    out.number_field("work", stats.reduct.frozen_positive_work)?;
    out.text("}")?;
    out.text(",\"reduct_query_regions\":{\"visited\":")?;
    out.text(&stats.reduct.regions.regions.to_string())?;
    out.number_field("refuted", stats.reduct.regions.refuted)?;
    out.number_field("leaves", stats.reduct.regions.leaves)?;
    out.number_field("propagations", stats.reduct.regions.propagations)?;
    out.number_field("reading_work", stats.reduct.regions.work)?;
    let preparation = stats.reduct.inner_preparation;
    out.text(",\"preparation\":{\"attempts\":")?;
    out.text(&preparation.attempts.to_string())?;
    out.number_field("prepared", preparation.prepared)?;
    out.number_field("declined", preparation.declined)?;
    out.number_field("saturation_units", preparation.saturation_units)?;
    out.number_field("groups", preparation.groups)?;
    out.number_field("probes", preparation.probes)?;
    out.number_field("refuted_patterns", preparation.refuted_patterns)?;
    out.number_field("rows", preparation.rows)?;
    out.number_field("inexact_hulls", preparation.inexact_hulls)?;
    out.number_field("root_refuted", preparation.root_refuted)?;
    out.number_field("work", preparation.work)?;
    out.text("}}}")
}

fn region_filter_statistics(
    out: &mut Buffer,
    statistics: Option<zetesis_stable::RegionFilterStatistics>,
) -> Result<(), RunError> {
    let Some(stats) = statistics else {
        return out.text("null");
    };
    out.text("{\"preparations\":")?;
    out.text(&stats.preparations.to_string())?;
    out.number_field("checks", stats.checks)?;
    out.number_field("refuted", stats.refuted)?;
    out.number_field("failed", stats.failed)?;
    out.text(",\"overflowed\":")?;
    out.text(if stats.overflowed { "true" } else { "false" })?;
    out.text("}")
}

fn terminal_statistics(
    out: &mut Buffer,
    statistics: Option<&zetesis_solve::TerminalExecutionStatistics>,
) -> Result<(), RunError> {
    let Some(statistics) = statistics else {
        return out.text("null");
    };
    out.text("{\"base\":")?;
    out.text(match statistics.base {
        zetesis_themelios::BaseKind::Eager => "\"eager\"",
        zetesis_themelios::BaseKind::Hybrid => "\"hybrid\"",
    })?;
    out.text(",\"base_answers\":")?;
    out.text(&statistics.base_answers.to_string())?;
    out.number_field("reconstructed", statistics.reconstructed)?;
    out.number_field("pending", statistics.pending)?;
    out.text(",\"reconstruction\":{\"attempts\":")?;
    out.text(&statistics.reconstruction.attempts.to_string())?;
    out.number_field("completed", statistics.reconstruction.completed)?;
    out.number_field("work", statistics.reconstruction.work)?;
    out.number_field("substitutions", statistics.reconstruction.substitutions)?;
    for (name, charges) in [
        ("admission", statistics.reconstruction.admission),
        ("allowance", statistics.reconstruction.allowance),
        ("latest", statistics.reconstruction.latest),
        ("peak", statistics.reconstruction.peak),
    ] {
        out.text(",\"")?;
        out.text(name)?;
        out.text("\":{\"work\":")?;
        out.text(&charges.work.to_string())?;
        out.number_field("substitutions", charges.substitutions)?;
        out.text("}")?;
    }
    out.text("}}")
}

fn hybrid_statistics(
    out: &mut Buffer,
    statistics: Option<&zetesis_solve::HybridExecutionStatistics>,
) -> Result<(), RunError> {
    let Some(statistics) = statistics else {
        return out.text("null");
    };
    out.text("{\"core_answers\":")?;
    out.text(&statistics.core_answers.to_string())?;
    out.number_field("accepted", statistics.accepted)?;
    out.number_field("rejected", statistics.rejected)?;
    out.number_field("pending", statistics.pending)?;
    out.text(",\"constraints\":{\"work\":")?;
    out.text(&statistics.constraints.work.to_string())?;
    out.number_field("substitutions", statistics.constraints.substitutions)?;
    out.number_field("scalar_bytes", statistics.constraints.scalar_bytes)?;
    let streamed = statistics.streamed;
    out.text("},\"streamed\":{\"templates\":")?;
    out.text(&streamed.templates.to_string())?;
    out.number_field("instances", streamed.instances)?;
    out.number_field("retained_templates", streamed.retained_templates)?;
    out.number_field("retained_instances", streamed.retained_instances)?;
    out.text("}}")
}

fn region_statistics(
    out: &mut Buffer,
    statistics: Option<&zetesis_stable::RegionSearchStatistics>,
) -> Result<(), RunError> {
    let Some(stats) = statistics else {
        return out.text("null");
    };
    let counts = stats.counts;
    out.text("{\"visited\":")?;
    out.text(&counts.regions.to_string())?;
    out.number_field("refuted", counts.refuted)?;
    out.number_field("leaves", counts.leaves)?;
    out.number_field("propagations", counts.propagations)?;
    out.number_field("held", counts.held)?;
    out.number_field("cut", counts.cut)?;
    out.text(",\"support_cut\":")?;
    out.string(if stats.producers {
        "applied"
    } else {
        "not_applicable"
    })?;
    out.number_field("reading_work", counts.work)?;
    out.number_field("support_work", counts.support_work)?;
    out.number_field("founded_work", counts.founded_work)?;
    let explanations = &counts.support_explanations;
    out.text(",\"support_explanations\":{\"attempts\":")?;
    out.text(&explanations.attempts.to_string())?;
    out.number_field("certified", explanations.certified)?;
    out.number_field("matched", explanations.matched)?;
    out.number_field("cut", explanations.cut)?;
    out.number_field("forced", explanations.forced)?;
    out.number_field("refuted", explanations.refuted)?;
    out.number_field("declined", explanations.declined)?;
    out.number_field("work", explanations.work)?;
    out.number_field("peak_bytes", explanations.peak_bytes)?;
    out.text("}")?;
    let generator = stats.generator_order;
    out.text(",\"generator_order\":{\"status\":")?;
    out.string(generator.status.label())?;
    out.number_field("work", generator.work)?;
    out.number_field("peak_bytes", generator.peak_bytes)?;
    out.number_field("retained_bytes", generator.retained_bytes)?;
    out.text("}")?;
    out.text(",\"frontier\":")?;
    frontier_statistics(out, stats.frontier)?;
    out.text(",\"frontier_order\":")?;
    match stats.frontier_order {
        Some(order) => out.string(order.label())?,
        None => out.text("null")?,
    }
    out.text(",\"workers\":")?;
    worker_statistics(out, stats.workers)?;
    out.text("}")
}

/// The parallel walk's workers, `null` on routes without one. Each state's
/// keys are `null` outside that state, and so is the delay of a walk whose
/// deadline overflows, which never engages. `query_division` holds the
/// division the walk ran with and its receipts.
fn worker_statistics(
    out: &mut Buffer,
    statistics: Option<zetesis_stable::RegionWorkerStatistics>,
) -> Result<(), RunError> {
    let Some(stats) = statistics else {
        return out.text("null");
    };
    let record = crate::statistics::WorkerRecord::from(stats.engagement);
    out.text("{\"configured\":")?;
    out.text(&stats.configured.to_string())?;
    out.text(",\"engagement_delay_ns\":")?;
    optional_number(out, stats.engagement_delay.map(|delay| delay.as_nanos()))?;
    out.text(",\"engagement\":")?;
    out.string(stats.engagement.label())?;
    for (key, value) in [
        ("elapsed_ns", record.elapsed),
        ("walking_ns", record.walking),
        ("engaged_after_ns", record.after),
    ] {
        out.text(",")?;
        out.string(key)?;
        out.text(":")?;
        optional_number(out, value.map(|duration| duration.as_nanos()))?;
    }
    out.text(",\"launched\":")?;
    match record.launched {
        Some(launched) => out.text(&launched.to_string())?,
        None => out.text("null")?,
    }
    out.text(",\"spawn_ns\":")?;
    optional_number(out, record.spawn.map(|duration| duration.as_nanos()))?;
    let division = stats.query_division;
    out.text(",\"query_division\":{\"division\":")?;
    out.string(division.division.label())?;
    out.number_field("published", division.published)?;
    out.number_field("served", division.served)?;
    out.number_field("stolen", division.stolen)?;
    out.number_field("servers", division.servers)?;
    out.number_field("peak_served", division.peak_served)?;
    out.text("}}")
}

fn frontier_statistics(
    out: &mut Buffer,
    statistics: Option<zetesis_stable::RegionFrontierStatistics>,
) -> Result<(), RunError> {
    let Some(stats) = statistics else {
        return out.text("null");
    };
    out.text("{\"regions\":")?;
    out.text(&stats.regions.to_string())?;
    out.number_field("capacity", stats.capacity)?;
    out.number_field("retained_bytes", stats.retained_bytes)?;
    out.number_field("peak_regions", stats.peak_regions)?;
    out.number_field("peak_capacity", stats.peak_capacity)?;
    out.number_field("peak_retained_bytes", stats.peak_retained_bytes)?;
    out.text("}")
}

fn execution_statistics(
    out: &mut Buffer,
    statistics: Option<&crate::FormulaExecutionStatistics>,
) -> Result<(), RunError> {
    let Some(stats) = statistics else {
        return out.text("null");
    };
    out.text("{\"adapter\":")?;
    out.optional_string((!stats.adapter.is_empty()).then_some(stats.adapter.as_str()))?;
    out.text(",\"gpu_limits\":")?;
    if let Some(limits) = stats.gpu_limits {
        out.text("{\"work_per_candidate\":")?;
        out.text(&limits.work_per_candidate.to_string())?;
        out.number_field("rounds_per_candidate", limits.rounds_per_candidate)?;
        out.text("}")?;
    } else {
        out.text("null")?;
    }
    out.text(",\"tight_work_per_candidate\":")?;
    out.optional_number(stats.tight_work_per_candidate)?;
    out.text(",\"gpu_scheduled_work\":")?;
    out.optional_number(stats.gpu_scheduled_work)?;
    tight_routes(out, stats)?;
    out.text(",\"gpu_residuals\":")?;
    if let Some(residuals) = stats.gpu_residuals {
        handovers::reasons(out, &residuals)?;
    } else {
        out.text("null")?;
    }
    out.text(",\"gpu_residual_handovers\":")?;
    handovers::write(out, stats.gpu_residual_handovers.as_ref())?;
    out.number_field("gpu_submitted_batches", stats.gpu_submitted_batches)?;
    out.number_field("gpu_submitted_candidates", stats.gpu_submitted_candidates)?;
    out.number_field("gpu_batches", stats.gpu_batches)?;
    out.number_field("gpu_slates", stats.gpu_slates)?;
    match stats.gpu_largest_slate {
        Some(largest) => out.number_field("gpu_largest_slate", largest)?,
        None => out.text(",\"gpu_largest_slate\":null")?,
    }
    out.number_field("gpu_slate_resets", stats.gpu_slate_resets)?;
    out.number_field("gpu_lookahead_slates", stats.gpu_lookahead_slates)?;
    out.text(",\"gpu_lookahead\":")?;
    match stats.gpu_lookahead {
        Some(decision) => out.string(decision.label())?,
        None => out.text("null")?,
    }
    out.number_field("gpu_lookahead_device_ns", stats.gpu_lookahead_device_ns)?;
    out.number_field(
        "gpu_lookahead_finish_wait_ns",
        stats.gpu_lookahead_finish_wait_ns,
    )?;
    out.number_field("gpu_lookahead_unmeasured", stats.gpu_lookahead_unmeasured)?;
    out.text(",\"first_answer_ns\":")?;
    out.optional_number(stats.first_answer_ns)?;
    out.number_field("gpu_candidates", stats.gpu_candidates)?;
    out.number_field("gpu_work", stats.gpu_work)?;
    out.number_field("gpu_rounds", stats.gpu_rounds)?;
    out.number_field("cpu_residuals", stats.cpu_residuals)?;
    out.number_field("gpu_decided", stats.gpu_decided)?;
    out.number_field("pending_candidates", stats.pending_candidates)?;
    out.number_field("queued_models", stats.queued_models)?;
    out.number_field("peak_accounted_bytes", stats.peak_accounted_bytes)?;
    out.text(",\"gpu_workgroup_size\":")?;
    out.optional_number(stats.gpu_workgroup_size.map(u64::from))?;
    out.text(",\"gpu_compute_timing\":{")?;
    let timing = &stats.gpu_compute_timing;
    out.text("\"status\":")?;
    out.string(timing.status.label())?;
    out.number_field("measured_batches", timing.measured_batches)?;
    out.number_field("unavailable_batches", timing.unavailable_batches)?;
    out.number_field("unresolved_batches", timing.unresolved_batches)?;
    out.number_field("flush_submissions", timing.flush_submissions)?;
    out.text(",\"elapsed_ns\":")?;
    optional_number(out, timing.elapsed.map(|elapsed| elapsed.as_nanos()))?;
    out.text(",\"complete\":")?;
    out.text(if timing.is_complete() {
        "true"
    } else {
        "false"
    })?;
    out.text(",\"scope\":\"decoded_device_compute_passes\",\"host_oracle\":\"excluded\"}")?;
    out.text(",\"completion\":{\"requested_workers\":")?;
    let stats = &stats.completion;
    out.text(&stats.requested_workers.to_string())?;
    out.number_field("effective_workers", stats.effective_workers)?;
    out.number_field("requested_scratch_bytes", stats.requested_scratch_bytes)?;
    out.number_field("peak_scratch_bytes", stats.peak_scratch_bytes)?;
    out.number_field("entered", stats.entered)?;
    out.number_field("residuals", stats.residuals)?;
    out.number_field("completed", stats.completed)?;
    out.number_field("failed", stats.failed)?;
    out.number_field("residual_completed", stats.residual_completed)?;
    out.number_field("residual_failed", stats.residual_failed)?;
    out.text(",\"complete\":")?;
    out.text(if stats.overflowed { "false" } else { "true" })?;
    out.text("}}")
}

/// The tight slots per route and the update route's availability, each `null`
/// before the first decoded tight batch.
fn tight_routes(
    out: &mut Buffer,
    stats: &crate::FormulaExecutionStatistics,
) -> Result<(), RunError> {
    out.text(",\"gpu_tight_routes\":")?;
    if let Some(routes) = stats.gpu_tight_routes {
        out.text("{\"complete\":")?;
        out.text(&routes.complete.to_string())?;
        out.number_field("dense_reuse", routes.dense_reuse)?;
        out.number_field("beyond_threshold", routes.beyond_threshold)?;
        out.number_field("update", routes.update)?;
        out.number_field("nodes_evaluated", routes.nodes_evaluated)?;
        out.text("}")?;
    } else {
        out.text("null")?;
    }
    out.text(",\"gpu_tight_updates\":")?;
    let Some(updates) = stats.gpu_tight_updates else {
        return out.text("null");
    };
    out.text("{\"availability\":")?;
    out.string(updates.availability.label())?;
    out.text(",\"count_bits\":")?;
    out.optional_number(updates.count_bits.map(u64::from))?;
    out.text(",\"steady_state\":")?;
    out.text(if updates.steady_state {
        "true"
    } else {
        "false"
    })?;
    out.number_field("guard_declined_batches", updates.guard_declined_batches)?;
    out.text("}")
}

#[cfg(test)]
pub(crate) mod document_fixture;
#[cfg(test)]
mod tests;
