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

fn hidden_choices(oracle: &str) -> (Report, Json) {
    let (result, value) = solve("{a}. {b}. #show.", &options(&["--oracle", oracle]));
    (result.unwrap(), value)
}

#[test]
fn elenctic_comments_do_not_change_answer_sets() {
    // Contradictory external test expectations must have no solver effect.
    let source = "a. {b}. #show a/0. #show b/0.";
    let annotated =
        format!("% @expect unsat\n% @count 0\n% @cost {{99}}\n% @model {{absent}}\n{source}");
    for oracle in ["closure", "countermodel"] {
        let options = options(&["--oracle", oracle]);
        let (plain, expected) = solve(source, &options);
        let (annotated, actual) = solve(&annotated, &options);
        for report in [plain.unwrap(), annotated.unwrap()] {
            assert_eq!(report.completion, Completion::Exhausted);
            assert_eq!(report.models, 2);
            assert_eq!(
                report.countermodel_statistics.is_some(),
                oracle == "countermodel"
            );
        }
        assert_eq!(actual["models"], expected["models"]);
    }
}

#[test]
fn explicit_oracles_select_distinct_routes() {
    for (oracle, formula_route) in [("closure", false), ("countermodel", true)] {
        let (report, _) = hidden_choices(oracle);
        assert_eq!(report.countermodel_statistics.is_some(), formula_route);
    }
}

#[test]
fn hidden_display_preserves_full_model_identity() {
    let a = json!({"predicate":"a","sign":"positive","arguments":[]});
    let b = json!({"predicate":"b","sign":"positive","arguments":[]});
    let expected = [json!([]), json!([a]), json!([b]), json!([a, b])]
        .into_iter()
        .map(|model| model.to_string())
        .collect::<std::collections::BTreeSet<_>>();
    for oracle in ["closure", "countermodel"] {
        let (report, value) = hidden_choices(oracle);
        assert_eq!(
            (report.models, report.completion),
            (4, Completion::Exhausted)
        );
        assert_eq!(value["outcome"]["published_models"], 4);
        assert_eq!(value["outcome"]["verified_models"], 4);
        assert_eq!(value["outcome"]["coverage"], "exhausted");
        let models = value["models"].as_array().unwrap();
        let mut full = std::collections::BTreeSet::new();
        for model in models {
            assert_eq!(model["model"]["shown"]["atom_indices"], json!([]));
            full.insert(model["model"]["full_model"].to_string());
        }
        assert_eq!(full, expected);
    }
}

#[test]
fn model_numbers_follow_publication_order() {
    for oracle in ["closure", "countermodel"] {
        let (_, value) = hidden_choices(oracle);
        let models = value["models"].as_array().unwrap();
        assert_eq!(models.len(), 4);
        for (index, model) in models.iter().enumerate() {
            assert_eq!(model["number"], index + 1);
        }
    }
}

#[test]
fn json_documents_identify_the_schema() {
    for oracle in ["closure", "countermodel"] {
        let (_, value) = hidden_choices(oracle);
        assert_eq!(value["schema"], 1);
    }
}

#[test]
fn statistics_are_absent_without_opt_in() {
    for oracle in ["closure", "countermodel"] {
        let (_, value) = hidden_choices(oracle);
        assert!(value["statistics"].is_null());
    }
}

#[test]
fn support_statistics_identify_the_outer_restriction() {
    for (source, status) in [("a | b.", "applied"), ("{a;b}.", "not_applicable")] {
        let (report, value) = solve(source, &options(&["--stats", "--oracle", "countermodel"]));
        let report = report.unwrap();
        let measured = report.countermodel_statistics.unwrap().support.unwrap();
        let support = &value["statistics"]["search"]["necessary_support"];
        assert_eq!(support["status"], status);
        assert_eq!(support["construction_work"], measured.construction_work);
        assert_eq!(support["encoding_work"], measured.encoding_work);
    }
}

#[test]
fn completion_statistics_distinguish_requested_storage() {
    let (report, value) = solve(
        "{a;b}.",
        &options(&[
            "--stats",
            "--oracle",
            "countermodel",
            "--completion-workers",
            "2",
        ]),
    );
    let measured = report.unwrap().formula_execution.unwrap().completion;
    let stats = &value["statistics"]["execution"]["completion"];
    assert!(measured.requested_scratch_bytes > 0);
    assert!(measured.peak_scratch_bytes > 0);
    assert_eq!(
        stats["requested_scratch_bytes"],
        measured.requested_scratch_bytes
    );
    assert_eq!(stats["peak_scratch_bytes"], measured.peak_scratch_bytes);
    assert_eq!(stats["entered"], 4);
}

