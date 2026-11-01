//! Machine-visible failure identity and output publication boundaries.

use super::document_fixture::Document;
use super::{Buffer, input_failure, write_interruption};
use crate::{Interruption, Options, RunError};
use clap::Parser;
use std::io::{self, Write};
use zetesis_test_support::io::BoundedWriter;

#[test]
fn displayed_diagnostics_preserve_json_escaping() {
    let error = RunError::Input(io::Error::other("bad \"name\"\n\\tail\té"));
    let mut output = Buffer::new(256);
    output.display_string(&error).unwrap();
    let decoded: String = serde_json::from_slice(&output.bytes).unwrap();
    assert_eq!(decoded, error.to_string());
}

#[test]
fn diagnostic_formatting_stops_at_the_json_ceiling() {
    struct Diagnostic(std::cell::Cell<usize>);
    impl std::fmt::Display for Diagnostic {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            for _ in 0..4096 {
                self.0.set(self.0.get() + 1);
                formatter.write_str("\"")?;
            }
            Ok(())
        }
    }
    let diagnostic = Diagnostic(std::cell::Cell::new(0));
    let mut output = Buffer::new(16);
    assert!(matches!(
        output.display_string(&diagnostic),
        Err(RunError::JsonRecord(
            zetesis_themelios::observation::ViewError::Bytes
        ))
    ));
    assert!(output.bytes.len() <= 16);
    // A temporary unbounded to_string() would visit all 4096 fragments first.
    assert!(diagnostic.0.get() < 4096);
}

#[test]
fn legacy_metadata_cannot_establish_optimality() {
    let options = Options::parse_from(["zetesis", "--json", "--backend", "cpu", "--models", "0"]);
    let source = format!("p(\"{}\"). #minimize{{0@1,k:p(X)}}.", "x".repeat(1500));
    let failure = crate::publication_fixture::json(
        &source,
        &crate::PublicationConfig::from(&options),
        1000,
        &mut Vec::new(),
        &zetesis_algebra::Cancellation::default(),
    )
    .unwrap_err()
    .into_legacy();
    let partial = failure.partial_report.as_ref().unwrap();
    assert_eq!(partial.completion, Some(crate::Completion::Exhausted));
    assert!(partial.optimization.is_some());
    // The compatibility conversion retains public counters, but discards
    // the session's immutable semantic outcome and its optimality evidence.
    let failure = crate::PublicationFailure::from(failure);
    assert!(failure.semantic().is_none());
    let record = super::document_fixture::summary(&Err(failure), 4096).unwrap();
    let mut document = b"{\"models\":[".to_vec();
    document.extend(record);
    let value: serde_json::Value = serde_json::from_slice(&document).unwrap();
    assert_eq!(value["outcome"]["optimization"]["optimal"], false);
}

#[test]
fn input_failure_survives_a_broken_json_prefix() {
    let mut writer = BoundedWriter::new(3);
    let error = input_failure(
        &mut writer,
        RunError::Input(io::Error::new(io::ErrorKind::NotFound, "source missing")).into(),
        &Options::parse_from(["zetesis", "--json"]),
    );
    assert!(matches!(*error.cause, RunError::Input(_)));
    assert!(error.to_string().contains("source missing"));
    assert!(error.partial_report.is_none());
    assert!(error.secondary_output.is_some());
    assert_eq!(writer.bytes(), b"{\"s");
}

#[test]
fn source_stops_have_distinct_machine_codes() {
    use zetesis_algebra::Stop;
    for (stop, code) in [
        (Stop::Cancelled, "cancelled"),
        (Stop::Deadline, "deadline"),
        (Stop::WorkLimit, "work_limit"),
        (Stop::DerivedAtomLimit, "derived_atom_limit"),
        (Stop::CandidateLimit, "candidate_limit"),
        (Stop::CarrierLimit, "carrier_limit"),
        (Stop::Allocation, "allocation"),
        (Stop::WrongProgram, "wrong_program"),
        (Stop::InvalidProgram, "invalid_program"),
    ] {
        let mut out = Buffer::new(4096);
        write_interruption(&mut out, Some(Interruption::Oracle(stop))).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&out.bytes).unwrap();
        assert_eq!(value["kind"], "oracle");
        assert_eq!(value["code"], code);
        assert_eq!(value["detail"], stop.to_string());
    }
}

