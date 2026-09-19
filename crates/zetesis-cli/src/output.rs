//! Streaming JSON document adapter over typed model views and driver outcomes.

use std::io::{self, Write};

use zetesis_themelios::observation::ViewError;

use crate::failure::Progress;
use crate::{
    Completion, Interruption, Options, PhaseTimings, PublicationFailure, PublicationOutcome,
    RunError, RunFailure, SolvePhase,
};

pub(crate) struct Document<'a, W> {
    sink: &'a mut W,
    json: bool,
    failed: bool,
}
impl<'a, W: Write> Document<'a, W> {
    pub(crate) fn new(sink: &'a mut W, json: bool) -> Result<Self, PublicationFailure> {
        let mut document = Self {
            sink,
            json,
            failed: false,
        };
        if json {
            let head = format!(
                "{{\"schema\":{},\"format\":\"zetesis\",\"models\":[",
                zetesis_themelios::observation::json::RECORD_SCHEMA_VERSION
            );
            document.write_all(head.as_bytes())?;
        }
        Ok(document)
    }

    pub(crate) fn finish(
        mut self,
        mut result: Result<Progress, PublicationFailure>,
        options: &Options,
    ) -> Result<PublicationOutcome, PublicationFailure> {
        if self.json && !self.failed {
            let emitted = summary(&result, options.max_json_record_bytes)
                .and_then(|record| self.write_all(&record).map_err(RunError::Output));
            match emitted {
                Ok(()) => match &mut result {
                    Ok(progress) => progress.publication.summary = true,
                    Err(failure) => {
                        failure.acknowledge_summary();
                    }
                },
                Err(error) => {
                    return Err(match result {
                        Ok(progress) => progress.fail(error),
                        Err(mut failure) => {
                            // The original error remains authoritative. A failed JSON
                            // footer is independently retained as secondary output.
                            failure.record_summary(match error {
                                RunError::Output(error) => error,
                                other => io::Error::other(other),
                            });
                            failure
                        }
                    });
                }
            }
        }
        result.and_then(Progress::finalize)
    }
}
impl<W: Write> Write for Document<'_, W> {
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
    options: &Options,
    control: &zetesis_cpu::Control,
) -> Result<(), RunError> {
    let prefix = format!(
        "{}{{\"number\":{number},\"model\":",
        if number == 1 { "" } else { "," }
    );
    let overhead = prefix.len() + 2;
    let maximum = options
        .max_json_record_bytes
        .checked_sub(overhead)
        .ok_or(RunError::JsonRecord(ViewError::Bytes))?;
    let record = view
        .record(
            atoms,
            zetesis_themelios::observation::ViewLimits {
                max_bytes: maximum,
                ..Default::default()
            },
            control,
        )
        .map_err(RunError::JsonRecord)?;
    control
        .poll()
        .map_err(|error| RunError::JsonRecord(ViewError::Stopped(error)))?;
    // All semantic/encoding failures precede publication. I/O can still fail
    // after a prefix; the caller increments its count only after all writes.
    output.write_all(prefix.as_bytes())?;
    output.write_all(record.as_bytes())?;
    output.write_all(b"}\n")?;
    Ok(())
}

