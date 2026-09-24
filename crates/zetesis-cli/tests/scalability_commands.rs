//! Scalability commands share sealed workloads while keeping test and timing populations distinct.

use zetesis_cli::{
    Invocation,
    benchmark::{self, BenchCommand},
    testing::{self, TestCommand},
};
use zetesis_validation::{
    performance::{Phase, matrix},
    selected,
};

fn bench(arguments: &[&str]) -> benchmark::CorpusOptions {
    let Invocation::Bench(BenchCommand::Corpus(options)) = Invocation::try_parse_from(
        ["zetesis", "bench", "corpus"]
            .into_iter()
            .chain(arguments.iter().copied()),
    )
    .unwrap() else {
        panic!("expected corpus benchmark")
    };
    *options
}

fn test(arguments: &[&str]) -> testing::ScalabilityOptions {
    let Invocation::Test(TestCommand::Scalability(options)) = Invocation::try_parse_from(
        ["zetesis", "test", "scalability"]
            .into_iter()
            .chain(arguments.iter().copied()),
    )
    .unwrap() else {
        panic!("expected scalability test")
    };
    options
}

#[test]
fn thread_comparison_varies_only_native_threads() {
    let options = bench(&[
        "--suite",
        "scalability",
        "--report",
        "new.json",
        "--compare-threads",
        "1,2,4,8,14",
        "--grounder",
        "eager",
        "--completion-workers",
        "2",
        "--batch-size",
        "17",
        "--max-expansion-work",
        "300000000",
    ]);
    let plan = options.plan().unwrap();
    assert_eq!(plan.suite(), matrix::Suite::Scalability);
    assert_eq!(
        plan.profiles()
            .iter()
            .map(|p| p.workers.get())
            .collect::<Vec<_>>(),
        [1, 2, 4, 8, 14]
    );
    let first = plan.profiles()[0];
    assert_eq!(first.max_expansion_work, Some(300_000_000));
    assert_eq!(first.completion_workers.get(), 2);
    assert_eq!(first.batch_size.get(), 17);
    for profile in plan.profiles() {
        let normalized = selected::NativeExecution {
            workers: first.workers,
            ..*profile
        };
        assert_eq!(
            serde_json::to_value(normalized).unwrap(),
            serde_json::to_value(first).unwrap()
        );
    }
}

#[test]
fn thread_comparison_only_qualifies_the_reference() {
    let plan = bench(&["--report", "new.json", "--compare-threads", "1,2"])
        .plan()
        .unwrap();
    let slots = plan.slots(1).unwrap();
    let reference = slots
        .iter()
        .filter(|s| s.producer == matrix::Producer::Reference)
        .collect::<Vec<_>>();
    assert_eq!(reference.len(), 1);
    assert_eq!(reference[0].phase, Phase::Qualification);
    assert!(slots.iter().any(|s| s.phase == Phase::Timed));
}

#[test]
fn authored_options_require_the_scalability_suite() {
    for arguments in [
        vec!["--report", "new.json", "--include-einstein"],
        vec!["--report", "new.json", "--examples", "examples"],
    ] {
        assert!(bench(&arguments).plan().is_err());
    }
}

#[test]
fn thread_comparison_refuses_competing_profile_axes() {
    for competing in [vec!["--threads", "2"], vec!["--compare-grounders"]] {
        assert!(
            Invocation::try_parse_from(
                [
                    "zetesis",
                    "bench",
                    "corpus",
                    "--report",
                    "new.json",
                    "--compare-threads",
                    "1,2"
                ]
                .into_iter()
                .chain(competing),
            )
            .is_err()
        );
    }
}

#[test]
fn scalability_tests_require_an_evidence_destination() {
    assert!(Invocation::try_parse_from(["zetesis", "test", "scalability"]).is_err());
}

