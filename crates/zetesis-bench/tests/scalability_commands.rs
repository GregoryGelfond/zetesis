//! Scalability benchmarks vary only the selected profile axis.

use clap::Parser as _;
use zetesis_bench::{self as benchmark, Cli, Command as BenchCommand};
use zetesis_validation::{
    performance::{Phase, matrix},
    selected,
};

fn bench(arguments: &[&str]) -> benchmark::CorpusOptions {
    let BenchCommand::Corpus(options) = Cli::try_parse_from(
        ["zetesis-bench", "corpus"]
            .into_iter()
            .chain(arguments.iter().copied()),
    )
    .unwrap()
    .command
    else {
        panic!("expected corpus benchmark")
    };
    *options
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
            Cli::try_parse_from(
                [
                    "zetesis-bench",
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