/// JSON for failures that precede the ordinary source/bundle driver entry.
pub(crate) fn input_failure(
    output: &mut impl Write,
    failure: RunFailure,
    options: &Options,
) -> RunFailure {
    match Document::new(output, true) {
        Ok(document) => document
            .finish(Err(failure.into()), options)
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

fn summary(
    result: &Result<Progress, PublicationFailure>,
    maximum: usize,
) -> Result<Vec<u8>, RunError> {
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
    let view = SummaryView::new(result);
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
        out.string(control_code(stop.reason()))?;
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
        "{{\"schema\":3,\"clock\":\"host_monotonic\",\"driver_elapsed_ns\":{},\"measurements\":{{",
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
        RunError::Input(_) => "input",
        RunError::TimeLimitRange { .. } => "time_limit_range",
        RunError::DeadlineTimer(_) => "deadline_timer",
        RunError::Observation(_) => "observation",
        RunError::Projection(_) => "answer_projection",
        RunError::JsonRecord(_) => "json_record",
        RunError::ObservationOutputLimit { .. } => "observation_output_limit",
        RunError::MixedStandardInput => "mixed_standard_input",
        RunError::Admission(_) => "admission",
        RunError::Expansion(_) => "expansion",
        RunError::BundleLoad(_) => "bundle_load",
        RunError::BundleAdmission(_) => "bundle_admission",
        RunError::Batch(_) => "batch",
        RunError::QueryObservation(_) => "query_observation",
        RunError::CompletionPool(_) => "completion_pool",
        RunError::CompletionUnavailable => "completion_unavailable",
        RunError::BackendUnavailable => "backend_unavailable",
        RunError::UnsupportedCombination { .. } => "unsupported_combination",
        RunError::UnsupportedOracle { .. } => "unsupported_oracle",
        RunError::UnsupportedSourceBatching => "unsupported_source_batching",
        RunError::ClosureReservation { .. } => "closure_reservation",
        RunError::SharedCpu(_) => "shared_cpu",
        RunError::PreparedInput { .. } => "prepared_input",
        RunError::Formula(_) => "formula",
        RunError::FormulaAdmission(_) => "formula_admission",
        RunError::FormulaBundleAdmission(_) => "formula_bundle_admission",
        RunError::Output(_) => "output",
        RunError::ExecutionObservation(_) => "execution_observation",
        RunError::PublicationStopped(_) => "publication_stopped",
        RunError::Static(_) => "static",
        RunError::Words(_) => "words",
        RunError::Model(_) => "model",
        RunError::FormulaBatchShape { .. } => "formula_batch_shape",
        RunError::CandidateStreamNotExhausted => "candidate_stream_not_exhausted",
        #[cfg(feature = "gpu")]
        RunError::Gpu(_) => "gpu",
        #[cfg(feature = "gpu")]
        RunError::LazyGpu(_) => "lazy_gpu",
        RunError::LazyStatisticsOverflow => "lazy_statistics_overflow",
        RunError::ClosureStatisticsOverflow => "closure_statistics_overflow",
    }
}

fn reason_code(reason: Interruption) -> &'static str {
    match reason {
        Interruption::Preparation(reason) | Interruption::Oracle(reason) => control_code(reason),
        Interruption::Countermodel(reason) => {
            use zetesis_sat::Incomplete;
            match reason {
                Incomplete::Cancelled => "cancelled",
                Incomplete::Deadline => "deadline",
                Incomplete::WorkLimit => "work_limit",
                Incomplete::DecisionLimit => "decision_limit",
                Incomplete::CandidateLimit => "candidate_limit",
                Incomplete::ProjectionLimit { resource, .. } => match resource {
                    zetesis_sat::ProjectionResource::Entries => "projection_entries",
                    zetesis_sat::ProjectionResource::Nodes => "projection_nodes",
                    zetesis_sat::ProjectionResource::Bytes => "projection_bytes",
                },
                Incomplete::PendingBytes => "pending_bytes",
                Incomplete::CompletionScratch => "completion_scratch",
                Incomplete::ReductStorage { .. } => "reduct_storage",
                Incomplete::BatchCandidateLimit => "batch_candidate_limit",
                Incomplete::PendingBatch => "pending_batch",
                Incomplete::Allocation => "allocation",
                Incomplete::Admission(_) => "admission",
                Incomplete::WrongTheory => "wrong_theory",
                Incomplete::RestrictionUniverse { .. } => "restriction_universe",
                Incomplete::ClosedEnumerator => "closed_enumerator",
                Incomplete::LateCertificate => "late_certificate",
                Incomplete::Certificate(_) => "certificate",
                Incomplete::Verification(_) => "verification",
                Incomplete::InvalidWitness => "invalid_witness",
                Incomplete::CounterOverflow => "counter_overflow",
            }
        }
        Interruption::Objective(error) => {
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
        Interruption::Incumbent(reason) => match reason {
            crate::OptimizationStop::Models => "models",
            crate::OptimizationStop::Atoms => "atoms",
            crate::OptimizationStop::Bytes => "bytes",
            crate::OptimizationStop::Overflow => "overflow",
            crate::OptimizationStop::Allocation => "allocation",
        },
    }
}

fn control_code(reason: zetesis_cpu::Stop) -> &'static str {
    use zetesis_cpu::Stop;
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
        serde_json::to_writer(&mut *self, value)
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
            Interruption::Objective(_) => "objective",
            Interruption::Incumbent(_) => "incumbent",
        };
        let detail = reason.to_string();
        out.text("{\"kind\":")?;
        out.string(kind)?;
        out.text(",\"code\":")?;
        out.string(reason_code(reason))?;
        out.text(",\"detail\":")?;
        out.string(&detail)?;
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
        out.text("{\"search\":")?;
        search_statistics(out, view.search)?;
        out.text(",\"candidate_restrictions\":")?;
        candidate_statistics(out, view.candidates)?;
        out.text(",\"execution\":")?;
        execution_statistics(out, view.execution)?;
        out.text(",\"lazy_execution\":")?;
        lazy_statistics(out, view.lazy_execution)?;
        out.text(",\"shared_execution\":")?;
        shared_statistics(out, view.shared_execution)?;
        out.text(",\"closure_execution\":")?;
        closure_statistics(out, view.closure_execution)?;
        out.text(",\"query_execution\":")?;
        query_statistics(out, view.query_execution)?;
        out.text(",\"expansion\":")?;
        expansion_usage(out, view.expansion)?;
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
        out.text(&format!(
            "{{\"optimal\":{},\"tied_models\":{},\"scored_models\":{},\"work\":{},\"costs\":[",
            optimum_proved, best.tied_models, best.scored_models, best.work
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
    search: Option<&'a zetesis_sat::Statistics>,
    candidates: Option<zetesis_cpu::CandidateStatistics>,
    execution: Option<&'a crate::FormulaExecutionStatistics>,
    lazy_execution: Option<&'a crate::LazyExecutionStatistics>,
    shared_execution: Option<&'a crate::SharedExecutionStatistics>,
    closure_execution: Option<&'a crate::ClosureExecutionStatistics>,
    query_execution: Option<&'a crate::QueryExecutionObservation>,
    expansion: Option<zetesis_themelios::ExpansionUsage>,
    timings: Option<&'a PhaseTimings>,
}
impl<'a> SummaryView<'a> {
    fn new(result: &'a Result<Progress, PublicationFailure>) -> Self {
        match result {
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
                    search: semantic.and_then(crate::SemanticOutcome::countermodel_statistics),
                    candidates: semantic.and_then(crate::SemanticOutcome::candidate_statistics),
                    execution: semantic.and_then(crate::SemanticOutcome::formula_execution),
                    lazy_execution: semantic.and_then(crate::SemanticOutcome::lazy_execution),
                    shared_execution: semantic.and_then(crate::SemanticOutcome::shared_execution),
                    closure_execution: semantic.and_then(crate::SemanticOutcome::closure_execution),
                    query_execution: semantic.and_then(crate::SemanticOutcome::query_execution),
                    expansion: progress.expansion,
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
                    search: partial.and_then(|p| p.countermodel_statistics.as_ref()),
                    candidates: partial.and_then(|p| p.candidate_statistics),
                    execution: partial.and_then(|p| p.formula_execution.as_ref()),
                    lazy_execution: partial.and_then(|p| p.lazy_execution.as_ref()),
                    shared_execution: partial.and_then(|p| p.shared_execution.as_ref()),
                    closure_execution: partial.and_then(|p| p.closure_execution.as_ref()),
                    query_execution: partial.and_then(|p| p.query_execution.as_ref()),
                    expansion: partial.and_then(|p| p.expansion),
                    timings: failure.phase_timings.as_deref(),
                }
            }
        }
    }
}

