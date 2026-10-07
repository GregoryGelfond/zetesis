//! Synthetic subprocesses qualify decisions and evidence, never solver parity.

use std::io::Write;
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use serde_json::{Value, json};

use crate::corpus_comparison::corpus::{Case, Contract, Loaded, Manifest};
use crate::corpus_comparison::{NativeOracle, Request as Options};
use zetesis_backend::{Backend, GpuApi};
use zetesis_test_support::io::Closed;

const NATIVE: &str = "Answer: 1\na\nSATISFIABLE\nCoverage: exhausted\nModels: 1\n";
const PHASE_TIMINGS: &str = zetesis_test_support::fixtures::PHASE_STATISTICS;
// Fixture publication/execution checks correctness under concurrent suite work,
// not latency. Deadline regressions retain their separately authored limits.
const FIXTURE_LIVENESS: Duration = Duration::from_secs(10);

mod stage_contracts;
mod cancellation;
mod exit_evidence;

fn reference() -> String {
    json!({
        "Solver": "synthetic protocol fixture",
        "Call": [{"Witnesses": [{"Value": ["a"]}]}],
        "Result": "SATISFIABLE", "Models": {"More": "no", "Number": 1}
    })
    .to_string()
}

fn script(directory: &Path, name: &str, body: &str) -> PathBuf {
    let path = directory.join(name);
    // A concurrent test's fork can briefly inherit a writable descriptor even
    // with CLOEXEC, making Linux refuse this fixture's exec with ETXTBSY. Keep
    // executable writes in a child and wait for its exit: the test process
    // never owns a writable descriptor that another test's fork can inherit.
    // Positional arguments preserve the literal payload without interpreting it
    // in the publisher; builtin printf avoids an additional pipe/cat child.
    let arguments: [std::ffi::OsString; 5] = [
        "-c".into(),
        "umask 077; printf '%s' \"$2\" > \"$1\" && /bin/chmod 700 \"$1\"".into(),
        "fixture-writer".into(),
        path.as_os_str().to_owned(),
        format!("#!/bin/sh\n{body}\n").into(),
    ];
    let output = crate::process::invoke(
        crate::process::Invocation {
            executable: Path::new("/bin/sh"),
            arguments: &arguments,
            directory,
        },
        crate::process::Limits {
            timeout: FIXTURE_LIVENESS,
            max_output_bytes: 4_096,
            cleanup_timeout: Duration::from_secs(1),
        },
    )
    .unwrap();
    let (capture, pending) = output.into_parts();
    if let Some(pending) = pending {
        panic!(
            "fixture publication left child {} unreaped",
            pending.abandon()
        );
    }
    assert_eq!(
        capture.stop(),
        crate::process::Stop::Completed,
        "{capture:?}"
    );
    assert!(capture.failure().is_none(), "{capture:?}");
    assert!(capture.cleanup_failure().is_none(), "{capture:?}");
    assert_eq!(capture.exit().unwrap().code, Some(0), "{capture:?}");
    assert!(capture.stdout().is_empty());
    assert!(capture.stderr().is_empty(), "{capture:?}");
    path
}

fn emitting(directory: &Path, name: &str, stdout: &str, stderr: &str, exit: u8) -> PathBuf {
    let quoted = |text: &str| text.replace('\'', "'\\''");
    script(
        directory,
        name,
        &format!(
            "printf '%s' '{}'\nprintf '%s' '{}' >&2\nexit {exit}",
            quoted(stdout),
            quoted(stderr),
        ),
    )
}

fn case() -> Case {
    Case {
        path: "synthetic.lp".into(),
        sha256: "synthetic fixture, not a pinned corpus identity".into(),
        includes: Vec::new(),
        contracts: vec![
            Contract {
                tag: "count".into(),
                arguments: "1".into(),
            },
            Contract {
                tag: "model".into(),
                arguments: "{a}".into(),
            },
        ],
        expected_satisfiability: "sat".into(),
        original_sha256: None,
        example_contract: None,
    }
}

fn loaded(directory: &Path, count: usize) -> Loaded {
    Loaded {
        root: directory.to_owned(),
        manifest: Manifest {
            revision: "synthetic runner input".into(),
            reference_toolchain: json!({"synthetic": true}),
            open_encodings: Vec::new(),
            cases: (0..count).map(|_| case()).collect(),
        },
        manifest_sha256: "synthetic runner fixture".into(),
        view: crate::corpus_comparison::corpus::SourceView::Original,
    }
}

// Bound unrelated synthetic process campaigns before their invocation deadlines
// begin. The explicit publication regression still runs four workers inside one
// permit; production capture/drain concurrency is unchanged.
fn fixture_campaign() -> MutexGuard<'static, ()> {
    static CAMPAIGN: Mutex<()> = Mutex::new(());
    // A failed test must still fail, but cannot poison later scheduling: there
    // is no shared fixture data or application state behind this mutex.
    CAMPAIGN
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

struct FixtureOptions {
    options: Options,
    _campaign: MutexGuard<'static, ()>,
}

impl Deref for FixtureOptions {
    type Target = Options;

    fn deref(&self) -> &Self::Target {
        &self.options
    }
}

impl DerefMut for FixtureOptions {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.options
    }
}

fn options(directory: &Path) -> FixtureOptions {
    let campaign = fixture_campaign();
    let options = Options {
        clingo: emitting(directory, "reference", &reference(), "", 30),
        zetesis: emitting(directory, "native", NATIVE, "", 0),
        timeout_ms: 2_000,
        max_output_bytes: 4_096,
        ..Options::default()
    };
    FixtureOptions {
        options,
        _campaign: campaign,
    }
}

