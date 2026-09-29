//! External integrity contracts for the independently readable selected corpus.

use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use zetesis_validation::curated::{self, Corpus, Error, Limits, Resource};

fn upstream_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../validation/upstream/clingo-5.8.2")
}
fn curated_root() -> PathBuf {
    upstream_root().join("curated")
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
fn full_models_match_independently_captured_reference_envelopes() {
    let records: Vec<Value> =
        serde_json::from_str(include_str!("../fixtures/selected/reports.json")).unwrap();
    let corpus = verified();
    assert_eq!(records.len(), corpus.cases().len());
    let mut occurrences = 0;
    for case in corpus.cases() {
        let matching: Vec<_> = records
            .iter()
            .filter(|record| record["id"] == case.id())
            .collect();
        assert_eq!(matching.len(), 1, "one reference envelope per source");
        let reference = &matching[0]["reference"];
        assert_eq!(reference["Models"]["More"], "no");
        let mut expected: Vec<Vec<String>> = reference["Call"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|call| call["Witnesses"].as_array().into_iter().flatten())
            .map(|witness| serde_json::from_value(witness["Value"].clone()).unwrap())
            .collect();
        for model in &mut expected {
            model.sort();
        }
        expected.sort();
        let mut actual = case.contract().full_models().to_vec();
        for model in &mut actual {
            model.sort();
        }
        actual.sort();
        assert_eq!(actual, expected, "{}", case.id());
        occurrences += actual.len();
    }
    assert_eq!(occurrences, 73);
}

#[test]
fn provenance_identifies_the_public_originals() {
    let corpus = verified();
    assert_eq!(corpus.origins().count(), 3);
    for origin in corpus.origins() {
        assert_eq!(
            origin.url(),
            format!(
                "https://github.com/potassco/clingo/blob/{}/{}",
                curated::UPSTREAM_REVISION,
                origin.path()
            )
        );
        assert_eq!(origin.sha256().len(), 64);
        assert!(
            origin
                .copyright_notice()
                .contains("Copyright 2017 Roland Kaminski")
        );
        assert!(
            origin
                .copyright_notice()
                .contains("Permission is hereby granted")
        );
        assert!(
            origin
                .copyright_notice()
                .contains("THE SOFTWARE IS PROVIDED")
        );
    }
    for case in corpus.cases() {
        assert_eq!(case.license(), "MIT");
        let provenance = case.provenance();
        assert!(
            corpus
                .origins()
                .any(|origin| origin.path() == provenance.source_file())
        );
        assert_eq!(provenance.bytes().len(), provenance.assertion().len());
        assert_eq!(
            *provenance.lines().end() - *provenance.lines().start(),
            provenance
                .assertion()
                .bytes()
                .filter(|byte| *byte == b'\n')
                .count()
        );
        assert!(case.id().ends_with(&format!(
            "/{}/{:02}",
            provenance.section(),
            provenance.assertion_ordinal()
        )));
    }
}

#[test]
fn license_bytes_obey_the_inclusive_limit() {
    let count = fs::read(curated_root().join("LICENSE.md")).unwrap().len();
    assert!(
        curated::open(
            &curated_root(),
            Limits {
                license_bytes: count,
                ..Limits::default()
            }
        )
        .is_ok()
    );
    assert_limit(
        &curated::open(
            &curated_root(),
            Limits {
                license_bytes: count - 1,
                ..Limits::default()
            },
        )
        .unwrap_err(),
        Resource::LicenseBytes,
        count,
        count - 1,
    );
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

#[test]
fn full_model_limits_preserve_every_recorded_occurrence() {
    let count = verified()
        .cases()
        .iter()
        .map(|case| case.contract().full_models().len())
        .sum::<usize>();
    assert!(
        curated::open(
            &curated_root(),
            Limits {
                models: count,
                ..Limits::default()
            }
        )
        .is_ok()
    );
    assert_limit(
        &curated::open(
            &curated_root(),
            Limits {
                models: count - 1,
                ..Limits::default()
            },
        )
        .unwrap_err(),
        Resource::Models,
        count,
        count - 1,
    );
}

#[test]
fn full_atom_limits_count_occurrences_across_models() {
    let corpus = verified();
    let count = corpus
        .cases()
        .iter()
        .flat_map(|case| case.contract().full_models())
        .map(Vec::len)
        .sum::<usize>();
    assert!(
        curated::open(
            &curated_root(),
            Limits {
                atoms: count,
                ..Limits::default()
            }
        )
        .is_ok()
    );
    assert_limit(
        &curated::open(
            &curated_root(),
            Limits {
                atoms: count - 1,
                ..Limits::default()
            },
        )
        .unwrap_err(),
        Resource::Atoms,
        count,
        count - 1,
    );
}
