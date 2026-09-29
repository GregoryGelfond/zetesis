//! Bench commands map to bounded library plans and independent views.
use clap::Parser as _;
use zetesis_bench::{self as benchmark, Cli, Command as BenchCommand, RunOptions};
use zetesis_presentation::Layout;
use zetesis_test_support::io::Closed;

fn command(arguments: &[&str]) -> BenchCommand {
    Cli::try_parse_from(
        ["zetesis-bench"]
            .into_iter()
            .chain(arguments.iter().copied()),
    )
    .unwrap()
    .command
}

#[test]
fn installed_run_defaults_to_canonical_solve() {
    let BenchCommand::Run(options) = command(&["run", "--report", "new.json"]) else {
        panic!("expected run")
    };
    assert_eq!(
        options.invocation(),
        zetesis_validation::performance::matrix::NativeInvocation::Solve
    );
    assert_eq!(
        options.threads.get(),
        std::thread::available_parallelism()
            .map_or(1, std::num::NonZeroUsize::get)
            .min(4)
    );
    assert_eq!(options.completion_workers.get(), 1);
    assert_eq!(options.clingo_threads.get(), 1);
    assert_eq!(
        options.plan().unwrap().suite(),
        zetesis_validation::performance::matrix::Suite::Corpus
    );
}

#[test]
fn an_explicit_executable_is_measured_through_solve() {
    let BenchCommand::Run(options) = command(&[
        "run",
        "--zetesis",
        "/opt/zetesis/bin/zetesis",
        "--report",
        "new.json",
    ]) else {
        panic!("expected run")
    };
    assert_eq!(
        options.invocation(),
        zetesis_validation::performance::matrix::NativeInvocation::Solve
    );
}

#[test]
fn the_legacy_interface_is_measured_only_on_request() {
    let BenchCommand::Run(options) = command(&[
        "run",
        "--zetesis",
        "/opt/zetesis/bin/zetesis",
        "--native-interface",
        "legacy",
        "--report",
        "new.json",
    ]) else {
        panic!("expected run")
    };
    assert_eq!(
        options.invocation(),
        zetesis_validation::performance::matrix::NativeInvocation::Legacy
    );
}

#[test]
fn ordinary_run_keeps_reference_measurements() {
    use zetesis_validation::performance::{Phase, matrix};
    let BenchCommand::Run(options) = command(&["run", "--report", "new.json"]) else {
        panic!("expected run")
    };
    let plan = options.plan().unwrap();
    assert_eq!(plan.profiles().len(), 1);
    assert_eq!(
        RunOptions::reference_policy(&plan),
        matrix::ReferencePolicy::AllPhases
    );
    assert!(
        plan.slots(1, Some(RunOptions::reference_policy(&plan)))
            .unwrap()
            .iter()
            .any(|slot| slot.producer == matrix::Producer::Reference && slot.phase == Phase::Timed)
    );
}

#[test]
fn grounder_comparison_varies_only_materialization() {
    use zetesis_validation::selected::Grounder;
    let BenchCommand::Run(options) = command(&[
        "run",
        "--report",
        "new.json",
        "--compare-grounders",
        "--threads",
        "4",
        "--completion-workers",
        "2",
        "--batch-size",
        "17",
    ]) else {
        panic!("expected run")
    };
    let plan = options.plan().unwrap();
    let [eager, lazy] = plan.profiles() else {
        panic!("expected matched profiles")
    };
    assert_eq!(eager.grounder, Grounder::Eager);
    assert_eq!(lazy.grounder, Grounder::Lazy);
    assert_eq!(eager.workers.get(), 4);
    assert_eq!(eager.completion_workers.get(), 2);
    assert_eq!(eager.batch_size.get(), 17);
    let normalized = zetesis_validation::selected::NativeExecution {
        grounder: eager.grounder,
        ..*lazy
    };
    assert_eq!(
        serde_json::to_value(eager).unwrap(),
        serde_json::to_value(normalized).unwrap()
    );
}

#[test]
fn grounder_comparison_qualifies_reference_once() {
    use zetesis_validation::performance::{Phase, matrix};
    let BenchCommand::Run(options) =
        command(&["run", "--report", "new.json", "--compare-grounders"])
    else {
        panic!("expected run")
    };
    let plan = options.plan().unwrap();
    assert_eq!(
        RunOptions::reference_policy(&plan),
        matrix::ReferencePolicy::QualificationOnly
    );
    let slots = plan
        .slots(1, Some(RunOptions::reference_policy(&plan)))
        .unwrap();
    let reference: Vec<_> = slots
        .iter()
        .filter(|slot| slot.producer == matrix::Producer::Reference)
        .collect();
    assert_eq!(reference.len(), 1);
    assert_eq!(reference[0].phase, Phase::Qualification);
    for profile in 0..2 {
        assert!(
            slots
                .iter()
                .any(|slot| slot.producer == matrix::Producer::Native { profile }
                    && slot.phase == Phase::Memory)
        );
    }
}

