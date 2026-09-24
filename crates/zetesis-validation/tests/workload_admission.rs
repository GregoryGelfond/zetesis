//! Explicit parameter workloads retain corpus provenance and bounded byte edits.

use sha2::{Digest, Sha256};
use std::path::Path;
use zetesis_validation::{
    examples,
    performance::matrix::{ConstantAmendment, Workload, WorkloadLimits},
};

fn corpus() -> examples::Corpus {
    examples::load(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/correctness"),
        examples::Limits::default(),
    )
    .unwrap()
}

fn amendment(path: &str, replacement: i32) -> ConstantAmendment<'_> {
    ConstantAmendment {
        source_path: path,
        name: "n",
        expected: 8,
        replacement,
    }
}

#[test]
fn all_queens_variants_have_sealed_parameter_workloads() {
    let corpus = corpus();
    let mut identities = std::collections::BTreeSet::new();
    for variant in 1..=6 {
        let path = format!("standalone/n-queens/variant-{variant:02}.lp");
        let source = corpus
            .files()
            .iter()
            .find(|source| source.path() == path)
            .unwrap();
        for size in [8, 10, 12] {
            let workload = Workload::amended(
                &corpus,
                &path,
                &[amendment(&path, size)],
                WorkloadLimits::default(),
            )
            .unwrap();
            assert!(identities.insert(workload.identity().to_owned()));
            let encoded = serde_json::to_value(&workload).unwrap();
            let derived = &encoded["sources"][0];
            assert_eq!(derived["base_sha256"], source.source_sha256());
            let edits: Vec<examples::Edit> =
                serde_json::from_value(derived["edits"].clone()).unwrap();
            assert_eq!(edits.len(), 1);
            assert_eq!(edits[0].before(), "8");
            assert_eq!(edits[0].after(), size.to_string());
            let text = examples::derive_source(source.source(), &edits, 1024 * 1024).unwrap();
            assert_eq!(derived["derived_bytes"], text.len());
            assert_eq!(
                derived["derived_sha256"],
                format!("{:x}", Sha256::digest(text.as_bytes()))
            );
            assert_eq!(
                encoded["default_contract"],
                serde_json::to_value(
                    corpus
                        .cases()
                        .iter()
                        .find(|case| case.path() == path)
                        .unwrap()
                        .contract()
                )
                .unwrap()
            );
        }
    }
    assert_eq!(identities.len(), 18);
}

#[test]
fn content_identity_ignores_noop_edit_metadata() {
    let corpus = corpus();
    let path = "standalone/n-queens/variant-01.lp";
    let original = Workload::original(&corpus, path, WorkloadLimits::default()).unwrap();
    let unchanged = Workload::amended(
        &corpus,
        path,
        &[amendment(path, 8)],
        WorkloadLimits::default(),
    )
    .unwrap();
    assert_eq!(original.identity(), unchanged.identity());
    assert!(!original.is_amended());
    assert!(unchanged.is_amended());
}

#[test]
fn workload_retention_obeys_each_byte_ceiling() {
    let corpus = corpus();
    let path = "standalone/n-queens/variant-01.lp";
    for limits in [
        WorkloadLimits {
            source_bytes: 0,
            ..WorkloadLimits::default()
        },
        WorkloadLimits {
            total_source_bytes: 0,
            ..WorkloadLimits::default()
        },
        WorkloadLimits {
            metadata_bytes: 0,
            ..WorkloadLimits::default()
        },
    ] {
        assert!(Workload::amended(&corpus, path, &[amendment(path, 12)], limits).is_err());
    }
}

#[test]
fn workload_byte_boundaries_are_inclusive() {
    let corpus = corpus();
    let path = "standalone/n-queens/variant-01.lp";
    let amendments = [amendment(path, 12)];
    let admitted =
        Workload::amended(&corpus, path, &amendments, WorkloadLimits::default()).unwrap();
    let encoded = serde_json::to_value(&admitted).unwrap();
    let limits = WorkloadLimits {
        source_bytes: usize::try_from(encoded["sources"][0]["derived_bytes"].as_u64().unwrap())
            .unwrap(),
        total_source_bytes: usize::try_from(encoded["source_bytes"].as_u64().unwrap()).unwrap(),
        metadata_bytes: usize::try_from(encoded["metadata_bytes"].as_u64().unwrap()).unwrap(),
        amendments: 1,
    };
    assert!(Workload::amended(&corpus, path, &amendments, limits).is_ok());
    for smaller in [
        WorkloadLimits {
            source_bytes: limits.source_bytes - 1,
            ..limits
        },
        WorkloadLimits {
            total_source_bytes: limits.total_source_bytes - 1,
            ..limits
        },
        WorkloadLimits {
            metadata_bytes: limits.metadata_bytes - 1,
            ..limits
        },
    ] {
        assert!(Workload::amended(&corpus, path, &amendments, smaller).is_err());
    }
}