fn check(options: &Options, loaded: &Loaded, expected: &str) -> Value {
    let mut pending = Vec::new();
    let result = super::check_case(
        options,
        loaded,
        &loaded.manifest.cases[0],
        super::NativeInvocation::Legacy,
        &mut pending,
        &std::sync::atomic::AtomicBool::new(false),
    );
    let result = result.to_json().unwrap();
    assert!(pending.is_empty());
    assert_eq!(result["status"], expected, "{result:#}");
    assert_eq!(result["path"], "synthetic.lp");
    if expected != "pass" && expected != "reference_pass" {
        assert!(
            result["detail"]
                .as_str()
                .is_some_and(|detail| !detail.is_empty())
        );
    }
    result
}

#[test]
fn concurrent_fixture_publication_preserves_exact_process_results() {
    let _campaign = fixture_campaign();
    let directory = tempfile::Builder::new()
        .prefix("runner fixture ' ")
        .tempdir()
        .unwrap();
    let directories: Vec<_> = (0..4)
        .map(|worker| {
            let path = directory.path().join(format!("worker {worker}"));
            std::fs::create_dir(&path).unwrap();
            path
        })
        .collect();
    let start = std::sync::Barrier::new(directories.len());
    std::thread::scope(|scope| {
        for (worker, path) in directories.iter().enumerate() {
            let start = &start;
            scope.spawn(move || {
                start.wait();
                for round in 0..4 {
                    let stdout = format!(
                        "worker={worker}; round={round}; single ' double \" dollar $HOME backtick `literal`\n"
                    );
                    let stderr = format!("stderr '{worker}/{round}'\n");
                    let executable = emitting(path, "solver ' name", &stdout, &stderr, 17);
                    let captured = crate::corpus_comparison::capture::invoke(
                        &executable,
                        &[],
                        path,
                        FIXTURE_LIVENESS,
                        4_096,
                    )
                    .unwrap();
                    assert_eq!(captured.status, crate::corpus_comparison::decision::CaptureStatus::Completed, "{captured:?}");
                    assert_eq!(captured.exit.0.unwrap().code, Some(17));
                    assert_eq!(captured.stdout, stdout);
                    assert_eq!(captured.stderr, stderr);
                }
            });
        }
    });
}

#[test]
fn reference_failures_stop_before_native_execution() {
    let directory = tempfile::tempdir().unwrap();
    let loaded = loaded(directory.path(), 1);
    let mut options = options(directory.path());
    options.zetesis = directory.path().join("must-not-run");
    options.clingo = directory.path().join("missing-reference");
    let result = check(&options, &loaded, "reference_invocation_error");
    assert!(result.get("reference_process").is_none());
    for (stdout, stderr, exit, expected) in [
        (reference(), "", 1, "reference_error"),
        ("not JSON".into(), "", 30, "reference_output_error"),
        (
            reference().replace("[\"a\"]", "[\"b\"]"),
            "",
            30,
            "reference_contract_mismatch",
        ),
    ] {
        options.clingo = emitting(directory.path(), "reference", &stdout, stderr, exit);
        let result = check(&options, &loaded, expected);
        assert!(result.get("native_process").is_none());
        assert!(result.get("native_arguments").is_none());
    }
    options.clingo = script(directory.path(), "reference", "exec sleep 1");
    options.timeout_ms = 100;
    check(&options, &loaded, "reference_timeout");
    options.timeout_ms = 2_000;
    options.max_output_bytes = 64;
    options.clingo = script(directory.path(), "reference", "printf '%010000d' 0");
    let result = check(&options, &loaded, "reference_output_limit");
    assert!(
        result["reference_process"]["stdout"]
            .as_str()
            .unwrap()
            .len()
            <= 64
    );
}

#[test]
fn native_failures_preserve_the_completed_reference_and_exact_request() {
    let directory = tempfile::tempdir().unwrap();
    let loaded = loaded(directory.path(), 1);
    let mut options = options(directory.path());
    options.native_oracle = NativeOracle::Countermodel;
    options.zetesis = directory.path().join("missing-native");
    check(&options, &loaded, "native_invocation_error");
    for (stdout, stderr, exit, expected) in [
        ("", "budget", 3, "native_incomplete"),
        ("INCOMPLETE: work\n", "", 0, "native_incomplete"),
        (
            "",
            "source admission: unsupported",
            2,
            "native_source_refused",
        ),
        ("", "S0: unsupported", 2, "native_source_refused"),
        ("", "source expansion: exceeded", 2, "native_source_refused"),
        ("", "unexpected failure", 2, "native_error"),
        ("", "unexpected failure", 1, "native_error"),
        (
            "UNSATISFIABLE\nCoverage: partial\nModels: 0\n",
            "",
            0,
            "native_incomplete",
        ),
        (
            "Answer: 1\nb\nSATISFIABLE\nCoverage: exhausted\nModels: 1\n",
            "",
            0,
            "mismatch",
        ),
        (NATIVE, "", 0, "pass"),
    ] {
        options.zetesis = emitting(directory.path(), "native", stdout, stderr, exit);
        let result = check(&options, &loaded, expected);
        assert_eq!(result["reference_answer"]["model_count"], 1);
        let argument: std::ffi::OsString =
            serde_json::from_value(result["native_arguments"][3].clone()).unwrap();
        assert_eq!(argument, "countermodel");
        assert_eq!(result["native_process"]["stdout"], stdout);
    }
    options.zetesis = script(directory.path(), "native", "exec sleep 1");
    options.timeout_ms = 100;
    check(&options, &loaded, "native_timeout");
    options.timeout_ms = 2_000;
    options.max_output_bytes = 512;
    options.zetesis = script(directory.path(), "native", "printf '%010000d' 0");
    check(&options, &loaded, "native_output_limit");
}

