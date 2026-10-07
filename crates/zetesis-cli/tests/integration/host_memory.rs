//! The host-memory reading behind the default allowance comes from the system,
//! not from whatever the caller's `PATH` supplies.

#[cfg(target_os = "macos")]
#[test]
fn the_host_memory_probe_ignores_a_sysctl_on_path() {
    use std::os::unix::fs::PermissionsExt as _;
    use std::process::Command;

    // A planted `sysctl` that reports an absurd memory size. Resolving the
    // probe through `PATH` would run it and print this value.
    const PLANTED: &str = "1234";
    let directory = tempfile::tempdir().unwrap();
    let planted = directory.path().join("sysctl");
    let invoked = directory.path().join("invoked");
    std::fs::write(
        &planted,
        format!("#!/bin/sh\n: > \"$ZETESIS_TEST_SYSCTL_MARKER\"\necho {PLANTED}\n"),
    )
    .unwrap();
    std::fs::set_permissions(&planted, std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = std::env::join_paths(std::iter::once(directory.path().to_owned()).chain(
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()),
    ))
    .unwrap();
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/book/examples/choices.lp");

    let result = Command::new(env!("CARGO_BIN_EXE_zetesis"))
        .args(["solve", "--backend", "cpu", "--all", "--json", "--stats"])
        .arg(&source)
        .env("PATH", path)
        .env("ZETESIS_TEST_SYSCTL_MARKER", &invoked)
        .output()
        .unwrap();

    assert!(result.status.success(), "{result:?}");
    let diagnostics = String::from_utf8(result.stderr).unwrap();
    let reported = diagnostics
        .split("(host physical memory ")
        .nth(1)
        .and_then(|rest| rest.split(')').next())
        .expect("the statistics header reports the host memory reading");
    assert_ne!(reported, PLANTED);
    assert!(!invoked.exists(), "the planted probe must not run");
    assert!(
        reported == "unreported" || reported.parse::<u64>().is_ok_and(|bytes| bytes > 0),
        "{reported}"
    );
}
