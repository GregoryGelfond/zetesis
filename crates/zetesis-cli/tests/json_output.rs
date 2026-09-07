//! Typed streaming JSON preserves the ordinary solver's semantic and output contracts.

use std::io::{self, Write};
use std::process::Command;

use clap::Parser;
use serde_json::{Value as Json, json};
use zetesis_cli::{
    Completion, Options, Report, RunError, RunFailure, run_detailed_with_diagnostics,
};
use zetesis_cpu::Control;

fn options(extra: &[&str]) -> Options {
    Options::try_parse_from(
        [
            "zetesis",
            "--json",
            "--backend",
            "cpu",
            "--workers",
            "1",
            "--models",
            "0",
        ]
        .into_iter()
        .chain(extra.iter().copied()),
    )
    .unwrap()
}
fn solve(source: &str, options: &Options) -> (Result<Report, RunFailure>, Json) {
    let mut output = Vec::new();
    let result = run_detailed_with_diagnostics(
        source.into(),
        options,
        &mut output,
        &mut io::sink(),
        &Control::default(),
    );
    let value = serde_json::from_slice(&output)
        .unwrap_or_else(|error| panic!("{error}: {}", String::from_utf8_lossy(&output)));
    (result, value)
}

#[test]
fn closure_and_formula_routes_emit_typed_full_models_with_distinct_hidden_displays() {
    for oracle in ["auto", "countermodel"] {
        let (result, value) = solve("{a;b}. #show.", &options(&["--oracle", oracle]));
        let report = result.unwrap();
        assert_eq!(
            (report.models, report.completion),
            (4, Completion::Exhausted)
        );
        assert_eq!(value["schema"], 1);
        assert_eq!(value["outcome"]["published_models"], 4);
        assert_eq!(value["outcome"]["verified_models"], 4);
        assert_eq!(value["outcome"]["coverage"], "exhausted");
        assert!(value["statistics"].is_null());
        let models = value["models"].as_array().unwrap();
        let mut full = std::collections::BTreeSet::new();
        for (index, model) in models.iter().enumerate() {
            assert_eq!(model["number"], index + 1);
            assert_eq!(model["model"]["shown"]["atom_indices"], json!([]));
            full.insert(model["model"]["full_model"].to_string());
        }
        assert_eq!(full.len(), 4);
    }
}

#[test]
fn observations_and_objective_ties_retain_channels_and_priorities() {
    for bounds in ["0", "10000000"] {
        let (result, value) = solve(
            "a. {b;c}. #show a. #minimize{0@2,k:a;0@-1,j:b}.",
            &options(&["--max-objective-bound-work", bounds]),
        );
        assert_eq!(result.unwrap().models, 4);
        assert_eq!(value["outcome"]["optimization"]["optimal"], true);
        assert_eq!(value["outcome"]["optimization"]["tied_models"], 4);
        for model in value["models"].as_array().unwrap() {
            assert_eq!(
                model["model"]["costs"],
                json!([{"priority":2,"value":0},{"priority":-1,"value":0}])
            );
            assert_eq!(
                model["model"]["shown"]["terms"],
                json!([[{"kind":"symbol","value":"a"}]])
            );
            assert!(
                !model["model"]["shown"]["atom_indices"]
                    .as_array()
                    .unwrap()
                    .is_empty()
            );
        }
    }
}

#[test]
fn empty_model_unsat_and_requested_coverage_are_different_outcomes() {
    for (source, expected_status, models) in [("", "satisfiable", 1), (":-.", "unsatisfiable", 0)] {
        let (result, value) = solve(source, &options(&[]));
        assert_eq!(result.unwrap().models, models);
        assert_eq!(value["outcome"]["status"], expected_status);
        assert_eq!(value["outcome"]["completion"], "exhausted");
    }
    let mut limited = options(&[]);
    limited.models = 1;
    let (result, value) = solve("{a}.", &limited);
    assert_eq!(result.unwrap().completion, Completion::RequestedModels);
    assert_eq!(value["outcome"]["completion"], "requested_models");
    assert_eq!(value["outcome"]["coverage"], "partial");
}