#[test]
fn aggregate_reports_distinguish_reference_only_from_the_full_native_gate() {
    let directory = tempfile::tempdir().unwrap();
    let mut options = options(directory.path());
    let incomplete_target = loaded(directory.path(), 1);
    let (report, passed) = run(&options, &incomplete_target);
    assert!(!passed);
    assert_eq!(report["status_counts"]["pass"], 1);
    assert_eq!(report["full_native_target_passed"], false);

    // The runner's population gate is exercised with synthetic protocols.
    // The independent corpus loader still enforces the actual manifest hashes.
    let complete_target = loaded(directory.path(), 94);
    options.native_oracle = NativeOracle::Closure;
    // An invalid optional measurement does not invalidate otherwise exact answers.
    options.zetesis = emitting(
        directory.path(),
        "native",
        NATIVE,
        "Stage timings: truncated\nPhase timings: truncated\n",
        0,
    );
    let (report, passed) = run(&options, &complete_target);
    assert!(passed);
    assert_eq!(report["native_oracle"], "closure");
    assert_eq!(report["status_counts"]["pass"], 94);
    assert_eq!(report["case_count"], 94);
    assert_eq!(report["full_native_target_passed"], true);
    assert_eq!(report["full_native_answer_parity_passed"], true);
    assert_eq!(report["native_backend"], "cpu");
    assert_eq!(report["native_threads"], 1);
    assert_eq!(report["effective_native_stats"], false);
    assert_eq!(
        report["phase_timing_cases"],
        json!({"available": 0, "complete": 0, "malformed": 94})
    );
    assert_eq!(
        report["stage_timing_cases"],
        json!({"available": 0, "complete": 0, "malformed": 94})
    );

    options.reference_only = true;
    options.zetesis = directory.path().join("must-not-run");
    let (report, passed) = run(&options, &complete_target);
    assert!(passed);
    assert_eq!(report["mode"], "reference_only");
    assert_eq!(report["status_counts"]["reference_pass"], 94);
    assert_eq!(report["requested_mode_passed"], true);
    assert_eq!(report["full_native_target_passed"], false);
    assert_eq!(
        report["phase_timing_cases"],
        json!({"available": 0, "complete": 0, "malformed": 0})
    );
    assert_eq!(
        report["stage_timing_cases"],
        json!({"available": 0, "complete": 0, "malformed": 0})
    );
    assert!(
        report["cases"]
            .as_array()
            .unwrap()
            .iter()
            .all(|case| case.get("native_process").is_none())
    );

    let (report, passed) = run(&options, &loaded(directory.path(), 0));
    assert!(!passed);
    assert_eq!(report["case_count"], 0);
    assert_eq!(report["status_counts"], json!({}));
}

#[test]
fn phase_measurements_preserve_failed_attempts_without_changing_semantic_decisions() {
    let directory = tempfile::tempdir().unwrap();
    let loaded = loaded(directory.path(), 1);
    let mut options = options(directory.path());
    options.native_stats = true;
    for (stdout, timing, exit, expected, available, complete, malformed) in [
        (NATIVE, PHASE_TIMINGS.to_owned(), 0, "pass", 1, 1, 0),
        (
            NATIVE,
            "Phase timings: truncated\n".to_owned(),
            0,
            "pass",
            0,
            0,
            1,
        ),
        (NATIVE, String::new(), 0, "pass", 0, 0, 0),
        (
            "",
            PHASE_TIMINGS.replace("complete=true", "complete=false"),
            2,
            "native_error",
            1,
            0,
            0,
        ),
        (
            "INCOMPLETE: work\n",
            PHASE_TIMINGS.to_owned(),
            3,
            "native_incomplete",
            1,
            1,
            0,
        ),
    ] {
        options.zetesis = emitting(directory.path(), "native", stdout, &timing, exit);
        let (report, passed) = run(&options, &loaded);
        assert!(!passed, "one synthetic case is never the full corpus");
        let result = &report["cases"][0];
        assert_eq!(result["status"], expected);
        assert_eq!(
            result["native_answer_parity_passed"] == true,
            expected == "pass"
        );
        assert_eq!(result["native_process"]["stderr"], timing);
        assert_eq!(
            report["phase_timing_cases"],
            json!({"available": available, "complete": complete, "malformed": malformed})
        );
        if available == 1 {
            assert_eq!(result["native_phase_timings"]["driver_elapsed_ns"], 1000);
            assert_eq!(
                result["native_phase_timings"]["phases"]["admission_materialization"]["calls"],
                1
            );
            assert_eq!(
                result["native_phase_timings"]["phases"]["admission_materialization"]["elapsed_ns"],
                40
            );
            assert!(result["native_phase_timings"]["phases"]["gpu_host_oracle"].is_null());
        }
    }
}

