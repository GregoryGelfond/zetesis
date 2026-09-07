//! External integrity contracts for the independently readable selected corpus.

use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use zetesis_validation::curated::{self, Corpus, Error, Limits, Resource};

fn legacy() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../validation/upstream/clingo-5.8.2")
}
fn curated_root() -> PathBuf {
    legacy().join("curated")
}
fn verified() -> Corpus {
    curated::open(&curated_root(), Limits::default()).unwrap()
}
fn copy_file(source: &Path, destination: &Path) {
    fs::create_dir_all(destination.parent().unwrap()).unwrap();
    fs::copy(source, destination).unwrap();
}
fn isolated() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    for name in ["manifest.json", "LICENSE.md"] {
        copy_file(&curated_root().join(name), &directory.path().join(name));
    }
    for case in verified().cases() {
        copy_file(
            &curated_root().join(case.path()),
            &directory.path().join(case.path()),
        );
    }
    directory
}
fn copied_legacy() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    copy_file(
        &legacy().join("cases.jsonl"),
        &directory.path().join("cases.jsonl"),
    );
    copy_file(
        &legacy().join("originals/LICENSE.md"),
        &directory.path().join("originals/LICENSE.md"),
    );
    for origin in verified().origins() {
        let relative = Path::new("originals").join(origin.path());
        copy_file(&legacy().join(&relative), &directory.path().join(relative));
    }
    directory
}
fn assert_limit(error: &Error, resource: Resource, observed: usize, limit: usize) {
    assert!(
        matches!(error, Error::Limit { resource: actual, observed: amount, limit: allowance }
        if *actual == resource && *amount == observed as u128 && *allowance == limit)
    );
}

#[test]
fn verification_needs_only_curated_data() {
    let directory = isolated();
    let corpus = curated::open(directory.path(), Limits::default()).unwrap();
    assert_eq!(corpus.cases().len(), 24);
    assert!(!directory.path().join("originals").exists());
    assert!(!directory.path().join("cases.jsonl").exists());
}

#[test]
fn retained_sources_match_every_legacy_byte() {
    let corpus = verified();
    let original = fs::read_to_string(legacy().join("cases.jsonl")).unwrap();
    for (case, line) in corpus.cases().iter().zip(original.lines()) {
        let old: Value = serde_json::from_str(line).unwrap();
        assert_eq!(case.id(), old["id"].as_str().unwrap());
        assert_eq!(
            case.source().as_bytes(),
            old["source"].as_str().unwrap().as_bytes()
        );
        assert_eq!(case.source_sha256(), old["source_sha256"].as_str().unwrap());
    }
}

#[test]
fn complete_model_contracts_retain_all_seventy_three_occurrences() {
    let corpus = verified();
    let original = fs::read_to_string(legacy().join("cases.jsonl")).unwrap();
    for (case, line) in corpus.cases().iter().zip(original.lines()) {
        let old: Value = serde_json::from_str(line).unwrap();
        let models: Vec<Vec<String>> = serde_json::from_value(old["models"].clone()).unwrap();
        assert_eq!(case.contract().full_models(), models);
    }
    assert_eq!(
        corpus
            .cases()
            .iter()
            .map(|case| case.contract().full_models().len())
            .sum::<usize>(),
        73
    );
}

#[test]
fn helper_views_retain_the_original_projection_contract() {
    let corpus = verified();
    let original = fs::read_to_string(legacy().join("cases.jsonl")).unwrap();
    for (case, line) in corpus.cases().iter().zip(original.lines()) {
        let old: Value = serde_json::from_str(line).unwrap();
        let prefixes: Vec<String> = serde_json::from_value(old["filters"].clone()).unwrap();
        let models: Vec<Vec<String>> =
            serde_json::from_value(old["expected_helper_models"].clone()).unwrap();
        assert_eq!(case.contract().prefixes(), prefixes);
        assert_eq!(case.contract().helper_models(), models);
    }
}

