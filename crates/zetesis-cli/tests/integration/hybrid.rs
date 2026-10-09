//! Explicit lazy formula admission preserves original answers and scopes core work.

use zetesis_test_support::document::spelled;

use std::{fs, io};

use clap::Parser;
use serde_json::Value as Json;
use zetesis_cli::{
    Completion, Grounder, Options, PublicationOutcome, RunError, SolveConfig, StatisticsView,
    run_bundle_finalized_with_diagnostics, run_finalized_with_diagnostics,
};
use zetesis_cpu::Cancellation;
use zetesis_themelios::{BundleLimits, FormulaFailure, HybridFeature, SourceBundle};
use zetesis_validation::answers::native_json;

const SOURCE: &str = "a|b. {hidden}. :- b. #show a/0.";

fn options(extra: &[&str]) -> Options {
    Options::try_parse_from(
        [
            "zetesis",
            "--json",
            "--stats",
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

fn solve(source: &str, options: &Options) -> (PublicationOutcome, Vec<u8>, String) {
    let mut output = Vec::new();
    let mut diagnostics = Vec::new();
    let result = run_finalized_with_diagnostics(
        source.into(),
        options,
        &mut output,
        &mut diagnostics,
        &Cancellation::default(),
    )
    .unwrap();
    (result, output, String::from_utf8(diagnostics).unwrap())
}

fn family(output: &[u8]) -> Vec<Vec<String>> {
    native_json::parse(output, native_json::Limits::default())
        .unwrap()
        .full_model_symbols(65_536)
        .unwrap()
}

fn expected() -> Vec<Vec<String>> {
    vec![vec!["a".into()], vec!["a".into(), "hidden".into()]]
}

#[test]
fn explicit_lazy_formula_preserves_hidden_full_answers() {
    let (outcome, output, _) = solve(SOURCE, &options(&["--grounder", "lazy"]));
    assert_eq!(outcome.semantic().completion(), Some(Completion::Exhausted));
    assert!(outcome.semantic().hybrid_execution().is_some());
    assert_eq!(family(&output), expected());
}

#[test]
fn hybrid_projection_selects_only_original_answers() {
    let source = "a|b. {c}. :-a,c. #project c/0. #show c/0.";
    let atom = |name| serde_json::json!({"predicate":name,"sign":"positive","arguments":[]});
    let original = [vec![atom("a")], vec![atom("b")], vec![atom("b"), atom("c")]];
    for grounder in ["eager", "lazy"] {
        let (outcome, output, _) = solve(source, &options(&["--grounder", grounder]));
        // Projected publication is not the full-family decoder's input contract.
        let document: Json = serde_json::from_slice(&output).unwrap();
        let selected: Vec<_> = document["models"]
            .as_array()
            .unwrap()
            .iter()
            .map(|record| spelled(&document, record))
            .collect();
        assert_eq!(selected.len(), 2);
        assert!(selected.iter().all(|answer| original.contains(answer)));
        assert_eq!(
            selected
                .iter()
                .filter(|answer| answer.contains(&atom("c")))
                .count(),
            1
        );
        assert_eq!(document["outcome"]["projection"]["complete"], true);
        assert_eq!(outcome.semantic().verified_models(), 3);
        assert_eq!(outcome.semantic().completion(), Some(Completion::Exhausted));
    }
}

#[test]
fn automatic_formula_admission_remains_eager() {
    for grounder in ["auto", "eager"] {
        let (outcome, output, _) = solve(SOURCE, &options(&["--grounder", grounder]));
        assert!(outcome.semantic().hybrid_execution().is_none());
        assert_eq!(family(&output), expected());
    }
}

#[test]
fn countermodel_lazy_selects_hybrid_formula_admission() {
    let (outcome, output, _) = solve(
        SOURCE,
        &options(&["--grounder", "lazy", "--oracle", "countermodel"]),
    );
    assert!(outcome.semantic().hybrid_execution().is_some());
    assert_eq!(family(&output), expected());
}

#[test]
fn hybrid_bounded_choices_keep_their_complete_family() {
    for oracle in ["auto", "countermodel"] {
        let (outcome, output, _) = solve(
            "1 {p(1..4)} 1.",
            &options(&["--grounder", "lazy", "--oracle", oracle]),
        );
        assert!(outcome.semantic().hybrid_execution().is_some());
        assert_eq!(
            family(&output),
            (1..=4)
                .map(|value| vec![format!("p({value})")])
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn ordinary_relational_lazy_keeps_its_closure_route() {
    let (outcome, output, _) = solve("a:-not b. b:-not a.", &options(&["--grounder", "lazy"]));
    assert!(outcome.semantic().hybrid_execution().is_none());
    assert!(outcome.semantic().closure_execution().is_some());
    assert_eq!(
        family(&output),
        vec![vec!["a".to_owned()], vec!["b".to_owned()]]
    );
}

#[test]
fn json_distinguishes_core_models_from_original_membership() {
    let (outcome, output, _) = solve(SOURCE, &options(&["--grounder", "lazy"]));
    let stats = outcome.semantic().hybrid_execution().unwrap();
    assert_eq!(
        (
            stats.core_answers,
            stats.accepted,
            stats.rejected,
            stats.pending
        ),
        (2, 2, 0, 0)
    );
    let document: Json = serde_json::from_slice(&output).unwrap();
    assert_eq!(document["statistics"]["search"]["scope"], "retained_core");
    let regions = outcome
        .semantic()
        .countermodel_statistics()
        .unwrap()
        .region_filter
        .unwrap();
    // The unit source constraint cuts b before a rejected region is formed.
    assert_eq!(regions.refuted, 0);
    assert!(regions.checks > 0);
    assert_eq!(
        document["statistics"]["search"]["region_filter"]["refuted"],
        regions.refuted
    );
    assert_eq!(
        document["statistics"]["hybrid_execution"]["accepted"],
        stats.accepted
    );
    assert_eq!(
        document["statistics"]["hybrid_execution"]["rejected"],
        stats.rejected
    );
    assert_eq!(document["outcome"]["verified_models"], stats.accepted);
    assert!(stats.constraints.work > 0);
    assert_eq!(
        document["statistics"]["hybrid_execution"]["constraints"]["work"],
        stats.constraints.work
    );
    assert!(!output.contains(&0x1b));
}

#[test]
fn requested_answers_count_only_completed_constraint_checks() {
    let mut config = options(&["--grounder", "lazy"]);
    config.models = 1;
    let (outcome, output, _) = solve(SOURCE, &config);
    let stats = outcome.semantic().hybrid_execution().unwrap();
    assert_eq!(
        outcome.semantic().completion(),
        Some(Completion::RequestedModels)
    );
    assert_eq!((stats.accepted, stats.pending), (1, 0));
    let document: Json = serde_json::from_slice(&output).unwrap();
    assert_eq!(document["models"].as_array().unwrap().len(), 1);
    assert_eq!(document["outcome"]["verified_models"], 1);
}

#[test]
fn human_statistics_name_the_hybrid_scope() {
    let mut config = options(&["--grounder", "lazy"]);
    config.statistics_view = StatisticsView::Human;
    let (_, _, diagnostics) = solve(SOURCE, &config);
    for text in [
        "CPU hybrid formula",
        "Core answers checked",
        "Constraint checks accepted",
        "Constraint checks rejected",
        "Constraint checks pending",
        "Constraint checking work",
        "original answers",
    ] {
        assert!(diagnostics.contains(text), "{text}: {diagnostics}");
    }
    assert!(!diagnostics.contains("certificate over the original theory"));
}

#[test]
fn recorded_statistics_identify_hybrid_grounding() {
    let (_, _, diagnostics) = solve(SOURCE, &options(&["--grounder", "lazy"]));
    let effective = diagnostics
        .lines()
        .find(|line| line.starts_with("  effective execution:"))
        .unwrap();
    assert!(effective.contains("grounder=hybrid;"), "{effective}");
    assert!(diagnostics.contains("eager retained core; streamed source constraints"));
}

#[test]
fn hybrid_warnings_remain_on_the_diagnostic_stream() {
    let (_, output, diagnostics) = solve(
        "d(0..2). p(X):-d(X),1/X=1.",
        &options(&["--grounder", "lazy", "--oracle", "countermodel"]),
    );
    assert_eq!(
        diagnostics
            .matches("warning[zetesis::zero-divisor]")
            .count(),
        1
    );
    assert!(diagnostics.contains("<input>:1:"));
    assert_eq!(
        family(&output),
        vec![vec![
            "d(0)".to_owned(),
            "d(1)".to_owned(),
            "d(2)".to_owned(),
            "p(1)".to_owned(),
        ]]
    );
}

#[test]
fn hybrid_retains_all_undefined_admission_refusal() {
    let failure = run_finalized_with_diagnostics(
        "d(0). p(X):-d(X),1/X=1.".into(),
        &options(&["--grounder", "lazy", "--oracle", "countermodel"]),
        &mut Vec::new(),
        &mut io::sink(),
        &Cancellation::default(),
    )
    .unwrap_err();
    assert!(matches!(
        failure.cause.as_ref(),
        RunError::FormulaAdmission(FormulaFailure::Expansion(
            zetesis_themelios::ExpansionFailure::Evaluation { .. }
        ))
    ));
    assert!(failure.semantic().is_none());
}

#[test]
fn hybrid_bundle_uses_original_include_sources() {
    let directory = tempfile::tempdir().unwrap();
    fs::write(
        directory.path().join("entry.lp"),
        "a|b. {hidden}. #include \"constraints.lp\".",
    )
    .unwrap();
    fs::write(directory.path().join("constraints.lp"), ":- b. #show a/0.").unwrap();
    let bundle =
        SourceBundle::load(directory.path().join("entry.lp"), BundleLimits::default()).unwrap();
    let mut output = Vec::new();
    let outcome = run_bundle_finalized_with_diagnostics(
        bundle,
        &options(&["--grounder", "lazy"]),
        &mut output,
        &mut io::sink(),
        &Cancellation::default(),
    )
    .unwrap();
    assert!(outcome.semantic().hybrid_execution().is_some());
    assert_eq!(family(&output), expected());
}

#[test]
fn hybrid_refuses_table_joins() {
    let mut config = options(&["--formula-joins", "table"]);
    config.grounder = Grounder::Lazy;
    let mut output = Vec::new();
    let failure = run_finalized_with_diagnostics(
        SOURCE.into(),
        &config,
        &mut output,
        &mut io::sink(),
        &Cancellation::default(),
    )
    .unwrap_err();
    assert!(matches!(
        failure.cause.as_ref(),
        RunError::FormulaAdmission(FormulaFailure::HybridUnsupported {
            feature: HybridFeature::TableJoins,
            ..
        })
    ));
    let document: Json = serde_json::from_slice(&output).unwrap();
    assert_eq!(document["outcome"]["status"], "failed");
    assert_eq!(document["outcome"]["coverage"], "unavailable");
    assert_eq!(document["models"].as_array().unwrap().len(), 0);
}

#[test]
fn hybrid_optimization_preserves_optimal_ties() {
    let source = include_str!("../fixtures/hybrid-objectives.lp");
    for grounder in ["eager", "lazy"] {
        let (outcome, output, _) = solve(source, &options(&["--grounder", grounder]));
        assert_eq!(outcome.semantic().completion(), Some(Completion::Exhausted));
        assert_eq!(family(&output), expected());
        let document: Json = serde_json::from_slice(&output).unwrap();
        let optimization = &document["outcome"]["optimization"];
        assert_eq!(optimization["optimal"], true);
        assert_eq!(optimization["tied_models"], 2);
        assert_eq!(
            optimization["costs"],
            serde_json::json!([{"priority": 0, "value": 1}])
        );
        assert_eq!(
            outcome.semantic().hybrid_execution().is_some(),
            grounder == "lazy"
        );
    }
}

#[test]
fn constraint_replay_uses_the_ordinary_resource_policy() {
    let options = options(&["--memory", "1048576"]);
    assert_eq!(
        SolveConfig::from(&options).constraints,
        options.resources().solve_config().constraints
    );
}
