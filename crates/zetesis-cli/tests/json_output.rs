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