#[test]
fn scalability_tests_have_only_qualification_positions() {
    let options = test(&["--report", "new.json", "--max-expansion-work", "300000000"]);
    assert!(!options.view.stats);
    let plan = options.plan().unwrap();
    let slots = plan.slots(9).unwrap();
    assert_eq!(slots.len(), 9 * 6);
    assert!(slots.iter().all(|s| s.phase == Phase::Qualification));
    assert!(
        plan.profiles()
            .iter()
            .all(|profile| profile.backend == selected::Backend::Cpu
                && profile.grounder == selected::Grounder::Eager
                && profile.formula_joins == Some(selected::FormulaJoins::Indexed)
                && profile.search == Some(selected::SearchMethod::Regions)
                && profile.completion_workers.get() == 1
                && profile.max_expansion_work == Some(300_000_000))
    );
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod campaigns {
    use super::*;
    use std::{path::PathBuf, sync::atomic::AtomicBool};
    use zetesis_presentation::Layout;

    struct Fixture {
        directory: tempfile::TempDir,
        native: PathBuf,
        reference: PathBuf,
        corpus: PathBuf,
        examples: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let directory = tempfile::tempdir().unwrap();
            let native = directory.path().join("native");
            let reference = directory.path().join("reference");
            std::fs::write(&native, b"native: cancelled before launch").unwrap();
            std::fs::write(&reference, b"reference: cancelled before launch").unwrap();
            let examples = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples");
            let corpus = examples.join("correctness");
            Self {
                directory,
                native,
                reference,
                corpus,
                examples,
            }
        }

        fn options<'a>(&'a self, report: &'a std::path::Path) -> Vec<&'a str> {
            vec![
                self.corpus.to_str().unwrap(),
                "--examples",
                self.examples.to_str().unwrap(),
                "--zetesis",
                self.native.to_str().unwrap(),
                "--clingo",
                self.reference.to_str().unwrap(),
                "--report",
                report.to_str().unwrap(),
            ]
        }
    }

    #[test]
    fn both_commands_select_the_same_sealed_workloads() {
        let fixture = Fixture::new();
        let test_report = fixture.directory.path().join("test.json");
        let bench_report = fixture.directory.path().join("bench.json");
        let mut test_arguments = fixture.options(&test_report);
        test_arguments.push("--json");
        let command = TestCommand::Scalability(test(&test_arguments));
        let mut output = Vec::new();
        assert_eq!(
            testing::execute_with_cancellation(
                &command,
                Layout::default(),
                &mut output,
                &mut Vec::new(),
                &AtomicBool::new(true),
            )
            .unwrap(),
            testing::Completion::NonPass
        );
        let view: serde_json::Value = serde_json::from_slice(&output).unwrap();
        assert_eq!(view["format"], "zetesis_scalability_conformance");
        assert_eq!(view["accounted"], true);
        assert!(
            view["checks"]
                .as_array()
                .unwrap()
                .iter()
                .all(|check| check["slot"]["phase"] == "qualification"
                    && check["decision"] == "not_attempted"
                    && check.get("elapsed_ns").is_none())
        );
        let mut arguments = fixture.options(&bench_report);
        arguments.extend([
            "--suite",
            "scalability",
            "--compare-threads",
            "1,2,4,8,14",
            "--memory-runs",
            "0",
            "--json",
        ]);
        assert_eq!(
            benchmark::execute_with_cancellation(
                &BenchCommand::Corpus(Box::new(bench(&arguments))),
                Layout::default(),
                &mut Vec::new(),
                &mut Vec::new(),
                &AtomicBool::new(true),
            )
            .unwrap(),
            benchmark::Completion::NonPass
        );
        let evidence = |path| {
            serde_json::from_slice::<serde_json::Value>(&std::fs::read(path).unwrap()).unwrap()
        };
        let qualified = evidence(test_report);
        let measured = evidence(bench_report);
        assert_eq!(
            qualified["report"]["workloads"],
            measured["report"]["workloads"]
        );
        assert_eq!(
            qualified["report"]["workloads"].as_array().unwrap().len(),
            9
        );
        assert!(
            qualified["report"]["samples"]
                .as_array()
                .unwrap()
                .iter()
                .all(|s| s["capture"].is_null())
        );
    }

    #[test]
    fn conformance_view_does_not_present_timings_by_default() {
        let fixture = Fixture::new();
        let destination = fixture.directory.path().join("human.json");
        let command = TestCommand::Scalability(test(&fixture.options(&destination)));
        let mut output = Vec::new();
        testing::execute_with_cancellation(
            &command,
            Layout::default(),
            &mut output,
            &mut Vec::new(),
            &AtomicBool::new(true),
        )
        .unwrap();
        let text = String::from_utf8(output).unwrap();
        assert!(text.starts_with("Scalability conformance — complete families"));
        assert!(!text.contains("Median"));
        assert!(!text.contains("Captured ms"));
        assert!(!text.contains("RSS"));
    }

    #[test]
    fn scalability_preparation_failure_is_structured() {
        let fixture = Fixture::new();
        let destination = fixture.directory.path().join("failure.json");
        let mut options = test(&fixture.options(&destination));
        options.root = fixture.directory.path().to_owned();
        options.view.json = true;
        let mut output = Vec::new();
        assert!(matches!(
            testing::execute(
                &TestCommand::Scalability(options),
                Layout::default(),
                &mut output,
                &mut Vec::new(),
            ),
            Err(testing::Error::Scalability(_))
        ));
        let failure: serde_json::Value = serde_json::from_slice(&output).unwrap();
        assert_eq!(failure["task"], "scalability");
        assert_eq!(failure["status"], "preparation_failed");
        assert_eq!(failure["kind"], "scalability_preparation");
        assert!(!destination.exists());
    }

    #[test]
    fn scalability_publication_failure_keeps_its_stage() {
        let fixture = Fixture::new();
        let destination = fixture.directory.path().join("oversized.json");
        let mut options = test(&fixture.options(&destination));
        options.report_bytes = 0;
        options.view.json = true;
        let mut output = Vec::new();
        assert!(matches!(
            testing::execute_with_cancellation(
                &TestCommand::Scalability(options),
                Layout::default(),
                &mut output,
                &mut Vec::new(),
                &AtomicBool::new(true),
            ),
            Err(testing::Error::ScalabilityPublication(_))
        ));
        let failure: serde_json::Value = serde_json::from_slice(&output).unwrap();
        assert_eq!(failure["status"], "failed");
        assert_eq!(failure["kind"], "scalability_publication");
    }

    #[test]
    fn compact_conformance_retains_work_refusal_diagnostics() {
        use std::os::unix::fs::PermissionsExt;

        let fixture = Fixture::new();
        // This controlled reference is a protocol fixture, not an answer oracle.
        // Its UNSAT families intentionally mismatch queens; the native work
        // refusal must still carry its own diagnosis and process exit.
        std::fs::write(&fixture.reference, concat!(
            "#!/bin/sh\n",
            "if [ \"$#\" -eq 1 ]; then echo fixture; exit 0; fi\n",
            "printf '%s' '{\"Result\":\"UNSATISFIABLE\",\"Models\":{\"More\":\"no\",\"Number\":0},\"Call\":[{}]}'\n",
        )).unwrap();
        std::fs::set_permissions(&fixture.reference, std::fs::Permissions::from_mode(0o700))
            .unwrap();
        let destination = fixture.directory.path().join("refused.json");
        let mut options = test(&fixture.options(&destination));
        options.zetesis = Some(PathBuf::from(env!("CARGO_BIN_EXE_zetesis")));
        options.threads = vec![std::num::NonZeroUsize::MIN];
        options.max_expansion_work = Some(0);
        options.view.json = true;
        let mut output = Vec::new();
        assert_eq!(
            testing::execute(
                &TestCommand::Scalability(options),
                Layout::default(),
                &mut output,
                &mut Vec::new(),
            )
            .unwrap(),
            testing::Completion::NonPass
        );
        let view: serde_json::Value = serde_json::from_slice(&output).unwrap();
        let evidence: serde_json::Value =
            serde_json::from_slice(&std::fs::read(destination).unwrap()).unwrap();
        let checks = view["checks"].as_array().unwrap();
        let native = checks
            .iter()
            .enumerate()
            .filter(|(_, check)| check["slot"]["producer"]["solver"] == "native")
            .collect::<Vec<_>>();
        assert_eq!(native.len(), 9);
        for (index, check) in native {
            assert_eq!(check["decision"], "refused", "{check}");
            let retained = &evidence["report"]["samples"][index];
            let envelope: serde_json::Value =
                serde_json::from_str(retained["capture"]["stdout"]["data"].as_str().unwrap())
                    .unwrap();
            let cause = envelope["outcome"]["error"]["detail"].as_str().unwrap();
            // File inputs classify an expansion failure by their bundle
            // admission door; the diagnostic retains the actual exhausted unit.
            assert!(
                cause.contains("TermWork") && cause.contains("exceeds 0"),
                "{cause}"
            );
            assert!(check["detail"].as_str().unwrap().contains(cause), "{check}");
            assert_eq!(check["detail"], retained["detail"]);
            assert_eq!(check["capture"]["exit"]["code"], 2);
            assert!(check.get("blocked_by").is_some());
            assert!(check.get("elapsed_ns").is_none());
            assert!(check["capture"].get("stdout").is_none());
        }
        assert!(view["before"].as_array().unwrap().len() >= 3);
    }
}