#[test]
fn interruption_preserves_incumbent_evidence_without_claiming_optimum() {
    let (result, value) = solve(
        "1 {a;b} 1. #minimize{1,a:a;2,b:b}.",
        &options(&[
            "--oracle",
            "countermodel",
            "--max-candidates",
            "1",
            "--max-objective-bound-work",
            "0",
        ]),
    );
    assert_eq!(result.unwrap().completion, Completion::Interrupted);
    assert_eq!(value["outcome"]["status"], "incomplete");
    assert_eq!(value["outcome"]["coverage"], "partial");
    assert_eq!(value["outcome"]["optimization"]["optimal"], false);
    assert_eq!(value["outcome"]["interruption"]["kind"], "countermodel");
    assert_eq!(value["outcome"]["interruption"]["code"], "candidate_limit");
}

#[test]
fn source_and_observation_failures_complete_error_documents_with_partial_evidence() {
    for (source, args, expected) in [
        ("p(.", vec![], "expansion"),
        (
            "a. #show a.",
            vec!["--max-observation-work", "0"],
            "observation",
        ),
    ] {
        let (result, value) = solve(source, &options(&args));
        assert!(result.is_err());
        assert_eq!(value["outcome"]["status"], "failed");
        assert_eq!(value["outcome"]["error"]["kind"], expected);
        assert!(value["outcome"]["completion"].is_null());
        assert_eq!(value["models"], json!([]));
        if expected == "observation" {
            assert_eq!(value["outcome"]["verified_models"], 1);
            assert!(
                result
                    .unwrap_err()
                    .partial_report
                    .unwrap()
                    .summary_published
            );
        }
    }
}

#[test]
fn typed_statistics_are_opt_in_and_do_not_reparse_phase_prose() {
    let (result, value) = solve(
        "a | b.",
        &options(&["--stats", "--completion-workers", "2"]),
    );
    let report = result.unwrap();
    assert!(report.phase_timings.is_some());
    assert_eq!(value["statistics"]["search"]["stable_models"], 2);
    assert_eq!(value["statistics"]["phase_timings"]["schema"], 2);
    assert!(value["statistics"]["phase_timings"]["measurements"]["closure_membership"].is_null());
    assert_eq!(
        value["statistics"]["execution"]["completion"]["requested_workers"],
        2
    );
}

