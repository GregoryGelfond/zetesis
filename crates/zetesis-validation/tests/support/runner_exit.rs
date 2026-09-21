//! Observed child exits survive corpus classification and compatibility rendering.

use crate::corpus_comparison::Decision;
use crate::process::Exit;

fn captured(script: &str, native: bool) -> crate::corpus_comparison::CaseResult {
    let directory = tempfile::tempdir().unwrap();
    let mut options = super::options(directory.path());
    let executable = super::script(directory.path(), "exit-fixture", script);
    if native {
        options.zetesis = executable;
    } else {
        options.clingo = executable;
    }
    let report = super::super::run(&options, super::loaded(directory.path(), 1), |_| {});
    assert!(report.cleanup.is_empty());
    assert!(!report.passed());
    report.cases.into_iter().next().unwrap()
}

#[test]
fn native_signal_exit_survives_the_report_boundary() {
    let result = captured("kill -TERM $$", true);
    assert_eq!(result.decision(), &Decision::NativeError);
    assert_eq!(
        result.native_exit(),
        Some(Exit {
            code: None,
            signal: Some(15)
        })
    );
    let json = result.to_json().unwrap();
    assert!(json["native_process"]["exit_code"].is_null());
    assert_eq!(json["native_process"]["exit_signal"], 15);
    assert!(json["native_process"]["pending_child_id"].is_null());
}

#[test]
fn reference_signal_exit_survives_the_report_boundary() {
    let result = captured("kill -TERM $$", false);
    assert_eq!(result.decision(), &Decision::ReferenceError);
    assert_eq!(
        result.reference_exit(),
        Some(Exit {
            code: None,
            signal: Some(15)
        })
    );
    assert!(result.native_exit().is_none());
    let json = result.to_json().unwrap();
    assert_eq!(json["reference_process"]["exit_signal"], 15);
    assert!(json.get("native_process").is_none());
}

#[test]
fn normal_nonzero_exit_is_distinct_from_signal_termination() {
    let result = captured("exit 17", true);
    assert_eq!(result.decision(), &Decision::NativeError);
    assert_eq!(
        result.native_exit(),
        Some(Exit {
            code: Some(17),
            signal: None
        })
    );
    let json = result.to_json().unwrap();
    assert_eq!(json["native_process"]["exit_code"], 17);
    assert!(json["native_process"]["exit_signal"].is_null());
}
