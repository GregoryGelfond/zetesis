//! Real authored defaults cross the CLI and complete-display contract boundary.
use clap::Parser;
use std::path::{Path, PathBuf};
use zetesis_cli::{Completion, Options, run_with_diagnostics};
use zetesis_cpu::Cancellation;
use zetesis_validation::{
    answers::native_json,
    performance::{
        matrix::{NativeInvocation, Workload, WorkloadLimits},
        scalability,
    },
};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples")
}

fn check(workload: &Workload) {
    // Exercise ordinary automatic admission with unchanged resource defaults.
    // The scalability profiles intentionally request complete eager grounding.
    let options = Options::try_parse_from([
        "zetesis",
        "--backend",
        "cpu",
        "--models",
        "0",
        "--json",
        "--time-limit",
        "60",
    ])
    .unwrap();
    let source = std::fs::read_to_string(root().join(workload.entry())).unwrap();
    let mut output = Vec::new();
    let report = run_with_diagnostics(
        source,
        &options,
        &mut output,
        &mut Vec::new(),
        &Cancellation::default(),
    )
    .unwrap();
    assert_eq!(
        report.completion,
        Completion::Exhausted,
        "{}: {report:?}",
        workload.entry()
    );
    let answers = native_json::parse(&output, native_json::Limits::default())
        .unwrap()
        .reported_displays(8 * 1024 * 1024)
        .unwrap();
    workload.contract().unwrap().check(&answers).unwrap();
}

#[test]
fn default_scalability_sources_satisfy_complete_contracts() {
    for workload in scalability::defaults(&root(), WorkloadLimits::default()).unwrap() {
        check(&workload);
    }
}

#[test]
fn default_limits_admit_the_unique_einstein_assignment() {
    check(&scalability::einstein(&root(), WorkloadLimits::default()).unwrap());
}

#[test]
fn worker_scaling_override_reaches_the_explicit_solve_options() {
    use zetesis_cli::Invocation;
    let profile = scalability::profiles(Some(300_000_000))[0];
    let invocation = Invocation::try_parse_from(
        std::iter::once("zetesis".into())
            .chain(NativeInvocation::Solve.arguments(&profile))
            .chain(["-".into()]),
    )
    .unwrap();
    let Invocation::Solve(options) = invocation else {
        panic!("expected solve")
    };
    // Parsing the maintained invocation establishes that the optional profile
    // flag is accepted by the actual command, rather than only by a mock runner.
    assert_eq!(options.max_expansion_work, Some(300_000_000));
}