#[test]
fn amendment_count_is_bounded_before_derivation() {
    let corpus = corpus();
    let path = "standalone/n-queens/variant-01.lp";
    let limits = WorkloadLimits {
        amendments: 0,
        ..WorkloadLimits::default()
    };
    assert!(Workload::amended(&corpus, path, &[amendment(path, 12)], limits).is_err());
    assert!(
        Workload::amended(
            &corpus,
            path,
            &vec![amendment(path, 12); 129],
            WorkloadLimits::default()
        )
        .is_err()
    );
}

#[test]
fn repeated_declaration_requests_are_refused() {
    let corpus = corpus();
    let path = "standalone/n-queens/variant-01.lp";
    assert!(
        Workload::amended(
            &corpus,
            path,
            &[amendment(path, 10), amendment(path, 12)],
            WorkloadLimits::default()
        )
        .is_err()
    );
}

#[test]
fn amendments_cannot_escape_the_entry_closure() {
    let corpus = corpus();
    let path = "standalone/n-queens/variant-01.lp";
    assert!(
        Workload::amended(
            &corpus,
            path,
            &[amendment("../foreign.lp", 12)],
            WorkloadLimits::default()
        )
        .is_err()
    );
}

#[test]
fn unknown_entries_are_refused() {
    assert!(Workload::original(&corpus(), "unknown.lp", WorkloadLimits::default()).is_err());
}

#[test]
fn amendment_names_are_resolved_as_declarations() {
    let corpus = corpus();
    let path = "standalone/n-queens/variant-01.lp";
    let requested = ConstantAmendment {
        name: "#include \"foreign.lp\".",
        ..amendment(path, 12)
    };
    assert!(Workload::amended(&corpus, path, &[requested], WorkloadLimits::default()).is_err());
}

#[test]
fn generated_workloads_are_identified_by_family_size_and_bytes() {
    use zetesis_validation::performance::families::Family;
    let chain = Workload::generated(Family::Chain, 4, WorkloadLimits::default()).unwrap();
    assert_eq!(chain.entry(), "generated/chain-4.lp");
    assert!(!chain.is_amended());
    assert_eq!(
        chain.identity(),
        Workload::generated(Family::Chain, 4, WorkloadLimits::default())
            .unwrap()
            .identity()
    );
    assert_ne!(
        chain.identity(),
        Workload::generated(Family::Chain, 5, WorkloadLimits::default())
            .unwrap()
            .identity()
    );
    assert_ne!(
        chain.identity(),
        Workload::generated(Family::TransitivePath, 4, WorkloadLimits::default())
            .unwrap()
            .identity()
    );
    let encoded = serde_json::to_value(&chain).unwrap();
    let source = Family::Chain.source(4).unwrap();
    assert_eq!(encoded["generated"]["family"], "chain");
    assert_eq!(encoded["generated"]["size"], 4);
    assert_eq!(encoded["generated"]["bytes"], source.len());
    assert_eq!(
        encoded["generated"]["sha256"],
        format!("{:x}", Sha256::digest(source.as_bytes()))
    );
    assert_eq!(encoded["default_contract"]["model_count"], 1);
    assert!(encoded["sources"].as_array().unwrap().is_empty());
    assert!(encoded.get("manifest_sha256").is_none());
}

#[test]
fn generated_workloads_are_bounded_by_family_and_byte_ceilings() {
    use zetesis_validation::performance::families::Family;
    assert!(Workload::generated(Family::Disjunction, 64, WorkloadLimits::default()).is_err());
    // The depth-one chain is 36 bytes; the ceiling admits it and nothing deeper.
    let tiny = WorkloadLimits {
        source_bytes: 36,
        ..WorkloadLimits::default()
    };
    assert!(Workload::generated(Family::Chain, 2, tiny).is_err());
    assert!(Workload::generated(Family::Chain, 1, tiny).is_ok());
}
