//! Synthetic processes exercise matrix accounting; they claim no solver parity.
#![cfg(any(target_os = "linux", target_os = "macos"))]
use std::fs;
use std::num::NonZeroUsize;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;
use zetesis_validation::{
    performance::{
        self, Phase,
        matrix::{self, Decision, Plan, Producer, Suite},
    },
    selected::NativeExecution,
};

struct Fixture {
    _directory: tempfile::TempDir,
    native: PathBuf,
    reference: PathBuf,
    report: PathBuf,
    corpus: PathBuf,
}
fn executable(path: &Path, body: &str) {
    fs::write(
        path,
        format!("#!/bin/sh\nif [ \"$#\" -eq 1 ]; then echo fixture; exit 0; fi\n{body}\n"),
    )
    .unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
}
impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let native = directory.path().join("native");
        let reference = directory.path().join("reference");
        executable(
            &native,
            "printf '%s' '{\"schema\":2,\"format\":\"zetesis\",\"models\":[],\"statistics\":null,\"outcome\":{\"status\":\"failed\",\"completion\":null,\"coverage\":\"unavailable\",\"published_models\":0,\"verified_models\":null,\"checked\":null,\"interruption\":null,\"optimization\":null,\"error\":{\"kind\":\"unsupported_combination\",\"secondary_output_failure\":false}}}'; exit 2",
        );
        executable(
            &reference,
            "printf '%s' '{\"Result\":\"UNSATISFIABLE\",\"Models\":{\"More\":\"no\",\"Number\":0},\"Call\":[{}]}'",
        );
        Self {
            corpus: Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/kr-domains"),
            report: directory.path().join("matrix.json"),
            native,
            reference,
            _directory: directory,
        }
    }
    fn request(&self, suite: Suite) -> matrix::Request<'_> {
        matrix::Request {
            corpus: &self.corpus,
            native: &self.native,
            reference: &self.reference,
            report: &self.report,
            plan: Plan::new(
                suite,
                vec![NativeExecution::default()],
                NonZeroUsize::new(1).unwrap(),
                0,
                1,
            )
            .unwrap(),
            limits: performance::Limits::default(),
            native_answers: zetesis_validation::answers::native_json::Limits::default(),
            max_spelling_bytes: 8_388_608,
            helper: None,
        }
    }
}

#[test]
fn memory_rounds_record_the_reference_peak_resident_set() {
    let fixture = Fixture::new();
    let helper = Path::new(env!("CARGO_BIN_EXE_zetesis-perf"));
    let mut request = fixture.request(Suite::Queens);
    request.plan = request.plan.with_memory(2).unwrap();
    request.helper = Some(helper);
    let workloads = [variant(&fixture, 10)];
    let report = matrix::run_workloads(&request, &workloads).unwrap();
    assert!(report.accounted());
    // Qualification, one timed round and two memory rounds, for two producers.
    assert_eq!(report.samples().len(), 8);
    let memory: Vec<_> = report
        .samples()
        .iter()
        .filter(|sample| sample.slot().phase == Phase::Memory)
        .collect();
    assert_eq!(memory.len(), 4);
    for sample in &memory {
        match sample.slot().producer {
            Producer::Reference => {
                assert_eq!(sample.decision(), Decision::Pass, "{sample:?}");
                let measurement = sample.memory().unwrap();
                assert!(measurement.peak_rss_bytes > 0);
                // The capture is the helper's; the record is its child's.
                let helper_child = sample.capture().unwrap().helper_child_id();
                assert!(helper_child.is_some());
                assert_ne!(Some(measurement.child), helper_child);
            }
            // The refused native cell launches nothing more, memory rounds included.
            Producer::Native { .. } => {
                assert_eq!(sample.decision(), Decision::NotAttempted);
                assert!(sample.memory().is_none());
            }
        }
    }
    // The helper is sealed with the executables.
    assert!(
        report
            .before()
            .iter()
            .any(|seal| seal.requested() == helper)
    );
    report.publish().unwrap();
    let encoded: serde_json::Value =
        serde_json::from_slice(&fs::read(&fixture.report).unwrap()).unwrap();
    assert!(
        encoded["report"]["peak_rss"]
            .as_str()
            .unwrap()
            .starts_with("memory_rounds:")
    );
    let recorded = encoded["report"]["samples"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|sample| sample["slot"]["phase"] == "memory" && sample["decision"] == "pass")
        .count();
    assert_eq!(recorded, 2);
}

