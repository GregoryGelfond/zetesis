//! Authored source admission shares the corpus parser and bounded derivation.
use sha2::{Digest, Sha256};
use std::{num::NonZeroU64, path::Path};
use zetesis_validation::{
    examples::Contract,
    performance::{
        matrix::{AuthoredProgram, ConstantAmendment, NativeInvocation, Workload, WorkloadLimits},
        scalability,
    },
    selected::NativeExecution,
};

fn admit(
    root: &Path,
    source: &str,
    contract: &Contract,
    amendments: &[ConstantAmendment<'_>],
    limits: WorkloadLimits,
) -> Result<Workload, zetesis_validation::performance::Error> {
    let hash = format!("{:x}", Sha256::digest(source));
    Workload::authored(
        AuthoredProgram {
            root,
            entry: "case.lp",
            sha256: &hash,
            contract,
        },
        amendments,
        limits,
    )
}

#[test]
fn authored_singletons_refuse_parsed_includes() {
    let root = tempfile::tempdir().unwrap();
    let contract = Contract::complete_family(NonZeroU64::MIN);
    for source in ["#include \"other.lp\". p.", "#include <incmode>. p."] {
        std::fs::write(root.path().join("case.lp"), source).unwrap();
        assert!(
            admit(
                root.path(),
                source,
                &contract,
                &[],
                WorkloadLimits::default()
            )
            .is_err()
        );
    }
}

#[test]
fn changed_source_bytes_are_refused() {
    let root = tempfile::tempdir().unwrap();
    let contract = Contract::complete_family(NonZeroU64::MIN);
    std::fs::write(root.path().join("case.lp"), "q.").unwrap();
    assert!(admit(root.path(), "p.", &contract, &[], WorkloadLimits::default()).is_err());
}

#[test]
fn contract_changes_alter_workload_identity() {
    let root = tempfile::tempdir().unwrap();
    let one = Contract::complete_family(NonZeroU64::MIN);
    let two = Contract::complete_family(NonZeroU64::new(2).unwrap());
    std::fs::write(root.path().join("case.lp"), "p.").unwrap();
    let original = admit(root.path(), "p.", &one, &[], WorkloadLimits::default()).unwrap();
    let changed = admit(root.path(), "p.", &two, &[], WorkloadLimits::default()).unwrap();
    assert_ne!(original.identity(), changed.identity());
}

#[test]
fn source_changes_alter_workload_identity() {
    let root = tempfile::tempdir().unwrap();
    let contract = Contract::complete_family(NonZeroU64::MIN);
    std::fs::write(root.path().join("case.lp"), "p.").unwrap();
    let original = admit(root.path(), "p.", &contract, &[], WorkloadLimits::default()).unwrap();
    std::fs::write(root.path().join("case.lp"), "q.").unwrap();
    let changed = admit(root.path(), "q.", &contract, &[], WorkloadLimits::default()).unwrap();
    assert_ne!(original.identity(), changed.identity());
}

#[test]
fn noop_edits_preserve_identity_and_contract() {
    let root = tempfile::tempdir().unwrap();
    let source = "#const n=8. p(n).";
    std::fs::write(root.path().join("case.lp"), source).unwrap();
    let contract = Contract::complete_family(NonZeroU64::MIN);
    let original = admit(
        root.path(),
        source,
        &contract,
        &[],
        WorkloadLimits::default(),
    )
    .unwrap();
    let amendment = ConstantAmendment {
        source_path: "case.lp",
        name: "n",
        expected: 8,
        replacement: 8,
    };
    let unchanged = admit(
        root.path(),
        source,
        &contract,
        &[amendment],
        WorkloadLimits::default(),
    )
    .unwrap();
    assert_eq!(original.identity(), unchanged.identity());
    assert!(!unchanged.is_amended());
    assert_eq!(original.contract(), unchanged.contract());
}

#[test]
fn constant_edits_refuse_wrong_expected_values() {
    let root = tempfile::tempdir().unwrap();
    let source = "#const n=8. p(n).";
    std::fs::write(root.path().join("case.lp"), source).unwrap();
    let contract = Contract::complete_family(NonZeroU64::MIN);
    let amendment = ConstantAmendment {
        source_path: "case.lp",
        name: "n",
        expected: 7,
        replacement: 9,
    };
    assert!(
        admit(
            root.path(),
            source,
            &contract,
            &[amendment],
            WorkloadLimits::default()
        )
        .is_err()
    );
}

#[test]
fn authored_admission_obeys_each_inclusive_byte_ceiling() {
    let root = tempfile::tempdir().unwrap();
    let source = "#const n=8. p(n).";
    std::fs::write(root.path().join("case.lp"), source).unwrap();
    let contract = Contract::complete_family(NonZeroU64::MIN);
    let workload = admit(
        root.path(),
        source,
        &contract,
        &[],
        WorkloadLimits::default(),
    )
    .unwrap();
    let metadata = serde_json::to_value(workload).unwrap()["metadata_bytes"]
        .as_u64()
        .unwrap();
    let limits = WorkloadLimits {
        source_bytes: source.len(),
        total_source_bytes: 2 * source.len(),
        metadata_bytes: usize::try_from(metadata).unwrap(),
        ..WorkloadLimits::default()
    };
    assert!(admit(root.path(), source, &contract, &[], limits).is_ok());
    for smaller in [
        WorkloadLimits {
            metadata_bytes: limits.metadata_bytes - 1,
            ..limits
        },
        WorkloadLimits {
            source_bytes: limits.source_bytes - 1,
            ..limits
        },
        WorkloadLimits {
            total_source_bytes: limits.total_source_bytes - 1,
            ..limits
        },
    ] {
        assert!(admit(root.path(), source, &contract, &[], smaller).is_err());
    }
}

#[test]
fn default_profiles_omit_the_expansion_override() {
    assert!(
        serde_json::to_value(NativeExecution::default())
            .unwrap()
            .get("max_expansion_work")
            .is_none()
    );
    for invocation in [NativeInvocation::Legacy, NativeInvocation::Solve] {
        assert!(
            !invocation
                .arguments(&NativeExecution::default())
                .iter()
                .any(|arg| arg == "--max-expansion-work")
        );
    }
}

#[test]
fn scaling_profiles_vary_only_the_requested_region_workers() {
    let profiles = scalability::profiles(None);
    assert_eq!(
        profiles
            .iter()
            .map(|profile| profile.threads.get())
            .collect::<Vec<_>>(),
        [1, 2, 4, 8, 14]
    );
    for profile in &profiles {
        let normalized = NativeExecution {
            threads: profiles[0].threads,
            ..*profile
        };
        assert_eq!(
            serde_json::to_value(normalized).unwrap(),
            serde_json::to_value(profiles[0]).unwrap()
        );
    }
}

#[test]
fn memory_allowances_are_retained_in_every_profile() {
    for profile in scalability::profiles(Some(300_000_000)) {
        assert_eq!(
            serde_json::to_value(profile).unwrap()["memory_bytes"],
            300_000_000
        );
    }
}

#[test]
fn memory_allowances_reach_both_native_interfaces() {
    for profile in scalability::profiles(Some(300_000_000)) {
        for invocation in [NativeInvocation::Legacy, NativeInvocation::Solve] {
            let args = invocation.arguments(&profile);
            let position = args.iter().position(|arg| arg == "--memory").unwrap();
            assert_eq!(args[position + 1], "300000000");
        }
    }
}