struct Cut {
    capacity: usize,
    bytes: Vec<u8>,
}
impl Write for Cut {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let count = bytes
            .len()
            .min(self.capacity.saturating_sub(self.bytes.len()));
        if count == 0 && !bytes.is_empty() {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        self.bytes.extend_from_slice(&bytes[..count]);
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn every_output_prefix_preserves_publication_counts_and_never_repairs_a_broken_record() {
    let options = options(&[]);
    let mut reference = Vec::new();
    run_detailed_with_diagnostics(
        "a.".into(),
        &options,
        &mut reference,
        &mut io::sink(),
        &Control::default(),
    )
    .unwrap();
    let model_end = reference.iter().position(|byte| *byte == b'\n').unwrap() + 1;
    for capacity in 0..reference.len() {
        let mut output = Cut {
            capacity,
            bytes: Vec::new(),
        };
        let failure = run_detailed_with_diagnostics(
            "a.".into(),
            &options,
            &mut output,
            &mut io::sink(),
            &Control::default(),
        )
        .unwrap_err();
        assert!(
            matches!(*failure.cause, RunError::Output(_)),
            "cut {capacity}"
        );
        assert_eq!(output.bytes, reference[..capacity], "cut {capacity}");
        if let Some(partial) = failure.partial_report {
            assert_eq!(
                partial.published_models,
                usize::from(capacity >= model_end),
                "cut {capacity}"
            );
            assert!(!partial.summary_published);
        }
        assert!(serde_json::from_slice::<Json>(&output.bytes).is_err());
    }
}

#[test]
fn model_record_limit_precedes_any_model_prefix_and_terminal_limit_stays_incomplete() {
    let source = format!("p(\"{}\").", "x".repeat(1500));
    let (result, value) = solve(&source, &options(&["--max-json-record-bytes", "1000"]));
    assert!(matches!(
        *result.unwrap_err().cause,
        RunError::JsonRecord(_)
    ));
    assert_eq!(value["models"], json!([]));
    assert_eq!(value["outcome"]["published_models"], 0);
    assert_eq!(value["outcome"]["verified_models"], 1);
    let mut output = Vec::new();
    let failure = run_detailed_with_diagnostics(
        "a.".into(),
        &options(&["--max-json-record-bytes", "0"]),
        &mut output,
        &mut io::sink(),
        &Control::default(),
    )
    .unwrap_err();
    assert!(matches!(*failure.cause, RunError::JsonRecord(_)));
    assert!(failure.secondary_output.is_some());
    assert!(serde_json::from_slice::<Json>(&output).is_err());
}

#[test]
fn process_input_refusal_is_json_and_human_output_remains_default() {
    let result = Command::new(env!("CARGO_BIN_EXE_zetesis"))
        .args(["--json", "/does/not/exist/zetesis-json-input.lp"])
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(2));
    let value: Json = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(value["outcome"]["error"]["kind"], "bundle_load");
    let mut human = options(&[]);
    human.json = false;
    let mut output = Vec::new();
    run_detailed_with_diagnostics(
        "a.".into(),
        &human,
        &mut output,
        &mut io::sink(),
        &Control::default(),
    )
    .unwrap();
    assert!(
        std::str::from_utf8(&output)
            .unwrap()
            .starts_with("Answer: 1\na\nSATISFIABLE\nCoverage: exhausted\n")
    );
}

#[test]
fn retryable_interrupted_write_does_not_poison_document_completion() {
    struct Interrupted {
        bytes: Vec<u8>,
        first: bool,
    }
    impl Write for Interrupted {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.first {
                self.first = false;
                return Err(io::ErrorKind::Interrupted.into());
            }
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let mut output = Interrupted {
        bytes: Vec::new(),
        first: true,
    };
    run_detailed_with_diagnostics(
        "a.".into(),
        &options(&[]),
        &mut output,
        &mut io::sink(),
        &Control::default(),
    )
    .unwrap();
    assert_eq!(
        serde_json::from_slice::<Json>(&output.bytes).unwrap()["outcome"]["status"],
        "satisfiable"
    );
}

#[test]
fn failed_second_optimal_tie_keeps_proved_coverage_and_first_publication() {
    let source = "a. {b}. #minimize{0@1,k:a}.";
    let options = options(&[]);
    let mut reference = Vec::new();
    run_detailed_with_diagnostics(
        source.into(),
        &options,
        &mut reference,
        &mut io::sink(),
        &Control::default(),
    )
    .unwrap();
    let endings: Vec<_> = reference
        .iter()
        .enumerate()
        .filter_map(|(index, byte)| (*byte == b'\n').then_some(index + 1))
        .collect();
    assert_eq!(endings.len(), 2);
    for capacity in [
        endings[0],
        endings[0] + 1,
        usize::midpoint(endings[0], endings[1]),
        endings[1] - 1,
    ] {
        let mut output = Cut {
            capacity,
            bytes: Vec::new(),
        };
        let failure = run_detailed_with_diagnostics(
            source.into(),
            &options,
            &mut output,
            &mut io::sink(),
            &Control::default(),
        )
        .unwrap_err();
        let partial = failure.partial_report.unwrap();
        assert_eq!(partial.completion, Some(Completion::Exhausted));
        assert_eq!((partial.published_models, partial.verified_models), (1, 2));
        assert_eq!(partial.optimization.unwrap().tied_models, 2);
        assert!(!partial.summary_published);
        assert_eq!(output.bytes, reference[..capacity]);
    }
}

#[test]
fn view_refusal_after_optimization_keeps_exhaustion_without_claiming_unsat() {
    let source = format!("p(\"{}\"). #minimize{{0@1,k:p(X)}}.", "x".repeat(1500));
    let (result, value) = solve(&source, &options(&["--max-json-record-bytes", "1000"]));
    let partial = result.unwrap_err().partial_report.unwrap();
    assert_eq!(partial.completion, Some(Completion::Exhausted));
    assert_eq!((partial.published_models, partial.verified_models), (0, 1));
    assert_eq!(partial.optimization.unwrap().tied_models, 1);
    assert!(partial.summary_published);
    assert_eq!(value["outcome"]["status"], "failed");
    assert_eq!(value["outcome"]["coverage"], "exhausted");
    assert_eq!(value["outcome"]["optimization"]["optimal"], true);
    assert_eq!(value["models"], json!([]));
}

#[test]
fn eager_and_lazy_stage_views_match_the_typed_partition() {
    for (source, extra, mode) in [
        ("{a}.", vec!["--stats", "--grounder", "eager"], "eager"),
        (
            "{a}.",
            vec!["--stats", "--grounder", "lazy"],
            "lazy_interleaved",
        ),
        ("a | b.", vec!["--stats"], "eager"),
    ] {
        let (result, value) = solve(source, &options(&extra));
        let report = result.unwrap();
        let typed = report.phase_timings.unwrap().stages;
        let stages = &value["statistics"]["stage_timings"];
        assert_eq!(stages["schema"], 1);
        assert_eq!(stages["grounding_mode"], mode);
        assert_eq!(stages["complete"], true);
        assert_eq!(stages["json_envelope"], "excluded");
        let mut elapsed = 0_u64;
        for stage in zetesis_cli::SolveStage::ALL {
            let measured = &stages["measurements"][stage.label()];
            match typed.get(stage) {
                Some(measurement) => {
                    assert_eq!(measured["calls"], measurement.calls);
                    assert_eq!(
                        measured["elapsed_ns"].as_u64().map(u128::from),
                        Some(measurement.elapsed.as_nanos())
                    );
                    elapsed += measured["elapsed_ns"].as_u64().unwrap();
                }
                None => assert!(measured.is_null()),
            }
        }
        assert_eq!(
            elapsed + stages["unattributed_elapsed_ns"].as_u64().unwrap(),
            stages["driver_elapsed_ns"].as_u64().unwrap()
        );
        if mode == "lazy_interleaved" {
            assert!(stages["measurements"]["grounding"].is_null());
        }
    }
}

#[test]
fn resource_interruptions_have_stable_codes_and_never_claim_unsatisfiability() {
    let cases: &[(&str, &[&str], &str, &str)] = &[
        (
            "{a}.",
            &["--oracle", "closure", "--max-work", "0"],
            "oracle",
            "work_limit",
        ),
        (
            "a.",
            &[
                "--oracle",
                "closure",
                "--grounder",
                "lazy",
                "--max-atoms",
                "0",
            ],
            "oracle",
            "derived_atom_limit",
        ),
        (
            "{a}.",
            &["--oracle", "closure", "--max-candidates", "0"],
            "oracle",
            "candidate_limit",
        ),
        (
            "{a}.",
            &["--oracle", "closure", "--max-carrier-atoms", "0"],
            "oracle",
            "carrier_limit",
        ),
        (
            "{a;b}.",
            &["--oracle", "countermodel", "--max-search-work", "0"],
            "countermodel",
            "work_limit",
        ),
        (
            "{a;b}.",
            &["--oracle", "countermodel", "--max-search-decisions", "0"],
            "countermodel",
            "decision_limit",
        ),
        (
            "a | b.",
            &[
                "--oracle",
                "countermodel",
                "--completion-workers",
                "2",
                "--max-completion-scratch-bytes",
                "0",
            ],
            "countermodel",
            "completion_scratch",
        ),
    ];
    for &(source, extra, kind, code) in cases {
        let (result, value) = solve(source, &options(extra));
        let report = result.unwrap();
        assert_eq!(report.completion, Completion::Interrupted, "{extra:?}");
        assert!(report.interruption.is_some());
        assert_eq!(value["outcome"]["status"], "incomplete", "{extra:?}");
        assert_eq!(value["outcome"]["coverage"], "partial");
        assert_eq!(value["outcome"]["interruption"]["kind"], kind);
        assert_eq!(value["outcome"]["interruption"]["code"], code, "{extra:?}");
        assert_eq!(value["outcome"]["published_models"], report.models);
        assert_eq!(value["models"].as_array().unwrap().len(), report.models);
        assert!(value["outcome"]["error"].is_null());
    }
}

#[test]
fn objective_and_incumbent_refusals_retain_typed_partial_evidence() {
    for (flag, code) in [
        ("--max-objective-work", "work_limit"),
        ("--max-objective-bindings", "binding_limit"),
        ("--max-objective-keys", "key_limit"),
        ("--max-objective-key-bytes", "key_bytes_limit"),
        ("--max-optimal-models", "models"),
        ("--max-optimal-atoms", "atoms"),
        ("--max-optimal-bytes", "bytes"),
    ] {
        let (result, value) = solve(
            "a. #minimize{1@0,k:a}.",
            &options(&[flag, "0", "--max-objective-bound-work", "0"]),
        );
        let report = result.unwrap();
        assert_eq!(report.completion, Completion::Interrupted, "{flag}");
        assert_eq!(value["outcome"]["status"], "incomplete");
        assert_eq!(value["outcome"]["verified_models"], 1);
        assert_eq!(value["outcome"]["published_models"], 0);
        assert_eq!(value["models"], json!([]));
        assert!(value["outcome"]["optimization"].is_null());
        assert_eq!(value["outcome"]["interruption"]["code"], code);
        let kind = if flag.starts_with("--max-objective") {
            "objective"
        } else {
            "incumbent"
        };
        assert_eq!(value["outcome"]["interruption"]["kind"], kind);
        match report.interruption.unwrap() {
            zetesis_cli::Interruption::Objective(error) => {
                assert!(matches!(
                    error.kind(),
                    zetesis_objective::ErrorKind::Stopped(_)
                ));
            }
            zetesis_cli::Interruption::Incumbent(_) => assert_eq!(kind, "incumbent"),
            other => panic!("unexpected interruption: {other:?}"),
        }
    }

    // A fully scored tie that could not be retained still belongs in the score
    // count. Published models are only the earlier retained, valid incumbent.
    let (result, value) = solve(
        "{a}. #minimize{0@0,k:a}.",
        &options(&[
            "--max-optimal-models",
            "1",
            "--max-objective-bound-work",
            "0",
        ]),
    );
    let best = result.unwrap().optimization.unwrap();
    assert_eq!((best.tied_models, best.scored_models), (2, 2));
    assert_eq!(value["outcome"]["optimization"]["optimal"], false);
    assert_eq!(value["outcome"]["optimization"]["tied_models"], 2);
    assert_eq!(value["outcome"]["published_models"], 1);
    assert_eq!(value["outcome"]["verified_models"], 2);
    assert_eq!(value["models"].as_array().unwrap().len(), 1);
}

#[test]
fn cancelled_and_expired_requests_are_complete_json_without_fabricated_models() {
    for oracle in ["closure", "countermodel", "auto"] {
        for expired in [false, true] {
            let control = if expired {
                Control::with_deadline(std::time::Instant::now())
            } else {
                let control = Control::default();
                control.cancel();
                control
            };
            // Auto must retry the formula route before observing this control;
            // closure and explicit countermodel exercise their own boundaries.
            let source = if oracle == "auto" { "a | b." } else { "a." };
            let mut bytes = Vec::new();
            let report = run_detailed_with_diagnostics(
                source.into(),
                &options(&["--oracle", oracle, "--stats"]),
                &mut bytes,
                &mut io::sink(),
                &control,
            )
            .unwrap();
            let value: Json = serde_json::from_slice(&bytes).unwrap();
            assert_eq!((report.models, report.checked), (0, 0));
            assert_eq!(report.completion, Completion::Interrupted);
            assert_eq!(value["models"], json!([]));
            assert_eq!(value["outcome"]["status"], "incomplete");
            assert_eq!(
                value["outcome"]["interruption"]["kind"],
                if oracle == "closure" {
                    "oracle"
                } else {
                    "countermodel"
                }
            );
            assert_eq!(
                value["outcome"]["interruption"]["code"],
                if expired { "deadline" } else { "cancelled" }
            );
            assert!(value["statistics"]["search"].is_null());
            assert!(value["statistics"]["execution"].is_null());
            assert!(value["statistics"]["stage_timings"].is_object());
        }
    }
}

#[test]
fn source_setup_refusals_emit_typed_error_envelopes_without_semantic_coverage() {
    for (source, extra, kind) in [
        (
            "a.",
            vec!["--oracle", "countermodel", "--grounder", "lazy"],
            "unsupported_oracle",
        ),
        ("a.", vec!["--grounder", "lazy"], "unsupported_combination"),
        ("p(.", vec!["--oracle", "countermodel"], "formula_admission"),
        (
            "a.",
            vec![
                "--oracle",
                "closure",
                "--grounder",
                "eager",
                "--max-ground-rules",
                "0",
            ],
            "static",
        ),
    ] {
        let mut configured = options(&extra);
        if kind == "unsupported_combination" {
            configured.backend = zetesis_cli::Backend::Nvidia;
        }
        let (result, value) = solve(source, &configured);
        let failure = result.unwrap_err();
        assert!(failure.partial_report.is_none(), "{extra:?}: {failure:?}");
        assert_eq!(value["outcome"]["error"]["kind"], kind);
        assert_eq!(value["outcome"]["status"], "failed");
        assert_eq!(value["outcome"]["coverage"], "unavailable");
        assert!(value["outcome"]["verified_models"].is_null());
        assert_eq!(value["models"], json!([]));
    }
}

#[test]
fn statistics_writer_failure_preserves_exhaustion_and_original_view_refusals() {
    for refuse_view in [false, true] {
        let source = if refuse_view {
            format!(
                "p(\"{}\"). #show shown:p(X). #minimize{{0@1,k:p(X)}}.",
                "x".repeat(1500)
            )
        } else {
            "a. {b}. #minimize{0@1,k:a}.".to_owned()
        };
        let mut configured = options(&["--stats"]);
        if refuse_view {
            configured.max_observation_work = 0;
        }
        // Stop diagnostics at the statistics boundary, after solving and model
        // rendering have already succeeded or established their original error.
        let mut reference = Vec::new();
        let _ = run_detailed_with_diagnostics(
            source.clone(),
            &configured,
            &mut io::sink(),
            &mut reference,
            &Control::default(),
        );
        let text = std::str::from_utf8(&reference).unwrap();
        let capacity = text.find("Statistics:").unwrap();
        let mut diagnostics = Cut {
            capacity,
            bytes: Vec::new(),
        };
        let mut bytes = Vec::new();
        let failure = run_detailed_with_diagnostics(
            source,
            &configured,
            &mut bytes,
            &mut diagnostics,
            &Control::default(),
        )
        .unwrap_err();
        let value: Json = serde_json::from_slice(&bytes).unwrap();
        let partial = failure.partial_report.unwrap();
        assert!(partial.summary_published);
        assert_eq!(partial.completion, Some(Completion::Exhausted));
        assert_eq!(value["outcome"]["status"], "failed");
        assert_eq!(value["outcome"]["coverage"], "exhausted");
        assert_eq!(value["outcome"]["optimization"]["optimal"], true);
        assert!(value["statistics"]["stage_timings"].is_object());
        assert_eq!(
            value["outcome"]["error"]["secondary_output_failure"],
            refuse_view
        );
        if refuse_view {
            assert!(matches!(*failure.cause, RunError::Observation(_)));
            assert!(failure.secondary_output.is_some());
            assert_eq!(value["outcome"]["error"]["kind"], "observation");
            assert_eq!((partial.published_models, partial.verified_models), (0, 1));
        } else {
            assert!(matches!(*failure.cause, RunError::Output(_)));
            assert!(failure.secondary_output.is_none());
            assert_eq!(value["outcome"]["error"]["kind"], "output");
            assert_eq!((partial.published_models, partial.verified_models), (2, 2));
        }
    }
}

#[test]
fn failed_error_footer_keeps_the_source_cause_and_exact_written_prefix() {
    let configured = options(&[]);
    let mut reference = Vec::new();
    let failure = run_detailed_with_diagnostics(
        "p(.".into(),
        &configured,
        &mut reference,
        &mut io::sink(),
        &Control::default(),
    )
    .unwrap_err();
    assert!(matches!(*failure.cause, RunError::Expansion(_)));
    let header_end = reference.iter().position(|byte| *byte == b'[').unwrap() + 1;
    for capacity in header_end..reference.len() {
        let mut output = Cut {
            capacity,
            bytes: Vec::new(),
        };
        let failure = run_detailed_with_diagnostics(
            "p(.".into(),
            &configured,
            &mut output,
            &mut io::sink(),
            &Control::default(),
        )
        .unwrap_err();
        assert!(
            matches!(*failure.cause, RunError::Expansion(_)),
            "cut {capacity}"
        );
        assert_eq!(
            failure.secondary_output.unwrap().kind(),
            io::ErrorKind::BrokenPipe
        );
        assert!(failure.partial_report.is_none());
        assert_eq!(output.bytes, reference[..capacity]);
        assert!(serde_json::from_slice::<Json>(&output.bytes).is_err());
    }
}

#[test]
fn terminal_record_ceiling_is_inclusive_and_cannot_erase_published_models() {
    let mut reference = Vec::new();
    let mut configured = options(&[]);
    run_detailed_with_diagnostics(
        String::new(),
        &configured,
        &mut reference,
        &mut io::sink(),
        &Control::default(),
    )
    .unwrap();
    let model_end = reference.iter().position(|byte| *byte == b'\n').unwrap() + 1;
    let footer_bytes = reference.len() - model_end;
    assert!(footer_bytes > model_end);
    for maximum in [footer_bytes - 1, footer_bytes, footer_bytes + 1] {
        configured.max_json_record_bytes = maximum;
        let mut bytes = Vec::new();
        let result = run_detailed_with_diagnostics(
            String::new(),
            &configured,
            &mut bytes,
            &mut io::sink(),
            &Control::default(),
        );
        if maximum < footer_bytes {
            let failure = result.unwrap_err();
            assert!(matches!(
                *failure.cause,
                RunError::JsonRecord(zetesis_themelios::observation::ViewError::Bytes)
            ));
            assert!(failure.secondary_output.is_none());
            let partial = failure.partial_report.unwrap();
            assert_eq!((partial.published_models, partial.verified_models), (1, 1));
            assert_eq!(partial.completion, Some(Completion::Exhausted));
            assert!(!partial.summary_published);
            assert_eq!(bytes, reference[..model_end]);
        } else {
            assert_eq!(result.unwrap().models, 1);
            assert_eq!(bytes, reference);
            let value: Json = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(value["outcome"]["status"], "satisfiable");
        }
    }
}