#[test]
fn native_public_controls_reach_the_solver() {
    let directory = tempfile::tempdir().unwrap();
    let loaded = loaded(directory.path(), 1);
    let mut options = options(directory.path());
    for (backend, label) in [
        (Backend::Cpu, "cpu"),
        (Backend::Gpu(None), "gpu"),
        (Backend::Gpu(Some(GpuApi::Metal)), "metal"),
        (Backend::Gpu(Some(GpuApi::Vulkan)), "vulkan"),
    ] {
        options.native_backend = backend;
        options.native_threads = 7.try_into().unwrap();
        options.native_memory_bytes = Some(4096);
        options.native_stats = true;
        let result = check(&options, &loaded, "pass");
        let arguments: Vec<std::ffi::OsString> =
            serde_json::from_value(result["native_arguments"].clone()).unwrap();
        assert_eq!(
            arguments,
            [
                "--backend".into(),
                label.into(),
                "--oracle".into(),
                "auto".into(),
                "--models".into(),
                "0".into(),
                "--workers".into(),
                "7".into(),
                "--memory".into(),
                "4096".into(),
                "--stats".into(),
                loaded.root.join("synthetic.lp").into_os_string(),
            ]
        );
        assert_eq!(result["native_answer_parity_passed"], true);
        assert!(
            result.get("native_formula_execution").is_none(),
            "an automatic oracle request alone is not a physical formula qualification"
        );
    }
}

#[test]
fn physical_formula_parity_requires_real_route_telemetry_even_when_answers_match() {
    let directory = tempfile::tempdir().unwrap();
    let loaded = loaded(directory.path(), 1);
    let mut options = options(directory.path());
    options.native_backend = Backend::Gpu(Some(GpuApi::Metal));
    options.native_oracle = NativeOracle::Countermodel;
    let rejected = check(&options, &loaded, "native_execution_unqualified");
    assert_eq!(rejected["native_answer_parity_passed"], true);
    let arguments: Vec<std::ffi::OsString> =
        serde_json::from_value(rejected["native_arguments"].clone()).unwrap();
    assert!(
        arguments.contains(&"--stats".into()),
        "physical formula qualification requests its evidence automatically"
    );
    options.zetesis = emitting(
        directory.path(),
        "native",
        NATIVE,
        include_str!("../../../tests/fixtures/formula_statistics.txt"),
        0,
    );
    let accepted = check(&options, &loaded, "pass");
    assert_eq!(
        accepted["native_formula_execution"]["status"],
        "gpu_exercised"
    );
    assert_eq!(accepted["native_formula_execution"]["gpu_candidates"], 1);
    let wrong_api = include_str!("../../../tests/fixtures/formula_statistics.txt")
        .replace("Metal; vendor", "Vulkan; vendor");
    options.zetesis = emitting(directory.path(), "native", NATIVE, &wrong_api, 0);
    check(&options, &loaded, "native_execution_unqualified");
}

#[test]
fn full_campaign_separates_answer_parity_device_route_and_exercised_membership() {
    let directory = tempfile::tempdir().unwrap();
    let mut options = options(directory.path());
    options.native_backend = Backend::Gpu(Some(GpuApi::Metal));
    options.native_oracle = NativeOracle::Countermodel;
    options.zetesis = emitting(
        directory.path(),
        "native",
        NATIVE,
        include_str!("../../../tests/fixtures/formula_statistics.txt"),
        0,
    );
    let (report, passed) = run(&options, &loaded(directory.path(), 94));
    assert!(passed);
    assert_eq!(report["full_native_answer_parity_passed"], true);
    assert_eq!(report["full_physical_formula_route_passed"], true);
    assert_eq!(report["physical_formula_status"], "qualified");
    assert_eq!(report["formula_execution_cases"]["gpu_exercised"], 94);
    assert_eq!(
        report["formula_execution_cases"]["outer_unsat_without_membership"],
        0
    );
    assert_eq!(report["effective_native_stats"], true);
    assert_eq!(report["native_stats"], false);

    let mut target = loaded(directory.path(), 94);
    for case in &mut target.manifest.cases {
        case.expected_satisfiability = "unsat".into();
        case.contracts = vec![Contract {
            tag: "count".into(),
            arguments: "0".into(),
        }];
    }
    let reference = json!({"Solver":"synthetic protocol fixture", "Call":[{}], "Result":"UNSATISFIABLE", "Models":{"More":"no", "Number":0}}).to_string();
    options.clingo = emitting(directory.path(), "reference", &reference, "", 20);
    let stats = include_str!("../../../tests/fixtures/formula_statistics.txt")
        .replace("batches=1; candidates=1; propagation work=12; completed sweeps=1; GPU-decided committed=1", "batches=0; candidates=0; propagation work=0; completed sweeps=0; GPU-decided committed=0")
        .replace("peak authored GPU bytes=128", "peak authored GPU bytes=0")
        .replace("candidates=1; queries=0", "candidates=0; queries=0")
        .replace("verified stable models=1", "verified stable models=0")
        .replace("classical queries=2", "classical queries=1");
    options.zetesis = emitting(
        directory.path(),
        "native",
        "UNSATISFIABLE\nCoverage: exhausted\nModels: 0\n",
        &stats,
        0,
    );
    let (report, passed) = run(&options, &target);
    assert!(
        !passed,
        "a campaign with no membership proposals never exercised a GPU oracle"
    );
    assert_eq!(report["full_native_answer_parity_passed"], true);
    assert_eq!(report["full_physical_formula_route_passed"], false);
    assert_eq!(report["physical_formula_status"], "not_exercised");
    assert_eq!(report["status_counts"]["pass"], 94);
    assert_eq!(report["formula_execution_cases"]["gpu_exercised"], 0);
    assert_eq!(
        report["formula_execution_cases"]["outer_unsat_without_membership"],
        94
    );
    assert_eq!(report["requested_mode_passed"], false);
}