#[test]
fn countermodel_stops_have_distinct_machine_codes() {
    use zetesis_stable::Incomplete;
    for (stop, code) in [
        (Incomplete::Cancelled, "cancelled"),
        (Incomplete::Deadline, "deadline"),
        (Incomplete::WorkLimit, "work_limit"),
        (Incomplete::DecisionLimit, "decision_limit"),
        (Incomplete::CandidateLimit, "candidate_limit"),
        (Incomplete::PendingBytes, "pending_bytes"),
        (Incomplete::CompletionScratch, "completion_scratch"),
        (
            Incomplete::ReductStorage {
                required: 65,
                limit: 64,
            },
            "reduct_storage",
        ),
        (
            Incomplete::NativeNarrowingStorage {
                required: 65,
                limit: 64,
            },
            "native_narrowing_storage",
        ),
        (Incomplete::BatchCandidateLimit, "batch_candidate_limit"),
        (Incomplete::PendingBatch, "pending_batch"),
        (Incomplete::Allocation, "allocation"),
        (Incomplete::WrongTheory, "wrong_theory"),
        (Incomplete::ClosedEnumerator, "closed_enumerator"),
        (Incomplete::LateCertificate, "late_certificate"),
        (Incomplete::LateCandidateBounds, "late_candidate_bounds"),
        (Incomplete::CandidateBoundPlan, "candidate_bound_plan"),
        (
            Incomplete::CandidateBoundNotImproving,
            "candidate_bound_not_improving",
        ),
        (Incomplete::InvalidWitness, "invalid_witness"),
        (
            Incomplete::InvalidRegionConsequence,
            "invalid_region_consequence",
        ),
        (Incomplete::CounterOverflow, "counter_overflow"),
    ] {
        let mut out = Buffer::new(4096);
        write_interruption(&mut out, Some(Interruption::Countermodel(stop))).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&out.bytes).unwrap();
        assert_eq!(value["kind"], "countermodel");
        assert_eq!(value["code"], code);
        assert_eq!(value["detail"], stop.to_string());
    }
}

#[test]
fn incumbent_stops_have_distinct_machine_codes() {
    use crate::OptimizationStop;
    for (stop, code) in [
        (OptimizationStop::Models, "models"),
        (OptimizationStop::Atoms, "atoms"),
        (OptimizationStop::Bytes, "bytes"),
        (OptimizationStop::Overflow, "overflow"),
        (OptimizationStop::Allocation, "allocation"),
    ] {
        let mut out = Buffer::new(4096);
        write_interruption(&mut out, Some(Interruption::Incumbent(stop))).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&out.bytes).unwrap();
        assert_eq!(value["kind"], "incumbent");
        assert_eq!(value["code"], code);
    }
}

struct FlushFailure(Vec<u8>);
impl Write for FlushFailure {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::BrokenPipe,
            "flush disconnected",
        ))
    }
}

#[test]
fn failed_flush_prevents_a_later_json_footer() {
    let mut sink = FlushFailure(Vec::new());
    let mut document = Document::new(&mut sink, true).unwrap();
    let error = document.flush().unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
    let error = document
        .finish(
            Err(RunError::Input(io::Error::other("original cause")).into()),
            65_536,
        )
        .unwrap_err();
    assert!(error.to_string().contains("original cause"));
    assert_eq!(sink.0, b"{\"schema\":3,\"format\":\"zetesis\",\"models\":[");
}

