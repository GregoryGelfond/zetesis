//! Integrity and reported-display contracts for the self-contained correctness examples.

use std::fs;

use crate::support::files::copy;
use serde_json::{Value, json};
use zetesis_test_support::repository::{
    correctness as clean_root, kr_domains as originals, root as repo,
};
use zetesis_validation::answers::{self, ReportedAnswers};
use zetesis_validation::examples::{
    self, Contract, ContractMismatch, Corpus, Error, Family, Limits, Resource, Satisfiability,
};

fn verified() -> Corpus {
    examples::load(&clean_root(), Limits::default()).unwrap()
}
fn isolated() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    for name in ["manifest.json", "LICENSE"] {
        copy(&clean_root().join(name), &directory.path().join(name));
    }
    for source in verified().files() {
        copy(
            &clean_root().join(source.path()),
            &directory.path().join(source.path()),
        );
    }
    directory
}
fn answer(displays: &[&[&str]], cost: Option<&[i64]>) -> ReportedAnswers {
    let mut witnesses = Vec::new();
    if cost.is_some() {
        witnesses.push(json!({"Value":displays[0],"Costs":cost}));
    }
    witnesses.extend(displays.iter().map(|display| match cost {
        Some(cost) => json!({"Value":display,"Costs":cost}),
        None => json!({"Value":display}),
    }));
    let mut summary = json!({"More":"no","Number":witnesses.len()});
    if let Some(cost) = cost {
        summary["Optimum"] = json!("yes");
        summary["Optimal"] = json!(displays.len());
        summary["Costs"] = json!(cost);
    }
    let report = json!({
        "Result":if displays.is_empty() { "UNSATISFIABLE" } else if cost.is_some() { "OPTIMUM FOUND" } else { "SATISFIABLE" },
        "Models":summary,
        "Call":[{"Witnesses":witnesses}]
    });
    answers::clingo_json(
        &serde_json::to_vec(&report).unwrap(),
        answers::Limits::default(),
    )
    .unwrap()
}
fn contract(mut changes: Value) -> Contract {
    let mut base = json!({"satisfiability":"sat","family":"all","model_count":null,"cost":null,"witnesses":[],"required_symbols":[],"notes":[]});
    for (key, value) in changes.as_object_mut().unwrap() {
        base[key] = value.take();
    }
    serde_json::from_value(base).unwrap()
}

#[test]
fn clean_loading_needs_no_original_tree() {
    let directory = isolated();
    let corpus = examples::load(directory.path(), Limits::default()).unwrap();
    assert_eq!(corpus.cases().len(), 94);
    assert!(!directory.path().join("validation").exists());
}

#[test]
fn all_sources_follow_the_recorded_derivation() {
    examples::verify_originals(&verified(), &originals(), Limits::default()).unwrap();
}

#[test]
fn cleaned_sources_have_no_annotation_lines() {
    for source in verified().files() {
        assert!(
            !source
                .source()
                .lines()
                .any(|line| line.trim_start().starts_with("% @")),
            "{}",
            source.path()
        );
    }
}

#[test]
fn original_inventory_retains_every_case() {
    let old: Value =
        serde_json::from_slice(&fs::read(repo().join("validation/corpus/manifest.json")).unwrap())
            .unwrap();
    let corpus = verified();
    assert_eq!(corpus.files().len(), 108);
    for (case, legacy) in corpus.cases().iter().zip(old["cases"].as_array().unwrap()) {
        assert_eq!(case.path(), legacy["path"]);
        assert_eq!(case.original_sha256(), legacy["sha256"]);
        assert_eq!(
            case.transitive_source_paths(),
            serde_json::from_value::<Vec<String>>(legacy["transitive_source_paths"].clone())
                .unwrap()
        );
    }
}