#[test]
fn memory_rounds_require_an_absolute_helper() {
    let fixture = Fixture::new();
    let mut request = fixture.request(Suite::Queens);
    request.plan = request.plan.with_memory(1).unwrap();
    let error = matrix::run(&request).unwrap_err();
    assert!(error.to_string().contains("helper"), "{error}");
    request.helper = Some(Path::new("zetesis-perf"));
    let error = matrix::run(&request).unwrap_err();
    assert!(error.to_string().contains("helper"), "{error}");
}
#[test]
fn every_corpus_cell_retains_its_refusal() {
    let fixture = Fixture::new();
    let report = matrix::run(&fixture.request(Suite::Corpus)).unwrap();
    assert!(report.accounted());
    assert!(!report.passed());
    assert_eq!(report.cases().len(), 94);
    let census: Vec<_> = report
        .samples()
        .iter()
        .filter(|s| {
            s.slot().phase == Phase::Qualification
                && matches!(s.slot().producer, Producer::Native { .. })
        })
        .collect();
    assert_eq!(census.len(), 94);
    assert!(
        census
            .iter()
            .all(|s| s.decision() == Decision::Refused && s.capture().is_some())
    );
    for sample in report.samples().iter().filter(|s| {
        s.slot().phase == Phase::Timed && matches!(s.slot().producer, Producer::Native { .. })
    }) {
        assert_eq!(sample.decision(), Decision::NotAttempted);
        assert!(sample.capture().is_none());
        let prior = &report.samples()[sample.blocked_by().unwrap()];
        assert_eq!(prior.slot().case, sample.slot().case);
        assert_eq!(prior.decision(), Decision::Refused);
    }
    report.publish().unwrap();
    let value: serde_json::Value =
        serde_json::from_slice(&fs::read(&fixture.report).unwrap()).unwrap();
    assert_eq!(value["accounted"], true);
    assert_eq!(value["passed"], false);
    assert_eq!(
        value["report"]["protocol"],
        "instrumented_explicit_profile_matrix_v1"
    );
    assert_eq!(value["report"]["schema"], 1);
    assert!(value["report"].get("workloads").is_none());
}

fn variant(fixture: &Fixture, size: i32) -> matrix::Workload {
    let corpus = zetesis_validation::examples::load(
        &fixture.corpus,
        zetesis_validation::examples::Limits::default(),
    )
    .unwrap();
    let path = "standalone/n-queens/variant-01.lp";
    matrix::Workload::amended(
        &corpus,
        path,
        &[matrix::ConstantAmendment {
            source_path: path,
            name: "n",
            expected: 8,
            replacement: size,
        }],
        matrix::WorkloadLimits::default(),
    )
    .unwrap()
}

#[test]
fn derived_cells_use_their_own_sealed_source_bytes() {
    let fixture = Fixture::new();
    let prior = fs::read_to_string(&fixture.native).unwrap();
    let metadata_end = prior.find('\n').unwrap() + 1;
    // Preserve the one-argument metadata branch before observing the source argument.
    let branch_end = prior[metadata_end..].find('\n').unwrap() + metadata_end + 1;
    fs::write(
        &fixture.native,
        format!(
            "{}for source do :; done\ncat \"$source\" >&2\n{}",
            &prior[..branch_end],
            &prior[branch_end..]
        ),
    )
    .unwrap();
    let workloads = [variant(&fixture, 10), variant(&fixture, 12)];
    let report = matrix::run_workloads(&fixture.request(Suite::Queens), &workloads).unwrap();
    assert!(report.accounted());
    assert_eq!(report.samples().len(), 8);
    assert!(
        report
            .after()
            .iter()
            .all(zetesis_validation::selected::Change::unchanged)
    );
    for (index, size) in [10, 12].into_iter().enumerate() {
        let sample = report
            .samples()
            .iter()
            .find(|sample| {
                sample.slot().case == index
                    && sample.slot().phase == Phase::Qualification
                    && matches!(sample.slot().producer, Producer::Native { .. })
            })
            .unwrap();
        assert_eq!(sample.decision(), Decision::Refused);
        let capture = sample.capture().unwrap();
        let source = std::str::from_utf8(capture.stderr()).unwrap();
        assert!(source.contains(&format!("#const n = {size}.")), "{source}");
        assert!(
            capture
                .directory()
                .ends_with(format!("workload-{index:02}"))
        );
    }
    assert!(
        report
            .samples()
            .iter()
            .filter(|sample| sample.slot().producer == Producer::Reference)
            .all(|sample| sample.decision() == Decision::Pass)
    );
    report.publish().unwrap();
    let encoded: serde_json::Value =
        serde_json::from_slice(&fs::read(&fixture.report).unwrap()).unwrap();
    assert_eq!(encoded["report"]["schema"], 2);
    assert_eq!(encoded["report"]["workloads"].as_array().unwrap().len(), 2);
    assert_ne!(
        encoded["report"]["workloads"][0]["identity"],
        encoded["report"]["workloads"][1]["identity"]
    );
}