#[test]
fn malformed_objective_reference_cannot_become_a_native_pass() {
    let directory = tempfile::tempdir().unwrap();
    let loaded = loaded(directory.path(), 1);
    let mut options = options(directory.path());
    options.zetesis = directory.path().join("must-not-run");
    let objective = json!({
        "Call": [{"Witnesses": [{"Value": ["a"], "Costs": [1]}, {"Value": ["a"], "Costs": [1]}]}],
        "Result": "OPTIMUM FOUND",
        "Models": {"More": "no", "Number": 2, "Optimum": "yes", "Optimal": 1, "Costs": [1]}
    });
    let mut corruptions = Vec::new();
    for (pointer, value) in [
        ("/Result", json!("UNKNOWN")),
        ("/Models/More", json!("yes")),
        ("/Models/Optimum", json!("no")),
        ("/Models/Costs", json!([1.5])),
        ("/Models/Number", json!(4)),
        ("/Models/Optimal", json!(2)),
        ("/Call", json!([])),
        ("/Call/0/Witnesses/0/Costs", json!([0])),
        ("/Call/0/Witnesses/0/Value", json!([17])),
        ("/Call/0/Witnesses/1/Costs", json!([])),
    ] {
        let mut malformed = objective.clone();
        *malformed.pointer_mut(pointer).unwrap() = value;
        corruptions.push(malformed);
    }
    for malformed in corruptions {
        options.clingo = emitting(
            directory.path(),
            "reference",
            &malformed.to_string(),
            "",
            30,
        );
        let result = check(&options, &loaded, "reference_output_error");
        assert!(result.get("native_arguments").is_none());
    }
}

#[test]
fn original_contracts_are_checked_independently_of_solver_agreement() {
    let answer = super::normalize::native(
        "Answer: 1\na p(\"x,y\")\nOptimization: 1 -2\nAnswer: 2\na q\nOptimization: 1 -2\nOPTIMUM FOUND\nCoverage: exhausted\nModels: 2\n",
        true,
    ).unwrap();
    let mut case = case();
    case.contracts = [
        ("expect", "sat"),
        ("note", "attributed fixture"),
        ("cost", "{1 -2}"),
        ("count", "optimal 2"),
        ("model", "{a, p(\"x,y\")}"),
        ("optimal", "{a,q}"),
        ("cautious", "optimal {a}"),
    ]
    .into_iter()
    .map(|(tag, arguments)| Contract {
        tag: tag.into(),
        arguments: arguments.into(),
    })
    .collect();
    super::normalize::contracts(&case, &answer).unwrap();
    for (tag, arguments, expected) in [
        ("cost", "{1 2}", "@cost mismatch"),
        ("cost", "1 -2", "braced"),
        ("cost", "{not-a-number}", "cost integer"),
        ("count", "optimal 1", "@count mismatch"),
        ("count", "two", "invalid digit"),
        ("model", "{a}", "@model witness mismatch"),
        ("optimal", "{q}", "@optimal witness mismatch"),
        (
            "cautious",
            "optimal {p(\"x,y\")}",
            "@cautious optimal mismatch",
        ),
        ("cautious", "all {a}", "unsupported cautious scope"),
        ("unrecognized", "anything", "unimplemented contract tag"),
    ] {
        case.contracts = vec![Contract {
            tag: tag.into(),
            arguments: arguments.into(),
        }];
        let error = super::normalize::contracts(&case, &answer).unwrap_err();
        assert!(error.contains(expected), "{tag} {arguments}: {error}");
    }
    case.expected_satisfiability = "unsat".into();
    assert_eq!(
        super::normalize::contracts(&case, &answer).unwrap_err(),
        "@expect mismatch"
    );
    let empty =
        super::normalize::native("UNSATISFIABLE\nCoverage: exhausted\nModels: 0\n", false).unwrap();
    case.contracts = vec![Contract {
        tag: "cautious".into(),
        arguments: "optimal {}".into(),
    }];
    assert_eq!(
        super::normalize::contracts(&case, &empty).unwrap_err(),
        "@cautious optimal mismatch"
    );
}

#[test]
fn malformed_native_records_cannot_supply_complete_costed_models() {
    use crate::answers::{Error, Issue};
    for (text, optimized, expected) in [
        (
            "Answer: 1\na\nOptimization: 1\nOptimization: 1\nOPTIMUM FOUND\nCoverage: exhausted\nModels: 1\n",
            true,
            Issue::Contradiction,
        ),
        (
            "Optimization: 1\nAnswer: 1\na\nOPTIMUM FOUND\nCoverage: exhausted\nModels: 1\n",
            true,
            Issue::MissingField,
        ),
        (
            "Answer: 1\na\nOptimization: bad\nOPTIMUM FOUND\nCoverage: exhausted\nModels: 1\n",
            true,
            Issue::MalformedField,
        ),
        (
            "UNSATISFIABLE\nCoverage: exhausted\nModels: 1\nAnswer: 1\na\n",
            false,
            Issue::Contradiction,
        ),
        (
            "SATISFIABLE\nCoverage: exhausted\nModels: 0\n",
            false,
            Issue::Contradiction,
        ),
        (
            "UNSATISFIABLE\nCoverage: exhausted\n",
            false,
            Issue::Contradiction,
        ),
        (
            "UNSATISFIABLE\nCoverage: exhausted\nModels: 0\nModels: 0\n",
            false,
            Issue::Contradiction,
        ),
        (
            "UNSATISFIABLE\nCoverage: exhausted\nModels: zero\n",
            false,
            Issue::MalformedField,
        ),
        (
            "SATISFIABLE\nCoverage: exhausted\nModels: 1\nAnswer: 1",
            false,
            Issue::MissingField,
        ),
        (
            "Answer: 1\np)\nSATISFIABLE\nCoverage: exhausted\nModels: 1\n",
            false,
            Issue::MalformedField,
        ),
    ] {
        let error = super::normalize::native(text, optimized).unwrap_err();
        assert!(
            matches!(error, Error::Invalid { issue, .. } if issue == expected),
            "{text}: {error}"
        );
    }
    let reference = r#"{"Call":[{}, {"Witnesses":[]}],"Result":"UNSATISFIABLE","Models":{"More":"no","Number":0}}"#;
    assert!(!super::normalize::reference(reference).unwrap().satisfiable);
    let unexpected_cost = reference.replace("\"Number\":0", "\"Number\":0,\"Costs\":[0]");
    assert!(
        super::normalize::reference(&unexpected_cost)
            .unwrap_err()
            .contains("nonoptimized")
    );
}