#[test]
fn every_original_annotation_has_deletion_coordinates() {
    let old: Value =
        serde_json::from_slice(&fs::read(repo().join("validation/corpus/manifest.json")).unwrap())
            .unwrap();
    let corpus = verified();
    for legacy in old["cases"].as_array().unwrap() {
        let source = corpus
            .files()
            .iter()
            .find(|source| source.path() == legacy["path"])
            .unwrap();
        let annotations = legacy["contracts"].as_array().unwrap();
        assert_eq!(source.removed_annotations().len(), annotations.len());
        for (actual, original) in source.removed_annotations().iter().zip(annotations) {
            assert_eq!(actual.line() as u64, original["line"].as_u64().unwrap());
            assert_eq!(actual.source(), original["source"]);
            let (tag, arguments) = actual
                .source()
                .trim_start()
                .strip_prefix("% @")
                .unwrap()
                .split_once(char::is_whitespace)
                .unwrap();
            assert_eq!(tag, original["tag"]);
            assert_eq!(arguments.trim(), original["arguments"]);
        }
    }
}

#[test]
fn original_satisfiability_partition_is_preserved() {
    let corpus = verified();
    assert_eq!(
        corpus
            .cases()
            .iter()
            .filter(|case| case.contract().satisfiability() == Satisfiability::Sat)
            .count(),
        79
    );
    assert_eq!(
        corpus
            .cases()
            .iter()
            .filter(|case| case.contract().satisfiability() == Satisfiability::Unsat)
            .count(),
        15
    );
}

#[test]
fn original_optimal_family_scope_is_preserved() {
    let corpus = verified();
    assert_eq!(
        corpus
            .cases()
            .iter()
            .filter(|case| case.contract().family() == Family::Optimal)
            .count(),
        72
    );
}

#[test]
fn manifest_tampering_is_refused() {
    let directory = isolated();
    fs::write(directory.path().join("manifest.json"), b"{}").unwrap();
    assert!(
        matches!(examples::load(directory.path(), Limits::default()), Err(Error::Digest { path, .. }) if path == "manifest.json")
    );
}

#[test]
fn cleaned_source_tampering_is_refused() {
    let directory = isolated();
    let path = verified().files()[0].path().to_owned();
    fs::write(directory.path().join(&path), b"corrupted.\n").unwrap();
    assert!(
        matches!(examples::load(directory.path(), Limits::default()), Err(Error::Digest { path: actual, .. }) if actual == path)
    );
}

#[test]
fn license_tampering_is_refused() {
    let directory = isolated();
    fs::write(directory.path().join("LICENSE"), b"changed").unwrap();
    assert!(
        matches!(examples::load(directory.path(), Limits::default()), Err(Error::Digest { path, .. }) if path == "LICENSE")
    );
}

#[cfg(unix)]
#[test]
fn source_symlinks_cannot_escape_the_corpus() {
    let directory = isolated();
    let outside = tempfile::NamedTempFile::new().unwrap();
    let path = directory.path().join(verified().files()[0].path());
    fs::remove_file(&path).unwrap();
    std::os::unix::fs::symlink(outside.path(), &path).unwrap();
    assert!(matches!(
        examples::load(directory.path(), Limits::default()),
        Err(Error::Path(_))
    ));
}

#[test]
fn original_source_tampering_is_refused() {
    let directory = tempfile::tempdir().unwrap();
    copy(
        &originals().join("LICENSE"),
        &directory.path().join("LICENSE"),
    );
    let corpus = verified();
    let first = corpus.files()[0].path();
    let path = directory.path().join(first);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, b"corrupted.\n").unwrap();
    assert!(
        matches!(examples::verify_originals(&corpus, directory.path(), Limits::default()), Err(Error::Digest { path, .. }) if path == first)
    );
}

