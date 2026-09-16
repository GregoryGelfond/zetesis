//! Ordinary views preserve prepared CPU ownership evidence and complete models.

use std::io::{self, Write};

use clap::Parser;
use zetesis_cli::{
    Completion, Grounder, Interruption, Options, run_detailed_with_diagnostics,
    run_with_diagnostics,
};
use zetesis_cpu::{Control, Stop};

const SOURCE: &str = "{a}. {b}. {c}.";

fn options(extra: &[&str]) -> Options {
    Options::try_parse_from(
        [
            "zetesis",
            "--models",
            "0",
            "--backend",
            "cpu",
            "--stats",
            "--json",
            "--batch-size",
            "2",
            "--workers",
            "2",
        ]
        .into_iter()
        .chain(extra.iter().copied()),
    )
    .unwrap()
}

fn solve_source(
    source: &str,
    options: &Options,
) -> (zetesis_cli::Report, serde_json::Value, String) {
    let mut output = Vec::new();
    let mut diagnostics = Vec::new();
    let report = run_with_diagnostics(
        source.into(),
        options,
        &mut output,
        &mut diagnostics,
        &Control::default(),
    )
    .unwrap();
    (
        report,
        serde_json::from_slice(&output).unwrap(),
        String::from_utf8(diagnostics).unwrap(),
    )
}

fn solve(extra: &[&str]) -> (zetesis_cli::Report, serde_json::Value, String) {
    solve_source(SOURCE, &options(extra))
}

#[test]
fn prepared_receipts_accompany_the_complete_family() {
    let (report, json, diagnostics) = solve(&[]);
    let (reference, expected, _) = solve(&["--source-batching", "union"]);
    assert_eq!(report.completion, Completion::Exhausted);
    assert_eq!(report.models, 8);
    assert_eq!(reference.models, 8);
    assert_eq!(json["models"], expected["models"]);
    let observation = report.query_execution.unwrap();
    assert!(observation.fault.is_none());
    let statistics = observation.statistics.unwrap();
    assert_eq!(statistics.preparation_builds, 1);
    assert!(statistics.reused_workspaces > 0);
    assert!(statistics.retained_bytes > 0);
    assert_eq!(
        json["statistics"]["query_execution"]["snapshot"]["preparation_builds"],
        1
    );
    assert_eq!(
        json["statistics"]["query_execution"]["snapshot"]["reused_workspaces"],
        statistics.reused_workspaces
    );
    assert!(json["statistics"]["query_execution"]["fault"].is_null());
    assert!(expected["statistics"]["query_execution"].is_null());
    assert!(reference.query_execution.is_none());
    assert!(diagnostics.contains("prepared CPU queries: builds=1"));
    assert!(diagnostics.contains("separate from candidate work"));
}

#[test]
fn source_work_refusal_remains_a_preparation_stop() {
    let (report, json, _) = solve(&["--max-source-work", "0"]);
    assert_eq!(report.models, 0);
    assert_eq!(report.completion, Completion::Interrupted);
    assert_eq!(
        report.interruption,
        Some(Interruption::Preparation(Stop::WorkLimit))
    );
    assert_eq!(json["models"].as_array().unwrap().len(), 0);
    assert_eq!(json["outcome"]["interruption"]["kind"], "preparation");
    let observation = report.query_execution.unwrap();
    assert!(observation.fault.is_none());
    assert_eq!(observation.statistics.unwrap().preparation_builds, 0);
    assert!(json["statistics"]["query_execution"]["snapshot"]["preparation"].is_null());
}

#[test]
fn candidate_storage_refusal_follows_admitted_preparation() {
    let owner =
        zetesis_themelios::admit("a.".into(), zetesis_themelios::AdmissionOptions::default())
            .unwrap();
    let prepared = zetesis_cpu::PreparedQueries::new(
        owner.program(),
        zetesis_cpu::PreparationLimits::default(),
        &Control::default(),
    )
    .unwrap();
    let mut bounded = options(&[]);
    // Admit exactly the immutable preparation. Mutable candidate storage has
    // no remaining allowance; this is a later boundary than a zero-byte setup.
    bounded.max_closure_bytes = Some(prepared.statistics().retained_bytes);
    let (report, json, _) = solve_source("a.", &bounded);
    assert_eq!(report.models, 0);
    assert_eq!(report.completion, Completion::Interrupted);
    assert_eq!(
        report.interruption,
        Some(Interruption::Oracle(Stop::StorageLimit))
    );
    assert_eq!(json["outcome"]["interruption"]["kind"], "oracle");
    assert_eq!(json["outcome"]["interruption"]["code"], "storage_limit");
    assert!(json["models"].as_array().unwrap().is_empty());
    let observed = report.query_execution.unwrap();
    assert!(observed.fault.is_none());
    let statistics = observed.statistics.unwrap();
    assert_eq!(statistics.preparation_builds, 1);
    assert_eq!(statistics.active_workspaces, 1);
    assert_eq!(statistics.preparation, Some(prepared.statistics()));

    let (complete, expected, _) = solve_source("a.", &options(&[]));
    assert_eq!(complete.completion, Completion::Exhausted);
    assert_eq!(complete.models, 1);
    assert_eq!(
        expected["models"][0]["model"]["full_model"],
        serde_json::json!([
            {"predicate":"a", "sign":"positive", "arguments":[]}
        ])
    );
}

