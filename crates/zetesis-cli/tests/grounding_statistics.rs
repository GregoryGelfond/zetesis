//! Ordinary solving presents the same typed attribution to Rust and JSON clients.

use clap::Parser;
use serde_json::Value;
use zetesis_cli::{GroundingOutcome, GroundingPhase, Options, run_detailed_with_diagnostics};
use zetesis_cpu::Control;

fn options(extra: &[&str]) -> Options {
    Options::try_parse_from(
        [
            "zetesis",
            "--backend",
            "cpu",
            "--workers",
            "1",
            "--models",
            "0",
            "--json",
            "--stats",
        ]
        .into_iter()
        .chain(extra.iter().copied()),
    )
    .unwrap()
}

#[test]
fn json_attribution_preserves_typed_measurements() {
    let mut output = Vec::new();
    let mut diagnostics = Vec::new();
    let report = run_detailed_with_diagnostics(
        "1{p;q}1.".into(),
        &options(&[]),
        &mut output,
        &mut diagnostics,
        &Control::default(),
    )
    .unwrap();
    let typed = report.phase_timings.unwrap().grounding;
    let document: Value = serde_json::from_slice(&output).unwrap();
    let view = &document["statistics"]["grounding_attribution"];
    assert_eq!(view["scope"], "eager_formula");
    for phase in GroundingPhase::ALL {
        let json = &view["measurements"][phase.label()];
        let measurement = typed.get(phase).unwrap();
        assert_eq!(
            json["elapsed_ns"].as_u64().map(u128::from),
            measurement.elapsed.map(|value| value.as_nanos())
        );
        for outcome in GroundingOutcome::ALL {
            assert_eq!(
                json["outcomes"][outcome.label()].as_u64(),
                measurement.count(outcome)
            );
        }
        assert_eq!(json["work"]["roots"].as_u64(), measurement.work.roots);
        for (name, count) in [
            ("domain_prepare_work", measurement.work.domain_prepare_work),
            ("domain_guard_rows", measurement.work.domain_guard_rows),
            ("domain_guard_checks", measurement.work.domain_guard_checks),
            (
                "domain_rejected_rows",
                measurement.work.domain_rejected_rows,
            ),
            (
                "domain_narrowed_candidates",
                measurement.work.domain_narrowed_candidates,
            ),
        ] {
            assert_eq!(json["work"][name].as_u64(), count);
        }
        assert_eq!(
            json["work"]["expression_nodes"].as_u64(),
            measurement.work.expression_nodes
        );
    }
    assert!(
        std::str::from_utf8(&diagnostics)
            .unwrap()
            .contains("Grounding attribution:")
    );
}

#[test]
fn the_ordinary_command_requests_the_domain_analysis() {
    // The choice head keeps this program outside the analysis's profile, so
    // the attempt is measured and declines: its work is the applicability
    // check alone, and no guard visits a row.
    let mut output = Vec::new();
    let mut diagnostics = Vec::new();
    let report = run_detailed_with_diagnostics(
        "1{p;q}1.".into(),
        &options(&[]),
        &mut output,
        &mut diagnostics,
        &Control::default(),
    )
    .unwrap();
    let typed = report.phase_timings.unwrap().grounding;
    let analysis = typed.get(GroundingPhase::DomainAnalysis).unwrap();
    assert_eq!(analysis.count(GroundingOutcome::Completed), Some(1));
    assert!(analysis.work.domain_prepare_work.unwrap() > 0);
    let rules = typed.get(GroundingPhase::RuleInstantiation).unwrap();
    assert_eq!(rules.work.domain_guard_rows, Some(0));
    assert_eq!(rules.work.domain_narrowed_candidates, Some(0));
}