#[test]
fn generated_cells_launch_their_exact_bytes_under_their_own_contract() {
    use zetesis_validation::performance::families::Family;
    let fixture = Fixture::new();
    let prior = fs::read_to_string(&fixture.native).unwrap();
    let metadata_end = prior.find('\n').unwrap() + 1;
    let branch_end = prior[metadata_end..].find('\n').unwrap() + metadata_end + 1;
    fs::write(
        &fixture.native,
        format!(
            "{}for source do :; done\ncat \"$source\" >&2\n{}",
            &prior[..branch_end],
            &prior[branch_end..]
        ),
    )
    .unwrap();
    let chain =
        matrix::Workload::generated(Family::Chain, 3, matrix::WorkloadLimits::default()).unwrap();
    let workloads = [chain, variant(&fixture, 10)];
    let report = matrix::run_workloads(&fixture.request(Suite::Queens), &workloads).unwrap();
    assert!(report.accounted());
    assert_eq!(
        report.cases(),
        ["generated/chain-3.lp", "standalone/n-queens/variant-01.lp"]
    );
    let native = report
        .samples()
        .iter()
        .find(|sample| {
            sample.slot().case == 0
                && sample.slot().phase == Phase::Qualification
                && matches!(sample.slot().producer, Producer::Native { .. })
        })
        .unwrap();
    let source = std::str::from_utf8(native.capture().unwrap().stderr()).unwrap();
    assert_eq!(source, Family::Chain.source(3).unwrap());
    assert!(
        native
            .capture()
            .unwrap()
            .directory()
            .ends_with("workload-00")
    );
    // The fixture reference reports no answers; a generated workload carries
    // its closed-form contract, so that reference fails parity, while the
    // amended queens workload has no default contract and passes.
    let reference = |case: usize| {
        report
            .samples()
            .iter()
            .find(|sample| {
                sample.slot().case == case
                    && sample.slot().phase == Phase::Qualification
                    && sample.slot().producer == Producer::Reference
            })
            .unwrap()
            .decision()
    };
    assert_eq!(reference(0), Decision::ParityMismatch);
    assert_eq!(reference(1), Decision::Pass);
    report.publish().unwrap();
    let encoded: serde_json::Value =
        serde_json::from_slice(&fs::read(&fixture.report).unwrap()).unwrap();
    assert_eq!(
        encoded["report"]["workloads"][0]["generated"]["family"],
        "chain"
    );
    assert_eq!(encoded["report"]["workloads"][0]["generated"]["size"], 3);
    assert!(
        encoded["report"]["before"]
            .as_array()
            .unwrap()
            .iter()
            .any(|seal| seal["requested"]
                .as_str()
                .is_some_and(|path| path.ends_with("workload-00/generated/chain-3.lp")))
    );
}

#[test]
fn original_workloads_keep_the_default_contract() {
    let fixture = Fixture::new();
    let corpus = zetesis_validation::examples::load(
        &fixture.corpus,
        zetesis_validation::examples::Limits::default(),
    )
    .unwrap();
    let original = matrix::Workload::original(
        &corpus,
        "standalone/n-queens/variant-01.lp",
        matrix::WorkloadLimits::default(),
    )
    .unwrap();
    let report = matrix::run_workloads(&fixture.request(Suite::Queens), &[original]).unwrap();
    assert!(report.accounted());
    let reference = report
        .samples()
        .iter()
        .find(|sample| {
            sample.slot().phase == Phase::Qualification
                && sample.slot().producer == Producer::Reference
        })
        .unwrap();
    assert_eq!(reference.decision(), Decision::ParityMismatch);
}

#[test]
fn explicit_workloads_require_distinct_identities() {
    let fixture = Fixture::new();
    let workload = variant(&fixture, 12);
    assert!(
        matrix::run_workloads(
            &fixture.request(Suite::Queens),
            &[workload.clone(), workload]
        )
        .is_err()
    );
}

#[test]
fn explicit_workloads_require_a_nonempty_population() {
    let fixture = Fixture::new();
    assert!(matrix::run_workloads(&fixture.request(Suite::Queens), &[]).is_err());
}