#[test]
fn public_resource_requests_retain_completion_evidence() {
    let directory = tempfile::tempdir().unwrap();
    let loaded = loaded(directory.path(), 1);
    let mut options = options(directory.path());
    options.native_backend = Backend::Gpu(Some(GpuApi::Metal));
    options.native_oracle = NativeOracle::Countermodel;
    options.native_memory_bytes = Some(268_435_456);
    for threads in [1, 2, 4] {
        options.native_threads = threads.try_into().unwrap();
        let stats = include_str!("../../../tests/fixtures/formula_statistics_completion.txt")
            .replace(
                "CPU completion requested workers=4",
                &format!("CPU completion requested workers={threads}"),
            );
        options.zetesis = emitting(directory.path(), "native", NATIVE, &stats, 0);
        let result = check(&options, &loaded, "pass");
        let arguments: Vec<std::ffi::OsString> =
            serde_json::from_value(result["native_arguments"].clone()).unwrap();
        for (flag, value) in [
            ("--workers", threads.to_string()),
            ("--memory", "268435456".into()),
        ] {
            let index = arguments.iter().position(|a| a == flag).unwrap();
            assert_eq!(arguments[index + 1], value.as_str());
        }
        let evidence = &result["native_formula_execution"]["completion"];
        assert_eq!(evidence["requested_workers"], threads);
        assert_eq!(evidence["max_logical_scratch_bytes"], 268_435_456);
    }
    options.native_memory_bytes = Some(268_435_455);
    let result = check(&options, &loaded, "native_execution_unqualified");
    assert_eq!(result["native_answer_parity_passed"], true);
}

#[test]
fn ordinary_memory_refusal_does_not_establish_completion() {
    let directory = tempfile::tempdir().unwrap();
    let loaded = loaded(directory.path(), 1);
    let mut options = options(directory.path());
    options.native_memory_bytes = Some(0);
    options.native_stats = true;
    options.zetesis = emitting(
        directory.path(),
        "native",
        "INCOMPLETE\n",
        "named memory allowance reached",
        3,
    );
    let result = check(&options, &loaded, "native_incomplete");
    let arguments: Vec<std::ffi::OsString> =
        serde_json::from_value(result["native_arguments"].clone()).unwrap();
    let index = arguments.iter().position(|a| a == "--memory").unwrap();
    assert_eq!(arguments[index + 1], "0");
    let (report, passed) = run(&options, &loaded);
    assert!(!passed);
    assert_eq!(report["native_resource_policy"], "ordinary");
    assert_eq!(report["native_memory_bytes"], 0);
    assert_eq!(report["status_counts"]["native_incomplete"], 1);
}

fn typed_loaded(directory: &Path) -> Loaded {
    let mut loaded = loaded(directory, 1);
    loaded.view = crate::corpus_comparison::corpus::SourceView::AnnotationCleaned;
    let case = &mut loaded.manifest.cases[0];
    case.original_sha256 = Some("synthetic original identity; not a pinned corpus hash".into());
    case.contracts.clear();
    case.example_contract = Some(
        serde_json::from_value(json!({
            "satisfiability":"sat", "family":"all", "model_count":1, "cost":null,
            "witnesses":[["a"]], "required_symbols":[], "notes":["synthetic protocol fixture"],
        }))
        .unwrap(),
    );
    loaded
}

#[test]
fn typed_reference_contract_requires_successful_exit() {
    let directory = tempfile::tempdir().unwrap();
    let loaded = typed_loaded(directory.path());
    let mut options = options(directory.path());
    options.clingo = emitting(directory.path(), "reference", &reference(), "", 1);
    options.zetesis = directory.path().join("must-not-run");
    let result = check(&options, &loaded, "reference_error");
    assert!(result.get("reference_answer").is_none());
    assert!(result.get("native_process").is_none());
}

#[test]
fn typed_native_contract_requires_successful_exit() {
    let directory = tempfile::tempdir().unwrap();
    let loaded = typed_loaded(directory.path());
    let mut options = options(directory.path());
    options.zetesis = emitting(directory.path(), "native", NATIVE, "", 1);
    let result = check(&options, &loaded, "native_error");
    assert!(result.get("native_answer").is_none());
    assert!(result.get("native_answer_parity_passed").is_none());
}

fn complete_prefix(stdout: &str) -> String {
    // Trailing whitespace keeps the report valid while making the shared byte
    // allowance large enough for the ordinary reference invocation to finish.
    format!("{stdout}\n{}", " ".repeat(reference().len()))
}

#[test]
fn typed_reference_contract_requires_complete_capture() {
    let directory = tempfile::tempdir().unwrap();
    let loaded = typed_loaded(directory.path());
    let mut options = options(directory.path());
    let prefix = complete_prefix(&reference());
    super::normalize::reference(&prefix).unwrap();
    options.max_output_bytes = prefix.len();
    options.clingo = emitting(directory.path(), "reference", &format!("{prefix}x"), "", 30);
    options.zetesis = directory.path().join("must-not-run");
    let result = check(&options, &loaded, "reference_output_limit");
    assert_eq!(result["reference_process"]["stdout"], prefix);
    assert!(result.get("reference_answer").is_none());
    assert!(result.get("native_process").is_none());
}

