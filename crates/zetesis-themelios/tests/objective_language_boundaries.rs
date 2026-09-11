//! Exact witnesses for the remaining aggregate-head and objective boundaries.
//!
//! The fixture retains complete clingo 5.8.2 byte captures from the unchanged
//! sources. Its guarded empty-extremum results are reference observations, not
//! an interpretation adopted by native admission. Cyclic objective refusals
//! are distinguished from invalid programs by exhaustive original-theory checks.

#[path = "support/source_records.rs"]
mod source_records;
#[path = "support/source_oracle_records.rs"]
mod source_oracle_records;

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::PathBuf;
use std::time::Duration;

use serde_json::Value as Json;
use themelios_base::source::SourceId;
use zetesis_themelios::{
    AdmissionFailure, AdmissionOptions, ExpansionFailure, ExpansionLimits, FormulaFailure,
    FormulaLimits, ProfileFeature, admit_formula,
};
use zetesis_validation::process::{Exit, Invocation, Limits, PendingChild, Stop, invoke};

const CASES: &str = include_str!("fixtures/objective-language-boundaries.jsonl");
const SOURCE: SourceId = SourceId::new(173);

fn cases() -> Vec<Json> {
    let cases: Vec<Json> = CASES
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(cases.len(), 7);
    let mut names = BTreeSet::new();
    let mut sources = BTreeSet::new();
    for case in &cases {
        assert!(names.insert(case["name"].as_str().unwrap()));
        assert!(sources.insert(case["source"].as_str().unwrap()));
    }
    cases
}

#[test]
fn residual_sources_have_located_profile_refusals() {
    for case in cases() {
        let source = case["source"].as_str().unwrap();
        let expected = match case["native_feature"].as_str().unwrap() {
            "HeadAggregateMissingValue" => ProfileFeature::HeadAggregateMissingValue,
            "AnalysisPool" => ProfileFeature::AnalysisPool,
            "ObjectiveSourceEligibility" => ProfileFeature::ObjectiveSourceEligibility,
            feature => panic!("unclassified boundary {feature}"),
        };
        let error = admit_formula(
            source.into(),
            AdmissionOptions {
                source_id: SOURCE,
                ..AdmissionOptions::default()
            },
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap_err();
        let FormulaFailure::Expansion(ExpansionFailure::Admission(AdmissionFailure::Profile {
            feature,
            location,
        })) = &error
        else {
            panic!("{source}: {error}");
        };
        assert_eq!(*feature, expected, "{source}");
        assert_eq!(location.source, SOURCE, "{source}");
        assert_eq!(
            [
                u64::from(location.span.start().get()),
                u64::from(location.span.end().get()),
            ],
            [
                case["native_span"][0].as_u64().unwrap(),
                case["native_span"][1].as_u64().unwrap(),
            ],
            "{source}"
        );
        let diagnostics = error.diagnostics();
        assert_eq!(diagnostics.len(), 1, "{source}");
        assert_eq!(diagnostics[0].primary().location, *location, "{source}");
    }
}

#[test]
fn cyclic_objective_gaps_preserve_original_answers() {
    let mut originals = 0;
    for case in cases() {
        let Some(original) = case["original_source"].as_str() else {
            continue;
        };
        assert_eq!(case["native_feature"], "ObjectiveSourceEligibility");
        assert_eq!(
            case["source"]
                .as_str()
                .unwrap()
                .split_once("#minimize")
                .unwrap()
                .0,
            original
        );
        let input = source_records::admit(original, &FormulaLimits::default()).unwrap();
        assert!(input.objectives().templates().is_empty());
        let expected: source_records::Records = case["original_records"]
            .as_array()
            .unwrap()
            .iter()
            .map(|record| {
                (
                    source_records::atoms(&record[0]),
                    source_records::costs(&record[1]),
                )
            })
            .collect();
        assert_eq!(source_records::exhaustive(&input), expected, "{original}");
        originals += 1;
    }
    assert_eq!(originals, 4);
}

#[test]
#[ignore = "requires independent clingo 5.8.2 on PATH or through CLINGO"]
fn original_boundary_sources_retain_reference_outcomes() {
    let executable = clingo();
    for case in cases() {
        let source = case["source"].as_str().unwrap();
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("source.lp");
        std::fs::write(&input, source).unwrap();
        let arguments = [
            input.clone().into_os_string(),
            OsString::from("0"),
            OsString::from("--outf=2"),
            OsString::from("--opt-mode=enum"),
        ];
        assert_eq!(
            case["reference_arguments"],
            serde_json::json!(["-", "0", "--outf=2", "--opt-mode=enum"])
        );
        let (capture, pending) = invoke(
            Invocation {
                executable: &executable,
                arguments: &arguments,
                directory: directory.path(),
            },
            Limits {
                timeout: Duration::from_secs(5),
                max_output_bytes: 65_536,
                cleanup_timeout: Duration::from_secs(1),
            },
        )
        .unwrap()
        .into_parts();
        if let Some(pending) = pending {
            let cleanup = pending.retry(Duration::from_secs(1));
            let abandoned = cleanup.pending.map(PendingChild::abandon);
            assert!(
                abandoned.is_none(),
                "unreaped child: {abandoned:?}; cleanup failure: {:?}",
                cleanup.failure
            );
            assert!(cleanup.failure.is_none(), "{:?}", cleanup.failure);
        }
        assert_eq!(capture.stop(), Stop::Completed, "{source}: {capture:?}");
        assert!(capture.failure().is_none(), "{capture:?}");
        assert!(capture.cleanup_failure().is_none(), "{capture:?}");
        assert_eq!(
            capture.exit(),
            Some(Exit {
                code: Some(i32::try_from(case["reference_exit"].as_i64().unwrap()).unwrap()),
                signal: None,
            }),
            "{source}"
        );
        let reference: Json =
            serde_json::from_str(case["reference_stdout"].as_str().unwrap()).unwrap();
        let actual: Json = serde_json::from_slice(capture.stdout()).unwrap();
        assert_eq!(reference["Solver"], "clingo version 5.8.2");
        assert_eq!(actual["Solver"], reference["Solver"], "{source}");
        assert_eq!(reference["Input"], serde_json::json!(["-"]));
        assert_eq!(actual["Input"], serde_json::json!([input]), "{source}");
        for field in ["Result", "Models", "Calls"] {
            assert_eq!(actual[field], reference[field], "{source}: {field}");
        }
        assert_eq!(
            source_oracle_records::model_records(&actual),
            source_oracle_records::model_records(&reference),
            "{source}"
        );
        // The original capture used stdin. Normalize only the corresponding
        // input filename; retain every diagnostic byte apart from that name.
        let diagnostics = capture
            .stderr_text()
            .unwrap()
            .replace(input.to_str().unwrap(), "-");
        assert_eq!(
            diagnostics,
            case["reference_stderr"].as_str().unwrap(),
            "{source}"
        );
    }
}

fn clingo() -> PathBuf {
    std::env::var_os("CLINGO")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::split_paths(&std::env::var_os("PATH")?)
                .map(|directory| directory.join("clingo"))
                .find(|path| path.is_file())
        })
        .expect("independent clingo on PATH or through CLINGO")
        .canonicalize()
        .expect("absolute oracle executable")
}