fn search_statistics(
    out: &mut Buffer,
    statistics: Option<&zetesis_sat::Statistics>,
) -> Result<(), RunError> {
    let Some(stats) = statistics else {
        return out.text("null");
    };
    out.text("{\"work\":")?;
    out.text(&stats.search.work.to_string())?;
    out.number_field("decisions", stats.search.decisions)?;
    out.number_field("candidate_queries", stats.candidate_queries)?;
    out.number_field("candidate_restrictions", stats.candidate_restrictions)?;
    out.number_field("candidates", stats.candidates)?;
    out.number_field("countermodel_queries", stats.countermodel_queries)?;
    out.number_field("countermodels", stats.countermodels)?;
    out.number_field("stable_models", stats.stable_models)?;
    out.text(",\"projection_history\":{\"entries\":")?;
    out.text(&stats.projections.entries.to_string())?;
    out.number_field("nodes", stats.projections.nodes)?;
    out.number_field("retained_bytes", stats.projections.retained_bytes)?;
    out.number_field("peak_bytes", stats.projections.peak_bytes)?;
    out.number_field("work", stats.projections.work)?;
    out.text("}")?;
    out.text(",\"necessary_support\":")?;
    support_statistics(out, stats.support)?;
    out.text(",\"candidate_regions\":")?;
    region_statistics(out, stats.regions)?;
    out.text(",\"reduct_query_regions\":{\"visited\":")?;
    out.text(&stats.reduct.regions.regions.to_string())?;
    out.number_field("refuted", stats.reduct.regions.refuted)?;
    out.number_field("leaves", stats.reduct.regions.leaves)?;
    out.number_field("propagations", stats.reduct.regions.propagations)?;
    out.number_field("reading_work", stats.reduct.regions.work)?;
    out.text("}}")
}

