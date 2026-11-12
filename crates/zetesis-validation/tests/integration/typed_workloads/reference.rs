//! Independent direct constructors for the reviewed finite selection.
//! The order and literal amendments are independent of schema-2 decoding.

use serde_json::Value;
use zetesis_test_support::repository;
use zetesis_validation::{
    examples,
    performance::{
        families::Family,
        matrix::{AuthoredProgram, ConstantAmendment, Workload, WorkloadLimits},
    },
};

// These requests retain the preexisting constructor population. The snapshot
// supplies original contracts and source seals, not active amended contracts.
type ConstantChange = (&'static str, i32, i32);

const AUTHORED: [(&str, &[ConstantChange]); 45] = [
    ("shifted-subset-sum", &[("target", 17, 19)]),
    ("shifted-subset-sum", &[("n", 8, 30), ("target", 17, 61)]),
    ("shifted-subset-sum", &[("n", 8, 30), ("target", 17, 63)]),
    ("two-qbf", &[("terms", 68, 52)]),
    ("two-qbf", &[("seed", 1, 2)]),
    ("two-qbf", &[("exists", 3, 2), ("terms", 68, 40)]),
    ("two-qbf", &[("forall", 8, 10), ("terms", 68, 85)]),
    ("two-qbf-parity", &[("stable", 1, 0)]),
    ("two-qbf-parity", &[("stable", 1, 2)]),
    ("two-qbf-parity", &[("exists", 2, 3), ("stable", 1, 3)]),
    (
        "two-qbf-parity",
        &[("stable", 1, 4), ("n", 6, 5), ("seed", 1, 2)],
    ),
    ("two-qbf-tseitin", &[("stable", 1, 0)]),
    ("two-qbf-tseitin", &[("stable", 1, 2)]),
    ("two-qbf-tseitin", &[("exists", 2, 3), ("stable", 1, 3)]),
    ("two-qbf-tseitin", &[("stable", 1, 4)]),
    ("two-qbf-pigeonhole", &[("stable", 1, 0)]),
    ("two-qbf-pigeonhole", &[("stable", 1, 2)]),
    ("two-qbf-pigeonhole", &[("exists", 2, 3), ("stable", 1, 3)]),
    ("two-qbf-pigeonhole", &[("stable", 1, 4), ("holes", 4, 3)]),
    ("two-qbf-parity", &[("n", 6, 10)]),
    ("two-qbf-parity", &[("n", 6, 14)]),
    ("two-qbf-parity", &[("n", 6, 16)]),
    ("two-qbf-tseitin", &[("vertices", 12, 18)]),
    ("two-qbf-tseitin", &[("vertices", 12, 24)]),
    ("two-qbf-tseitin", &[("vertices", 12, 30)]),
    ("two-qbf-pigeonhole", &[("holes", 4, 5)]),
    ("two-qbf-pigeonhole", &[("holes", 4, 6)]),
    ("strategic-companies", &[("seed", 1, 2)]),
    (
        "strategic-companies",
        &[("companies", 12, 8), ("products", 24, 16), ("seed", 1, 2)],
    ),
    (
        "strategic-companies",
        &[("companies", 12, 10), ("products", 24, 20), ("seed", 1, 3)],
    ),
    (
        "strategic-companies",
        &[("companies", 12, 16), ("products", 24, 32)],
    ),
    (
        "strategic-companies",
        &[("companies", 12, 20), ("products", 24, 40)],
    ),
    ("parity-tseitin", &[("p", 5, 2)]),
    ("parity-tseitin", &[("p", 5, 3), ("odd", 1, 0)]),
    ("parity-tseitin", &[("seed", 1, 2)]),
    ("parity-chain", &[("p", 5, 2)]),
    ("parity-chain", &[("p", 5, 3), ("parity", 1, 0)]),
    ("parity-chain", &[("seed", 1, 2)]),
    ("pigeonhole-clauses", &[("holes", 4, 3)]),
    ("pigeonhole-clauses", &[("holes", 4, 3), ("extra", 1, 0)]),
    ("pigeonhole-clauses", &[("holes", 4, 2)]),
    ("parity-mixed", &[("clauses", 8, 0)]),
    ("parity-mixed", &[("seed", 1, 2)]),
    ("parity-mixed", &[("p", 5, 3), ("clauses", 8, 4)]),
    ("parity-mixed", &[("clauses", 8, 20)]),
];

pub(super) fn workloads(expected: &[Value]) -> Vec<Workload> {
    let mut result = Vec::new();
    for ((directory, specifications), record) in AUTHORED.iter().zip(expected) {
        let entry = record["entry"].as_str().unwrap();
        let contract: examples::Contract =
            serde_json::from_value(record["default_contract"].clone()).unwrap();
        let amendments: Vec<_> = specifications
            .iter()
            .map(|&(name, before, after)| ConstantAmendment {
                source_path: entry,
                name,
                expected: before,
                replacement: after,
            })
            .collect();
        result.push(
            Workload::authored(
                AuthoredProgram {
                    root: &repository::examples().join("scalability").join(directory),
                    entry,
                    sha256: record["sources"][0]["base_sha256"].as_str().unwrap(),
                    contract: &contract,
                },
                &amendments,
                WorkloadLimits::default(),
            )
            .unwrap(),
        );
    }
    let corpus = examples::load(&repository::correctness(), examples::Limits::default()).unwrap();
    for (variant, sizes) in [
        (1, &[4, 5, 6][..]),
        (3, &[4, 5, 6][..]),
        (2, &[4, 5][..]),
        (4, &[4, 5][..]),
        (5, &[4, 5][..]),
        (6, &[4, 5][..]),
    ] {
        let entry = format!("standalone/n-queens/variant-{variant:02}.lp");
        for &replacement in sizes {
            result.push(
                Workload::amended(
                    &corpus,
                    &entry,
                    &[ConstantAmendment {
                        source_path: &entry,
                        name: "n",
                        expected: 8,
                        replacement,
                    }],
                    WorkloadLimits::default(),
                )
                .unwrap(),
            );
        }
    }
    for (family, sizes) in [
        (Family::MonotoneChoices, [6, 8, 10]),
        (Family::RedundantTransitivity, [8, 12, 16]),
    ] {
        for size in sizes {
            result.push(Workload::generated(family, size, WorkloadLimits::default()).unwrap());
        }
    }
    result
}
