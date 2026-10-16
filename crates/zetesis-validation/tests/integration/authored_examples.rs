//! Fixture integrity is portable; complete clingo contract checks are opt-in.
use std::collections::BTreeSet;
use zetesis_test_support::repository::examples as root;
use zetesis_validation::{
    examples,
    performance::{
        bundles,
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
            "scalability/mastermind-nested.lp",
            "scalability/prime-sieve.lp",
            "scalability/subset-sum.lp",
            "sudoku.lp",
            "einstein-riddle.lp"
        ]
    );
    for (workload, count) in workloads.iter().zip([92, 0, 6_080, 1, 1, 15, 1, 1]) {
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
    assert_eq!(workloads[3].contract().unwrap().witnesses()[0].len(), 6);
    assert_eq!(workloads[6].contract().unwrap().witnesses()[0].len(), 81);
    assert_eq!(workloads[7].contract().unwrap().witnesses()[0].len(), 5);
}

#[test]
fn scaling_population_has_distinct_sizes_and_reference_qualified_amendments() {
    let corpus = examples::load(&root().join("correctness"), examples::Limits::default()).unwrap();
    let workloads =
        scalability::workloads(&corpus, &root(), true, WorkloadLimits::default()).unwrap();
    assert_eq!(workloads.len(), 21);
    assert_eq!(
        workloads
            .iter()
            .map(Workload::identity)
            .collect::<BTreeSet<_>>()
            .len(),
        21
    );
    assert_eq!(
        workloads
            .iter()
            .filter(|workload| workload.is_authored())
            .count(),
        18
    );
    assert_eq!(
        workloads
            .iter()
            .filter(|workload| workload.is_amended())
            .count(),
        10
    );
    assert_eq!(workloads[9].entry(), "scalability/mastermind-nested.lp");
    assert_eq!(workloads[13].entry(), "sudoku.lp");
    assert_eq!(workloads[20].entry(), "einstein-riddle.lp");
    for workload in &workloads {
        assert_eq!(workload.contract().is_none(), workload.is_amended());
    }
    let edits: Vec<_> = workloads[..9]
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
    assert_eq!(edits[8][0]["before"], "6");
    assert_eq!(edits[8][0]["after"], "8");
}

#[test]
fn arithmetic_sizes_follow_the_established_population() {
    let corpus = examples::load(&root().join("correctness"), examples::Limits::default()).unwrap();
    let workloads =
        scalability::workloads(&corpus, &root(), false, WorkloadLimits::default()).unwrap();
    assert_eq!(workloads.len(), 20);
    assert_eq!(
        workloads[..14]
            .iter()
            .map(Workload::entry)
            .collect::<Vec<_>>(),
        [
            "scalability/n-queens.lp",
            "scalability/n-queens.lp",
            "scalability/n-queens.lp",
            "scalability/pigeonhole.lp",
            "scalability/pigeonhole.lp",
            "scalability/pigeonhole.lp",
            "scalability/mastermind.lp",
            "scalability/mastermind.lp",
            "scalability/mastermind.lp",
            "scalability/mastermind-nested.lp",
            "standalone/n-queens/variant-02.lp",
            "standalone/send-money/send-money.lp",
            "scenarios/task-allocation/variant-04/05-larger-mix.lp",
            "sudoku.lp",
        ]
    );
    for (workload, entry, before, after) in [
        (&workloads[14], "prime-sieve", "30", None),
        (&workloads[15], "prime-sieve", "30", Some("100")),
        (&workloads[16], "prime-sieve", "30", Some("300")),
        (&workloads[17], "subset-sum", "12", None),
        (&workloads[18], "subset-sum", "12", Some("20")),
        (&workloads[19], "subset-sum", "12", Some("30")),
    ] {
        assert_eq!(workload.entry(), format!("scalability/{entry}.lp"));
        let value = serde_json::to_value(workload).unwrap();
        let source = &value["sources"][0];
        let edits = source["edits"].as_array().unwrap();
        if let Some(after) = after {
            assert_eq!(edits.len(), 1);
            assert_eq!(edits[0]["before"], before);
            assert_eq!(edits[0]["after"], after);
            assert_ne!(source["base_sha256"], source["derived_sha256"]);
            assert!(workload.contract().is_none());
        } else {
            assert!(edits.is_empty());
            assert_eq!(source["base_sha256"], source["derived_sha256"]);
            assert!(workload.contract().is_some());
        }
    }
}