#[test]
fn admission_ceilings_are_inclusive() {
    let corpus = verified();
    let source_bytes = corpus
        .files()
        .iter()
        .map(|source| source.source().len())
        .chain([fs::read(clean_root().join("LICENSE")).unwrap().len()])
        .max()
        .unwrap();
    let limits = Limits {
        manifest_bytes: fs::read(clean_root().join("manifest.json")).unwrap().len(),
        source_bytes,
        total_source_bytes: corpus
            .files()
            .iter()
            .map(|source| source.source().len())
            .sum(),
        files: 108,
        cases: 94,
        contract_symbols: corpus
            .cases()
            .iter()
            .map(|case| {
                case.contract()
                    .witnesses()
                    .iter()
                    .map(Vec::len)
                    .sum::<usize>()
                    + case.contract().required_symbols().len()
            })
            .sum(),
    };
    assert!(examples::load(&clean_root(), limits).is_ok());
    for resource in [
        Resource::ManifestBytes,
        Resource::SourceBytes,
        Resource::TotalSourceBytes,
        Resource::Files,
        Resource::Cases,
        Resource::ContractSymbols,
    ] {
        let mut below = limits;
        let ceiling = match resource {
            Resource::ManifestBytes => &mut below.manifest_bytes,
            Resource::SourceBytes => &mut below.source_bytes,
            Resource::TotalSourceBytes => &mut below.total_source_bytes,
            Resource::Files => &mut below.files,
            Resource::Cases => &mut below.cases,
            Resource::ContractSymbols => &mut below.contract_symbols,
        };
        *ceiling -= 1;
        assert!(
            matches!(examples::load(&clean_root(), below), Err(Error::Limit {resource: actual,..}) if actual == resource),
            "{resource:?}"
        );
    }
}

#[test]
fn count_contracts_retain_equal_display_multiplicity() {
    let expected = contract(json!({"model_count":2}));
    assert!(expected.check(&answer(&[&["a"], &["a"]], None)).is_ok());
    assert_eq!(
        expected.check(&answer(&[&["a"]], None)),
        Err(ContractMismatch::Count {
            expected: 2,
            actual: 1
        })
    );
}

#[test]
fn witness_contracts_retain_repeated_printed_symbols() {
    let expected = contract(json!({"witnesses":[["a","a"]]}));
    assert!(expected.check(&answer(&[&["a", "a"]], None)).is_ok());
    assert_eq!(
        expected.check(&answer(&[&["a"]], None)),
        Err(ContractMismatch::Witness { index: 0 })
    );
}

#[test]
fn witness_order_is_canonicalized_by_report_parsing() {
    let expected = contract(json!({"witnesses":[["a","b"]]}));
    assert!(expected.check(&answer(&[&["b", "a"]], None)).is_ok());
}

#[test]
fn required_symbols_cover_every_selected_display() {
    let expected = contract(json!({"required_symbols":["a"]}));
    assert!(
        expected
            .check(&answer(&[&["a"], &["a", "b"]], None))
            .is_ok()
    );
    assert_eq!(
        expected.check(&answer(&[&["a"], &["b"]], None)),
        Err(ContractMismatch::RequiredSymbol { index: 0 })
    );
}

#[test]
fn optimal_contracts_refuse_ordinary_reports() {
    let expected = contract(json!({"family":"optimal","cost":[2]}));
    assert_eq!(
        expected.check(&answer(&[&["a"]], None)),
        Err(ContractMismatch::Family)
    );
}

#[test]
fn ordinary_contracts_refuse_optimal_reports() {
    assert_eq!(
        contract(json!({})).check(&answer(&[&["a"]], Some(&[2]))),
        Err(ContractMismatch::Family)
    );
}

#[test]
fn optimum_costs_keep_priority_order() {
    let expected = contract(json!({"family":"optimal","cost":[2,3]}));
    assert!(expected.check(&answer(&[&["a"]], Some(&[2, 3]))).is_ok());
    assert_eq!(
        expected.check(&answer(&[&["a"]], Some(&[3, 2]))),
        Err(ContractMismatch::Cost)
    );
}

#[test]
fn optimum_counts_exclude_discovery_replay() {
    let expected = contract(json!({"family":"optimal","model_count":2}));
    assert!(
        expected
            .check(&answer(&[&["a"], &["a"]], Some(&[2])))
            .is_ok()
    );
}

