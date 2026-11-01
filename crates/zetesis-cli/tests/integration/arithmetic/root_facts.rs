//! Authored parity sources retain their facts, full families and root receipts.

use super::*;
use zetesis_test_support::repository::examples;
use zetesis_validation::{
    answers::native_json,
    examples::{Edit, derive_source},
};

const TSEITIN: &str = "scalability/parity-tseitin/parity_tseitin.lp";
const CHAIN: &str = "scalability/parity-chain/parity_chain.lp";
const MIXED: &str = "scalability/parity-mixed/parity_mixed.lp";

fn source(path: &str) -> String {
    std::fs::read_to_string(examples().join(path)).unwrap()
}

fn even_source(path: &str, name: &str) -> String {
    let original = source(path);
    let declaration = format!("#const {name} = 1.");
    let matches: Vec<_> = original.match_indices(&declaration).collect();
    assert_eq!(matches.len(), 1);
    let start = matches[0].0 + declaration.len() - 2;
    let edit = Edit::replacement(start, start + 1, "1".into(), "0".into()).unwrap();
    derive_source(&original, &[edit], original.len()).unwrap()
}

fn run(source: &str, grounder: &[&str], oracle: Oracle) -> (Report, Json) {
    let arguments: Vec<_> = ["--json", "--stats"]
        .into_iter()
        .chain(grounder.iter().copied())
        .collect();
    let mut options = serial(&arguments);
    options.oracle = oracle;
    if grounder.is_empty() {
        assert_eq!(options.grounder, Grounder::Auto);
    }
    let (report, output, _) = detailed(source, &options);
    (report, serde_json::from_str(&output).unwrap())
}

fn scope(report: &Report, document: &Json, grounder: &[&str]) {
    let actual = &document["statistics"]["search"]["scope"];
    if grounder == ["--grounder", "lazy"] {
        assert!(report.hybrid_execution.is_some());
        assert_eq!(actual, "retained_core");
    } else {
        assert!(report.hybrid_execution.is_none());
        assert_eq!(actual, "original_theory");
    }
}

#[test]
fn authored_odd_parity_refutes_before_region_search() {
    for path in [TSEITIN, CHAIN] {
        let original = source(path);
        for grounder in GROUNDERS {
            for oracle in [Oracle::Auto, Oracle::Countermodel] {
                let (report, document) = run(&original, grounder, oracle);
                complete(&report, &document, 0);
                assert_eq!(report.checked, 0);
                scope(&report, &document, grounder);
                let statistics = report.countermodel_statistics.unwrap();
                let regions = statistics.regions.unwrap();
                assert_eq!(regions.parity.status, PreparationStatus::Prepared);
                assert!(regions.parity.rows > 0, "{path}, {grounder:?}, {oracle:?}");
                assert!(regions.parity.source_roots > 0);
                assert!(
                    regions.parity.root_refuted,
                    "{path}, {grounder:?}, {oracle:?}"
                );
                assert!(regions.parity.phases.provenance > 0);
                assert!(regions.parity.phases.reduction > 0);
                assert_eq!(regions.counts.regions, 0);
                assert_eq!(statistics.candidates, 0);
                assert_eq!(statistics.countermodel_queries, 0);
                assert_eq!(
                    document["statistics"]["search"]["candidate_regions"]["visited"],
                    0
                );
                receipts(&document, regions.parity, regions.cardinality);
            }
        }
    }
}

// The unchanged p=5, seed=1 graph has ten cycle edges and five matching edges.
const GRAPH: [(usize, usize); 15] = [
    (0, 1),
    (1, 2),
    (2, 3),
    (3, 4),
    (4, 5),
    (5, 6),
    (6, 7),
    (7, 8),
    (8, 9),
    (9, 0),
    (0, 8),
    (1, 5),
    (2, 9),
    (3, 6),
    (4, 7),
];

fn graph_facts(charged: bool) -> BTreeSet<String> {
    let mut facts = BTreeSet::new();
    for vertex in 0..10 {
        facts.insert(format!("vertex({vertex})"));
        if charged {
            facts.insert(format!("charge({vertex},0)"));
        }
        let incident: Vec<_> = GRAPH
            .iter()
            .enumerate()
            .filter(|(_, (left, right))| *left == vertex || *right == vertex)
            .map(|(index, _)| index + 1)
            .collect();
        assert_eq!(incident.len(), 3);
        facts.insert(format!(
            "edges({vertex},{},{},{})",
            incident[0], incident[1], incident[2]
        ));
    }
    for (index, (left, right)) in GRAPH.iter().enumerate() {
        let edge = index + 1;
        facts.insert(format!("edge({edge},{left},{right})"));
        facts.insert(format!("incident({left},{edge})"));
        facts.insert(format!("incident({right},{edge})"));
    }
    facts
}

fn even_subgraphs() -> impl Iterator<Item = u16> {
    (0_u16..1 << GRAPH.len()).filter(|mask| {
        (0..10).all(|vertex| {
            GRAPH
                .iter()
                .enumerate()
                .filter(|(index, (left, right))| {
                    (*left == vertex || *right == vertex) && mask & (1 << index) != 0
                })
                .count()
                % 2
                == 0
        })
    })
}