#[test]
fn candidate_statistics_preserve_restriction_accounting() {
    let (report, value) = solve(
        "{a}. {b}. :- a,b.",
        &options(&["--stats", "--grounder", "lazy"]),
    );
    let report = report.unwrap();
    assert_eq!(report.models, 3);
    let measured = report.candidate_statistics.unwrap();
    let stats = &value["statistics"]["candidate_restrictions"];
    assert_eq!(stats["work"], measured.restriction_work);
    assert_eq!(stats["conjunctions"], 1);
    assert_eq!(stats["skipped_intervals"], 1);
    assert_eq!(
        stats["prepared_atom_occurrences"],
        measured.restriction_atoms
    );
    assert_eq!(stats["copied_payload_bytes"], measured.restriction_bytes);
    assert_eq!(
        stats["peak_copied_payload_bytes"],
        measured.restriction_peak_bytes
    );
}

#[test]
fn interrupted_restriction_work_remains_visible() {
    let (report, value) = solve(
        "{a}. {b}. :- a,b.",
        &options(&["--stats", "--grounder", "lazy", "--max-search-work", "1"]),
    );
    let report = report.unwrap();
    assert_eq!(report.completion, Completion::Interrupted);
    assert_eq!(value["statistics"]["candidate_restrictions"]["work"], 1);
    assert_eq!(value["outcome"]["coverage"], "partial");
}

fn priority_ties(bounds: &str) -> (Report, Json) {
    let (result, value) = solve(
        "a. {b;c}. #show a. #minimize{0@2,k:a;0@-1,j:b}.",
        &options(&["--max-objective-bound-work", bounds]),
    );
    (result.unwrap(), value)
}

#[test]
fn optimal_ties_retain_priority_cost_vectors() {
    for bounds in ["0", "10000000"] {
        let (report, value) = priority_ties(bounds);
        assert_eq!(report.models, 4);
        assert_eq!(value["outcome"]["optimization"]["optimal"], true);
        assert_eq!(value["outcome"]["optimization"]["tied_models"], 4);
        for model in value["models"].as_array().unwrap() {
            assert_eq!(
                model["model"]["costs"],
                json!([{"priority":2,"value":0},{"priority":-1,"value":0}])
            );
        }
    }
}

