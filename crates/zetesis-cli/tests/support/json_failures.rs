//! Machine-visible failure identity and output publication boundaries.

use super::{Buffer, Document, input_failure, write_interruption};
use crate::test_writer::BoundedWriter;
use crate::{Interruption, Options, RunError};
use clap::Parser;
use std::io::{self, Write};

#[test]
fn legacy_metadata_cannot_establish_optimality() {
    let options = Options::parse_from([
        "zetesis",
        "--json",
        "--backend",
        "cpu",
        "--models",
        "0",
        "--max-json-record-bytes",
        "1000",
    ]);
    let source = format!("p(\"{}\"). #minimize{{0@1,k:p(X)}}.", "x".repeat(1500));
    let failure = crate::run_detailed_with_diagnostics(
        source,
        &options,
        &mut Vec::new(),
        &mut io::sink(),
        &zetesis_cpu::Control::default(),
    )
    .unwrap_err();
    let partial = failure.partial_report.as_ref().unwrap();
    assert_eq!(partial.completion, Some(crate::Completion::Exhausted));
    assert!(partial.optimization.is_some());
    // The compatibility conversion retains public counters, but discards
    // the session's immutable semantic outcome and its optimality evidence.
    let failure = crate::PublicationFailure::from(failure);
    assert!(failure.semantic().is_none());
    let record = super::summary(&Err(failure), 4096).unwrap();
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
    use zetesis_cpu::Stop;
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
    use zetesis_sat::Incomplete;
    for (stop, code) in [
        (Incomplete::Cancelled, "cancelled"),
        (Incomplete::Deadline, "deadline"),
        (Incomplete::WorkLimit, "work_limit"),
        (Incomplete::DecisionLimit, "decision_limit"),
        (Incomplete::CandidateLimit, "candidate_limit"),
        (Incomplete::PendingBytes, "pending_bytes"),
        (Incomplete::CompletionScratch, "completion_scratch"),
        (Incomplete::BatchCandidateLimit, "batch_candidate_limit"),
        (Incomplete::PendingBatch, "pending_batch"),
        (Incomplete::Allocation, "allocation"),
        (Incomplete::WrongTheory, "wrong_theory"),
        (Incomplete::ClosedEnumerator, "closed_enumerator"),
        (Incomplete::LateCertificate, "late_certificate"),
        (Incomplete::InvalidWitness, "invalid_witness"),
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
            &Options::parse_from(["zetesis", "--json"]),
        )
        .unwrap_err();
    assert!(error.to_string().contains("original cause"));
    assert_eq!(sink.0, b"{\"schema\":1,\"format\":\"zetesis\",\"models\":[");
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
        (RunError::Batch(zetesis_cpu::BatchError::Busy), "batch"),
        (RunError::BackendUnavailable, "backend_unavailable"),
        (
            RunError::UnsupportedCombination {
                backend: crate::Backend::Metal,
                grounder: crate::Grounder::Lazy,
            },
            "unsupported_combination",
        ),
        (
            RunError::PreparedInput {
                profile: crate::PreparedProfile::Relational,
                oracle: crate::Oracle::Countermodel,
                grounder: crate::Grounder::Lazy,
            },
            "prepared_input",
        ),
        (
            RunError::Formula(zetesis_ferraris::AdmissionError::Limit),
            "formula",
        ),
        (
            RunError::PublicationStopped(zetesis_cpu::Stop::Cancelled),
            "publication_stopped",
        ),
        (RunError::Words(zetesis_core::WordError::TailBits), "words"),
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
        (RunError::LazyStatisticsOverflow, "lazy_statistics_overflow"),
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
    let statistics = crate::FormulaExecutionStatistics {
        gpu_limits: Some(crate::FormulaDeviceLimits {
            work_per_candidate: 789,
            rounds_per_candidate: 17,
        }),
        gpu_submitted_batches: 3,
        gpu_submitted_candidates: 10,
        gpu_batches: 2,
        gpu_candidates: 7,
        ..Default::default()
    };
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