#[test]
fn explicit_workloads_stay_within_the_allowed_suite() {
    let fixture = Fixture::new();
    let corpus = zetesis_validation::examples::load(
        &fixture.corpus,
        zetesis_validation::examples::Limits::default(),
    )
    .unwrap();
    let workload = matrix::Workload::original(
        &corpus,
        "standalone/send-money/send-money.lp",
        matrix::WorkloadLimits::default(),
    )
    .unwrap();
    assert!(matrix::run_workloads(&fixture.request(Suite::Queens), &[workload]).is_err());
}
#[test]
fn scheduling_deadline_keeps_unlaunched_positions() {
    let fixture = Fixture::new();
    let mut request = fixture.request(Suite::Queens);
    request.limits.campaign_timeout = Duration::ZERO;
    let report = matrix::run(&request).unwrap();
    assert!(report.accounted());
    assert_eq!(report.samples().len(), 24);
    assert!(
        report
            .samples()
            .iter()
            .all(|sample| sample.capture().is_none())
    );
    assert!(
        report
            .faults()
            .iter()
            .any(|fault| matches!(fault, performance::Fault::Deadline))
    );
}
#[test]
fn metadata_capture_ceiling_preserves_complete_schedule() {
    let fixture = Fixture::new();
    let mut request = fixture.request(Suite::Baseline);
    request.limits.max_total_capture_bytes = 1;
    let report = matrix::run(&request).unwrap();
    assert!(report.accounted());
    assert!(
        report
            .samples()
            .iter()
            .all(|sample| sample.decision() == Decision::NotAttempted)
    );
    assert!(!report.faults().is_empty());
}
#[test]
fn report_cannot_replace_a_primary_executable() {
    let fixture = Fixture::new();
    let before = fs::read(&fixture.native).unwrap();
    let mut request = fixture.request(Suite::Baseline);
    request.report = &fixture.native;
    assert!(matrix::run(&request).is_err());
    assert_eq!(fs::read(&fixture.native).unwrap(), before);
}
#[test]
fn report_hardlink_cannot_alias_a_primary_executable() {
    let fixture = Fixture::new();
    fs::hard_link(&fixture.native, &fixture.report).unwrap();
    assert!(matrix::run(&fixture.request(Suite::Baseline)).is_err());
    assert_eq!(
        fs::read(&fixture.native).unwrap(),
        fs::read(&fixture.report).unwrap()
    );
}
#[test]
fn report_publication_never_replaces_existing_evidence() {
    let fixture = Fixture::new();
    let mut request = fixture.request(Suite::Baseline);
    request.limits.campaign_timeout = Duration::ZERO;
    let report = matrix::run(&request).unwrap();
    report.publish().unwrap();
    let before = fs::read(&fixture.report).unwrap();
    assert!(report.publish().is_err());
    assert_eq!(fs::read(&fixture.report).unwrap(), before);
}
#[test]
fn serialized_byte_ceiling_prevents_partial_publication() {
    let fixture = Fixture::new();
    let mut request = fixture.request(Suite::Baseline);
    request.limits.campaign_timeout = Duration::ZERO;
    request.limits.max_report_bytes = 1;
    let report = matrix::run(&request).unwrap();
    assert!(report.publish().is_err());
    assert!(!fixture.report.exists());
}