fn graph_family(
    facts: &BTreeSet<String>,
    masks: impl Iterator<Item = u16>,
) -> BTreeSet<BTreeSet<String>> {
    masks
        .map(|mask| {
            let mut model = facts.clone();
            for index in 0..GRAPH.len() {
                if mask & (1 << index) != 0 {
                    model.insert(format!("on({})", index + 1));
                }
            }
            model
        })
        .collect()
}

fn chain_family() -> BTreeSet<BTreeSet<String>> {
    let orders = [[1, 2, 3, 4, 5], [4, 1, 5, 2, 3]];
    let mut facts: BTreeSet<_> = (1..=13).map(|atom| format!("variable({atom})")).collect();
    for (chain, order) in orders.iter().enumerate() {
        let mut previous = order[0];
        for (step, &input) in order.iter().enumerate() {
            let result = if step == 0 {
                input
            } else {
                5 + 4 * chain + step
            };
            facts.insert(format!("input({},{},{input})", chain + 1, step + 1));
            facts.insert(format!("result({},{},{result})", chain + 1, step + 1));
            if step > 0 {
                facts.insert(format!("gate({previous},{input},{result})"));
            }
            previous = result;
        }
    }
    facts.insert("final(9,13)".into());
    facts.insert("target(0)".into());
    (0_u8..32)
        .map(|mask| {
            let mut model = facts.clone();
            for (chain, order) in orders.iter().enumerate() {
                let mut parity = false;
                for (step, &input) in order.iter().enumerate() {
                    let present = mask & (1 << (input - 1)) != 0;
                    parity ^= present;
                    if present {
                        model.insert(format!("x({input})"));
                        model.insert(format!("on({input})"));
                    }
                    if step > 0 && parity {
                        model.insert(format!("x({})", 5 + 4 * chain + step));
                    }
                }
            }
            model
        })
        .collect()
}

fn mixed_family() -> BTreeSet<BTreeSet<String>> {
    let mut facts = graph_facts(false);
    let mut clauses = Vec::new();
    for clause in 1..=8 {
        let keys = [1, 2, 3].map(|slot| {
            let mut key: usize = (7919 + clause * 104_729 + slot * 1_299_709) % 46_337;
            for offset in [12_345, 54_321, 7] {
                key = (key * key + offset) % 46_337;
            }
            facts.insert(format!("key({clause},{slot},{key})"));
            key
        });
        let start = keys[0] % 15;
        let step = 1 + keys[1] % 13;
        let further = 1 + keys[2] % (14 - step);
        let edges = [
            start + 1,
            (start + step) % 15 + 1,
            (start + step + further) % 15 + 1,
        ];
        let signs = keys.map(|key| (key / 61) % 2);
        facts.insert(format!(
            "clause({clause},{},{},{},{},{},{})",
            edges[0], signs[0], edges[1], signs[1], edges[2], signs[2]
        ));
        clauses.push((edges, signs));
    }
    graph_family(
        &facts,
        even_subgraphs().filter(|mask| {
            clauses.iter().all(|(edges, signs)| {
                edges
                    .iter()
                    .zip(signs)
                    .any(|(&edge, &sign)| (mask & (1 << (edge - 1)) != 0) == (sign == 1))
            })
        }),
    )
}

fn check_family(original: &str, expected: &BTreeSet<BTreeSet<String>>) {
    for grounder in GROUNDERS {
        for oracle in [Oracle::Auto, Oracle::Countermodel] {
            let (report, document) = run(original, grounder, oracle);
            complete(&report, &document, expected.len());
            scope(&report, &document, grounder);
            let parsed = native_json::parse(
                &serde_json::to_vec(&document).unwrap(),
                native_json::Limits::default(),
            )
            .unwrap();
            let actual: BTreeSet<BTreeSet<String>> = parsed
                .full_model_symbols(8 * 1024 * 1024)
                .unwrap()
                .into_iter()
                .map(|model| model.into_iter().collect())
                .collect();
            assert_eq!(actual.len(), report.models, "duplicate complete answer");
            assert_eq!(&actual, expected, "{grounder:?}, {oracle:?}");
            let regions = report.countermodel_statistics.unwrap().regions.unwrap();
            assert_eq!(regions.parity.status, PreparationStatus::Prepared);
            assert!(regions.parity.rows > 0);
            assert!(!regions.parity.root_refuted);
            assert!(!regions.cardinality.root_refuted);
            receipts(&document, regions.parity, regions.cardinality);
        }
    }
}

#[test]
fn authored_even_tseitin_preserves_all_full_answers() {
    let expected = graph_family(&graph_facts(true), even_subgraphs());
    assert_eq!(expected.len(), 64);
    check_family(&even_source(TSEITIN, "odd"), &expected);
}

#[test]
fn authored_equal_chains_preserve_all_full_answers() {
    let expected = chain_family();
    assert_eq!(expected.len(), 32);
    check_family(&even_source(CHAIN, "parity"), &expected);
}

#[test]
fn authored_mixed_parity_preserves_all_full_answers() {
    let expected = mixed_family();
    assert_eq!(expected.len(), 21);
    check_family(&source(MIXED), &expected);
}
