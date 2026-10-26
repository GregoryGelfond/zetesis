//! Real authored defaults cross the CLI and complete-display contract boundary.
use clap::Parser;
use zetesis_cli::{Completion, Options, run_with_diagnostics};
use zetesis_cpu::Cancellation;
use zetesis_test_support::repository::examples as root;
use zetesis_validation::{
    answers::native_json,
    performance::{
        bundles,
        matrix::{NativeInvocation, Workload, WorkloadLimits},
        scalability,
    },
};

fn check(workload: &Workload) {
    check_source(&root().join(workload.entry()), workload);
}

/// A contributed workload's default, solved from the directory its manifest
/// names as the workload's root.
fn check_bundle(directory: &str) {
    let directory = root().join(directory);
    let mut workloads =
        bundles::load(&directory.join("manifest.json"), WorkloadLimits::default()).unwrap();
    assert_eq!(workloads.len(), 1);
    let workload = workloads.pop().unwrap();
    check_source(&directory.join(workload.entry()), &workload);
}

fn check_source(path: &std::path::Path, workload: &Workload) {
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
    let source = std::fs::read_to_string(path).unwrap();
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
fn default_limits_admit_the_complete_sudoku_grid() {
    check(&scalability::sudoku(&root(), WorkloadLimits::default()).unwrap());
}

#[test]
fn two_qbf_default_satisfies_its_complete_contract() {
    // The corpus's first disjunctive program: every membership check runs the
    // proper-subset query over the candidate's frozen reduct.
    check_bundle("scalability/two-qbf");
}

#[test]
fn strategic_companies_default_satisfies_its_complete_contract() {
    // Disjunctive heads with control rules through strategic companies: every
    // candidate runs the proper-subset query over its frozen reduct.
    check_bundle("scalability/strategic-companies");
}

#[test]
fn memory_override_reaches_the_explicit_solve_options() {
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
    assert_eq!(options.memory, 300_000_000);
}