#[test]
fn cli_profiles_preserve_their_execution_arguments() {
    let fixture = Fixture::new();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_zetesis-perf"))
        .arg(&fixture.corpus)
        .arg("--zetesis")
        .arg(&fixture.native)
        .arg("--clingo")
        .arg(&fixture.reference)
        .arg("--report")
        .arg(&fixture.report)
        .args([
            "--suite",
            "queens",
            "--profile",
            "cpu-eager",
            "--profile",
            "metal-lazy",
            "--workers",
            "2",
            "--completion-workers",
            "3",
            "--clingo-workers",
            "4",
            "--batch-size",
            "7",
            "--formula-joins",
            "table",
            "--warmups",
            "0",
            "--repetitions",
            "1",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let value: serde_json::Value =
        serde_json::from_slice(&fs::read(&fixture.report).unwrap()).unwrap();
    let report = &value["report"];
    assert_eq!(report["plan"]["profiles"].as_array().unwrap().len(), 2);
    assert_eq!(report["plan"]["profiles"][1]["backend"], "metal");
    assert_eq!(report["plan"]["profiles"][1]["grounder"], "lazy");
    assert_eq!(report["plan"]["profiles"][0]["formula_joins"], "table");
    assert_eq!(report["plan"]["profiles"][1]["formula_joins"], "table");
    assert_eq!(report["plan"]["reference_workers"], 4);
    let native: Vec<_> = report["samples"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["capture"].is_object() && s["slot"]["producer"]["solver"] == "native")
        .collect();
    assert!(!native.is_empty());
    for sample in native {
        let arguments = sample["capture"]["arguments"].as_array().unwrap();
        assert!(
            arguments
                .iter()
                .any(|a| a == &serde_json::to_value(std::ffi::OsString::from("--json")).unwrap())
        );
        assert!(
            arguments
                .iter()
                .any(|a| a == &serde_json::to_value(std::ffi::OsString::from("--stats")).unwrap())
        );
        for (flag, value) in [
            ("--workers", "2"),
            ("--completion-workers", "3"),
            ("--batch-size", "7"),
            ("--formula-joins", "table"),
        ] {
            let flag = serde_json::to_value(std::ffi::OsString::from(flag)).unwrap();
            let index = arguments.iter().position(|a| a == &flag).unwrap();
            assert_eq!(
                arguments[index + 1],
                serde_json::to_value(std::ffi::OsString::from(value)).unwrap()
            );
        }
    }
    let reference: Vec<_> = report["samples"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["capture"].is_object() && s["slot"]["producer"]["solver"] == "reference")
        .collect();
    assert!(!reference.is_empty());
    let native_flag = serde_json::to_value(std::ffi::OsString::from("--formula-joins")).unwrap();
    assert!(reference.iter().all(|sample| {
        !sample["capture"]["arguments"]
            .as_array()
            .unwrap()
            .contains(&native_flag)
    }));
}

#[test]
fn legacy_mode_refuses_silently_ignored_matrix_controls() {
    let fixture = Fixture::new();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_zetesis-perf"))
        .arg(&fixture.corpus)
        .arg("--zetesis")
        .arg(&fixture.native)
        .arg("--clingo")
        .arg(&fixture.reference)
        .arg("--report")
        .arg(&fixture.report)
        .args(["--workers", "2"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(!fixture.report.exists());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("require --profile, --suite corpus or --suite series")
    );
}

#[test]
fn corpus_cli_defaults_to_the_four_explicit_profiles() {
    let fixture = Fixture::new();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_zetesis-perf"))
        .arg(&fixture.corpus)
        .arg("--zetesis")
        .arg(&fixture.native)
        .arg("--clingo")
        .arg(&fixture.reference)
        .arg("--report")
        .arg(&fixture.report)
        .args(["--suite", "corpus", "--campaign-seconds", "0"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let value: serde_json::Value =
        serde_json::from_slice(&fs::read(&fixture.report).unwrap()).unwrap();
    let report = &value["report"];
    assert_eq!(report["plan"]["profiles"].as_array().unwrap().len(), 4);
    assert_eq!(report["plan"]["repetitions"], 20);
    assert_eq!(report["samples"].as_array().unwrap().len(), 94 * 5 * 24);
    assert_eq!(value["accounted"], true);
}

#[test]
fn matrix_cli_records_its_explicit_decoder_ceiling() {
    let fixture = Fixture::new();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_zetesis-perf"))
        .arg(&fixture.corpus)
        .arg("--zetesis")
        .arg(&fixture.native)
        .arg("--clingo")
        .arg(&fixture.reference)
        .arg("--report")
        .arg(&fixture.report)
        .args([
            "--suite",
            "corpus",
            "--campaign-seconds",
            "0",
            "--native-report-bytes",
            "33554432",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let value: serde_json::Value =
        serde_json::from_slice(&fs::read(&fixture.report).unwrap()).unwrap();
    assert_eq!(
        value["report"]["native_normalization_limits"]["input_bytes"],
        33_554_432
    );
}

#[test]
fn legacy_cli_refuses_a_matrix_decoder_ceiling() {
    let fixture = Fixture::new();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_zetesis-perf"))
        .arg(&fixture.corpus)
        .arg("--zetesis")
        .arg(&fixture.native)
        .arg("--clingo")
        .arg(&fixture.reference)
        .arg("--report")
        .arg(&fixture.report)
        .args(["--native-report-bytes", "33554432"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(!fixture.report.exists());
}

#[test]
fn matrix_startup_identifies_the_evidence_destination() {
    let fixture = Fixture::new();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_zetesis-perf"))
        .arg(&fixture.corpus)
        .arg("--zetesis")
        .arg(&fixture.native)
        .arg("--clingo")
        .arg(&fixture.reference)
        .arg("--report")
        .arg(&fixture.report)
        .args(["--suite", "corpus", "--campaign-seconds", "0"])
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        format!(
            "Recording instrumented solver matrix; evidence will be written to {}\n",
            fixture.report.display()
        )
    );
}
