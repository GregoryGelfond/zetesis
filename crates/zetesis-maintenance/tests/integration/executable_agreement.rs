//! The installer, INSTALL.md, the live documentation and the workspace's
//! binaries name one set of executables.

use std::path::{Path, PathBuf};
use std::process::Command;
use zetesis_maintenance::{install, invocations};

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(path: &str) -> String {
    std::fs::read_to_string(repository().join(path)).unwrap()
}

/// The workspace's packages and targets as Cargo itself reports them.
fn metadata() -> Vec<u8> {
    let output = Command::new(env!("CARGO"))
        .args([
            "metadata",
            "--no-deps",
            "--format-version",
            "1",
            "--offline",
        ])
        .arg("--manifest-path")
        .arg(repository().join("Cargo.toml"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

/// Replace one occurrence, refusing a fixture that no longer contains it.
fn altered(text: &str, from: &str, to: &str) -> String {
    assert!(text.contains(from), "{from:?} is no longer present");
    text.replacen(from, to, 1)
}

fn refusal(installer: &str, guide: &str) -> String {
    install::check(installer, guide, &metadata())
        .unwrap_err()
        .to_string()
}

#[test]
fn the_installer_the_guide_and_the_packages_agree() {
    install::check(
        &read("scripts/install.sh"),
        &read("INSTALL.md"),
        &metadata(),
    )
    .unwrap();
}

#[test]
fn a_tool_the_guide_does_not_describe_is_refused() {
    let guide = altered(
        &read("INSTALL.md"),
        "| `zetesis-bench` |",
        "| `zetesis-other` |",
    );
    let refusal = refusal(&read("scripts/install.sh"), &guide);
    assert!(refusal.starts_with("INSTALL.md describes"), "{refusal}");
}

#[test]
fn a_tool_no_built_package_provides_is_refused() {
    let installer = altered(
        &read("scripts/install.sh"),
        "tools='zetesis zetesis-bench'",
        "tools='zetesis zetesis-bench zetesis-validate'",
    );
    let refusal = refusal(&installer, &read("INSTALL.md"));
    assert!(
        refusal.starts_with("the installer's packages build"),
        "{refusal}"
    );
}

#[test]
fn a_cargo_command_for_another_package_is_refused() {
    let guide = altered(
        &read("INSTALL.md"),
        "--path crates/zetesis-bench",
        "--path crates/zetesis-validation",
    );
    let refusal = refusal(&read("scripts/install.sh"), &guide);
    assert!(
        refusal.starts_with("INSTALL.md's Cargo commands install"),
        "{refusal}"
    );
}

/// Every Markdown document of the repository, skipping hidden and build
/// directories, as (path relative to the repository, text).
fn documents() -> Vec<(String, String)> {
    fn visit(directory: &Path, root: &Path, documents: &mut Vec<(String, String)>) {
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            let name = path.file_name().unwrap().to_string_lossy();
            if path.is_dir() {
                if !name.starts_with('.') && name != "target" {
                    visit(&path, root, documents);
                }
            } else if path.extension().is_some_and(|extension| extension == "md") {
                let relative = path.strip_prefix(root).unwrap().display().to_string();
                documents.push((relative, std::fs::read_to_string(&path).unwrap()));
            }
        }
    }
    let root = repository().canonicalize().unwrap();
    let mut documents = Vec::new();
    visit(&root, &root, &mut documents);
    documents
}

fn invocation_refusal(document: &str) -> String {
    invocations::check([("fixture.md", document)], &metadata())
        .unwrap_err()
        .to_string()
}

#[test]
fn live_documents_name_only_built_executables() {
    let documents = documents();
    // The walk must reach the documents that name executables.
    assert!(documents.iter().any(|(path, _)| path == "INSTALL.md"));
    invocations::check(
        documents
            .iter()
            .map(|(path, text)| (path.as_str(), text.as_str())),
        &metadata(),
    )
    .unwrap();
}

#[test]
fn a_command_running_a_removed_executable_is_refused() {
    let refusal =
        invocation_refusal("```sh\nzetesis-perf examples/correctness \\\n  --suite series\n```\n");
    assert!(
        refusal.contains("fixture.md:2: `zetesis-perf` is not a binary or package"),
        "{refusal}"
    );
}

#[test]
fn a_tool_table_listing_a_removed_executable_is_refused() {
    let refusal = invocation_refusal(
        "| Command | Purpose |\n|---|---|\n| `zetesis-series` | Compare reports. |\n",
    );
    assert!(
        refusal.contains("fixture.md:3: `zetesis-series` is not a binary or package"),
        "{refusal}"
    );
}

#[test]
fn the_retired_bench_command_is_refused() {
    let refusal =
        invocation_refusal("```sh\nNO_COLOR=1 zetesis bench corpus --report run.json\n```\n");
    assert!(
        refusal.contains("fixture.md:2: `zetesis bench` is not a command"),
        "{refusal}"
    );
}

#[test]
fn a_dated_record_keeps_the_spellings_it_records() {
    let record = format!(
        "# A measurement\n\n{}\n\n```sh\nzetesis-perf examples/correctness\n```\n",
        invocations::RECORD
    );
    invocations::check([("record.md", record.as_str())], &metadata()).unwrap();
}

#[test]
fn a_document_quoting_the_record_marker_is_still_checked() {
    // The contributor guide names the marker in prose; only a record carries
    // it on a line of its own.
    let refusal = invocation_refusal(&format!(
        "# Contributing\n\nA record carries `{}`.\n\n```sh\nzetesis-perf examples/correctness\n```\n",
        invocations::RECORD
    ));
    assert!(
        refusal.contains("fixture.md:6: `zetesis-perf`"),
        "{refusal}"
    );
}