fn region_statistics(
    out: &mut Buffer,
    statistics: Option<zetesis_sat::RegionSearchStatistics>,
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
    out.text("}")
}

fn candidate_statistics(
    out: &mut Buffer,
    statistics: Option<zetesis_cpu::CandidateStatistics>,
) -> Result<(), RunError> {
    let Some(stats) = statistics else {
        return out.text("null");
    };
    out.text("{\"work\":")?;
    out.text(&stats.restriction_work.to_string())?;
    out.number_field("conjunctions", stats.restriction_conjunctions)?;
    out.number_field("skipped_intervals", stats.conflicts)?;
    out.number_field("prepared_atom_occurrences", stats.restriction_atoms)?;
    out.number_field("copied_payload_bytes", stats.restriction_bytes)?;
    out.number_field("peak_copied_payload_bytes", stats.restriction_peak_bytes)?;
    // The carrier narrowing and regions, under the words of the text lines.
    out.text(",\"carrier_narrowing\":{\"passes\":")?;
    out.text(&stats.narrowing_passes.to_string())?;
    out.number_field("cut_gate_atoms", stats.cut_gate_atoms)?;
    out.number_field("held_gate_atoms", stats.held_gate_atoms)?;
    out.text(",\"refuted\":")?;
    out.text(if stats.root_refuted { "true" } else { "false" })?;
    out.text(",\"stopped\":")?;
    match &stats.narrowing_stop {
        None => out.text("null")?,
        Some(stop) => out.string(&stop.to_string())?,
    }
    out.text("},\"carrier_regions\":{\"visited\":")?;
    out.text(&stats.regions.to_string())?;
    out.number_field("refuted", stats.regions_refuted)?;
    out.number_field("decided", stats.regions_decided)?;
    out.number_field("counted", stats.regions_counted)?;
    out.number_field("narrowing_passes", stats.region_passes)?;
    out.text("}}")
}

