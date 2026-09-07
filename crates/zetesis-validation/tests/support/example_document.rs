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
