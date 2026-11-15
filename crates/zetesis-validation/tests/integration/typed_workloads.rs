//! Typed selection loading preserves the direct constructors' exact population.

mod reference;

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, path::PathBuf};
use zetesis_validation::performance::{
    bundles,
    matrix::{Workload, WorkloadLimits},
};

const KEYS: [&str; 8] = [
    "entry",
    "identity",
    "sources",
    "default_contract",
    "amended",
    "generated",
    "manifest_sha256",
    "source_bytes",
];

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/typed-selection65.json")
}

fn selected() -> Vec<Workload> {
    bundles::load(
        &fixture(),
        WorkloadLimits {
            metadata_bytes: 1_048_576,
            amendments: 128,
            ..WorkloadLimits::default()
        },
    )
    .unwrap()
}

#[test]
fn typed_selection_preserves_all_direct_constructor_records() {
    let expected: Value =
        serde_json::from_str(include_str!("../fixtures/typed-selection65-expected.json")).unwrap();
    assert_eq!(expected["keys"], serde_json::json!(KEYS));
    let records = expected["workloads"].as_array().unwrap();
    let reference = reference::workloads(records);
    let loaded = selected();
    assert_eq!(records.len(), 65);
    assert_eq!(reference.len(), 65);
    assert_eq!(loaded.len(), 65);
    assert_eq!(
        loaded
            .iter()
            .map(Workload::identity)
            .collect::<BTreeSet<_>>()
            .len(),
        65
    );
    for (index, ((actual, direct), prior)) in loaded.iter().zip(&reference).zip(records).enumerate()
    {
        let actual_record = serde_json::to_value(actual).unwrap();
        let direct_record = serde_json::to_value(direct).unwrap();
        for key in KEYS {
            assert_eq!(actual_record[key], prior[key], "loaded case {index}, {key}");
            assert_eq!(direct_record[key], prior[key], "direct case {index}, {key}");
        }
        assert_eq!(
            actual.contract(),
            direct.contract(),
            "active contract {index}"
        );
        assert_eq!(actual.contract().is_none(), index < 59, "contract {index}");
        assert_eq!(actual.is_generated(), index >= 59, "constructor {index}");
        assert_eq!(actual.is_authored(), index < 45, "authored {index}");
    }
}

#[test]
fn typed_selection_seals_its_manifest() {
    let path = fixture();
    let digest = format!("{:x}", Sha256::digest(std::fs::read(&path).unwrap()));
    let canonical = path.canonicalize().unwrap();
    for workload in selected() {
        let record = serde_json::to_value(workload).unwrap();
        let seal = &record["selection_manifest"];
        assert_eq!(seal["sha256"], digest);
        assert_eq!(seal["canonical"], canonical.to_str().unwrap());
        assert!(record.get("bundle").is_none());
    }
}
