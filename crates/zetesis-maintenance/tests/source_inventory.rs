//! Source inventory boundaries without a compiler or physical device.
use std::{fs, path::Path, process::Command};
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
fn source_command_uses_the_library_inventory() {
    let f = fixture();
    let output = Command::new(env!("CARGO_BIN_EXE_zetesis-maintenance"))
        .args(["sources", "--root"])
        .arg(f.path())
        .output()
        .unwrap();
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
