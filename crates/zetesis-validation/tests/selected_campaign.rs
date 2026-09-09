//! Public campaign contracts exercised through bounded, deliberately fallible producers.
#![cfg(any(target_os = "linux", target_os = "macos"))]

use std::fmt::Write as _;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{Value, json};
use zetesis_validation::{curated, selected};

struct Fixture {
    directory: tempfile::TempDir,
    corpus: PathBuf,
    reference: PathBuf,
    native: PathBuf,
    report: PathBuf,
}

fn quote(path: &Path) -> String {
    format!("'{}'", path.to_str().unwrap().replace('\'', "'\\''"))
}

fn executable(path: &Path, body: &str) {
    fs::write(path, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
}

impl Fixture {
    fn new(mut change: impl FnMut(usize, &mut Value)) -> Self {
        let directory = tempfile::tempdir().unwrap();
        let corpus = directory.path().join("corpus");
        fs::create_dir(&corpus).unwrap();
        let original = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../validation/upstream/clingo-5.8.2/curated");
        let checked = curated::open(&original, curated::Limits::default()).unwrap();
        for name in ["manifest.json", "LICENSE.md"] {
            fs::copy(original.join(name), corpus.join(name)).unwrap();
        }
        let mut records: Vec<Value> =
            serde_json::from_str(include_str!("support/selected/reports.json")).unwrap();
        let mut reference_script = String::from("for input do :; done\n");
        let mut native_script = reference_script.clone();
        for (index, (case, record)) in checked.cases().iter().zip(&mut records).enumerate() {
            assert_eq!(record["id"], case.id());
            change(index, record);
            let source = corpus.join(case.path());
            fs::create_dir_all(source.parent().unwrap()).unwrap();
            fs::write(&source, case.source()).unwrap();
            for (producer, script) in [
                ("reference", &mut reference_script),
                ("native", &mut native_script),
            ] {
                let output = directory.path().join(format!("{producer}-{index}.json"));
                fs::write(&output, serde_json::to_vec(&record[producer]).unwrap()).unwrap();
                writeln!(
                    script,
                    "if /usr/bin/cmp -s \"$input\" {}; then /bin/cat {}; exit 0; fi",
                    quote(&source),
                    quote(&output)
                )
                .unwrap();
            }
        }
        let reference = directory.path().join("reference");
        let native = directory.path().join("native");
        executable(&reference, &(reference_script + "exit 3"));
        executable(&native, &(native_script + "exit 3"));
        let report = directory.path().join("report.json");
        Self {
            directory,
            corpus,
            reference,
            native,
            report,
        }
    }

    fn request(&self) -> selected::Request<'_> {
        selected::Request {
            corpus: &self.corpus,
            reference: &self.reference,
            native: &self.native,
            report: &self.report,
            execution: selected::NativeExecution::default(),
            limits: selected::Limits::default(),
        }
    }

    fn run(&self) -> selected::Report {
        selected::run(&self.request()).unwrap()
    }
}

#[test]
fn exact_full_model_campaign_passes() {
    let fixture = Fixture::new(|_, _| {});
    let report = fixture.run();
    assert!(report.passed(), "{report:?}");
    assert_eq!(report.cases().len(), selected::CASE_COUNT);
    assert!(
        report
            .cases()
            .iter()
            .all(|case| case.decision() == selected::Decision::Pass)
    );
    report.publish().unwrap();
    let stored: Value = serde_json::from_slice(&fs::read(&fixture.report).unwrap()).unwrap();
    assert_eq!(
        stored["cases"].as_array().unwrap().len(),
        selected::CASE_COUNT
    );
    assert_eq!(stored["physical_execution_qualified"], false);
    assert_eq!(stored["passed"], true);
}

#[test]
fn campaign_schema_names_the_license_ceiling() {
    let fixture = Fixture::new(|_, _| {});
    let report = fixture.run();
    let stored = serde_json::to_value(&report).unwrap();
    assert_eq!(stored["schema"], 2);
    let limits = &stored["requested_limits"]["corpus"];
    assert_eq!(
        limits["license_bytes"],
        curated::Limits::default().license_bytes
    );
    assert!(limits.get("original_bytes").is_none());
}

#[test]
fn hidden_identity_mismatch_cannot_pass() {
    let fixture = Fixture::new(|index, record| {
        if index == 0 {
            record["native"]["models"][1]["model"]["full_model"][0]["predicate"] = "r".into();
            // Suppress every display: this test must compare typed full identity.
            for model in record["native"]["models"].as_array_mut().unwrap() {
                model["model"]["shown"]["atom_indices"] = json!([]);
            }
        }
    });
    let report = fixture.run();
    assert!(!report.passed());
    assert_eq!(
        report.cases()[0].decision(),
        selected::Decision::ModelMismatch
    );
}

