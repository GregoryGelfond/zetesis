//! Scalability commands share sealed workloads while keeping test and timing populations distinct.

use zetesis_cli::{
    Invocation,
    testing::{self, TestCommand},
};
use zetesis_test_support::repository;
use zetesis_validation::{
    performance::{Phase, matrix::ReferencePolicy},
    selected,
};

fn test(arguments: &[&str]) -> testing::ScalabilityOptions {
    let Invocation::Test(command) = Invocation::try_parse_from(
        ["zetesis", "test", "scalability"]
            .into_iter()
            .chain(arguments.iter().copied()),
    )
    .unwrap() else {
        panic!("expected scalability test")
    };
    let TestCommand::Scalability(options) = *command else {
        panic!("expected scalability test")
    };
    options
}

#[test]
fn scalability_tests_require_an_evidence_destination() {
    assert!(Invocation::try_parse_from(["zetesis", "test", "scalability"]).is_err());
}

#[test]
fn scalability_tests_have_only_qualification_positions() {
    let options = test(&["--report", "new.json", "--memory", "300000000"]);
    assert!(!options.view.stats);
    let plan = options.plan().unwrap();
    let slots = plan
        .slots(10, Some(ReferencePolicy::QualificationOnly))
        .unwrap();
    assert_eq!(slots.len(), 10 * 6);
    assert!(slots.iter().all(|s| s.phase == Phase::Qualification));
    assert!(
        plan.profiles()
            .iter()
            .all(|profile| profile.backend == selected::Backend::Cpu
                && profile.grounder == selected::Grounder::Eager
                && profile.formula_joins == Some(selected::FormulaJoins::Indexed)
                && profile.search == Some(selected::SearchMethod::Regions)
                && profile.memory_bytes == Some(300_000_000))
    );
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod campaigns {
    use super::*;
    use clap::Parser as _;
    use std::{path::PathBuf, sync::atomic::AtomicBool};
    use zetesis_bench::{self as benchmark, Command as BenchCommand};
    use zetesis_presentation::Layout;

    fn bench(arguments: &[&str]) -> benchmark::RunOptions {
        let BenchCommand::Run(options) = benchmark::Cli::try_parse_from(
            ["zetesis-bench", "run"]
                .into_iter()
                .chain(arguments.iter().copied()),
        )
        .unwrap()
        .command
        else {
            panic!("expected a benchmark run")
        };
        *options
    }

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
            // A named clingo must be a runnable file; this one fails if it is
            // ever run, and cancellation launches nothing.
            std::fs::write(&reference, b"#!/bin/sh\nexit 99\n").unwrap();
            {
                use std::os::unix::fs::PermissionsExt as _;
                std::fs::set_permissions(&reference, std::fs::Permissions::from_mode(0o700))
                    .unwrap();
            }
            let examples = repository::examples();
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
                &BenchCommand::Run(Box::new(bench(&arguments))),
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
        for report in [&qualified, &measured] {
            let normalization = &report["report"]["native_normalization_limits"];
            assert_eq!(normalization["input_bytes"], 16 * 1024 * 1024);
            assert_eq!(normalization["atoms"], 4 * 1024 * 1024);
            assert_eq!(normalization["value_nodes"], 8 * 1024 * 1024);
            assert_eq!(
                report["report"]["limits"]["process"]["max_output_bytes"],
                16 * 1024 * 1024
            );
            assert!(
                report["report"]["plan"]["profiles"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|profile| profile.get("max_expansion_work").is_none())
            );
        }
        assert_eq!(
            qualified["report"]["workloads"],
            measured["report"]["workloads"]
        );
        assert_eq!(
            qualified["report"]["workloads"].as_array().unwrap().len(),
            14
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
    fn compact_conformance_retains_memory_refusal_diagnostics() {
        use std::os::unix::fs::PermissionsExt;

        let fixture = Fixture::new();
        // This controlled reference is a protocol fixture, not an answer oracle.
        // Its UNSAT families intentionally mismatch queens; the native memory
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
        // 384 KiB admits this refusal record (12 KiB) while the wide
        // eager producer family cannot fit its named source/support capacities.
        // Zero memory would also refuse the diagnostic's own output buffer.
        options.memory = Some(384 * 1024);
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
        assert_eq!(native.len(), 14);
        // Other workloads can fail to publish their larger diagnostic footer.
        // That remains an invalid report, never a qualified memory refusal.
        // Require retained evidence of this exact publication cause; malformed
        // reports without that cause still fail with their case identity.
        for (index, check) in &native {
            if check["decision"] == "invalid_report" {
                let retained = &evidence["report"]["samples"][*index];
                let stdout = retained["capture"]["stdout"]["data"].as_str().unwrap();
                let stderr = retained["capture"]["stderr"]["data"].as_str().unwrap();
                let error = serde_json::from_str::<serde_json::Value>(stdout)
                    .expect_err("an incomplete diagnostic cannot be valid JSON");
                assert!(error.is_eof(), "{check}: {error}; stderr: {stderr}");
                assert!(
                    stderr.contains("secondary output: model JSON view refused: Bytes"),
                    "{check}; stderr: {stderr}"
                );
                assert_eq!(check["detail"], retained["detail"], "{check}");
                assert_eq!(check["capture"]["exit"]["code"], 2, "{check}");
                assert!(check.get("elapsed_ns").is_none(), "{check}");
            }
        }
        assert_eq!(
            evidence["report"]["plan"]["profiles"][0]["memory_bytes"],
            384 * 1024
        );
        // The maintained wide Mastermind case is colors=8. Other cases may
        // complete within this allowance; their controlled reference is not an
        // answer oracle. This proposition concerns a real, retained refusal.
        let workload = &evidence["report"]["workloads"][8];
        assert_eq!(workload["entry"], "scalability/mastermind.lp");
        assert_eq!(workload["sources"][0]["edits"][0]["after"], "8");
        let wide = native
            .iter()
            .find(|(_, check)| check["slot"]["case"] == 8)
            .unwrap();
        assert_eq!(
            wide.1["decision"], "refused",
            "{}; retained stderr: {}",
            wide.1, evidence["report"]["samples"][wide.0]["capture"]["stderr"]["data"]
        );
        let support_bytes = zetesis_solve::Resources::new(384 * 1024, std::num::NonZeroUsize::MIN)
            .formula_limits()
            .max_support_bytes;
        assert!(
            wide.1["detail"]
                .as_str()
                .unwrap()
                .contains(&format!("SupportBytes limit {support_bytes} exceeded")),
            "{}",
            wide.1
        );
        for (index, check) in native
            .into_iter()
            .filter(|(_, check)| check["decision"] == "refused")
        {
            let retained = &evidence["report"]["samples"][index];
            let envelope: serde_json::Value =
                serde_json::from_str(retained["capture"]["stdout"]["data"].as_str().unwrap())
                    .unwrap();
            let cause = envelope["outcome"]["error"]["detail"].as_str().unwrap();
            // Memory also bounds retained populations such as source origins;
            // preserve each actual diagnosis rather than infer it from a noun.
            assert!(!cause.is_empty(), "{check}");
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

#[test]
fn scalability_refuses_removed_work_tuning() {
    let error = Invocation::try_parse_from([
        "zetesis",
        "test",
        "scalability",
        "--report",
        "new.json",
        "--max-expansion-work",
        "1",
    ])
    .unwrap_err();
    assert_eq!(error.kind(), clap::error::ErrorKind::UnknownArgument);
}
