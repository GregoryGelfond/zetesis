//! Source admission work is independent of answer publication and search.

use std::{io, num::NonZeroUsize, time::Duration};

use clap::Parser;
use zetesis_cpu::Cancellation;
use zetesis_themelios::{SourceAdmission, SourceEvaluationStatistics};

use crate::failure::Progress;
use crate::{Backend, Options, PreparedInput, RunError, Session, SolveConfig};

use super::{Buffer, document_fixture::Document, source_evaluation_statistics};

fn config() -> SolveConfig {
    SolveConfig {
        backend: Backend::Cpu,
        workers: NonZeroUsize::MIN,
        models: 0,
        ..SolveConfig::default()
    }
}

fn evaluated(source: &str, interrupted: bool) -> (Progress, SourceEvaluationStatistics) {
    let preparation = zetesis_reference_support::prepared(source);
    let SourceAdmission::Admitted(owner) = preparation.try_source().unwrap() else {
        panic!("fixture must use direct source evaluation");
    };
    let receipt = owner.statistics();
    let cancellation = Cancellation::default();
    let mut session = Session::new(
        PreparedInput::source(&owner),
        config(),
        cancellation.clone(),
    )
    .unwrap();
    if interrupted {
        cancellation.cancel();
    }
    for answer in session.by_ref() {
        answer.unwrap();
    }
    let mut progress = Progress::new();
    progress.apply(session.stop());
    progress.phase_timings = crate::SolveMeasurements::new(true).snapshot();
    (progress, receipt)
}

#[test]
fn source_receipt_serializes_full_width_distinct_fields_with_bounded_writes() {
    for (work, substitutions) in [(0, 0), (17, 23), (u64::MAX, u64::MAX - 1)] {
        let receipt = SourceEvaluationStatistics {
            work,
            substitutions,
        };
        let mut complete = Buffer::new(256);
        source_evaluation_statistics(&mut complete, Some(&receipt)).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&complete.bytes).unwrap();
        assert_eq!(
            value,
            serde_json::json!({
                "scope":"source_admission", "work":work, "substitutions":substitutions,
            })
        );
        for maximum in 0..complete.bytes.len() {
            let mut refused = Buffer::new(maximum);
            assert!(source_evaluation_statistics(&mut refused, Some(&receipt)).is_err());
            assert!(refused.bytes.len() <= maximum);
        }
    }
    let mut absent = Buffer::new(4);
    source_evaluation_statistics(&mut absent, None).unwrap();
    assert_eq!(absent.bytes, b"null");
}

#[test]
fn source_receipts_survive_report_and_output_failure() {
    for (source, interrupted, verified, checked) in [
        ("a. b :- a.", false, 1, 1),
        ("a. :- a.", false, 0, 1),
        ("a. b :- a.", true, 0, 0),
    ] {
        let (progress, receipt) = evaluated(source, interrupted);
        assert!(receipt.work > 0);
        let semantic = progress.semantic().unwrap();
        assert_eq!(semantic.verified_models(), verified);
        assert_eq!(semantic.candidate_progress(), checked);
        assert_eq!(semantic.source_evaluation(), Some(&receipt));
        let mut report = progress.report().unwrap();
        assert_eq!(report.source_evaluation, Some(receipt));
        // Compatibility reports cannot mutate the independent live evidence.
        report.source_evaluation = None;
        assert!(report.source_evaluation.is_none());
        let failure = progress.fail(RunError::Output(io::Error::new(
            io::ErrorKind::BrokenPipe,
            "source answer sink closed",
        )));
        let partial = failure.partial_report.as_ref().unwrap();
        assert_eq!(partial.source_evaluation, Some(receipt));
        assert_eq!(partial.verified_models, verified);
        assert_eq!(partial.checked, checked);
        assert_eq!(partial.published_models, 0);
        assert_eq!(
            failure.semantic().unwrap().source_evaluation(),
            Some(&receipt)
        );
        let mut bytes = Vec::new();
        let document = Document::new(&mut bytes, true).unwrap();
        let failure = document.finish(Err(failure), 65_536).unwrap_err();
        assert!(matches!(&*failure.cause,
            RunError::Output(error) if error.to_string() == "source answer sink closed"));
        assert_source_json(&bytes, receipt);
        assert!(failure.publication().unwrap().summary());

        let (progress, receipt) = evaluated(source, interrupted);
        let mut bytes = Vec::new();
        Document::new(&mut bytes, true)
            .unwrap()
            .finish(Ok(progress), 65_536)
            .unwrap();
        assert_source_json(&bytes, receipt);
    }
}

fn assert_source_json(bytes: &[u8], receipt: SourceEvaluationStatistics) {
    let document: serde_json::Value = serde_json::from_slice(bytes).unwrap();
    let stats = &document["statistics"];
    assert_eq!(
        stats["source_evaluation"],
        serde_json::json!({
            "scope":"source_admission", "work":receipt.work, "substitutions":receipt.substitutions,
        })
    );
    for field in [
        "search",
        "execution",
        "closure_execution",
        "query_execution",
    ] {
        assert!(stats[field].is_null(), "no invented {field} receipt");
    }
}

#[test]
fn source_human_views_report_admission_without_closure_oracle_claims() {
    let (progress, receipt) = evaluated("a. b :- a.", false);
    let options = Options::try_parse_from([
        "zetesis",
        "--backend",
        "cpu",
        "--workers",
        "1",
        "--models",
        "0",
    ])
    .unwrap();
    let mut records = Vec::new();
    crate::statistics::write_progress(&mut records, &options, Ok(&progress), Duration::ZERO)
        .unwrap();
    let records = String::from_utf8(records).unwrap();
    assert!(records.contains("operation=direct source evaluation"));
    assert!(records.contains(&format!(
        "source evaluation: work={}; substitutions={}; scope=source_admission",
        receipt.work, receipt.substitutions,
    )));
    assert!(records.contains("completed source evaluations=1"));
    assert!(!records.contains("oracle=closure"));
    assert!(!records.contains("oracle work: unavailable"));
    let config = config();
    let view = crate::statistics_view::Statistics {
        requested: &config,
        timings: progress.phase_timings.as_ref().unwrap(),
        semantic: progress.semantic(),
        publication: Some(progress.publication),
        failed: false,
    };
    let mut table = Vec::new();
    view.write_human(
        &mut table,
        zetesis_presentation::Layout::new(NonZeroUsize::new(256).unwrap(), crate::ColorMode::Never),
    )
    .unwrap();
    let table = String::from_utf8(table).unwrap();
    assert!(table.contains("CPU direct source evaluation during admission"));
    assert!(table.contains("Source evaluation work"));
    assert!(table.contains("Source substitutions"));
    assert!(table.contains("Completed source evaluations"));
    assert!(!table.contains("Closure work"));
}
