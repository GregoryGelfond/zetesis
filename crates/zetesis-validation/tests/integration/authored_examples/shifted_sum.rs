//! An authored numeric gap and its complete nearby control families.
use super::{BTreeSet, Workload, WorkloadLimits, bundles, examples, root};
use zetesis_validation::performance::matrix::{AuthoredProgram, ConstantAmendment};

const ENTRY: &str = "subset_sum.lp";
const SOURCE_SHA256: &str = "366ed74d26e462b7f1fc02fc73bf4891daa1843b71a7351b01c8e5c137e627de";
const MANIFEST_SHA256: &str = "4e7cee8d859cd7f54c8ee215056401bf106ae155c7b234476c8f09adf583fef5";
const CASES: [(i32, i32); 4] = [(8, 17), (8, 19), (30, 61), (30, 63)];

fn workload() -> Workload {
    let mut workloads = bundles::load(
        &root().join("scalability/shifted-subset-sum/manifest.json"),
        WorkloadLimits::default(),
    )
    .unwrap();
    assert_eq!(workloads.len(), 1);
    workloads.pop().unwrap()
}

fn expected(n: i32, target: i32) -> examples::Contract {
    if target == 2 * n + 1 {
        workload().contract().unwrap().clone()
    } else {
        assert_eq!(target, 2 * n + 3);
        serde_json::from_value(serde_json::json!({
            "satisfiability": "sat", "family": "all", "model_count": 1,
            "cost": null, "witnesses": [["selected(1)", "selected(2)"]],
            "required_symbols": [], "notes": []
        }))
        .unwrap()
    }
}

#[test]
fn shifted_sum_seals_its_complete_default_contract() {
    let workload = workload();
    assert!(workload.is_bundle());
    assert_eq!(workload.entry(), ENTRY);
    let record = serde_json::to_value(&workload).unwrap();
    assert_eq!(record["bundle"]["manifest"]["sha256"], MANIFEST_SHA256);
    assert_eq!(record["bundle"]["files"][0]["sha256"], SOURCE_SHA256);
    let contract = workload.contract().unwrap();
    assert_eq!(contract.satisfiability(), examples::Satisfiability::Unsat);
    assert_eq!(contract.family(), examples::Family::All);
    assert_eq!(contract.model_count(), Some(0));
    assert_eq!(contract.cost(), None);
    assert!(contract.witnesses().is_empty());
}

#[test]
fn small_shifted_sum_targets_have_exact_complete_families() {
    for target in [17, 19] {
        let actual = (0_u32..1 << 8)
            .map(|mask| {
                (1..=8)
                    .filter(|item| mask & (1 << (item - 1)) != 0)
                    .collect::<Vec<_>>()
            })
            .filter(|items| items.iter().map(|item| 8 + item).sum::<i32>() == target)
            .map(|items| {
                items
                    .into_iter()
                    .map(|item| format!("selected({item})"))
                    .collect::<Vec<_>>()
            })
            .collect::<BTreeSet<_>>();
        let contract = expected(8, target);
        assert_eq!(
            u64::try_from(actual.len()).unwrap(),
            contract.model_count().unwrap()
        );
        assert_eq!(actual, contract.witnesses().iter().cloned().collect());
    }
}

#[test]
fn larger_shifted_sum_targets_have_only_the_recorded_pair() {
    for (n, target) in CASES {
        // Empty and singleton selections are too small; every triple and
        // larger selection is too large. Enumerating pairs is therefore complete.
        assert!(2 * n < target);
        assert!(3 * n + 6 > target);
        let pairs = (1..=n)
            .flat_map(|a| (a + 1..=n).map(move |b| (a, b)))
            .filter(|(a, b)| 2 * n + a + b == target)
            .map(|(a, b)| vec![format!("selected({a})"), format!("selected({b})")])
            .collect::<BTreeSet<_>>();
        let contract = expected(n, target);
        assert_eq!(
            u64::try_from(pairs.len()).unwrap(),
            contract.model_count().unwrap()
        );
        assert_eq!(pairs, contract.witnesses().iter().cloned().collect());
    }
}

#[test]
fn shifted_sum_parameter_edits_retain_checked_provenance() {
    let original = workload();
    let directory = root().join("scalability/shifted-subset-sum");
    let program = AuthoredProgram {
        root: &directory,
        entry: ENTRY,
        sha256: SOURCE_SHA256,
        contract: original.contract().unwrap(),
    };
    let mut identities = BTreeSet::new();
    for (n, target) in CASES {
        let edits = [("n", 8, n), ("target", 17, target)]
            .into_iter()
            .filter(|(_, before, after)| before != after)
            .map(|(name, expected, replacement)| ConstantAmendment {
                source_path: ENTRY,
                name,
                expected,
                replacement,
            })
            .collect::<Vec<_>>();
        let amended = Workload::authored(program, &edits, WorkloadLimits::default()).unwrap();
        let record = serde_json::to_value(&amended).unwrap();
        let recorded = record["sources"][0]["edits"].as_array().unwrap();
        assert_eq!(recorded.len(), edits.len());
        for (recorded, edit) in recorded.iter().zip(&edits) {
            assert_eq!(recorded["before"], edit.expected.to_string());
            assert_eq!(recorded["after"], edit.replacement.to_string());
        }
        assert_eq!(amended.is_amended(), (n, target) != (8, 17));
        assert_eq!(amended.contract().is_none(), amended.is_amended());
        assert!(identities.insert(amended.identity().to_owned()));
    }
    let stale = ConstantAmendment {
        source_path: ENTRY,
        name: "n",
        expected: 9,
        replacement: 30,
    };
    assert!(Workload::authored(program, &[stale], WorkloadLimits::default()).is_err());
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
#[ignore = "requires clingo: shifted sums have their complete recorded families"]
fn shifted_sum_complete_families_satisfy_their_contracts() {
    use std::time::Duration;
    use zetesis_clingo_support as oracle;
    for (n, target) in CASES {
        let n_arg = format!("n={n}");
        let target_arg = format!("target={target}");
        let run = oracle::run_in(
            &root().join("scalability/shifted-subset-sum"),
            [
                "--models=0",
                "--outf=2",
                "--const",
                &n_arg,
                "--const",
                &target_arg,
                ENTRY,
            ],
            &oracle::DECIDED,
            oracle::Limits {
                timeout: Duration::from_mins(1),
                max_output_bytes: 8 * 1024 * 1024,
            },
        );
        expected(n, target).check(&oracle::answers(&run)).unwrap();
    }
}