#[test]
fn failure_envelopes_never_invent_search_coverage() {
    let errors = [
        (
            RunError::ObservationOutputLimit {
                observed: 5,
                limit: 4,
            },
            "observation_output_limit",
        ),
        (RunError::MixedStandardInput, "mixed_standard_input"),
        (RunError::BackendUnavailable, "backend_unavailable"),
        (
            RunError::GpuExecutableUnavailable {
                path: Some("zetesis-libexec/0/zetesis".into()),
                installed: true,
                error: std::io::ErrorKind::NotFound.into(),
            },
            "gpu_executable_unavailable",
        ),
        (
            RunError::UnsupportedCombination {
                backend: crate::Backend::Gpu(Some(zetesis_backend::GpuApi::Metal)),
                grounder: crate::Grounder::Lazy,
            },
            "unsupported_combination",
        ),
        (
            RunError::PreparedInput {
                profile: crate::PreparedProfile::AcyclicSource,
                oracle: crate::Oracle::Countermodel,
                grounder: crate::Grounder::Lazy,
            },
            "prepared_input",
        ),
        (
            RunError::PublicationStopped(zetesis_algebra::Stop::Cancelled),
            "publication_stopped",
        ),
        (
            RunError::Model(zetesis_core::ModelError::Position {
                position: 1,
                atoms: 1,
            }),
            "model",
        ),
        (
            RunError::FormulaBatchShape {
                expected: 2,
                actual: 1,
            },
            "formula_batch_shape",
        ),
        (
            RunError::DeadlineTimer(io::Error::other("no thread")),
            "deadline_timer",
        ),
    ];
    for (error, kind) in errors {
        let original = error.to_string();
        let mut bytes = Vec::new();
        let retained = input_failure(
            &mut bytes,
            error.into(),
            &Options::parse_from(["zetesis", "--json"]),
        );
        assert_eq!(retained.to_string(), original);
        assert!(retained.partial_report.is_none());
        assert!(retained.secondary_output.is_none());
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value["outcome"]["error"]["kind"], kind);
        assert_eq!(value["outcome"]["status"], "failed");
        assert_eq!(value["outcome"]["coverage"], "unavailable");
        assert!(value["outcome"]["checked"].is_null());
        assert!(value["outcome"]["verified_models"].is_null());
        assert_eq!(value["models"], serde_json::json!([]));
    }
}

#[test]
fn formula_json_keeps_submission_limits_and_decoding_distinct() {
    let mut statistics = crate::FormulaExecutionStatistics::default();
    statistics.gpu_limits = Some(crate::FormulaDeviceLimits {
        work_per_candidate: 789,
        rounds_per_candidate: 17,
    });
    statistics.gpu_submitted_batches = 3;
    statistics.gpu_submitted_candidates = 10;
    statistics.gpu_batches = 2;
    statistics.gpu_candidates = 7;
    let mut out = Buffer::new(4096);
    super::execution_statistics(&mut out, Some(&statistics)).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&out.bytes).unwrap();
    assert_eq!(
        value["gpu_limits"],
        serde_json::json!({
            "work_per_candidate": 789, "rounds_per_candidate": 17
        })
    );
    assert_eq!(value["gpu_submitted_batches"], 3);
    assert_eq!(value["gpu_submitted_candidates"], 10);
    assert_eq!(value["gpu_batches"], 2);
    assert_eq!(value["gpu_candidates"], 7);
    let mut out = Buffer::new(4096);
    super::execution_statistics(
        &mut out,
        Some(&crate::FormulaExecutionStatistics::default()),
    )
    .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&out.bytes).unwrap();
    assert!(value["gpu_limits"].is_null());
}

#[test]
fn slate_json_reports_committed_slates_their_largest_and_resets() {
    let mut statistics = crate::FormulaExecutionStatistics::default();
    statistics.gpu_slates = 7;
    statistics.gpu_largest_slate = Some(32);
    statistics.gpu_slate_resets = 2;
    let mut out = Buffer::new(4096);
    super::execution_statistics(&mut out, Some(&statistics)).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&out.bytes).unwrap();
    assert_eq!(value["gpu_slates"], 7);
    assert_eq!(value["gpu_largest_slate"], 32);
    assert_eq!(value["gpu_slate_resets"], 2);
    let mut out = Buffer::new(4096);
    super::execution_statistics(
        &mut out,
        Some(&crate::FormulaExecutionStatistics::default()),
    )
    .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&out.bytes).unwrap();
    // Before the first commit, and on CPU routes, no slate was committed.
    assert_eq!(value["gpu_slates"], 0);
    assert!(value["gpu_largest_slate"].is_null());
    assert_eq!(value["gpu_slate_resets"], 0);
}