#[test]
fn grounder_comparison_refuses_an_explicit_grounder() {
    for grounder in ["auto", "eager", "lazy"] {
        let error = Cli::try_parse_from([
            "zetesis-bench",
            "run",
            "--report",
            "new.json",
            "--compare-grounders",
            "--grounder",
            grounder,
        ])
        .unwrap_err();
        assert_eq!(error.kind(), clap::error::ErrorKind::ArgumentConflict);
    }
}

#[test]
fn json_preparation_failure_is_one_document() {
    let command = command(&["compare", "not-labelled", "--json"]);
    let mut output = Vec::new();
    assert!(matches!(
        benchmark::execute(&command, Layout::default(), &mut output, &mut Vec::new()),
        Err(benchmark::Error::ReportArgument(_))
    ));
    let document: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(document["format"], "zetesis-benchmark-failure");
    assert_eq!(document["schema"], 1);
    assert_eq!(document["status"], "failed");
    assert_eq!(document["command"], "compare");
    assert_eq!(document["error"]["kind"], "report_argument");
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn cancelled_run_publishes_unattempted_positions() {
    use std::os::unix::fs::PermissionsExt as _;
    let directory = tempfile::tempdir().unwrap();
    let executable = directory.path().join("must-not-launch");
    std::fs::write(&executable, b"no executable is needed before cancellation").unwrap();
    // A named clingo must be a runnable file; this one fails if it is ever run.
    let reference = directory.path().join("reference-must-not-launch");
    std::fs::write(&reference, b"#!/bin/sh\nexit 99\n").unwrap();
    std::fs::set_permissions(&reference, std::fs::Permissions::from_mode(0o700)).unwrap();
    let report = directory.path().join("cancelled.json");
    let corpus =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/correctness");
    let command = command(&[
        "run",
        corpus.to_str().unwrap(),
        "--suite",
        "baseline",
        "--zetesis",
        executable.to_str().unwrap(),
        "--clingo",
        reference.to_str().unwrap(),
        "--report",
        report.to_str().unwrap(),
        "--warmups",
        "0",
        "--repetitions",
        "1",
        "--memory-runs",
        "0",
        "--json",
    ]);
    let mut output = Vec::new();
    let completion = benchmark::execute_with_cancellation(
        &command,
        Layout::default(),
        &mut output,
        &mut Vec::new(),
        &std::sync::atomic::AtomicBool::new(true),
    )
    .unwrap();
    assert_eq!(completion, benchmark::Completion::NonPass);
    let summary: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(summary["passed"], false);
    assert_eq!(summary["accounted"], true);
    let evidence: serde_json::Value =
        serde_json::from_slice(&std::fs::read(report).unwrap()).unwrap();
    assert_eq!(evidence["report"]["metadata"], serde_json::json!([]));
    assert_eq!(
        evidence["report"]["faults"],
        serde_json::json!([{ "kind": "cancelled" }])
    );
    let samples = evidence["report"]["samples"].as_array().unwrap();
    assert_eq!(samples.len(), 12);
    assert!(
        samples
            .iter()
            .all(|sample| sample["decision"] == "not_attempted" && sample["capture"].is_null())
    );
}

#[test]
fn failed_failure_publication_retains_preparation() {
    let command = command(&["compare", "not-labelled", "--json"]);
    let Err(benchmark::Error::Reporting { primary, secondary }) =
        benchmark::execute(&command, Layout::default(), &mut Closed, &mut Vec::new())
    else {
        panic!("both failures must survive")
    };
    assert!(matches!(*primary, benchmark::Error::ReportArgument(_)));
    assert!(matches!(*secondary, benchmark::Error::Json(_)));
}

#[test]
fn a_named_clingo_conflicts_with_measuring_zetesis_alone() {
    let error = Cli::try_parse_from([
        "zetesis-bench",
        "run",
        "--clingo",
        "/usr/local/bin/clingo",
        "--without-clingo",
    ])
    .unwrap_err();
    assert_eq!(error.kind(), clap::error::ErrorKind::ArgumentConflict);
}

#[test]
fn compared_backends_refuse_an_explicit_backend() {
    let error = Cli::try_parse_from([
        "zetesis-bench",
        "run",
        "--compare-backends",
        "cpu,gpu",
        "--backend",
        "cpu",
    ])
    .unwrap_err();
    assert_eq!(error.kind(), clap::error::ErrorKind::ArgumentConflict);
}

// Backends vary slowest and thread counts fastest; every other setting is the
// same in each profile.
#[test]
fn profile_axes_combine_into_their_product() {
    use zetesis_validation::selected::{Backend, Grounder};
    let BenchCommand::Run(options) = command(&[
        "run",
        "--compare-backends",
        "cpu,gpu",
        "--compare-grounders",
        "--compare-threads",
        "1,2",
        "--batch-size",
        "17",
    ]) else {
        panic!("expected run")
    };
    let plan = options.plan().unwrap();
    let axes: Vec<_> = plan
        .profiles()
        .iter()
        .map(|profile| (profile.backend, profile.grounder, profile.workers.get()))
        .collect();
    let mut expected = Vec::new();
    for backend in [Backend::Cpu, Backend::Gpu(None)] {
        for grounder in [Grounder::Eager, Grounder::Lazy] {
            for workers in [1, 2] {
                expected.push((backend, grounder, workers));
            }
        }
    }
    assert_eq!(axes, expected);
    assert!(
        plan.profiles()
            .iter()
            .all(|profile| profile.batch_size.get() == 17)
    );
    assert_eq!(
        RunOptions::reference_policy(&plan),
        zetesis_validation::performance::matrix::ReferencePolicy::QualificationOnly
    );
}

#[test]
fn more_than_eight_profiles_are_refused() {
    let BenchCommand::Run(options) = command(&[
        "run",
        "--compare-backends",
        "cpu,gpu",
        "--compare-grounders",
        "--compare-threads",
        "1,2,4",
    ]) else {
        panic!("expected run")
    };
    assert!(matches!(
        options.plan(),
        Err(zetesis_validation::performance::Error::Configuration(_))
    ));
}

#[test]
fn a_case_selection_joins_the_plan() {
    let BenchCommand::Run(options) = command(&[
        "run",
        "--suite",
        "baseline",
        "--case",
        "standalone/send-money/send-money.lp",
    ]) else {
        panic!("expected run")
    };
    assert_eq!(
        options.plan().unwrap().selection(),
        Some(["standalone/send-money/send-money.lp".to_owned()].as_slice())
    );
}

#[test]
fn the_scalability_suite_defaults_to_indexed_region_search() {
    use zetesis_validation::selected::{FormulaJoins, SearchMethod};
    let BenchCommand::Run(options) = command(&["run", "--suite", "scalability"]) else {
        panic!("expected run")
    };
    let plan = options.plan().unwrap();
    assert_eq!(
        plan.profiles()[0].formula_joins,
        Some(FormulaJoins::Indexed)
    );
    assert_eq!(plan.profiles()[0].search, Some(SearchMethod::Regions));
}

#[test]
fn explicit_search_and_joins_replace_the_suite_defaults() {
    use zetesis_validation::selected::{FormulaJoins, SearchMethod};
    let BenchCommand::Run(options) = command(&[
        "run",
        "--suite",
        "scalability",
        "--formula-joins",
        "table",
        "--search",
        "clauses",
    ]) else {
        panic!("expected run")
    };
    let plan = options.plan().unwrap();
    assert_eq!(plan.profiles()[0].formula_joins, Some(FormulaJoins::Table));
    assert_eq!(plan.profiles()[0].search, Some(SearchMethod::Clauses));
}

#[test]
fn compare_takes_its_reports_as_operands() {
    let BenchCommand::Compare(options) = command(&[
        "compare",
        "baseline=old.json",
        "candidate=new.json",
        "--input-bytes",
        "1024",
    ]) else {
        panic!("expected compare")
    };
    assert_eq!(options.reports, ["baseline=old.json", "candidate=new.json"]);
    assert_eq!(options.input_bytes, 1024);
}

#[test]
fn compare_prints_markdown_or_json_but_not_both() {
    let error = Cli::try_parse_from([
        "zetesis-bench",
        "compare",
        "a=a.json",
        "--markdown",
        "--json",
    ])
    .unwrap_err();
    assert_eq!(error.kind(), clap::error::ErrorKind::ArgumentConflict);
}