#[test]
fn provenance_resolves_to_exact_original_assertions() {
    for case in verified().cases() {
        let provenance = case.provenance();
        let original =
            fs::read_to_string(legacy().join("originals").join(provenance.source_file())).unwrap();
        assert_eq!(&original[provenance.bytes()], provenance.assertion());
        assert_eq!(
            original[..provenance.bytes().start]
                .bytes()
                .filter(|&byte| byte == b'\n')
                .count()
                + 1,
            *provenance.lines().start()
        );
        assert_eq!(
            original[..provenance.bytes().end]
                .bytes()
                .filter(|&byte| byte == b'\n')
                .count()
                + 1,
            *provenance.lines().end()
        );
        assert!(case.id().ends_with(&format!(
            "/{}/{:02}",
            provenance.section(),
            provenance.assertion_ordinal()
        )));
        assert!(provenance.helper_arguments().len() <= 1);
        assert!(!provenance.expected_helper_output().is_empty());
    }
}

#[test]
fn copyright_notices_remain_verbatim() {
    let corpus = verified();
    for origin in corpus.origins() {
        let original = fs::read_to_string(legacy().join("originals").join(origin.path())).unwrap();
        let notice = &original[..original.find("// }}}").unwrap() + 6];
        assert_eq!(origin.copyright_notice(), notice);
        assert_eq!(
            origin.url(),
            format!(
                "https://github.com/potassco/clingo/blob/{}/{}",
                curated::UPSTREAM_REVISION,
                origin.path()
            )
        );
        assert_eq!(origin.sha256().len(), 64);
    }
    assert!(corpus.cases().iter().all(|case| case.license() == "MIT"));
}

#[test]
fn changed_files_do_not_rewrite_an_existing_corpus() {
    let directory = isolated();
    let corpus = curated::open(directory.path(), Limits::default()).unwrap();
    let first = &corpus.cases()[0];
    let before = first.source().to_owned();
    fs::write(directory.path().join(first.path()), b"changed.").unwrap();
    assert_eq!(first.source(), before);
}

#[test]
fn changed_source_bytes_fail_reverification() {
    let directory = isolated();
    let first = verified().cases()[0].path().to_owned();
    fs::write(directory.path().join(&first), b"changed.").unwrap();
    assert!(
        matches!(curated::open(directory.path(), Limits::default()), Err(Error::Digest { path, .. }) if path == first)
    );
}

#[test]
fn changed_manifest_whitespace_fails_the_seal() {
    let directory = isolated();
    let path = directory.path().join("manifest.json");
    let mut bytes = fs::read(&path).unwrap();
    bytes.push(b'\n');
    fs::write(path, bytes).unwrap();
    assert!(
        matches!(curated::open(directory.path(), Limits::default()), Err(Error::Digest { path, .. }) if path == "manifest.json")
    );
}

#[test]
fn changed_license_bytes_fail_the_seal() {
    let directory = isolated();
    fs::write(directory.path().join("LICENSE.md"), b"changed").unwrap();
    assert!(
        matches!(curated::open(directory.path(), Limits::default()), Err(Error::Digest { path, .. }) if path == "LICENSE.md")
    );
}

#[test]
fn serialized_manifest_bytes_have_an_inclusive_limit() {
    let length = fs::read(curated_root().join("manifest.json"))
        .unwrap()
        .len();
    let limits = Limits {
        manifest_bytes: length,
        ..Limits::default()
    };
    assert!(curated::open(&curated_root(), limits).is_ok());
    assert_limit(
        &curated::open(
            &curated_root(),
            Limits {
                manifest_bytes: length - 1,
                ..limits
            },
        )
        .unwrap_err(),
        Resource::ManifestBytes,
        length,
        length - 1,
    );
}