#[test]
fn lookahead_json_reports_slates_ahead_the_decision_and_hidden_device_time() {
    let mut statistics = crate::FormulaExecutionStatistics::default();
    statistics.gpu_lookahead_slates = 6;
    statistics.gpu_lookahead = Some(zetesis_solve::GpuLookahead::Declined(
        zetesis_solve::GpuLookaheadDeclined::RequestedAnswers,
    ));
    statistics.gpu_lookahead_device_ns = 900;
    statistics.gpu_lookahead_finish_wait_ns = 300;
    statistics.gpu_lookahead_unmeasured = 2;
    statistics.first_answer_ns = Some(1_234);
    let mut out = Buffer::new(4096);
    super::execution_statistics(&mut out, Some(&statistics)).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&out.bytes).unwrap();
    assert_eq!(value["gpu_lookahead_slates"], 6);
    assert_eq!(value["gpu_lookahead"], "requested_answers");
    assert_eq!(value["gpu_lookahead_device_ns"], 900);
    assert_eq!(value["gpu_lookahead_finish_wait_ns"], 300);
    assert_eq!(value["gpu_lookahead_unmeasured"], 2);
    assert_eq!(value["first_answer_ns"], 1_234);
    let mut out = Buffer::new(4096);
    super::execution_statistics(
        &mut out,
        Some(&crate::FormulaExecutionStatistics::default()),
    )
    .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&out.bytes).unwrap();
    // On CPU routes, and before a device session's first slate, nothing was decided.
    assert!(value["gpu_lookahead"].is_null());
    assert!(value["first_answer_ns"].is_null());
}

#[test]
fn tight_json_distinguishes_scheduled_and_completed_work() {
    let mut statistics = crate::FormulaExecutionStatistics::default();
    statistics.tight_work_per_candidate = Some(123);
    statistics.gpu_scheduled_work = Some(246);
    statistics.gpu_submitted_candidates = 2;
    let mut out = Buffer::new(4096);
    super::execution_statistics(&mut out, Some(&statistics)).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&out.bytes).unwrap();
    assert!(value["gpu_limits"].is_null());
    assert_eq!(value["tight_work_per_candidate"], 123);
    assert_eq!(value["gpu_scheduled_work"], 246);
    assert_eq!(value["gpu_work"], 0);
    assert_eq!(value["gpu_candidates"], 0);
}

#[test]
fn tight_json_separates_routes_from_update_availability() {
    let json = |statistics: &crate::FormulaExecutionStatistics| {
        let mut out = Buffer::new(4096);
        super::execution_statistics(&mut out, Some(statistics)).unwrap();
        serde_json::from_slice::<serde_json::Value>(&out.bytes).unwrap()
    };
    let mut statistics = crate::FormulaExecutionStatistics::default();
    let absent = json(&statistics);
    assert!(absent["gpu_tight_routes"].is_null());
    assert!(absent["gpu_tight_updates"].is_null());
    let mut routes = zetesis_solve::TightRouteStatistics::default();
    routes.complete = 4;
    routes.dense_reuse = 3;
    routes.beyond_threshold = 1;
    routes.update = 9;
    routes.nodes_evaluated = 85;
    statistics.gpu_tight_routes = Some(routes);
    let mut updates = zetesis_solve::TightUpdateStatistics::default();
    updates.availability = zetesis_solve::TightUpdateAvailability::GuardDeclined;
    updates.count_bits = Some(16);
    updates.steady_state = true;
    updates.guard_declined_batches = 3;
    statistics.gpu_tight_updates = Some(updates);
    let value = json(&statistics);
    assert_eq!(
        value["gpu_tight_routes"],
        serde_json::json!({
            "complete": 4, "dense_reuse": 3, "beyond_threshold": 1, "update": 9,
            "nodes_evaluated": 85,
        })
    );
    assert_eq!(
        value["gpu_tight_updates"],
        serde_json::json!({
            "availability": "guard_declined", "count_bits": 16, "steady_state": true,
            "guard_declined_batches": 3,
        })
    );
}

