//! Fixture integrity is portable; complete clingo contract checks are opt-in.
use std::collections::BTreeSet;
use zetesis_test_support::repository::examples as root;
use zetesis_validation::{
    examples,
    performance::{
        matrix::{Workload, WorkloadLimits},
        scalability,
    },
};

fn defaults() -> Vec<Workload> {
    let mut workloads = scalability::defaults(&root(), WorkloadLimits::default()).unwrap();
    workloads.push(scalability::sudoku(&root(), WorkloadLimits::default()).unwrap());
    workloads.push(scalability::einstein(&root(), WorkloadLimits::default()).unwrap());
    workloads
}

#[test]
fn reviewed_default_sources_retain_their_shared_typed_contracts() {
    let workloads = defaults();
    assert_eq!(
        workloads.iter().map(Workload::entry).collect::<Vec<_>>(),
        [
            "scalability/n-queens.lp",
            "scalability/pigeonhole.lp",
            "scalability/mastermind.lp",
            "sudoku.lp",
            "einstein-riddle.lp"
        ]
    );
    for (workload, count) in workloads.iter().zip([92, 0, 6_080, 1, 1]) {
        assert!(workload.is_authored());
        assert!(!workload.is_amended());
        assert!(!workload.is_generated());
        let contract = workload.contract().unwrap();
        assert_eq!(contract.model_count(), Some(count));
        assert_eq!(contract.family(), examples::Family::All);
        assert_eq!(
            contract.satisfiability(),
            if count == 0 {
                examples::Satisfiability::Unsat
            } else {
                examples::Satisfiability::Sat
            }
        );
        let value = serde_json::to_value(workload).unwrap();
        assert!(value.get("manifest_sha256").is_none());
        assert!(value.get("authored").is_some());
        assert_eq!(
            value["sources"][0]["base_sha256"],
            value["sources"][0]["derived_sha256"]
        );
    }
    assert_eq!(workloads[3].contract().unwrap().witnesses()[0].len(), 81);
    assert_eq!(workloads[4].contract().unwrap().witnesses()[0].len(), 5);
}

#[test]
fn scaling_population_has_distinct_sizes_and_reference_qualified_amendments() {
    let corpus = examples::load(&root().join("correctness"), examples::Limits::default()).unwrap();
    let workloads =
        scalability::workloads(&corpus, &root(), true, WorkloadLimits::default()).unwrap();
    assert_eq!(workloads.len(), 13);
    assert_eq!(
        workloads
            .iter()
            .map(Workload::identity)
            .collect::<BTreeSet<_>>()
            .len(),
        13
    );
    assert_eq!(
        workloads
            .iter()
            .filter(|workload| workload.is_authored())
            .count(),
        10
    );
    assert_eq!(
        workloads
            .iter()
            .filter(|workload| workload.is_amended())
            .count(),
        5
    );
    assert_eq!(workloads[11].entry(), "sudoku.lp");
    assert_eq!(workloads[12].entry(), "einstein-riddle.lp");
    for workload in &workloads {
        assert_eq!(workload.contract().is_none(), workload.is_amended());
    }
    let edits: Vec<_> = workloads[..8]
        .iter()
        .map(|workload| {
            let value = serde_json::to_value(workload).unwrap();
            value["sources"][0]["edits"].clone()
        })
        .collect();
    assert_eq!(edits[0], serde_json::json!([]));
    assert_eq!(edits[5], serde_json::json!([]));
    assert_eq!(edits[1][0]["before"], "8");
    assert_eq!(edits[1][0]["after"], "9");
    assert_eq!(edits[2][0]["after"], "10");
    assert_eq!(edits[3][0]["before"], "7");
    assert_eq!(edits[3][0]["after"], "5");
    assert_eq!(edits[4][0]["after"], "6");
    assert_eq!(edits[6][0]["before"], "6");
    assert_eq!(edits[6][0]["after"], "5");
    assert_eq!(edits[7], serde_json::json!([]));
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
#[ignore = "requires clingo: default authored families satisfy their contracts"]
fn default_authored_families_satisfy_their_contracts() {
    use std::time::Duration;
    use zetesis_clingo_support as oracle;
    for workload in defaults() {
        let run = oracle::run_in(
            &root(),
            ["--models=0", "--outf=2", workload.entry()],
            &oracle::DECIDED,
            oracle::Limits {
                timeout: Duration::from_mins(1),
                max_output_bytes: 8 * 1024 * 1024,
            },
        );
        let answers = oracle::answers(&run);
        workload.contract().unwrap().check(&answers).unwrap();
        println!(
            "pass: {} complete_models={}",
            workload.entry(),
            answers.model_count()
        );
    }
}