#[test]
fn typed_native_contract_requires_complete_capture() {
    let directory = tempfile::tempdir().unwrap();
    let loaded = typed_loaded(directory.path());
    let mut options = options(directory.path());
    let prefix = complete_prefix(NATIVE);
    super::normalize::native(&prefix, false).unwrap();
    options.max_output_bytes = prefix.len();
    options.zetesis = emitting(directory.path(), "native", &format!("{prefix}x"), "", 0);
    let result = check(&options, &loaded, "native_output_limit");
    assert_eq!(result["native_process"]["stdout"], prefix);
    assert!(result.get("native_answer").is_none());
    assert!(result.get("native_answer_parity_passed").is_none());
}

#[test]
fn typed_contract_failure_prevents_native_invocation() {
    let directory = tempfile::tempdir().unwrap();
    let mut loaded = typed_loaded(directory.path());
    let case = &mut loaded.manifest.cases[0];
    let mut contract = serde_json::to_value(case.example_contract.as_ref().unwrap()).unwrap();
    contract["witnesses"] = json!([["b"]]);
    case.example_contract = Some(serde_json::from_value(contract).unwrap());
    let mut options = options(directory.path());
    options.zetesis = directory.path().join("must-not-run");
    let result = check(&options, &loaded, "reference_contract_mismatch");
    assert!(result["detail"].as_str().unwrap().contains("Witness"));
    assert!(result.get("native_process").is_none());
}

#[test]
fn typed_adaptation_preserves_equal_display_counts() {
    let directory = tempfile::tempdir().unwrap();
    let mut loaded = typed_loaded(directory.path());
    let case = &mut loaded.manifest.cases[0];
    let mut contract = serde_json::to_value(case.example_contract.as_ref().unwrap()).unwrap();
    contract["model_count"] = json!(2);
    case.example_contract = Some(serde_json::from_value(contract).unwrap());
    let answer = super::normalize::native(
        "Answer: 1\na\nAnswer: 2\na\nSATISFIABLE\nCoverage: exhausted\nModels: 2\n",
        false,
    )
    .unwrap();
    assert_eq!(answer.models.len(), 1);
    super::normalize::contracts(case, &answer).unwrap();
    let fewer = super::normalize::native(NATIVE, false).unwrap();
    assert!(
        super::normalize::contracts(case, &fewer)
            .unwrap_err()
            .contains("Count")
    );
}

#[test]
fn typed_adaptation_preserves_repeated_printed_symbols() {
    let directory = tempfile::tempdir().unwrap();
    let mut loaded = typed_loaded(directory.path());
    let case = &mut loaded.manifest.cases[0];
    let mut contract = serde_json::to_value(case.example_contract.as_ref().unwrap()).unwrap();
    contract["witnesses"] = json!([["a", "a"]]);
    case.example_contract = Some(serde_json::from_value(contract).unwrap());
    let answer = super::normalize::native(
        "Answer: 1\na a\nSATISFIABLE\nCoverage: exhausted\nModels: 1\n",
        false,
    )
    .unwrap();
    super::normalize::contracts(case, &answer).unwrap();
    let fewer = super::normalize::native(NATIVE, false).unwrap();
    assert!(
        super::normalize::contracts(case, &fewer)
            .unwrap_err()
            .contains("Witness")
    );
}

#[test]
fn clean_source_metadata_survives_failed_capture() {
    let directory = tempfile::tempdir().unwrap();
    let loaded = typed_loaded(directory.path());
    let mut options = options(directory.path());
    options.clingo = directory.path().join("missing-reference");
    let result = check(&options, &loaded, "reference_invocation_error");
    let case = &loaded.manifest.cases[0];
    assert_eq!(result["sha256"], case.sha256);
    assert_eq!(result["original_sha256"], json!(case.original_sha256));
    assert_eq!(result["example_contract"], json!(case.example_contract));
}

fn run(request: &Options, loaded: &Loaded) -> (Value, bool) {
    let report = super::run(request, loaded.clone(), |_| {});
    let passed = report.passed();
    let value = report.to_json().unwrap();
    assert_eq!(value["requested_mode_passed"], passed);
    assert_eq!(
        value["full_native_answer_parity_passed"],
        report.answer_parity_passed()
    );
    for (case, view) in report
        .cases()
        .iter()
        .zip(value["cases"].as_array().unwrap())
    {
        assert_eq!(view["status"], case.decision().label());
        assert_eq!(view["path"], case.path());
    }
    (value, passed)
}

#[test]
fn progress_write_failure_does_not_change_acceptance() {
    let directory = tempfile::tempdir().unwrap();
    let options = options(directory.path());
    let loaded = loaded(directory.path(), 94);
    let mut failures = Vec::new();
    let report = super::run(&options, loaded, |case| {
        if let Err(error) = Closed.write_all(case.path().as_bytes()) {
            failures.push(error.kind());
        }
    });
    if failures != vec![std::io::ErrorKind::BrokenPipe; 94] || !report.passed() {
        // Preserve the actual failing producer, raw streams, elapsed time and
        // cleanup evidence. A bare acceptance assertion loses this distinction
        // and TempDir would otherwise delete its fixture during unwinding.
        let retained = directory.keep();
        let path = retained.join("failed-report.json");
        let saved = report
            .to_json()
            .and_then(|view| serde_json::to_vec_pretty(&view))
            .map_err(|error| error.to_string())
            .and_then(|bytes| std::fs::write(&path, bytes).map_err(|error| error.to_string()));
        panic!(
            "progress fixture retained at {}; report retention: {saved:?}; progress errors: {failures:?}; report: {report:#?}",
            retained.display()
        );
    }
    assert_eq!(failures, vec![std::io::ErrorKind::BrokenPipe; 94]);
    assert!(report.passed());
}