#[test]
fn incomplete_enumeration_cannot_pass() {
    let fixture = Fixture::new(|index, record| {
        if index == 0 {
            record["native"]["outcome"]["coverage"] = "stopped".into();
        }
    });
    let report = fixture.run();
    assert_eq!(
        report.cases()[0].decision(),
        selected::Decision::InvalidReport
    );
    assert!(report.cases()[0].detail().is_some());
}

#[test]
fn reference_mismatch_cannot_pass() {
    let fixture = Fixture::new(|index, record| {
        if index == 0 {
            record["reference"]["Call"][0]["Witnesses"][1]["Value"] = json!(["r"]);
        }
    });
    let report = fixture.run();
    assert_eq!(
        report.cases()[0].decision(),
        selected::Decision::ModelMismatch
    );
}

#[test]
fn nonzero_native_exit_preserves_failure_evidence() {
    let fixture = Fixture::new(|_, _| {});
    executable(&fixture.native, "printf diagnosis >&2; exit 7");
    let report = fixture.run();
    assert!(!report.passed());
    for case in report.cases() {
        assert_eq!(case.decision(), selected::Decision::InvocationFailure);
        let native = case.native().unwrap();
        assert_eq!(native.exit().unwrap().code, Some(7));
        assert_eq!(native.stderr(), b"diagnosis");
    }
    report.publish().unwrap();
}

#[test]
fn zero_campaign_capture_limit_is_respected() {
    let fixture = Fixture::new(|_, _| {});
    let mut request = fixture.request();
    request.limits.max_total_capture_bytes = 0;
    let report = selected::run(&request).unwrap();
    assert!(!report.passed());
    for case in report.cases() {
        assert!(case.reference().stdout().is_empty());
        assert!(case.native().unwrap().stdout().is_empty());
    }
}

#[test]
fn output_stop_preserves_a_bounded_raw_prefix() {
    let fixture = Fixture::new(|_, _| {});
    executable(&fixture.native, "printf partial; printf warning >&2");
    let mut request = fixture.request();
    request.limits.process.max_output_bytes = 5;
    let report = selected::run(&request).unwrap();
    let native = report.cases()[0].native().unwrap();
    assert_eq!(
        native.stop(),
        Some(zetesis_validation::process::Stop::OutputLimit)
    );
    assert_eq!(native.stdout().len() + native.stderr().len(), 5);
    assert!(b"partial".starts_with(native.stdout()));
    assert!(b"warning".starts_with(native.stderr()));
    assert_eq!(
        report.cases()[0].decision(),
        selected::Decision::InvocationFailure
    );
}

#[test]
fn report_aliases_cannot_replace_an_executable() {
    let fixture = Fixture::new(|_, _| {});
    let linked = fixture.directory.path().join("report-link");
    let hard = fixture.directory.path().join("report-hard");
    std::os::unix::fs::symlink(&fixture.native, &linked).unwrap();
    fs::hard_link(&fixture.native, &hard).unwrap();
    let before = fs::read(&fixture.native).unwrap();
    for path in [&fixture.native, &linked, &hard] {
        let request = selected::Request {
            report: path,
            ..fixture.request()
        };
        assert!(matches!(
            selected::run(&request),
            Err(selected::Error::Path { .. })
        ));
    }
    assert_eq!(fs::read(&fixture.native).unwrap(), before);
}

#[test]
fn zero_deadline_retains_an_incomplete_campaign() {
    let fixture = Fixture::new(|_, _| {});
    let mut request = fixture.request();
    request.limits.process.timeout = Duration::ZERO;
    let report = selected::run(&request).unwrap();
    assert!(!report.passed());
    assert!(
        report
            .cases()
            .iter()
            .all(|case| case.decision() == selected::Decision::InvocationFailure)
    );
}

#[test]
fn source_mutation_invalidates_input_seals() {
    let fixture = Fixture::new(|_, _| {});
    let source = fixture
        .corpus
        .join("programs/lparse/bug-disjunction-range/01.lp");
    executable(
        &fixture.native,
        &format!("printf changed > {}; exit 7", quote(&source)),
    );
    let report = fixture.run();
    assert!(!report.passed());
    assert!(report.after().iter().any(|change| !change.unchanged()));
}

#[test]
fn missing_executable_is_a_setup_error() {
    let fixture = Fixture::new(|_, _| {});
    fs::remove_file(&fixture.native).unwrap();
    assert!(matches!(
        selected::run(&fixture.request()),
        Err(selected::Error::Io { .. })
    ));
}

#[test]
fn existing_report_is_never_overwritten() {
    let fixture = Fixture::new(|_, _| {});
    fs::write(&fixture.report, b"keep").unwrap();
    assert!(matches!(
        selected::run(&fixture.request()),
        Err(selected::Error::Path { .. })
    ));
    assert_eq!(fs::read(&fixture.report).unwrap(), b"keep");
}

