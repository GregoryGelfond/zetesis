//! Optional independent source oracle; clingo is never a production dependency.

use std::collections::BTreeSet;

use zetesis_clingo_support as oracle;
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{
    AdmissionLimits, AggregateComparison as Comparison, AggregateElement as Element,
    AggregateLimits, Interpretation, Limits, Node, Theory, append_aggregate, check,
};

type Models = BTreeSet<BTreeSet<String>>;

fn clingo(source: &str) -> Models {
    let run = oracle::run(
        source,
        &["0", "--outf=2", "--warn=none"],
        oracle::Limits::default(),
    );
    let json = oracle::json(&run);
    assert!(
        matches!(
            json["Result"].as_str(),
            Some("SATISFIABLE" | "UNSATISFIABLE")
        ),
        "{json}"
    );
    assert_eq!(json["Models"]["More"].as_str(), Some("no"));
    let mut result = Models::new();
    let mut count = 0;
    for call in json["Call"].as_array().unwrap() {
        if let Some(witnesses) = call["Witnesses"].as_array() {
            for witness in witnesses {
                count += 1;
                assert!(
                    result.insert(
                        witness["Value"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|atom| atom.as_str().unwrap().to_owned())
                            .collect()
                    )
                );
            }
        }
    }
    assert_eq!(json["Models"]["Number"].as_u64(), Some(count));
    result
}

fn compare(
    source: &str,
    elements: &[Element],
    comparison: Comparison,
    bound: i64,
    negations: usize,
    cycle: bool,
) {
    let mut nodes = vec![
        Node::False,
        Node::Implies(0, 0),
        Node::Atom(0),
        Node::Atom(1),
        Node::Implies(2, 0),
        Node::Implies(3, 0),
        Node::Or(2, 4),
    ];
    let aggregate = append_aggregate(
        &mut nodes,
        elements,
        comparison,
        bound,
        AggregateLimits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    let mut body = aggregate.root();
    for _ in 0..negations {
        let next = nodes.len();
        nodes.push(Node::Implies(body, 0));
        body = next;
    }
    let mut roots = vec![nodes.len()];
    nodes.push(Node::Implies(body, if cycle { 3 } else { 2 }));
    if cycle {
        roots.push(nodes.len());
        nodes.push(Node::Implies(3, 2));
    }
    let theory = Theory::new(2, nodes, roots, AdmissionLimits::default()).unwrap();
    let mut result = Models::new();
    for mask in 0u8..4 {
        let candidate =
            Interpretation::new(&theory, (0..2).filter(|atom| mask & (1 << atom) != 0)).unwrap();
        if check(
            &theory,
            &candidate,
            Limits::default(),
            &Cancellation::default(),
        )
        .unwrap()
        .accepted()
        {
            result.insert(
                candidate
                    .atoms()
                    .map(|atom| if atom == 0 { "p" } else { "q" }.to_owned())
                    .collect(),
            );
        }
    }
    assert_eq!(result, clingo(source), "source: {source}");
}

#[test]
#[ignore = "requires independently installed clingo; bounded five-second/64-KiB subprocesses"]
fn recursive_ground_aggregates_preserve_complete_clingo_model_sets() {
    // Fixed sources distinguish != from not(=), mixed-sign implication from
    // classical disjunction, and whole-tuple OR from classical true replacement.
    compare(
        "p :- #count {1:p} != 0.",
        &[Element {
            weight: 1,
            condition: 2,
        }],
        Comparison::Ne,
        0,
        0,
        false,
    );
    compare(
        "p :- not #count {1:p} = 0.",
        &[Element {
            weight: 1,
            condition: 2,
        }],
        Comparison::Eq,
        0,
        1,
        false,
    );
    compare(
        "q :- #sum {1:p; -1:q} >= 0. p :- q.",
        &[
            Element {
                weight: 1,
                condition: 2,
            },
            Element {
                weight: -1,
                condition: 3,
            },
        ],
        Comparison::Ge,
        0,
        0,
        true,
    );
    compare(
        "p :- #sum {1:p; -1:not p} >= 0.",
        &[
            Element {
                weight: 1,
                condition: 2,
            },
            Element {
                weight: -1,
                condition: 4,
            },
        ],
        Comparison::Ge,
        0,
        0,
        false,
    );
    compare(
        "p :- #sum {1:p; -1:p} = 0.",
        &[
            Element {
                weight: 1,
                condition: 2,
            },
            Element {
                weight: -1,
                condition: 2,
            },
        ],
        Comparison::Eq,
        0,
        0,
        false,
    );
    compare(
        "p :- #count {1:p; 1:not p} = 1.",
        &[Element {
            weight: 1,
            condition: 6,
        }],
        Comparison::Eq,
        1,
        0,
        false,
    );
    compare(
        "p :- #sum {2,k:p; 2,k:not p} >= 2.",
        &[Element {
            weight: 2,
            condition: 6,
        }],
        Comparison::Ge,
        2,
        0,
        false,
    );
    compare("p :- #sum {} = 0.", &[], Comparison::Eq, 0, 0, false);
}

#[test]
#[ignore = "requires independently installed clingo; bounded five-second/64-KiB subprocesses"]
fn generated_recursive_signed_and_default_negated_guards_match_clingo() {
    for (comparison, operator) in [
        (Comparison::Eq, "="),
        (Comparison::Ne, "!="),
        (Comparison::Lt, "<"),
        (Comparison::Le, "<="),
        (Comparison::Gt, ">"),
        (Comparison::Ge, ">="),
    ] {
        for weights in [[1, 1], [0, 2], [-1, 1], [-2, -1]] {
            for bound in [-1, 0, 1] {
                for (negations, negative) in [(0, ""), (1, "not ")] {
                    let source = format!(
                        "q :- {negative}#sum {{{},x:p; {},y:not q}} {operator} {bound}. p :- q.",
                        weights[0], weights[1]
                    );
                    compare(
                        &source,
                        &[
                            Element {
                                weight: weights[0],
                                condition: 2,
                            },
                            Element {
                                weight: weights[1],
                                condition: 5,
                            },
                        ],
                        comparison,
                        bound,
                        negations,
                        true,
                    );
                }
            }
        }
    }
}
