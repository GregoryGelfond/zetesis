//! The modern command requests the same complete machine workload through solve.
use std::ffi::OsString;
use zetesis_validation::{performance::matrix::NativeInvocation, selected::NativeExecution};

#[test]
fn modern_invocation_uses_canonical_solve_arguments() {
    let arguments = NativeInvocation::Solve.arguments(&NativeExecution::default());
    let expected: Vec<OsString> = [
        "solve",
        "--backend",
        "cpu",
        "--oracle",
        "auto",
        "--grounder",
        "eager",
        "--threads",
        "1",
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

#[test]
fn ordinary_profiles_record_only_public_resource_controls() {
    let profile = NativeExecution {
        memory_bytes: Some(0),
        time_limit_seconds: std::num::NonZeroU64::new(7),
        ..NativeExecution::default()
    };
    let value = serde_json::to_value(profile).unwrap();
    assert_eq!(value["resource_policy"], "ordinary");
    assert_eq!(value["threads"], 1);
    assert_eq!(value["memory_bytes"], 0);
    assert_eq!(value["time_limit_seconds"], 7);
    for field in [
        "workers",
        "completion_workers",
        "batch_size",
        "max_completion_scratch_bytes",
        "max_expansion_work",
    ] {
        assert!(value.get(field).is_none(), "{field}");
    }
    for invocation in [NativeInvocation::Legacy, NativeInvocation::Solve] {
        let args = invocation.arguments(&profile);
        for (flag, expected) in [("--memory", "0"), ("--time-limit", "7")] {
            let at = args.iter().position(|arg| arg == flag).unwrap();
            assert_eq!(args[at + 1], expected);
        }
        assert!(
            !args
                .iter()
                .any(|arg| arg.to_string_lossy().starts_with("--max-")
                    || arg == "--batch-size"
                    || arg == "--completion-workers")
        );
    }
}