#[test]
fn the_largest_source_fits_its_exact_limit() {
    let length = verified()
        .cases()
        .iter()
        .map(|case| case.source().len())
        .max()
        .unwrap();
    let limits = Limits {
        source_bytes: length,
        ..Limits::default()
    };
    assert!(curated::open(&curated_root(), limits).is_ok());
    assert_limit(
        &curated::open(
            &curated_root(),
            Limits {
                source_bytes: length - 1,
                ..limits
            },
        )
        .unwrap_err(),
        Resource::SourceBytes,
        length,
        length - 1,
    );
}

#[test]
fn retained_source_bytes_have_a_cumulative_limit() {
    let length = verified()
        .cases()
        .iter()
        .map(|case| case.source().len())
        .sum::<usize>();
    let limits = Limits {
        total_source_bytes: length,
        ..Limits::default()
    };
    assert!(curated::open(&curated_root(), limits).is_ok());
    assert_limit(
        &curated::open(
            &curated_root(),
            Limits {
                total_source_bytes: length - 1,
                ..limits
            },
        )
        .unwrap_err(),
        Resource::TotalSourceBytes,
        length,
        length - 1,
    );
}

#[test]
fn an_insufficient_case_limit_cannot_narrow_the_target() {
    assert_limit(
        &curated::open(
            &curated_root(),
            Limits {
                cases: 23,
                ..Limits::default()
            },
        )
        .unwrap_err(),
        Resource::Cases,
        24,
        23,
    );
}

#[test]
fn import_reproduces_the_sealed_curated_data() {
    let parent = tempfile::tempdir().unwrap();
    let destination = parent.path().join("curated");
    let corpus = curated::import_legacy(&legacy(), &destination, Limits::default()).unwrap();
    assert_eq!(corpus.root(), destination.canonicalize().unwrap());
    for name in ["manifest.json", "LICENSE.md"] {
        assert_eq!(
            fs::read(destination.join(name)).unwrap(),
            fs::read(curated_root().join(name)).unwrap()
        );
    }
    for case in corpus.cases() {
        assert_eq!(
            fs::read(destination.join(case.path())).unwrap(),
            fs::read(curated_root().join(case.path())).unwrap()
        );
    }
}

#[test]
fn import_never_replaces_an_existing_directory() {
    let parent = tempfile::tempdir().unwrap();
    fs::write(parent.path().join("retained"), b"parent data").unwrap();
    assert!(matches!(
        curated::import_legacy(&legacy(), parent.path(), Limits::default()),
        Err(Error::DestinationExists(_))
    ));
    assert_eq!(
        fs::read(parent.path().join("retained")).unwrap(),
        b"parent data"
    );
}

#[test]
fn import_never_replaces_an_existing_file() {
    let parent = tempfile::tempdir().unwrap();
    let destination = parent.path().join("retained");
    fs::write(&destination, b"existing data").unwrap();
    assert!(matches!(
        curated::import_legacy(&legacy(), &destination, Limits::default()),
        Err(Error::DestinationExists(_))
    ));
    assert_eq!(fs::read(destination).unwrap(), b"existing data");
}

#[test]
fn import_never_replaces_a_hard_link_to_its_catalog() {
    let parent = tempfile::tempdir().unwrap();
    let catalog = parent.path().join("catalog");
    fs::copy(legacy().join("cases.jsonl"), &catalog).unwrap();
    let destination = parent.path().join("alias");
    fs::hard_link(&catalog, &destination).unwrap();
    let before = fs::read(&catalog).unwrap();
    assert!(matches!(
        curated::import_legacy(&legacy(), &destination, Limits::default()),
        Err(Error::DestinationExists(_))
    ));
    assert_eq!(fs::read(catalog).unwrap(), before);
}

#[test]
fn import_cannot_publish_inside_preserved_originals() {
    let directory = copied_legacy();
    let destination = directory.path().join("originals/curated");
    assert!(matches!(
        curated::import_legacy(directory.path(), &destination, Limits::default()),
        Err(Error::Path(_))
    ));
    assert!(!destination.exists());
}