#[test]
fn residual_json_distinguishes_decoding_from_cpu_completion() {
    let mut statistics = crate::FormulaExecutionStatistics::default();
    statistics.gpu_residuals = Some(crate::FormulaResidualStatistics {
        fixed_point: 2,
        round_limit: 3,
        work_limit: 1,
    });
    let mut out = Buffer::new(4096);
    super::execution_statistics(&mut out, Some(&statistics)).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&out.bytes).unwrap();
    assert_eq!(
        value["gpu_residuals"],
        serde_json::json!({
            "fixed_point": 2, "round_limit": 3, "work_limit": 1,
        })
    );
    assert_eq!(value["cpu_residuals"], 0);
    let mut out = Buffer::new(4096);
    super::execution_statistics(
        &mut out,
        Some(&crate::FormulaExecutionStatistics::default()),
    )
    .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&out.bytes).unwrap();
    assert!(value["gpu_residuals"].is_null());
}

#[test]
fn compute_timing_json_distinguishes_absence_from_zero() {
    use zetesis_backend::{BatchComputeTime, ComputeTiming, ComputeTimingStatus};
    for (status, report, expected) in [
        (
            ComputeTimingStatus::NotRequested,
            BatchComputeTime::Unavailable,
            serde_json::Value::Null,
        ),
        (
            ComputeTimingStatus::Unsupported,
            BatchComputeTime::Unavailable,
            serde_json::Value::Null,
        ),
        (
            ComputeTimingStatus::Enabled,
            BatchComputeTime::Unavailable,
            serde_json::Value::Null,
        ),
        (
            ComputeTimingStatus::Enabled,
            BatchComputeTime::Measured(std::time::Duration::ZERO),
            serde_json::json!(0),
        ),
    ] {
        let mut timing = ComputeTiming::new(status);
        timing.record(report);
        let measured_report = matches!(report, BatchComputeTime::Measured(_));
        let mut statistics = crate::FormulaExecutionStatistics::default();
        statistics.gpu_compute_timing = timing;
        let mut out = Buffer::new(4096);
        super::execution_statistics(&mut out, Some(&statistics)).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&out.bytes).unwrap();
        let measured = &value["gpu_compute_timing"];
        assert_eq!(measured["status"], status.label());
        assert_eq!(measured["elapsed_ns"], expected);
        assert_eq!(measured["complete"], measured_report);
        assert_eq!(measured["measured_batches"], u64::from(measured_report));
        assert_eq!(
            measured["unavailable_batches"],
            u64::from(status != ComputeTimingStatus::NotRequested && !measured_report)
        );
        assert_eq!(measured["scope"], "decoded_device_compute_passes");
        assert_eq!(measured["host_oracle"], "excluded");
    }
}

#[test]
fn compute_timing_json_reports_unresolved_batches_and_flushes() {
    use zetesis_backend::{BatchComputeTime, ComputeTiming, ComputeTimingStatus};
    let mut timing = ComputeTiming::new(ComputeTimingStatus::Enabled);
    timing.record(BatchComputeTime::Measured(std::time::Duration::from_nanos(
        7,
    )));
    timing.unresolved_batches = 1;
    timing.record_flush_submission();
    let mut statistics = crate::FormulaExecutionStatistics::default();
    statistics.gpu_compute_timing = timing;
    let mut out = Buffer::new(4096);
    super::execution_statistics(&mut out, Some(&statistics)).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&out.bytes).unwrap();
    let measured = &value["gpu_compute_timing"];
    assert_eq!(measured["unresolved_batches"], 1);
    assert_eq!(measured["flush_submissions"], 1);
    assert_eq!(measured["complete"], false);
}