struct RefuseQueryStatistics;
impl Write for RefuseQueryStatistics {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes
            .windows(b"prepared CPU queries".len())
            .any(|window| window == b"prepared CPU queries")
        {
            Err(io::Error::other("query statistics sink refused"))
        } else {
            Ok(bytes.len())
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn publication_failure_retains_query_ownership_evidence() {
    let mut output = Vec::new();
    let failure = run_detailed_with_diagnostics(
        SOURCE.into(),
        &options(&[]),
        &mut output,
        &mut RefuseQueryStatistics,
        &Control::default(),
    )
    .unwrap_err();
    let partial = failure.partial_report.as_ref().unwrap();
    assert_eq!(partial.verified_models, 8);
    assert_eq!(partial.published_models, 8);
    assert_eq!(partial.completion, Some(Completion::Exhausted));
    let observation = partial.query_execution.as_ref().unwrap();
    assert!(observation.fault.is_none());
    assert_eq!(observation.statistics.unwrap().preparation_builds, 1);
    assert!(observation.statistics.unwrap().reused_workspaces > 0);
    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["models"].as_array().unwrap().len(), 8);
    assert_eq!(
        json["statistics"]["query_execution"]["snapshot"]["preparation_builds"],
        1
    );
}

#[test]
fn closure_receipts_sum_every_completed_check() {
    // Eight candidates, each an answer set; every atom lies in four closures.
    let (report, json, diagnostics) = solve(&[]);
    let closure = report.closure_execution.unwrap();
    assert_eq!(closure.grounder, Grounder::Lazy);
    assert_eq!(closure.completed_checks, 8);
    assert_eq!(closure.stopped_checks, 0);
    assert_eq!(closure.derived_atoms, 12);
    assert!(closure.work > 0);
    let joins = closure.joins.unwrap();
    assert!(joins.peak_closure_bytes > 0);
    let encoded = &json["statistics"]["closure_execution"];
    assert_eq!(encoded["grounder"], "lazy");
    assert_eq!(encoded["completed_checks"], 8);
    assert_eq!(encoded["stopped_checks"], 0);
    assert_eq!(encoded["derived_atoms"], 12);
    assert_eq!(encoded["work"], closure.work);
    assert_eq!(encoded["joins"]["bindings"], joins.bindings);
    assert_eq!(
        encoded["joins"]["peak_closure_bytes"],
        joins.peak_closure_bytes
    );
    assert!(
        diagnostics.contains("independent closure: checks completed=8; stopped=0;"),
        "{diagnostics}"
    );
    assert!(
        diagnostics.contains("closure joins: catalog work="),
        "{diagnostics}"
    );
    assert!(
        !diagnostics.contains("oracle work=unavailable"),
        "{diagnostics}"
    );
}

#[test]
fn eager_closure_receipts_carry_no_join_counters() {
    let (report, json, diagnostics) = solve(&["--grounder", "eager"]);
    let closure = report.closure_execution.unwrap();
    assert_eq!(closure.grounder, Grounder::Eager);
    assert_eq!(closure.completed_checks, 8);
    assert_eq!(closure.derived_atoms, 12);
    assert!(closure.joins.is_none());
    assert_eq!(json["statistics"]["closure_execution"]["grounder"], "eager");
    assert!(json["statistics"]["closure_execution"]["joins"].is_null());
    assert!(report.query_execution.is_none());
    assert!(
        diagnostics.contains("independent closure: checks completed=8; stopped=0;"),
        "{diagnostics}"
    );
    assert!(!diagnostics.contains("closure joins:"), "{diagnostics}");
}