#[test]
fn the_ordinary_command_narrows_candidates_by_comparison() {
    // X < 3 excludes 198 of the 200 candidates the domain of d/1 offers
    // before any row is read; the two answers are the instances that remain.
    let mut output = Vec::new();
    let mut diagnostics = Vec::new();
    let report = run_detailed_with_diagnostics(
        "d(1..200). p(X) :- d(X), X < 3.".into(),
        &options(&[]),
        &mut output,
        &mut diagnostics,
        &Control::default(),
    )
    .unwrap();
    let typed = report.phase_timings.unwrap().grounding;
    let analysis = typed.get(GroundingPhase::DomainAnalysis).unwrap();
    assert_eq!(analysis.work.domain_narrowed_candidates, Some(198));
    let rules = typed.get(GroundingPhase::RuleInstantiation).unwrap();
    assert_eq!(rules.work.domain_rejected_rows, Some(198));
    let document: Value = serde_json::from_slice(&output).unwrap();
    let json = &document["statistics"]["grounding_attribution"]["measurements"]
        [GroundingPhase::DomainAnalysis.label()]["work"];
    assert_eq!(json["domain_narrowed_candidates"], 198);
    let models = document["models"].as_array().unwrap();
    assert_eq!(models.len(), 1);
    let instances: Vec<_> = models[0]["model"]["full_model"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|atom| atom["predicate"] == "p")
        .map(|atom| atom["arguments"][0][0]["value"].as_i64().unwrap())
        .collect();
    assert_eq!(instances, [1, 2]);
}

#[test]
fn ordinary_formula_solving_uses_requested_table_joins() {
    let mut output = Vec::new();
    let mut diagnostics = Vec::new();
    let report = run_detailed_with_diagnostics(
        "edge(1,1). edge(1,2). edge(2,1). edge(2,2). 1{choose(1);choose(2)}1. witness(X,Y):-choose(X),edge(X,Y). diagonal(X):-edge(X,X).".into(),
        &options(&["--formula-joins", "table"]),
        &mut output,
        &mut diagnostics,
        &Control::default(),
    ).unwrap();
    let grounding = report.phase_timings.unwrap().grounding;
    let work = GroundingPhase::ALL
        .into_iter()
        .filter_map(|phase| grounding.get(phase))
        .fold(
            zetesis_cli::GroundingWork::default(),
            |work, measurement| work.checked_sum(measurement.work),
        );
    assert!(work.table_preparations.unwrap() > 0);
    assert!(work.table_probes.unwrap() > 0);
    assert!(work.table_rows.unwrap() > 0);
    let document: Value = serde_json::from_slice(&output).unwrap();
    for phase in GroundingPhase::ALL {
        let Some(measurement) = grounding.get(phase) else {
            assert_eq!(phase, GroundingPhase::DomainAnalysis);
            continue;
        };
        let view =
            &document["statistics"]["grounding_attribution"]["measurements"][phase.label()]["work"];
        assert_eq!(
            view["table_preparations"].as_u64(),
            measurement.work.table_preparations
        );
        assert_eq!(view["table_probes"].as_u64(), measurement.work.table_probes);
        assert_eq!(
            view["support_peak_bytes"].as_u64(),
            measurement.work.support_peak_bytes
        );
    }
}

#[test]
fn relational_grounding_stays_unmeasured_in_formula_attribution() {
    for grounder in ["eager", "lazy"] {
        let mut output = Vec::new();
        let report = run_detailed_with_diagnostics(
            "p.".into(),
            &options(&["--grounder", grounder]),
            &mut output,
            &mut std::io::sink(),
            &Control::default(),
        )
        .unwrap();
        let typed = report.phase_timings.unwrap().grounding;
        let document: Value = serde_json::from_slice(&output).unwrap();
        for phase in GroundingPhase::ALL {
            assert!(typed.get(phase).is_none());
            assert!(
                document["statistics"]["grounding_attribution"]["measurements"][phase.label()]
                    .is_null()
            );
        }
    }
}

#[test]
fn a_grounding_failure_retains_its_phase_outcome() {
    let mut output = Vec::new();
    let mut options = options(&[]);
    options.max_atoms = 0;
    let failure = run_detailed_with_diagnostics(
        "1{p;q}1.".into(),
        &options,
        &mut output,
        &mut std::io::sink(),
        &Control::default(),
    )
    .unwrap_err();
    let typed = failure.phase_timings.unwrap().grounding;
    let support = typed.get(GroundingPhase::SupportCompletion).unwrap();
    assert_eq!(support.count(GroundingOutcome::Failed), Some(1));
    assert!(typed.get(GroundingPhase::RuleInstantiation).is_none());
    let document: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(
        document["statistics"]["grounding_attribution"]["measurements"]["support_completion"]["outcomes"]
            ["failed"],
        1
    );
}