#[test]
fn prime_default_contract_lists_exactly_the_primes() {
    let workloads = defaults();
    let contract = workloads[4].contract().unwrap();
    let mut primes = (2_u32..=30)
        .filter(|number| (2..*number).all(|divisor| number % divisor != 0))
        .map(|number| format!("prime({number})"))
        .collect::<Vec<_>>();
    primes.sort();
    assert_eq!(primes.len(), 10);
    assert_eq!(contract.witnesses(), [primes]);
    assert_eq!(contract.model_count(), Some(1));
}

#[test]
fn subset_default_contract_lists_every_distinct_weight_solution() {
    let workloads = defaults();
    let contract = workloads[5].contract().unwrap();
    let expected = (0_u32..(1 << 12))
        .map(|mask| {
            (1_u32..=12)
                .filter(|item| mask & (1 << (item - 1)) != 0)
                .collect::<Vec<_>>()
        })
        .filter(|items| items.iter().sum::<u32>() == 12)
        .map(|items| {
            items
                .iter()
                .map(|item| format!("selected({item})"))
                .collect::<BTreeSet<_>>()
        })
        .collect::<BTreeSet<_>>();
    let recorded = contract
        .witnesses()
        .iter()
        .map(|witness| witness.iter().cloned().collect::<BTreeSet<_>>())
        .collect::<BTreeSet<_>>();
    assert_eq!(expected.len(), 15);
    assert_eq!(recorded, expected);
    assert_eq!(contract.witnesses().len(), 15);
    assert_eq!(contract.model_count(), Some(15));
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

fn poker() -> Workload {
    let mut workloads = bundles::load(
        &root().join("scalability/poker/manifest.json"),
        WorkloadLimits::default(),
    )
    .unwrap();
    assert_eq!(workloads.len(), 1);
    workloads.pop().unwrap()
}

#[test]
fn poker_bundle_preserves_its_reviewed_include_graph() {
    let workload = poker();
    assert!(workload.is_bundle());
    assert_eq!(workload.entry(), "draw_poker_tests.lp");
    let record = serde_json::to_value(&workload).unwrap();
    assert_eq!(
        record["bundle"]["manifest"]["sha256"],
        "0384bacc2a0cc947165f72b769fb72404612b577dd39f50266e439850373837c"
    );
    assert_eq!(record["bundle"]["files"].as_array().unwrap().len(), 2);
    assert_eq!(
        record["bundle"]["files"][0]["includes"],
        serde_json::json!([{ "spelling": "draw_poker.lp", "target": "draw_poker.lp" }])
    );
}

#[test]
fn poker_contract_lists_every_nonroyal_straight_flush() {
    let workload = poker();
    let contract = workload.contract().unwrap();
    let expected = ["clubs", "diamonds", "hearts", "spades"]
        .into_iter()
        .flat_map(|suit| {
            (1_u8..=9).map(move |low| {
                let mut display = (low..=low + 4)
                    .map(|rank| {
                        let value = if rank == 1 { 14 } else { rank };
                        format!("hand(hero,{value},{suit})")
                    })
                    .collect::<BTreeSet<_>>();
                display.insert(format!("top_hand(hero,9,{low},{},na)", low + 4));
                display
            })
        })
        .collect::<BTreeSet<_>>();
    let recorded = contract
        .witnesses()
        .iter()
        .map(|witness| witness.iter().cloned().collect::<BTreeSet<_>>())
        .collect::<BTreeSet<_>>();
    assert_eq!(expected.len(), 36);
    assert_eq!(recorded, expected);
    assert_eq!(contract.witnesses().len(), 36);
    assert!(
        contract
            .witnesses()
            .iter()
            .all(|witness| witness.len() == 6)
    );
    assert_eq!(contract.model_count(), Some(36));
    assert_eq!(contract.family(), examples::Family::All);
    assert_eq!(contract.satisfiability(), examples::Satisfiability::Sat);
    assert_eq!(contract.cost(), None);
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
#[ignore = "requires clingo: poker has exactly the recorded 36 displayed answers"]
fn poker_complete_family_satisfies_its_contract() {
    use std::time::Duration;
    use zetesis_clingo_support as oracle;
    let workload = poker();
    let run = oracle::run_in(
        &root().join("scalability/poker"),
        ["--models=0", "--outf=2", workload.entry()],
        &oracle::DECIDED,
        oracle::Limits {
            timeout: Duration::from_mins(1),
            max_output_bytes: 8 * 1024 * 1024,
        },
    );
    let answers = oracle::answers(&run);
    workload.contract().unwrap().check(&answers).unwrap();
}