fn support_statistics(
    out: &mut Buffer,
    statistics: Option<zetesis_sat::SupportStatistics>,
) -> Result<(), RunError> {
    let Some(stats) = statistics else {
        return out.text("null");
    };
    out.text("{\"status\":")?;
    out.string(match stats.status {
        zetesis_sat::SupportStatus::Applied => "applied",
        zetesis_sat::SupportStatus::NotApplicable => "not_applicable",
        zetesis_sat::SupportStatus::FormulaLimit => "formula_limit",
        zetesis_sat::SupportStatus::EncodingLimit(_) => "encoding_limit",
    })?;
    out.number_field("construction_work", stats.construction_work)?;
    out.number_field("encoding_work", stats.encoding_work)?;
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
    out.number_field("gpu_submitted_batches", stats.gpu_submitted_batches)?;
    out.number_field("gpu_submitted_candidates", stats.gpu_submitted_candidates)?;
    out.number_field("gpu_batches", stats.gpu_batches)?;
    out.number_field("gpu_candidates", stats.gpu_candidates)?;
    out.number_field("gpu_work", stats.gpu_work)?;
    out.number_field("gpu_rounds", stats.gpu_rounds)?;
    out.number_field("cpu_residuals", stats.cpu_residuals)?;
    out.number_field("gpu_decided", stats.gpu_decided)?;
    out.number_field("pending_candidates", stats.pending_candidates)?;
    out.number_field("queued_models", stats.queued_models)?;
    out.number_field("peak_accounted_bytes", stats.peak_accounted_bytes)?;
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

fn expansion_usage(
    out: &mut Buffer,
    usage: Option<zetesis_themelios::ExpansionUsage>,
) -> Result<(), RunError> {
    let Some(usage) = usage else {
        return out.text("null");
    };
    out.text("{\"term_work\":")?;
    out.text(&usage.term_work.to_string())?;
    out.number_field("templates", usage.templates)?;
    out.number_field("values", usage.values)?;
    out.number_field("scalar_bytes", usage.scalar_bytes)?;
    out.number_field("origin_locations", usage.origin_locations)?;
    out.text("}")
}

fn closure_statistics(
    out: &mut Buffer,
    statistics: Option<&crate::ClosureExecutionStatistics>,
) -> Result<(), RunError> {
    let Some(stats) = statistics else {
        return out.text("null");
    };
    out.text("{\"backend\":\"cpu\",\"grounder\":")?;
    out.string(stats.route.label())?;
    out.number_field("completed_checks", stats.completed_checks)?;
    out.number_field("stopped_checks", stats.stopped_checks)?;
    out.number_field("rounds", stats.rounds)?;
    out.number_field("work", stats.work)?;
    out.number_field("derived_atoms", stats.derived_atoms)?;
    out.text(",\"joins\":")?;
    match stats.route {
        crate::ClosureRoute::Eager => out.text("null")?,
        crate::ClosureRoute::Lazy(joins) => {
            out.text("{\"catalog_work\":")?;
            out.text(&joins.catalog_work.to_string())?;
            out.number_field("bindings", joins.bindings)?;
            out.number_field("tuple_probes", joins.tuple_probes)?;
            out.number_field("dense_heads", joins.dense_heads)?;
            out.number_field("row_steps", joins.row_steps)?;
            out.number_field("peak_closure_bytes", joins.peak_closure_bytes)?;
            out.text("}")?;
        }
    }
    out.text("}")
}

fn query_statistics(
    out: &mut Buffer,
    observation: Option<&crate::QueryExecutionObservation>,
) -> Result<(), RunError> {
    let Some(observation) = observation else {
        return out.text("null");
    };
    out.text("{\"backend\":\"cpu\",\"source_batching\":\"independent\",\"snapshot\":")?;
    if let Some(stats) = observation.statistics {
        out.text("{\"preparation\":")?;
        if let Some(preparation) = stats.preparation {
            out.text("{\"work\":")?;
            out.text(&preparation.work.to_string())?;
            out.number_field("retained_bytes", preparation.retained_bytes)?;
            out.number_field("predicates", preparation.predicates)?;
            out.number_field("dense_predicates", preparation.dense_predicates)?;
            out.text("}")?;
        } else {
            out.text("null")?;
        }
        out.number_field("preparation_builds", stats.preparation_builds)?;
        out.number_field("retained_workspaces", stats.retained_workspaces)?;
        out.number_field("active_workspaces", stats.active_workspaces)?;
        out.number_field("reused_workspaces", stats.reused_workspaces)?;
        out.number_field("retained_bytes", stats.retained_bytes)?;
        out.number_field("reserved_bytes", stats.reserved_bytes)?;
        out.text("}")?;
    } else {
        out.text("null")?;
    }
    out.text(",\"fault\":")?;
    if let Some(fault) = &observation.fault {
        query_fault(out, fault)?;
    } else {
        out.text("null")?;
    }
    out.text("}")
}

fn query_fault(out: &mut Buffer, fault: &zetesis_cpu::BatchError) -> Result<(), RunError> {
    use zetesis_cpu::BatchError;
    out.text("{\"kind\":")?;
    out.string(match fault {
        BatchError::Pool(_) => "pool",
        BatchError::Busy => "busy",
        BatchError::Poisoned => "poisoned",
        BatchError::Preparation(_) => "preparation",
        BatchError::ClosureStorage { .. } => "closure_storage",
        BatchError::Capacity { .. } => "capacity",
    })?;
    match fault {
        BatchError::Preparation(stop) => {
            out.text(",\"reason\":")?;
            out.string(control_code(*stop))?;
        }
        BatchError::ClosureStorage { required, limit } => {
            out.number_field("required", *required)?;
            out.number_field("limit", *limit)?;
        }
        BatchError::Capacity { limit, actual } => {
            out.number_field("limit", *limit)?;
            out.number_field("actual", *actual)?;
        }
        _ => {}
    }
    out.text(",\"detail\":")?;
    out.string(&fault.to_string())?;
    out.text("}")
}

fn shared_statistics(
    out: &mut Buffer,
    statistics: Option<&crate::SharedExecutionStatistics>,
) -> Result<(), RunError> {
    let Some(stats) = statistics else {
        return out.text("null");
    };
    out.text("{\"requested_backend\":")?;
    out.string(stats.requested_backend.label())?;
    out.text(",\"backend\":\"cpu\",\"source_batching\":")?;
    out.string(match stats.selection {
        zetesis_cpu::lazy::SourceSelection::Union => "union",
        zetesis_cpu::lazy::SourceSelection::Worlds => "worlds",
    })?;
    out.number_field("workers", stats.workers)?;
    out.number_field("batches", stats.batches)?;
    out.number_field("submitted_candidates", stats.submitted_candidates)?;
    out.number_field("completed_candidates", stats.completed_candidates)?;
    out.number_field("stopped_candidates", stats.stopped_candidates)?;
    out.number_field("queued_results", stats.queued_results)?;
    out.number_field("source_rounds", stats.source_rounds)?;
    out.number_field("source_work", stats.source_work)?;
    out.number_field("source_instances", stats.source_instances)?;
    out.number_field("peak_catalog_atoms", stats.peak_catalog_atoms)?;
    out.number_field("mask_words", stats.mask_words)?;
    out.number_field("pruned_prefixes", stats.pruned_prefixes)?;
    out.number_field("peak_mask_bytes", stats.peak_mask_bytes)?;
    out.number_field("world_work", stats.world_work)?;
    out.number_field("world_instances", stats.world_instances)?;
    out.text(",\"last_stop\":")?;
    match stats.last_stop {
        None => out.text("null")?,
        Some(zetesis_cpu::lazy::shared::Cause::Source(stop)) => {
            out.text("{\"scope\":\"source\",\"reason\":")?;
            out.string(control_code(stop))?;
            out.text("}")?;
        }
        Some(zetesis_cpu::lazy::shared::Cause::World { index, stop }) => {
            out.text("{\"scope\":\"world\",\"reason\":")?;
            out.string(control_code(stop))?;
            out.number_field("index", index)?;
            out.text("}")?;
        }
        Some(zetesis_cpu::lazy::shared::Cause::InvalidOutput) => {
            out.text("{\"scope\":\"protocol\",\"reason\":\"invalid_output\"}")?;
        }
    }
    out.text("}")
}

fn lazy_statistics(
    out: &mut Buffer,
    statistics: Option<&crate::LazyExecutionStatistics>,
) -> Result<(), RunError> {
    let Some(stats) = statistics else {
        return out.text("null");
    };
    out.text("{\"requested_backend\":")?;
    out.string(stats.requested_backend.label())?;
    out.text(",\"adapter\":")?;
    out.string(&stats.adapter)?;
    out.text(",\"backend\":")?;
    out.string(&stats.backend)?;
    out.number_field("batches", stats.batches)?;
    out.number_field("submitted_candidates", stats.submitted_candidates)?;
    out.number_field("completed_candidates", stats.completed_candidates)?;
    out.number_field("stopped_candidates", stats.stopped_candidates)?;
    out.number_field("queued_results", stats.queued_results)?;
    out.number_field("source_rounds", stats.source_rounds)?;
    out.number_field("source_work", stats.source_work)?;
    out.number_field("source_instances", stats.source_instances)?;
    out.number_field("peak_catalog_atoms", stats.peak_catalog_atoms)?;
    out.number_field("dispatches", stats.dispatches)?;
    out.number_field("world_instances", stats.world_instances)?;
    out.number_field("uploaded_bytes", stats.uploaded_bytes)?;
    out.number_field("downloaded_bytes", stats.downloaded_bytes)?;
    out.number_field("transport_allocations", stats.transport_allocations)?;
    out.number_field("transport_reuses", stats.transport_reuses)?;
    out.number_field("peak_transport_bytes", stats.peak_transport_bytes)?;
    out.text(",\"transport_replacements\":{")?;
    out.text("\"initial\":")?;
    out.text(&stats.transport_replacements.initial.to_string())?;
    out.number_field(
        "offsets_growth",
        stats.transport_replacements.offsets_growth,
    )?;
    out.number_field(
        "records_growth",
        stats.transport_replacements.records_growth,
    )?;
    out.number_field(
        "snapshots_growth",
        stats.transport_replacements.snapshots_growth,
    )?;
    out.number_field("seeds_growth", stats.transport_replacements.seeds_growth)?;
    out.number_field("result_shape", stats.transport_replacements.result_shape)?;
    out.number_field("budget", stats.transport_replacements.budget)?;
    out.number_field(
        "accounting_overflow",
        stats.transport_replacements.accounting_overflow,
    )?;
    out.text("}")?;
    out.text(",\"transport_usage\":")?;
    lazy_transport_usage(out, stats.transport_usage)?;
    out.number_field("host_wait_ns", stats.host_wait.as_nanos())?;
    out.text("}")
}

fn lazy_transport_usage(
    out: &mut Buffer,
    usage: crate::LazyTransportUsage,
) -> Result<(), RunError> {
    out.text("{")?;
    for (index, (name, binding)) in [
        ("uniform", usage.uniform),
        ("offsets", usage.offsets),
        ("records", usage.records),
        ("snapshots", usage.snapshots),
        ("seeds", usage.seeds),
        ("output", usage.output),
        ("readback", usage.readback),
    ]
    .into_iter()
    .enumerate()
    {
        if index > 0 {
            out.text(",")?;
        }
        out.string(name)?;
        out.text(":{\"allocations\":")?;
        out.text(&binding.allocations.to_string())?;
        out.number_field("reuses", binding.reuses)?;
        out.text("}")?;
    }
    out.number_field("budget_releases", usage.budget_releases)?;
    out.number_field(
        "accounting_overflow_releases",
        usage.accounting_overflow_releases,
    )?;
    out.text("}")
}

#[cfg(test)]
#[path = "../tests/support/lazy_statistics_fixture.rs"]
pub(crate) mod fixtures;

#[cfg(test)]
mod lazy_tests {
    use super::{Buffer, lazy_statistics};

    #[test]
    fn lazy_json_retains_requested_and_observed_execution() {
        let fixture = super::fixtures::lazy_statistics();
        let mut out = Buffer::new(4096);
        lazy_statistics(&mut out, Some(&fixture)).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&out.bytes).unwrap();
        assert_eq!(value["requested_backend"], "metal");
        assert_eq!(value["backend"], "Metal");
        assert_eq!(value["adapter"], fixture.adapter);
        assert_eq!(value["submitted_candidates"], 7);
        assert_eq!(value["completed_candidates"], 4);
        assert_eq!(value["stopped_candidates"], 3);
        assert_eq!(value["queued_results"], 2);
        assert_eq!(value["transport_allocations"], 2);
        assert_eq!(value["transport_reuses"], 3);
        assert_eq!(value["peak_transport_bytes"], 1024);
        assert_eq!(value["host_wait_ns"], 123);
        assert_eq!(
            value["transport_replacements"],
            serde_json::json!({
                "initial": 1,
                "offsets_growth": 0,
                "records_growth": 1,
                "snapshots_growth": 0,
                "seeds_growth": 0,
                "result_shape": 1,
                "budget": 0,
                "accounting_overflow": 0
            })
        );
        assert_eq!(
            value["transport_usage"],
            serde_json::json!({
                "uniform": {"allocations": 1, "reuses": 4},
                "offsets": {"allocations": 1, "reuses": 4},
                "records": {"allocations": 2, "reuses": 3},
                "snapshots": {"allocations": 1, "reuses": 4},
                "seeds": {"allocations": 1, "reuses": 4},
                "output": {"allocations": 2, "reuses": 3},
                "readback": {"allocations": 2, "reuses": 3},
                "budget_releases": 0,
                "accounting_overflow_releases": 0
            })
        );
    }

    #[test]
    fn lazy_json_obeys_the_record_byte_limit() {
        let fixture = super::fixtures::lazy_statistics();
        let mut out = Buffer::new(4096);
        lazy_statistics(&mut out, Some(&fixture)).unwrap();
        for capacity in 0..out.bytes.len() {
            let mut bounded = Buffer::new(capacity);
            assert!(lazy_statistics(&mut bounded, Some(&fixture)).is_err());
            assert!(bounded.bytes.len() <= capacity);
        }
    }
}

#[cfg(test)]
#[path = "../tests/support/json_failures.rs"]
mod failure_tests;

#[cfg(test)]
#[path = "../tests/support/footer_admission.rs"]
mod footer_admission_tests;
