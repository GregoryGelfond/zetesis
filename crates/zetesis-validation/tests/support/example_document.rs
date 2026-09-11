//! Structural contracts remain enforced independently of the outer manifest seal.

use super::*;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/kr-domains")
        .canonicalize()
        .unwrap()
}
fn document() -> Document {
    serde_json::from_slice(&std::fs::read(root().join("manifest.json")).unwrap()).unwrap()
}
fn invalid(change: impl FnOnce(&mut Document)) {
    let mut document = document();
    change(&mut document);
    assert!(matches!(
        validate_cases(&document, &root(), Limits::default()),
        Err(Error::Contract(_))
    ));
}

#[test]
fn target_metadata_cannot_redefine_the_pinned_population() {
    for field in [
        "schema_version",
        "upstream",
        "revision",
        "license",
        "copyright",
        "original_manifest_sha256",
    ] {
        let mut value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(root().join("manifest.json")).unwrap()).unwrap();
        value[field] = if field == "schema_version" {
            serde_json::json!(99)
        } else {
            serde_json::json!("changed")
        };
        let document: Document = serde_json::from_value(value).unwrap();
        assert!(
            matches!(
                validate_header(&document, Limits::default()),
                Err(Error::Contract(_))
            ),
            "{field}"
        );
    }
}

#[test]
fn missing_entry_sources_are_refused() {
    invalid(|document| {
        document.cases[0].path = "missing.lp".into();
    });
}

#[test]
fn duplicate_entry_sources_are_refused() {
    invalid(|document| {
        let first = document.cases[0].path.clone();
        document.cases[1].path = first;
    });
}

#[test]
fn entry_hashes_must_match_the_source_table() {
    invalid(|document| {
        document.cases[0].source_sha256 = "different bytes".into();
    });
}

#[test]
fn direct_include_records_cannot_be_omitted() {
    invalid(|document| {
        document.cases[0].includes.clear();
    });
}

#[test]
fn dependency_closures_cannot_omit_the_entry() {
    invalid(|document| {
        let entry = document.cases[0].path.clone();
        document.cases[0]
            .transitive_source_paths
            .retain(|path| path != &entry);
    });
}

#[test]
fn missing_include_targets_are_refused() {
    invalid(|document| {
        let target = document.cases[0].includes[0].clone();
        document.files.retain(|source| source.path != target);
    });
}

#[test]
fn recorded_include_paths_must_match_their_spelling() {
    invalid(|document| {
        let entry = document.cases[0].path.clone();
        let source = document
            .files
            .iter_mut()
            .find(|source| source.path == entry)
            .unwrap();
        source.includes[0].spelling = "../../LICENSE".into();
    });
}

#[test]
fn repeated_closure_entries_are_refused() {
    invalid(|document| {
        let duplicate = document.cases[0].transitive_source_paths[0].clone();
        document.cases[0].transitive_source_paths.push(duplicate);
    });
}

#[test]
fn unsatisfiable_contracts_cannot_require_models() {
    let mut contract = document().cases.remove(0).contract;
    contract.satisfiability = Satisfiability::Unsat;
    contract.cost = None;
    contract.witnesses.clear();
    contract.required_symbols.clear();
    contract.model_count = Some(1);
    assert!(validate_contract(&contract).is_err());
}

#[test]
fn ordinary_contracts_cannot_select_a_cost_vector() {
    let mut contract = document().cases.remove(0).contract;
    contract.family = Family::All;
    assert!(validate_contract(&contract).is_err());
}

#[test]
fn positive_model_claims_cannot_require_zero_models() {
    let mut contract = document().cases.remove(0).contract;
    contract.model_count = Some(0);
    assert!(validate_contract(&contract).is_err());
}

fn original_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../validation/corpus/kr-domains")
}

fn provenance_case() -> Corpus {
    let mut corpus = load(&root(), Limits::default()).unwrap();
    let path = corpus.cases[0].path.clone();
    corpus.files.retain(|source| source.path == path);
    corpus.cases.retain(|case| case.path == path);
    corpus
}

