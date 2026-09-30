//! Source inventory boundaries without a compiler or physical device.
#[cfg(any(target_os = "linux", target_os = "macos"))]
use crate::support::process as subprocess;
use std::{fs, path::Path};
#[cfg(any(target_os = "linux", target_os = "macos"))]
use subprocess::Command;
use zetesis_maintenance::inventory::{self, Limits};
fn fixture() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    for (name, content) in [
        ("Cargo.toml", "workspace"),
        ("Cargo.lock", "locked"),
        ("rust-toolchain.toml", "pinned"),
        ("crates/one/Cargo.toml", "crate"),
        ("crates/one/src/lib.rs", "Rust"),
        ("crates/one/src/kernel.wgsl", "shader"),
        ("crates/one/README.md", "not source"),
        ("crates/one/notCargo.toml", "not a manifest"),
        ("target/stale.rs", "not source"),
    ] {
        let path = directory.path().join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }
    directory
}
#[test]
fn source_inventory_matches_the_declared_boundary() {
    let f = fixture();
    let sources = inventory::sources(f.path(), Limits::default()).unwrap();
    let expected = [
        "Cargo.lock",
        "Cargo.toml",
        "crates/one/Cargo.toml",
        "crates/one/src/kernel.wgsl",
        "crates/one/src/lib.rs",
        "rust-toolchain.toml",
    ];
    assert_eq!(
        sources.keys().map(String::as_str).collect::<Vec<_>>(),
        expected
    );
}
#[test]
fn source_inventory_observes_byte_changes() {
    let f = fixture();
    let before = inventory::sources(f.path(), Limits::default()).unwrap();
    fs::write(f.path().join("crates/one/src/lib.rs"), "changed").unwrap();
    let after = inventory::sources(f.path(), Limits::default()).unwrap();
    assert_ne!(
        before["crates/one/src/lib.rs"],
        after["crates/one/src/lib.rs"]
    );
}
#[test]
fn source_inventory_enforces_read_limits() {
    let f = fixture();
    for limits in [
        Limits {
            file_bytes: 0,
            ..Limits::default()
        },
        Limits {
            total_bytes: 0,
            ..Limits::default()
        },
        Limits {
            entries: 0,
            ..Limits::default()
        },
    ] {
        assert!(inventory::sources(f.path(), limits).is_err());
    }
}
#[test]
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn source_command_uses_the_library_inventory() {
    let f = fixture();
    let output = Command::new(env!("CARGO_BIN_EXE_zetesis-maintenance"))
        .args(["sources", "--root"])
        .arg(f.path())
        .bounded_output();
    assert!(output.status.success());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap(),
        serde_json::to_value(inventory::sources(f.path(), Limits::default()).unwrap()).unwrap()
    );
}
#[test]
fn policy_reads_have_an_inclusive_byte_limit() {
    let f = fixture();
    let path = f.path().join("Cargo.toml");
    assert_eq!(inventory::read(&path, 9).unwrap(), b"workspace");
    assert!(inventory::read(&path, 8).is_err());
    assert!(inventory::read(Path::new("/missing/maintenance-input"), 10).is_err());
}

fn source_file(root: &Path, path: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, "fn f() {}\n").unwrap();
}

#[test]
fn inventory_selects_maintained_rust_roots() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let expected = [
        "crates/example/build.rs",
        "crates/example/examples/demo.rs",
        "crates/example/src/lib.rs",
        "crates/example/src/target/mod.rs",
        "crates/example/tests/.git/hidden.rs",
        "crates/example/tests/nested/case.rs",
        "crates/target/src/lib.rs",
        "docs/book/examples/session.rs",
        "experiments/gate-transfer/src/lib.rs",
        "refinement/membership/rust/src/lib.rs",
        "validation/reference/src/lib.rs",
    ];
    for package in [
        "crates/example",
        "crates/target",
        "validation/reference",
        "experiments/gate-transfer",
        "refinement/membership/rust",
    ] {
        let directory = root.join(package);
        fs::create_dir_all(&directory).unwrap();
        let name = directory.file_name().unwrap().to_str().unwrap();
        fs::write(
            directory.join("Cargo.toml"),
            format!("[package]\nname = \"{name}\"\nversion = \"0.0.0\"\n"),
        )
        .unwrap();
    }
    for source in expected {
        source_file(root, source);
    }
    for ignored in [
        "crates/example/target/generated.rs",
        "crates/example/docs/old.rs",
        "docs/verification/historical.rs",
        "validation/upstream/original.rs",
    ] {
        source_file(root, ignored);
    }
    let actual = inventory::authored(root, Limits::default()).unwrap();
    assert_eq!(actual, expected);
}
