//! The installer, INSTALL.md, the live documentation and the workspace's
//! binaries name one set of executables.

use std::process::Command;
use zetesis_maintenance::{install, invocations};

use crate::support::{documents, repository};

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

/// The documents that state where the default install keeps its sibling.
const LAYOUT_DOCUMENTS: [&str; 3] = [
    "INSTALL.md",
    "README.md",
    "docs/book/architecture/execution.md",
];

/// Check the layout with `installer` and `front` against the documents, with
/// `document` replacing the one at its path.
fn layout(installer: &str, front: &str, document: Option<(&str, &str)>) -> Result<(), String> {
    let texts: Vec<(&str, String)> = LAYOUT_DOCUMENTS
        .iter()
        .map(|&path| match document {
            Some((replaced, text)) if replaced == path => (path, text.to_owned()),
            _ => (path, read(path)),
        })
        .collect();
    let documents: Vec<(&str, &str)> = texts
        .iter()
        .map(|(path, text)| (*path, text.as_str()))
        .collect();
    install::check_sibling_directory(installer, front, &documents)
        .map_err(|error| error.to_string())
}

#[test]
fn the_installer_places_the_sibling_where_the_front_looks() {
    layout(
        &read("scripts/install.sh"),
        &read("crates/zetesis-cli/src/sibling.rs"),
        None,
    )
    .unwrap();
}

#[test]
fn a_sibling_directory_the_front_does_not_look_in_is_refused() {
    let installer = altered(
        &read("scripts/install.sh"),
        "libexec='zetesis-libexec'",
        "libexec='zetesis-helpers'",
    );
    let refusal = layout(&installer, &read("crates/zetesis-cli/src/sibling.rs"), None).unwrap_err();
    assert!(refusal.contains("but the front looks in"), "{refusal}");
}

#[test]
fn a_front_looking_in_another_directory_is_refused() {
    let front = altered(
        &read("crates/zetesis-cli/src/sibling.rs"),
        "const LIBEXEC: &str = \"zetesis-libexec\";",
        "const LIBEXEC: &str = \"zetesis-helpers\";",
    );
    let refusal = layout(&read("scripts/install.sh"), &front, None).unwrap_err();
    assert!(refusal.contains("but the front looks in"), "{refusal}");
}

#[test]
fn a_layout_document_that_does_not_name_the_directory_is_refused() {
    for path in LAYOUT_DOCUMENTS {
        let document = read(path).replace("`zetesis-libexec", "`zetesis-helpers");
        let refusal = layout(
            &read("scripts/install.sh"),
            &read("crates/zetesis-cli/src/sibling.rs"),
            Some((path, &document)),
        )
        .unwrap_err();
        assert!(
            refusal.contains(&format!("{path} does not name")),
            "{refusal}"
        );
    }
}

/// The packages the CLI's front profile resolves to, as `cargo tree` reports
/// its normal dependencies, for the installer's selection and features.
fn front_packages() -> Vec<String> {
    let output = Command::new(env!("CARGO"))
        .args([
            "tree",
            "--edges",
            "normal",
            "--package",
            "zetesis-cli",
            "--no-default-features",
            "--features",
            "front",
            "--prefix",
            "none",
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
    String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .map(str::to_owned)
        .collect()
}

#[test]
fn the_front_profile_builds_without_the_gpu_stack() {
    // The front exists so that CPU runs start without the GPU stack; a feature
    // that brought it back would give up that start without failing a build.
    let packages = front_packages();
    assert!(packages.iter().any(|name| name == "zetesis-cli"));
    for gpu in ["zetesis-wgpu", "wgpu"] {
        assert!(!packages.iter().any(|name| name == gpu), "{gpu}");
    }
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
fn a_tilde_fenced_command_running_a_removed_executable_is_refused() {
    let refusal = invocation_refusal("~~~sh\nzetesis-perf examples/correctness\n~~~\n");
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
fn a_marker_inside_a_code_block_does_not_make_a_record() {
    // A document showing the marker in a code block, as a guide might, is
    // still a live document.
    let refusal = invocation_refusal(&format!(
        "# Records\n\n```md\n{}\n```\n\n```sh\nzetesis-perf examples/correctness\n```\n",
        invocations::RECORD
    ));
    assert!(
        refusal.contains("fixture.md:8: `zetesis-perf`"),
        "{refusal}"
    );
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