#[test]
fn a_damaged_original_fails_before_publication() {
    let directory = copied_legacy();
    let origin = verified().origins().next().unwrap().path().to_owned();
    fs::write(directory.path().join("originals").join(origin), b"damaged").unwrap();
    let destination = directory.path().join("curated");
    assert!(matches!(
        curated::import_legacy(directory.path(), &destination, Limits::default()),
        Err(Error::Digest { .. })
    ));
    assert!(!destination.exists());
}

#[test]
fn a_changed_assertion_coordinate_fails_before_publication() {
    let directory = copied_legacy();
    let catalog = fs::read_to_string(directory.path().join("cases.jsonl")).unwrap();
    let mut lines = catalog.lines();
    let mut first: Value = serde_json::from_str(lines.next().unwrap()).unwrap();
    first["byte_start"] = Value::from(0);
    let mut changed = serde_json::to_string(&first).unwrap();
    changed.push('\n');
    for line in lines {
        changed.push_str(line);
        changed.push('\n');
    }
    fs::write(directory.path().join("cases.jsonl"), changed).unwrap();
    let destination = directory.path().join("curated");
    assert!(matches!(
        curated::import_legacy(directory.path(), &destination, Limits::default()),
        Err(Error::Contract(_))
    ));
    assert!(!destination.exists());
}

#[test]
fn native_admission_labels_do_not_change_the_curated_target() {
    let directory = copied_legacy();
    let catalog = fs::read_to_string(directory.path().join("cases.jsonl")).unwrap();
    let mut changed = String::new();
    for line in catalog.lines() {
        let mut case: Value = serde_json::from_str(line).unwrap();
        case["native"] = Value::from("policy-owned-elsewhere");
        changed.push_str(&serde_json::to_string(&case).unwrap());
        changed.push('\n');
    }
    fs::write(directory.path().join("cases.jsonl"), changed).unwrap();
    let destination = directory.path().join("curated");
    curated::import_legacy(directory.path(), &destination, Limits::default()).unwrap();
    assert_eq!(
        fs::read(destination.join("manifest.json")).unwrap(),
        fs::read(curated_root().join("manifest.json")).unwrap()
    );
}

#[cfg(unix)]
#[test]
fn a_source_symlink_cannot_escape_the_corpus_root() {
    let directory = isolated();
    let external = tempfile::tempdir().unwrap();
    let first = verified().cases()[0].path().to_owned();
    let target = external.path().join("source.lp");
    fs::copy(directory.path().join(&first), &target).unwrap();
    fs::remove_file(directory.path().join(&first)).unwrap();
    std::os::unix::fs::symlink(target, directory.path().join(first)).unwrap();
    assert!(matches!(
        curated::open(directory.path(), Limits::default()),
        Err(Error::Path(_))
    ));
}

#[cfg(unix)]
#[test]
fn import_never_replaces_a_dangling_symlink() {
    let parent = tempfile::tempdir().unwrap();
    let destination = parent.path().join("dangling");
    std::os::unix::fs::symlink("missing", &destination).unwrap();
    assert!(matches!(
        curated::import_legacy(&legacy(), &destination, Limits::default()),
        Err(Error::DestinationExists(_))
    ));
    assert_eq!(fs::read_link(destination).unwrap(), Path::new("missing"));
}

#[test]
fn the_verify_command_reports_integrity_without_claiming_a_solver_run() {
    let output = Command::new(env!("CARGO_BIN_EXE_zetesis-corpus"))
        .arg("verify")
        .arg(curated_root())
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["schema"], 1);
    assert_eq!(report["integrity"], "verified");
    assert_eq!(report["semantic_solver_run"], false);
    assert_eq!(report["manifest_sha256"], curated::MANIFEST_SHA256);
    assert_eq!(report["cases"], 24);
    assert_eq!(report["full_model_occurrences"], 73);
}

#[test]
fn the_verify_command_does_not_publish_success_for_a_missing_corpus() {
    let directory = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_zetesis-corpus"))
        .arg("verify")
        .arg(directory.path())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(!output.stderr.is_empty());
}
