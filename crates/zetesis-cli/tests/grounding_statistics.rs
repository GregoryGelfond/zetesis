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
        let measurement = typed.get(phase).expect("all formula phases entered");
        let json = &view["measurements"][phase.label()];
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