#[test]
fn unsatisfiable_contracts_require_no_witness() {
    let expected = contract(json!({"satisfiability":"unsat"}));
    assert!(expected.check(&answer(&[], None)).is_ok());
    assert_eq!(
        expected.check(&answer(&[&[]], None)),
        Err(ContractMismatch::Satisfiability)
    );
}

#[test]
fn contradictory_contracts_are_refused() {
    let expected = contract(json!({"satisfiability":"unsat","witnesses":[[]]}));
    assert_eq!(
        expected.check(&answer(&[], None)),
        Err(ContractMismatch::InvalidContract)
    );
}

#[test]
fn notes_do_not_impose_hidden_assertions() {
    assert!(
        contract(json!({"notes":["an explanatory note"]}))
            .check(&answer(&[&[]], None))
            .is_ok()
    );
}

#[test]
fn exposed_coordinates_reconstruct_clean_sources() {
    for source in verified().files() {
        let original = fs::read_to_string(originals().join(source.path())).unwrap();
        let mut cleaned = String::new();
        let mut retained_start = 0;
        for annotation in source.removed_annotations() {
            cleaned.push_str(&original[retained_start..annotation.start_byte()]);
            retained_start = annotation.end_byte();
        }
        cleaned.push_str(&original[retained_start..]);
        let mut edited = String::new();
        retained_start = 0;
        for edit in source.edits() {
            assert_eq!(&cleaned[edit.start_byte()..edit.end_byte()], edit.before());
            edited.push_str(&cleaned[retained_start..edit.start_byte()]);
            edited.push_str(edit.after());
            retained_start = edit.end_byte();
        }
        edited.push_str(&cleaned[retained_start..]);
        assert_eq!(edited, source.source(), "{}", source.path());
    }
}

#[test]
fn exposed_case_includes_match_source_dependencies() {
    let corpus = verified();
    for case in corpus.cases() {
        let source = corpus
            .files()
            .iter()
            .find(|source| source.path() == case.path())
            .unwrap();
        assert_eq!(
            case.includes(),
            source
                .includes()
                .iter()
                .map(examples::Include::path)
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn exposed_provenance_preserves_upstream_identity() {
    assert_eq!(
        verified().upstream(),
        "https://github.com/GregoryGelfond/kr-domains"
    );
}

#[test]
fn contract_notes_preserve_explanatory_spelling() {
    let text = "A note may describe a witness without requiring it.";
    assert_eq!(contract(json!({"notes":[text]})).notes(), [text]);
}

#[test]
fn missing_original_roots_retain_the_input_path() {
    let directory = tempfile::tempdir().unwrap();
    let absent = directory.path().join("absent");
    let error = examples::verify_originals(&verified(), &absent, Limits::default()).unwrap_err();
    assert!(
        matches!(error, Error::Io {path, source} if path == absent && source.kind() == std::io::ErrorKind::NotFound)
    );
}

#[test]
fn original_license_identity_is_independently_checked() {
    let directory = tempfile::tempdir().unwrap();
    fs::write(directory.path().join("LICENSE"), b"different license").unwrap();
    assert!(
        matches!(examples::verify_originals(&verified(), directory.path(), Limits::default()),
        Err(Error::Digest {path, ..}) if path == "LICENSE")
    );
}

#[test]
fn missing_original_sources_retain_the_input_path() {
    let directory = tempfile::tempdir().unwrap();
    copy(
        &originals().join("LICENSE"),
        &directory.path().join("LICENSE"),
    );
    let corpus = verified();
    let first = directory
        .path()
        .canonicalize()
        .unwrap()
        .join(corpus.files()[0].path());
    assert!(
        matches!(examples::verify_originals(&corpus, directory.path(), Limits::default()),
        Err(Error::Io {path, source}) if path == first && source.kind() == std::io::ErrorKind::NotFound)
    );
}
