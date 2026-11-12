//! JSON reads objective work from semantic evidence independently of incumbents.

use clap::Parser;
use std::io;
use zetesis_algebra::Cancellation;

fn options() -> crate::Options {
    crate::Options::parse_from([
        "zetesis",
        "--backend",
        "cpu",
        "--workers",
        "1",
        "--models",
        "0",
        "--oracle",
        "countermodel",
        "--json",
        "--stats",
    ])
}

#[test]
fn json_objective_work_matches_the_semantic_receipt() {
    for maximum in [1, crate::SolveConfig::DEFAULT.max_objective_work] {
        let mut config = crate::PublicationConfig::from(&options());
        config.solve.max_objective_work = maximum;
        let mut output = Vec::new();
        let outcome = crate::publication_fixture::json(
            "{a}. #minimize {1,a:a}.",
            &config,
            65_536,
            &mut output,
            &Cancellation::default(),
        )
        .unwrap();
        let semantic = outcome.semantic();
        let document: serde_json::Value = serde_json::from_slice(&output).unwrap();
        assert_eq!(
            document["statistics"]["objective_work"],
            semantic.objective_work()
        );
        assert!(semantic.objective_work() > 0);
        if maximum == 1 {
            assert_eq!(semantic.objective_work(), 1);
            assert!(semantic.incumbent().is_none());
            assert!(document["outcome"]["optimization"].is_null());
        } else {
            assert_eq!(
                semantic.incumbent().unwrap().work,
                semantic.objective_work()
            );
        }
    }
}

#[test]
fn publication_failure_keeps_the_objective_receipt_in_a_later_summary() {
    let config = crate::PublicationConfig::from(&options());
    let failure = crate::publication_fixture::json(
        "{a}. #minimize {1,a:a}.",
        &config,
        1,
        &mut io::sink(),
        &Cancellation::default(),
    )
    .unwrap_err();
    let expected = failure.semantic().unwrap().objective_work();
    assert!(expected > 0);
    let result = Err(failure);
    let record = super::document_fixture::summary(&result, 65_536).unwrap();
    let mut document = b"{\"models\":[".to_vec();
    document.extend(record);
    let value: serde_json::Value = serde_json::from_slice(&document).unwrap();
    assert_eq!(value["statistics"]["objective_work"], expected);
}

#[test]
fn projected_refusal_keeps_native_report_completion_and_work() {
    let mut case = crate::publication_fixture::FormulaCase::new(options());
    case.config.solve.max_objective_keys = 1;
    let mut output = Vec::new();
    let mut diagnostics = Vec::new();
    let outcome = case
        .run(
            "{a;b}. :- a,b. #minimize {1,a:a;2,b:b}.",
            &mut output,
            &mut diagnostics,
            &Cancellation::default(),
        )
        .unwrap();
    let semantic = outcome.semantic();
    assert_eq!(semantic.completion(), Some(crate::Completion::Exhausted));
    assert!(semantic.optimum_proved());
    assert_eq!(semantic.scored_models(), 3);
    let text = std::str::from_utf8(&diagnostics).unwrap();
    assert_eq!(text.matches("Objective projection unavailable:").count(), 1);
    assert!(text.contains("detailed scoring is subject to the remaining objective allowance"));
    assert!(!text.contains("Objective preparation unavailable:"));
    let document: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(
        document["statistics"]["objective_work"],
        semantic.objective_work()
    );
    assert!(semantic.objective_work() > 0);
    let checked = zetesis_validation::answers::native_json::parse(
        &output,
        zetesis_validation::answers::native_json::Limits::default(),
    )
    .unwrap();
    let [optimum] = checked.records() else {
        panic!("the unique optimum must be published")
    };
    assert!(optimum.full_model().is_empty());
    assert_eq!(optimum.costs(), Some(&[(0, 0)][..]));
    assert_eq!(checked.costs(), Some(&[(0, 0)][..]));
}