#[test]
fn unrepresentable_report_path_preserves_acceptance() {
    use std::os::unix::ffi::OsStrExt;
    let directory = tempfile::tempdir().unwrap();
    let options = options(directory.path());
    let mut report = super::run(&options, loaded(directory.path(), 94), |_| {});
    // Exercise retained Unix path bytes without requiring a filesystem that
    // permits non-UTF-8 names. Only the serialization input is varied here.
    report
        .corpus
        .root
        .push(std::ffi::OsStr::from_bytes(b"source-\xff"));
    assert!(report.to_json().is_err());
    assert!(report.passed());
}

#[test]
fn displayed_status_text_is_not_finish_evidence() {
    let directory = tempfile::tempdir().unwrap();
    let mut loaded = loaded(directory.path(), 1);
    let mut options = options(directory.path());
    for symbol in [
        "p(\"INCOMPLETE: embedded\")",
        "p(\"first\nINCOMPLETE: embedded\nlast\")",
    ] {
        loaded.manifest.cases[0].contracts[1].arguments = format!("{{{symbol}}}");
        let mut reference: Value = serde_json::from_str(&reference()).unwrap();
        reference["Call"][0]["Witnesses"][0]["Value"] = json!([symbol]);
        options.clingo = emitting(
            directory.path(),
            "reference",
            &reference.to_string(),
            "",
            30,
        );
        let native = format!("Answer: 1\n{symbol}\nSATISFIABLE\nCoverage: exhausted\nModels: 1\n");
        options.zetesis = emitting(directory.path(), "native", &native, "", 0);
        check(&options, &loaded, "pass");
    }
}

#[test]
fn incomplete_metadata_remains_a_typed_failure() {
    let directory = tempfile::tempdir().unwrap();
    let loaded = loaded(directory.path(), 1);
    let mut options = options(directory.path());
    for marker in ["INCOMPLETE", "INCOMPLETE: work"] {
        let native = format!("{NATIVE}{marker}\n");
        options.zetesis = emitting(directory.path(), "native", &native, "", 0);
        let mut pending = Vec::new();
        let result = super::check_case(
            &options,
            &loaded,
            &loaded.manifest.cases[0],
            super::NativeInvocation::Legacy,
            &mut pending,
            &std::sync::atomic::AtomicBool::new(false),
        );
        assert!(matches!(
            result.decision(),
            super::Decision::NativeIncomplete
        ));
        assert!(result.native_answers().is_none());
        assert!(pending.is_empty());
    }
}

fn structured_native() -> Value {
    json!({"schema":2,"format":"zetesis","models":[
        {"number":1,"model":{"atoms":[{"predicate":"a","sign":"positive","arguments":[]}],"full_model":[0],"shown":{"atom_indices":[0],"terms":[]},"costs":null}}
    ],"outcome":{"status":"satisfiable","completion":"exhausted","coverage":"exhausted","published_models":1,"verified_models":1,"checked":1,"interruption":null,"optimization":null,"error":null},"statistics":null})
}

#[test]
fn modern_invocation_checks_structured_answers() {
    let directory = tempfile::tempdir().unwrap();
    let mut options = options(directory.path());
    options.zetesis = emitting(
        directory.path(),
        "native-json",
        &structured_native().to_string(),
        "",
        0,
    );
    let report = super::run_with_invocation(
        &options,
        loaded(directory.path(), 1),
        super::NativeInvocation::Solve,
        |_| {},
    );
    assert_eq!(report.cases()[0].decision(), &super::Decision::Passed);
    let value = report.to_json().unwrap();
    let arguments: Vec<std::ffi::OsString> =
        serde_json::from_value(value["cases"][0]["native_arguments"].clone()).unwrap();
    assert_eq!(arguments[0], "solve");
    assert!(arguments.iter().any(|value| value == "--json"));
    assert!(arguments.iter().any(|value| value == "--all"));
}

#[test]
fn malformed_modern_output_is_a_retained_nonpass() {
    let directory = tempfile::tempdir().unwrap();
    let mut options = options(directory.path());
    options.zetesis = emitting(directory.path(), "native-json", "{", "", 0);
    let report = super::run_with_invocation(
        &options,
        loaded(directory.path(), 1),
        super::NativeInvocation::Solve,
        |_| {},
    );
    assert!(matches!(
        report.cases()[0].decision(),
        super::Decision::NativeOutputUnsupported(_)
    ));
    assert!(report.cases()[0].native_answers().is_none());
}

#[test]
fn incomplete_modern_output_is_a_retained_nonpass() {
    let directory = tempfile::tempdir().unwrap();
    let mut options = options(directory.path());
    let mut output = structured_native();
    output["outcome"]["coverage"] = "partial".into();
    options.zetesis = emitting(directory.path(), "native-json", &output.to_string(), "", 0);
    let report = super::run_with_invocation(
        &options,
        loaded(directory.path(), 1),
        super::NativeInvocation::Solve,
        |_| {},
    );
    assert_eq!(
        report.cases()[0].decision(),
        &super::Decision::NativeIncomplete
    );
}