#[test]
fn observations_retain_both_output_channels() {
    for bounds in ["0", "10000000"] {
        let (report, value) = priority_ties(bounds);
        assert_eq!(report.models, 4);
        for model in value["models"].as_array().unwrap() {
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
fn empty_model_differs_from_unsatisfiability() {
    for (source, expected_status, models) in [("", "satisfiable", 1), (":-.", "unsatisfiable", 0)] {
        let (result, value) = solve(source, &options(&[]));
        assert_eq!(result.unwrap().models, models);
        assert_eq!(value["outcome"]["status"], expected_status);
        assert_eq!(value["outcome"]["completion"], "exhausted");
    }
}

#[test]
fn model_caps_leave_coverage_partial() {
    let mut limited = options(&[]);
    limited.models = 1;
    let (result, value) = solve("{a}.", &limited);
    assert_eq!(result.unwrap().completion, Completion::RequestedModels);
    assert_eq!(value["outcome"]["completion"], "requested_models");
    assert_eq!(value["outcome"]["coverage"], "partial");
}

#[test]
fn interruption_retains_an_unproved_incumbent() {
    // The incumbent records established evidence while its optimality remains open.
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
    let report = result.unwrap();
    assert_eq!(report.completion, Completion::Interrupted);
    assert_eq!((report.models, report.checked), (1, 1));
    assert_eq!(value["outcome"]["status"], "incomplete");
    assert_eq!(value["outcome"]["coverage"], "partial");
    assert_eq!(value["outcome"]["verified_models"], 1);
    assert_eq!(value["outcome"]["published_models"], 1);
    assert_eq!(value["outcome"]["checked"], 1);
    assert_eq!(value["outcome"]["optimization"]["optimal"], false);
    assert_eq!(value["outcome"]["optimization"]["tied_models"], 1);
    assert_eq!(value["outcome"]["optimization"]["scored_models"], 1);
    assert_eq!(value["outcome"]["interruption"]["kind"], "countermodel");
    assert_eq!(value["outcome"]["interruption"]["code"], "candidate_limit");
    let models = value["models"].as_array().unwrap();
    assert_eq!(models.len(), 1);
    assert_eq!(models[0]["number"], 1);
    let full = models[0]["model"]["full_model"].as_array().unwrap();
    assert_eq!(full.len(), 1);
    // Either stable model may be found first; its retained cost must match it.
    let (name, cost) = match full[0]["predicate"].as_str() {
        Some("a") => ("a", 1),
        Some("b") => ("b", 2),
        other => panic!("unexpected incumbent atom: {other:?}"),
    };
    assert_eq!(
        full[0],
        json!({"predicate":name,"sign":"positive","arguments":[]})
    );
    let costs = json!([{"priority":0,"value":cost}]);
    assert_eq!(models[0]["model"]["costs"], costs);
    assert_eq!(value["outcome"]["optimization"]["costs"], costs);
    let best = report.optimization.unwrap();
    assert_eq!((best.tied_models, best.scored_models), (1, 1));
    assert_eq!(best.score.costs(), &[(0, cost)]);
}

#[test]
fn admission_failures_publish_error_documents() {
    let (result, value) = solve("p(.", &options(&[]));
    assert!(result.is_err());
    assert_eq!(value["outcome"]["status"], "failed");
    assert_eq!(value["outcome"]["error"]["kind"], "expansion");
    assert!(value["outcome"]["completion"].is_null());
    assert_eq!(value["models"], json!([]));
}

#[test]
fn observation_refusals_retain_verified_evidence() {
    let (result, value) = solve("a. #show a.", &options(&["--max-observation-work", "0"]));
    assert!(result.is_err());
    assert_eq!(value["outcome"]["status"], "failed");
    assert_eq!(value["outcome"]["error"]["kind"], "observation");
    assert!(value["outcome"]["completion"].is_null());
    assert_eq!(value["models"], json!([]));
    assert_eq!(value["outcome"]["verified_models"], 1);
    assert!(
        result
            .unwrap_err()
            .partial_report
            .unwrap()
            .summary_published
    );
}

fn formula_statistics() -> (Report, Json) {
    let (result, value) = solve(
        "a | b.",
        &options(&["--stats", "--completion-workers", "2"]),
    );
    (result.unwrap(), value)
}

#[test]
fn search_statistics_count_stable_models() {
    let (_, value) = formula_statistics();
    assert_eq!(value["statistics"]["search"]["stable_models"], 2);
}

#[test]
fn formula_phases_exclude_closure_measurements() {
    let (report, value) = formula_statistics();
    assert!(report.phase_timings.is_some());
    assert_eq!(value["statistics"]["phase_timings"]["schema"], 2);
    assert!(value["statistics"]["phase_timings"]["measurements"]["closure_membership"].is_null());
}

#[test]
fn completion_statistics_retain_worker_request() {
    let (_, value) = formula_statistics();
    assert_eq!(
        value["statistics"]["execution"]["completion"]["requested_workers"],
        2
    );
}

struct PrefixWriter {
    capacity: usize,
    bytes: Vec<u8>,
}
impl Write for PrefixWriter {
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
fn write_failure_preserves_the_committed_prefix() {
    // Only complete model records commit; a footer cannot repair an incomplete prefix.
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
        let mut output = PrefixWriter {
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
fn model_ceiling_precedes_publication() {
    let source = format!("p(\"{}\").", "x".repeat(1500));
    let (result, value) = solve(&source, &options(&["--max-json-record-bytes", "1000"]));
    assert!(matches!(
        *result.unwrap_err().cause,
        RunError::JsonRecord(_)
    ));
    assert_eq!(value["models"], json!([]));
    assert_eq!(value["outcome"]["published_models"], 0);
    assert_eq!(value["outcome"]["verified_models"], 1);
}

#[test]
fn zero_ceiling_prevents_document_completion() {
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
fn input_refusals_use_json_error_envelopes() {
    let result = Command::new(env!("CARGO_BIN_EXE_zetesis"))
        .args(["--json", "/does/not/exist/zetesis-json-input.lp"])
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(2));
    let value: Json = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(value["outcome"]["error"]["kind"], "bundle_load");
}

#[test]
fn human_output_remains_the_default() {
    let human = Options::try_parse_from([
        "zetesis",
        "--backend",
        "cpu",
        "--workers",
        "1",
        "--models",
        "0",
    ])
    .unwrap();
    assert!(!human.json);
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
fn interrupted_writes_remain_retryable() {
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
fn tie_write_failure_preserves_semantic_coverage() {
    // Delivery may stop partway through a tie without undoing completed search.
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
        let mut output = PrefixWriter {
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
fn view_refusal_preserves_proved_optimality() {
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
fn stage_views_preserve_the_typed_partition() {
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
fn resource_stops_encode_partial_coverage() {
    let cases: &[(&str, &[&str], &str, &str)] = &[
        (
            "a.",
            &[
                "--oracle",
                "closure",
                "--grounder",
                "lazy",
                "--max-closure-bytes",
                "0",
            ],
            "oracle",
            "storage_limit",
        ),
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
fn objective_refusals_publish_no_incumbent() {
    for (flag, code) in [
        ("--max-objective-work", "work_limit"),
        ("--max-objective-bindings", "binding_limit"),
        ("--max-objective-keys", "key_limit"),
        ("--max-objective-key-bytes", "key_bytes_limit"),
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
        assert_eq!(value["outcome"]["interruption"]["kind"], "objective");
        match report.interruption.unwrap() {
            zetesis_cli::Interruption::Objective(error) => {
                assert!(matches!(
                    error.kind(),
                    zetesis_objective::ErrorKind::Stopped(_)
                ));
            }
            other => panic!("unexpected interruption: {other:?}"),
        }
    }
}

#[test]
fn projection_limits_preserve_checked_partial_answers() {
    for (flag, code) in [
        ("--max-projection-entries", "projection_entries"),
        ("--max-projection-nodes", "projection_nodes"),
    ] {
        let (result, value) = solve(
            "{a;b}.",
            &options(&["--oracle", "countermodel", "--stats", flag, "0"]),
        );
        let report = result.unwrap();
        assert_eq!(report.completion, Completion::Interrupted);
        assert_eq!(report.models, 1);
        assert_eq!(value["outcome"]["coverage"], "partial");
        assert_eq!(value["outcome"]["interruption"]["code"], code);
        assert_eq!(value["outcome"]["verified_models"], 1);
        assert_eq!(value["models"].as_array().unwrap().len(), 1);
        let owner = &value["statistics"]["search"]["projection_history"];
        let receipt = report.countermodel_statistics.unwrap().projections;
        assert_eq!(receipt.entries, 0);
        assert_eq!(owner["entries"], receipt.entries);
        assert_eq!(owner["nodes"], receipt.nodes);
        assert_eq!(
            owner["retained_bytes"].as_u64().map(u128::from),
            Some(receipt.retained_bytes)
        );
        assert_eq!(
            owner["peak_bytes"].as_u64().map(u128::from),
            Some(receipt.peak_bytes)
        );
        assert_eq!(owner["work"], receipt.work);
    }
}

#[test]
fn incumbent_refusals_publish_no_model() {
    for (flag, code) in [
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
        assert_eq!(value["outcome"]["interruption"]["kind"], "incumbent");
        assert!(matches!(
            report.interruption.unwrap(),
            zetesis_cli::Interruption::Incumbent(_)
        ));
    }
}

#[test]
fn unretained_ties_remain_in_score_counts() {
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
fn stopped_requests_publish_no_models() {
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
            assert_eq!(value["outcome"]["interruption"]["kind"], "preparation");
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
fn setup_refusals_have_unavailable_coverage() {
    for (source, extra, kind) in [
        (
            "a.",
            vec!["--oracle", "countermodel", "--grounder", "lazy"],
            "unsupported_oracle",
        ),
        ("p(.", vec!["--grounder", "lazy"], "expansion"),
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
        if kind == "expansion" {
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

fn statistics_write_failure(source: String, configured: &Options) -> (RunFailure, Json) {
    // Stop diagnostics at the statistics boundary, after solving and model
    // rendering have already succeeded or established their original error.
    let mut reference = Vec::new();
    let _ = run_detailed_with_diagnostics(
        source.clone(),
        configured,
        &mut io::sink(),
        &mut reference,
        &Control::default(),
    );
    let text = std::str::from_utf8(&reference).unwrap();
    let capacity = text.find("Statistics:").unwrap();
    let mut diagnostics = PrefixWriter {
        capacity,
        bytes: Vec::new(),
    };
    let mut bytes = Vec::new();
    let failure = run_detailed_with_diagnostics(
        source,
        configured,
        &mut bytes,
        &mut diagnostics,
        &Control::default(),
    )
    .unwrap_err();
    let value = serde_json::from_slice(&bytes).unwrap();
    (failure, value)
}

#[test]
fn statistics_failure_preserves_exhaustion() {
    let (failure, value) = statistics_write_failure(
        "a. {b}. #minimize{0@1,k:a}.".to_owned(),
        &options(&["--stats"]),
    );
    let partial = failure.partial_report.unwrap();
    assert!(partial.summary_published);
    assert_eq!(partial.completion, Some(Completion::Exhausted));
    assert_eq!(value["outcome"]["status"], "failed");
    assert_eq!(value["outcome"]["coverage"], "exhausted");
    assert_eq!(value["outcome"]["optimization"]["optimal"], true);
    assert!(value["statistics"]["stage_timings"].is_object());
    assert_eq!(value["outcome"]["error"]["secondary_output_failure"], false);
    assert!(matches!(*failure.cause, RunError::Output(_)));
    assert!(failure.secondary_output.is_none());
    assert_eq!(value["outcome"]["error"]["kind"], "output");
    assert_eq!((partial.published_models, partial.verified_models), (2, 2));
}

#[test]
fn statistics_failure_preserves_observation_cause() {
    let source = format!(
        "p(\"{}\"). #show shown:p(X). #minimize{{0@1,k:p(X)}}.",
        "x".repeat(1500)
    );
    let mut configured = options(&["--stats"]);
    configured.max_observation_work = 0;
    let (failure, value) = statistics_write_failure(source, &configured);
    let partial = failure.partial_report.unwrap();
    assert!(partial.summary_published);
    assert_eq!(partial.completion, Some(Completion::Exhausted));
    assert_eq!(value["outcome"]["status"], "failed");
    assert_eq!(value["outcome"]["coverage"], "exhausted");
    assert_eq!(value["outcome"]["optimization"]["optimal"], true);
    assert!(value["statistics"]["stage_timings"].is_object());
    assert_eq!(value["outcome"]["error"]["secondary_output_failure"], true);
    assert!(matches!(*failure.cause, RunError::Observation(_)));
    assert!(failure.secondary_output.is_some());
    assert_eq!(value["outcome"]["error"]["kind"], "observation");
    assert_eq!((partial.published_models, partial.verified_models), (0, 1));
}

#[test]
fn footer_write_failure_is_secondary() {
    // The original source refusal remains authoritative after any footer prefix.
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
        let mut output = PrefixWriter {
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

fn empty_model_document() -> (Vec<u8>, usize) {
    let mut reference = Vec::new();
    run_detailed_with_diagnostics(
        String::new(),
        &options(&[]),
        &mut reference,
        &mut io::sink(),
        &Control::default(),
    )
    .unwrap();
    let model_end = reference.iter().position(|byte| *byte == b'\n').unwrap() + 1;
    (reference, model_end)
}

#[test]
fn terminal_ceiling_is_inclusive() {
    let (reference, model_end) = empty_model_document();
    let footer_bytes = reference.len() - model_end;
    assert!(footer_bytes > model_end);
    for maximum in [footer_bytes - 1, footer_bytes, footer_bytes + 1] {
        let mut configured = options(&[]);
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
            assert!(matches!(
                *result.unwrap_err().cause,
                RunError::JsonRecord(zetesis_themelios::observation::ViewError::Bytes)
            ));
        } else {
            assert_eq!(result.unwrap().models, 1);
            assert_eq!(bytes, reference);
            let value: Json = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(value["outcome"]["status"], "satisfiable");
        }
    }
}

#[test]
fn terminal_refusal_preserves_published_models() {
    let (reference, model_end) = empty_model_document();
    let footer_bytes = reference.len() - model_end;
    assert!(footer_bytes > model_end);
    let mut configured = options(&[]);
    configured.max_json_record_bytes = footer_bytes - 1;
    let mut bytes = Vec::new();
    let failure = run_detailed_with_diagnostics(
        String::new(),
        &configured,
        &mut bytes,
        &mut io::sink(),
        &Control::default(),
    )
    .unwrap_err();
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
}
