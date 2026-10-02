//! JSON reads objective work from semantic evidence independently of incumbents.

use clap::Parser;
use std::io;
use zetesis_cpu::Cancellation;

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
        let mut options = options();
        options.max_objective_work = maximum;
        let mut output = Vec::new();
        let outcome = crate::run_finalized_with_diagnostics(
            "{a}. #minimize {1,a:a}.".into(),
            &options,
            &mut output,
            &mut io::sink(),
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
    let mut options = options();
    options.max_json_record_bytes = 1;
    let failure = crate::run_finalized_with_diagnostics(
        "{a}. #minimize {1,a:a}.".into(),
        &options,
        &mut io::sink(),
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