fn provenance_error(corpus: &Corpus, expected: &str) {
    let error =
        crate::examples::verify_originals(corpus, &original_root(), Limits::default()).unwrap_err();
    assert!(matches!(&error, Error::Contract(detail) if detail.contains(expected)));
    assert!(error.to_string().contains(&corpus.files[0].path));
}

#[test]
fn original_deletion_must_reproduce_retained_text() {
    let mut corpus = provenance_case();
    // Exercise the derivation check independently of the outer manifest seal.
    corpus.files[0].text.push_str("changed.\n");
    provenance_error(&corpus, "recorded source derivation differs");
}

#[test]
fn original_include_coordinates_are_one_based() {
    let mut corpus = provenance_case();
    corpus.files[0].includes[0].line = 0;
    provenance_error(&corpus, "original include coordinate differs");
}

#[test]
fn original_contracts_must_reproduce_typed_notes() {
    let mut corpus = provenance_case();
    corpus.cases[0]
        .contract
        .notes
        .push("unrecorded note".into());
    provenance_error(&corpus, "original display contract differs");
}

#[test]
fn shared_sources_cannot_own_case_annotations() {
    let mut corpus = provenance_case();
    corpus.cases.clear();
    provenance_error(&corpus, "unowned annotations");
}

fn original_fixture(corpus: &Corpus, bytes: &[u8]) -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    std::fs::copy(
        original_root().join("LICENSE"),
        directory.path().join("LICENSE"),
    )
    .unwrap();
    let path = directory.path().join(corpus.files[0].path());
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
    directory
}

#[test]
fn original_decoding_retains_the_utf8_cause() {
    let mut corpus = provenance_case();
    let bytes = [0xff];
    // Seal the invalid bytes at this private boundary so decoding, rather than
    // identity checking, supplies the refusal under test.
    corpus.files[0].original_sha256 = files::hash(&bytes);
    let directory = original_fixture(&corpus, &bytes);
    let error = crate::examples::verify_originals(&corpus, directory.path(), Limits::default())
        .unwrap_err();
    assert!(error.to_string().starts_with("examples source: "));
    let cause = std::error::Error::source(&error)
        .unwrap()
        .downcast_ref::<std::string::FromUtf8Error>()
        .unwrap();
    assert_eq!(cause.as_bytes(), bytes);
}

#[test]
fn original_sources_obey_their_independent_byte_cap() {
    let corpus = provenance_case();
    let limit = std::fs::read(original_root().join("LICENSE"))
        .unwrap()
        .len();
    let directory = original_fixture(&corpus, &vec![b'a'; limit + 1]);
    let error = crate::examples::verify_originals(
        &corpus,
        directory.path(),
        Limits {
            source_bytes: limit,
            ..Limits::default()
        },
    )
    .unwrap_err();
    assert!(matches!(error, Error::Limit {
        resource: Resource::SourceBytes, observed, limit: actual,
    } if observed == (limit + 1) as u128 && actual == limit));
}

#[test]
fn malformed_document_errors_retain_schema_evidence() {
    let error = Error::Json(
        serde_json::from_str::<Document>("{\"schema_version\":true}")
            .err()
            .unwrap(),
    );
    assert!(error.to_string().starts_with("examples manifest: "));
    let cause = std::error::Error::source(&error)
        .unwrap()
        .downcast_ref::<serde_json::Error>()
        .unwrap();
    assert!(cause.is_data());
    assert_eq!(cause.line(), 1);
}

#[test]
fn absent_include_spellings_retain_filesystem_evidence() {
    let mut document = document();
    let path = document.cases[0].path.clone();
    let source = document
        .files
        .iter_mut()
        .find(|source| source.path == path)
        .unwrap();
    source.includes[0].spelling = "absent.lp".into();
    let expected = root()
        .join(Path::new(&path).parent().unwrap())
        .join("absent.lp");
    let error = validate_cases(&document, &root(), Limits::default()).unwrap_err();
    assert!(matches!(error, Error::Io { path, source }
        if path == expected && source.kind() == std::io::ErrorKind::NotFound));
}

#[test]
fn consistent_case_metadata_cannot_admit_invalid_contracts() {
    invalid(|document| document.cases[0].contract.model_count = Some(0));
}
