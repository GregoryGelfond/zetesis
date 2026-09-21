//! The modern command requests the same complete machine workload through solve.
use std::ffi::OsString;
use zetesis_validation::{performance::matrix::NativeInvocation, selected::NativeExecution};

#[test]
fn modern_invocation_uses_canonical_solve_arguments() {
    let arguments = NativeInvocation::Solve.arguments(&NativeExecution::default());
    let expected: Vec<OsString> = [
        "solve",
        "--device",
        "cpu",
        "--oracle",
        "auto",
        "--grounder",
        "eager",
        "--threads",
        "1",
        "--completion-workers",
        "1",
        "--batch-size",
        "64",
        "--max-completion-scratch-bytes",
        "268435456",
        "--all",
    ]
    .into_iter()
    .map(Into::into)
    .collect();
    assert_eq!(arguments, expected);
}

#[test]
fn legacy_invocation_retains_flat_model_selection() {
    let arguments = NativeInvocation::Legacy.arguments(&NativeExecution::default());
    assert_eq!(arguments.first().unwrap(), "--backend");
    assert_eq!(
        &arguments[arguments.len() - 2..],
        &[OsString::from("--models"), OsString::from("0")]
    );
    assert!(arguments.contains(&OsString::from("--workers")));
    assert!(!arguments.contains(&OsString::from("solve")));
}