#[test]
fn report_publication_rechecks_the_destination() {
    let fixture = Fixture::new(|_, _| {});
    let report = fixture.run();
    fs::write(&fixture.report, b"new owner").unwrap();
    assert!(matches!(
        report.publish(),
        Err(selected::Error::Path { .. })
    ));
    assert_eq!(fs::read(&fixture.report).unwrap(), b"new owner");
}

#[test]
fn report_ceiling_is_checked_before_publication() {
    let fixture = Fixture::new(|_, _| {});
    let mut request = fixture.request();
    request.limits.max_report_bytes = 0;
    let report = selected::run(&request).unwrap();
    assert!(matches!(
        report.publish(),
        Err(selected::Error::Bytes { limit: 0, .. })
    ));
    assert!(!fixture.report.exists());
}

#[test]
fn report_inside_the_corpus_is_refused() {
    let fixture = Fixture::new(|_, _| {});
    let path = fixture.corpus.join("report.json");
    let request = selected::Request {
        report: &path,
        ..fixture.request()
    };
    assert!(matches!(
        selected::run(&request),
        Err(selected::Error::Path { .. })
    ));
}

#[test]
fn executable_aliases_are_refused() {
    let fixture = Fixture::new(|_, _| {});
    fs::remove_file(&fixture.native).unwrap();
    fs::hard_link(&fixture.reference, &fixture.native).unwrap();
    assert!(matches!(
        selected::run(&fixture.request()),
        Err(selected::Error::Path { .. })
    ));
}

#[test]
fn relative_executables_are_refused() {
    let fixture = Fixture::new(|_, _| {});
    let request = selected::Request {
        native: Path::new("zetesis"),
        ..fixture.request()
    };
    assert!(matches!(
        selected::run(&request),
        Err(selected::Error::Path { .. })
    ));
}

#[test]
fn executable_sealing_has_an_exact_byte_ceiling() {
    let fixture = Fixture::new(|_, _| {});
    let mut request = fixture.request();
    request.limits.max_executable_bytes = 0;
    assert!(matches!(
        selected::run(&request),
        Err(selected::Error::Bytes { limit: 0, .. })
    ));
}

#[test]
fn failed_start_remains_a_case_result() {
    let fixture = Fixture::new(|_, _| {});
    fs::set_permissions(&fixture.native, fs::Permissions::from_mode(0o600)).unwrap();
    let report = fixture.run();
    assert!(!report.passed());
    assert_eq!(
        report.cases()[0]
            .native()
            .unwrap()
            .failure()
            .unwrap()
            .kind(),
        selected::InvocationFault::Spawn
    );
    assert!(
        report
            .cases()
            .iter()
            .all(|case| case.native().unwrap().stop().is_none())
    );
}

#[test]
fn campaign_exposes_its_execution_request() {
    let fixture = Fixture::new(|_, _| {});
    let report = fixture.run();
    let native = report.cases()[0].native().unwrap();
    assert_eq!(native.executable(), fixture.native);
    assert!(
        native
            .arguments()
            .windows(2)
            .any(|pair| pair == ["--models", "0"])
    );
    assert!(
        native
            .arguments()
            .iter()
            .any(|argument| argument == "--json")
    );
    assert!(native.elapsed_ns().is_some());
    assert!(native.failure().is_none());
    assert!(native.cleanup_failure().is_none());
    assert!(report.faults().is_empty());
    assert!(report.unresolved_children().is_empty());
}

fn changed_input_report(fixture: &Fixture, path: &Path) -> selected::Report {
    let original = fs::read_to_string(&fixture.native).unwrap();
    executable(
        &fixture.native,
        &format!("printf '\\n# changed\\n' >> {}\n{original}", quote(path)),
    );
    fixture.run()
}

#[test]
fn post_run_license_mutation_invalidates_a_pass() {
    let fixture = Fixture::new(|_, _| {});
    let report = changed_input_report(&fixture, &fixture.corpus.join("LICENSE.md"));
    assert!(
        report
            .cases()
            .iter()
            .all(|case| case.decision() == selected::Decision::Pass)
    );
    assert!(!report.passed());
}

#[test]
fn post_run_executable_mutation_invalidates_a_pass() {
    let fixture = Fixture::new(|_, _| {});
    let report = changed_input_report(&fixture, &fixture.reference);
    assert!(
        report
            .cases()
            .iter()
            .all(|case| case.decision() == selected::Decision::Pass)
    );
    assert!(!report.passed());
}

#[test]
fn cli_publishes_a_complete_campaign() {
    let fixture = Fixture::new(|_, _| {});
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_zetesis-corpus"))
        .arg("compare")
        .arg(&fixture.corpus)
        .arg("--clingo")
        .arg(&fixture.reference)
        .arg("--zetesis")
        .arg(&fixture.native)
        .arg("--report")
        .arg(&fixture.report)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(fixture.report.exists());
    assert!(fixture.directory.path().exists());
}
