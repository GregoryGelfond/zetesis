//! Live documentation links only to what the repository holds.

use std::path::Path;
use zetesis_maintenance::{invocations, links};

use crate::support::{documents, repository};

/// Check one document at `path` over a working tree holding exactly `held`.
fn checked(path: &str, document: &str, held: &[&str]) -> Result<(), String> {
    links::check([(path, document)], |candidate| {
        held.iter().any(|held| Path::new(held) == candidate)
    })
    .map_err(|error| error.to_string())
}

#[test]
fn live_documents_link_only_to_what_the_repository_holds() {
    let root = repository();
    let documents = documents();
    // The walk must reach the pages that link most.
    assert!(
        documents
            .iter()
            .any(|(path, _)| path == "docs/book/reference/language.md")
    );
    links::check(
        documents
            .iter()
            .map(|(path, text)| (path.as_str(), text.as_str())),
        |path| root.join(path).exists(),
    )
    .unwrap();
}

#[test]
fn the_manual_declares_the_checked_repository_and_source() {
    let book = std::fs::read_to_string(repository().join("book.toml")).unwrap();
    assert!(book.contains(&format!("git-repository-url = \"{}\"", links::REPOSITORY)));
    assert!(book.contains(&format!("src = \"{}\"", links::BOOK)));
}

#[test]
fn links_to_present_files_and_directories_are_accepted() {
    let document = format!(
        "[file](crates/a/src/lib.rs), [directory](crates/a/) and \
         [lines]({}/blob/main/crates/a/src/lib.rs#L3).\n\n[defined]: crates/a/src/lib.rs \"Title\"\n",
        links::REPOSITORY
    );
    checked("README.md", &document, &["crates/a/src/lib.rs", "crates/a"]).unwrap();
}

#[test]
fn a_relative_link_to_a_missing_file_is_refused() {
    let refusal = checked(
        "docs/guide.md",
        "See [the tests](../crates/a/tests/gone.rs).\n",
        &["crates/a/tests"],
    )
    .unwrap_err();
    assert!(
        refusal.contains("docs/guide.md:1: ../crates/a/tests/gone.rs"),
        "{refusal}"
    );
}

#[test]
fn a_main_branch_link_to_a_missing_path_is_refused() {
    // Written as a link or bare in prose, to a file or to a directory.
    let document = format!(
        "[moved]({0}/blob/main/crates/a/tests/gone.rs)\nSee {0}/tree/main/crates/gone.\n",
        links::REPOSITORY
    );
    let refusal = checked("README.md", &document, &["crates/a"]).unwrap_err();
    let lines: Vec<&str> = refusal.lines().collect();
    for expected in [
        format!(
            "README.md:1: {}/blob/main/crates/a/tests/gone.rs",
            links::REPOSITORY
        ),
        // The sentence's full stop is not part of the link.
        format!("README.md:2: {}/tree/main/crates/gone", links::REPOSITORY),
    ] {
        assert!(lines.contains(&expected.as_str()), "{refusal}");
    }
}

#[test]
fn a_link_climbing_above_the_repository_is_refused() {
    let refusal = checked("README.md", "[up](../outside.md)\n", &["../outside.md"]).unwrap_err();
    assert!(refusal.contains("README.md:1: ../outside.md"), "{refusal}");
}

#[test]
fn an_api_reference_link_names_a_workspace_crate() {
    let refusal = checked(
        "docs/book/rust/page.md",
        "[gone](../../doc/zetesis_gone/index.html) and [held](../../doc/zetesis_a/struct.A.html)\n",
        &["crates/zetesis-a/Cargo.toml"],
    )
    .unwrap_err();
    assert!(refusal.contains("zetesis_gone"), "{refusal}");
    assert!(!refusal.contains("zetesis_a"), "{refusal}");
}

#[test]
fn a_dated_record_keeps_the_links_it_recorded() {
    let record = format!(
        "# A measurement\n\n{}\n\n[run](../gone.rs)\n",
        invocations::RECORD
    );
    checked("docs/record.md", &record, &[]).unwrap();
}

#[test]
fn links_inside_code_are_not_checked() {
    let document = "```md\n[fenced](gone.md)\n```\n\nInline `[span](gone.md)` code.\n";
    checked("README.md", document, &[]).unwrap();
}

#[test]
fn links_to_other_sites_are_not_checked() {
    let document = "[site](https://example.org/gone) and [mail](mailto:someone@example.org)\n";
    checked("README.md", document, &[]).unwrap();
}

#[test]
fn a_link_within_the_document_is_not_checked() {
    checked("README.md", "[below](#a-section)\n", &[]).unwrap();
}

#[test]
fn links_pinned_to_a_commit_are_not_checked() {
    let document = format!("[pinned]({}/blob/0123abc/gone.rs)\n", links::REPOSITORY);
    checked("README.md", &document, &[]).unwrap();
}
