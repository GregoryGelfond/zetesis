//! Optional independent source oracle; clingo is never a production dependency.

use std::collections::BTreeSet;
use std::fs::File;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{
    AdmissionLimits, AggregateComparison as Comparison, AggregateElement as Element,
    AggregateExtremum as Extremum, AggregateLimits, ExtremumBound as Bound, Interpretation, Limits,
    Node, Theory, append_extremum, check,
};

type Models = BTreeSet<BTreeSet<String>>;

fn clingo(source: &str) -> Models {
    let directory = tempfile::tempdir().unwrap();
    let source_path = directory.path().join("case.lp");
    let output_path = directory.path().join("out.json");
    let error_path = directory.path().join("err.txt");
    std::fs::write(&source_path, source).unwrap();
    let output = File::create(&output_path).unwrap();
    let errors = File::create(&error_path).unwrap();
    let start = Instant::now();
    let mut child = Command::new("clingo")
        .args(["0", "--outf=2", "--warn=none"])
        .arg(&source_path)
        .stdin(Stdio::null())
        .stdout(output.try_clone().unwrap())
        .stderr(errors.try_clone().unwrap())
        .spawn()
        .expect("independently installed clingo on PATH");
    let status = loop {
        let within_limit = start.elapsed() <= Duration::from_secs(5)
            && output.metadata().unwrap().len() + errors.metadata().unwrap().len() <= 65_536;
        if !within_limit {
            let _ = child.kill();
            let _ = child.wait();
            panic!("oracle exceeded time or output ceiling: {source}");
        }
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    assert!(
        matches!(status.code(), Some(10 | 20 | 30)),
        "{source}: {}",
        std::fs::read_to_string(error_path).unwrap()
    );
    let bytes = std::fs::read(output_path).unwrap();
    assert!(bytes.len() <= 65_536);
    let json: Value = serde_json::from_slice(&bytes).unwrap();
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
    extremum: Extremum,
    comparison: Comparison,
    bound: Bound,
    negations: usize,
    cycle: bool,
) {
    assert_eq!(
        native(elements, extremum, comparison, bound, negations, cycle),
        clingo(source),
        "source: {source}"
    );
}

fn native(
    elements: &[Element],
    extremum: Extremum,
    comparison: Comparison,
    bound: Bound,
    negations: usize,
    cycle: bool,
) -> Models {
    let mut nodes = vec![
        Node::False,
        Node::Implies(0, 0),
        Node::Atom(0),
        Node::Atom(1),
        Node::Implies(2, 0),
        Node::Implies(3, 0),
        Node::Or(2, 4),
    ];
    let aggregate = append_extremum(
        &mut nodes,
        elements,
        extremum,
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
    result
}

const COMPARISONS: [(Comparison, &str); 6] = [
    (Comparison::Eq, "="),
    (Comparison::Ne, "!="),
    (Comparison::Lt, "<"),
    (Comparison::Le, "<="),
    (Comparison::Gt, ">"),
    (Comparison::Ge, ">="),
];
const EXTREMA: [(Extremum, &str); 2] = [(Extremum::Min, "min"), (Extremum::Max, "max")];
const GUARDS: [(Bound, &str); 5] = [
    (Bound::NegativeInfinity, "#inf"),
    (Bound::Number(-1), "-1"),
    (Bound::Number(0), "0"),
    (Bound::Number(1), "1"),
    (Bound::PositiveInfinity, "#sup"),
];

#[test]
#[ignore = "requires independently installed clingo; bounded five-second/64-KiB subprocesses"]
fn empty_extrema_keep_both_infinite_sentinels_and_all_comparisons() {
    for (extremum, name) in EXTREMA {
        for (comparison, operator) in COMPARISONS {
            for (bound, guard) in GUARDS {
                compare(
                    &format!("p :- #{name} {{}} {operator} {guard}."),
                    &[],
                    extremum,
                    comparison,
                    bound,
                    0,
                    false,
                );
            }
        }
    }
}

#[test]
#[ignore = "requires independently installed clingo; bounded five-second/64-KiB subprocesses"]
fn recursive_not_equal_coalesced_tuples_and_extreme_values_match_clingo() {
    for (extremum, name, empty) in [
        (Extremum::Min, "min", Bound::PositiveInfinity),
        (Extremum::Max, "max", Bound::NegativeInfinity),
    ] {
        let empty_text = if extremum == Extremum::Min {
            "#sup"
        } else {
            "#inf"
        };
        // != is an aggregate operator, not default negation of equality.
        compare(
            &format!("p :- #{name} {{1:p}} != {empty_text}."),
            &[Element {
                weight: 1,
                condition: 2,
            }],
            extremum,
            Comparison::Ne,
            empty,
            0,
            false,
        );
        compare(
            &format!("p :- not #{name} {{1:p}} = {empty_text}."),
            &[Element {
                weight: 1,
                condition: 2,
            }],
            extremum,
            Comparison::Eq,
            empty,
            1,
            false,
        );
        // These two raw elements have the same full tuple; their conditions
        // coalesce as p OR not p, which must retain its reduct structure.
        compare(
            &format!("p :- #{name} {{1,k:p; 1,k:not p}} = 1."),
            &[Element {
                weight: 1,
                condition: 6,
            }],
            extremum,
            Comparison::Eq,
            1.into(),
            0,
            false,
        );
        // Equal numeric values with different tuple tails remain two elements.
        compare(
            &format!("q :- #{name} {{1,x:p; 1,y:not q}} = 1. p :- q."),
            &[
                Element {
                    weight: 1,
                    condition: 2,
                },
                Element {
                    weight: 1,
                    condition: 5,
                },
            ],
            extremum,
            Comparison::Eq,
            1.into(),
            0,
            true,
        );
        for weight in [i32::MIN, i32::MIN + 1, i32::MAX - 1, i32::MAX] {
            for (comparison, operator) in COMPARISONS {
                // Numeric endpoint guards have separately recorded clingo
                // mismatches. Interior neighbours test all six comparisons.
                if (weight == i32::MIN || weight == i32::MAX) && comparison != Comparison::Eq {
                    continue;
                }
                compare(
                    &format!("p :- #{name} {{{weight},k:not p}} {operator} {weight}."),
                    &[Element {
                        weight,
                        condition: 4,
                    }],
                    extremum,
                    comparison,
                    weight.into(),
                    0,
                    false,
                );
            }
        }
        // Extreme element values are also valid with an interior scalar guard.
        for weight in [i32::MIN, i32::MAX] {
            for (comparison, operator) in COMPARISONS {
                compare(
                    &format!("p :- #{name} {{{weight},k:not p}} {operator} 0."),
                    &[Element {
                        weight,
                        condition: 4,
                    }],
                    extremum,
                    comparison,
                    0.into(),
                    0,
                    false,
                );
            }
        }
    }
}

#[test]
#[ignore = "characterizes six known clingo 5.8.2 endpoint mismatches; these are NOT equivalence passes"]
fn known_clingo_integer_endpoint_gaps_are_reported_separately() {
    let exact = Models::from([BTreeSet::new(), BTreeSet::from(["p".to_owned()])]);
    let observed = Models::from([BTreeSet::from(["p".to_owned()])]);
    for (extremum, name, weight, comparison, operator) in [
        (Extremum::Min, "min", i32::MIN, Comparison::Ne, "!="),
        (Extremum::Min, "min", i32::MAX, Comparison::Ne, "!="),
        (Extremum::Max, "max", i32::MIN, Comparison::Ne, "!="),
        (Extremum::Max, "max", i32::MAX, Comparison::Ne, "!="),
        (Extremum::Min, "min", i32::MAX, Comparison::Gt, ">"),
        (Extremum::Max, "max", i32::MIN, Comparison::Lt, "<"),
    ] {
        let source = format!("p :- #{name} {{{weight},k:not p}} {operator} {weight}.");
        assert_eq!(
            native(
                &[Element {
                    weight,
                    condition: 4
                }],
                extremum,
                comparison,
                weight.into(),
                0,
                false
            ),
            exact
        );
        assert_eq!(
            clingo(&source),
            observed,
            "The recorded oracle boundary changed; reassess this known compatibility gap: {source}"
        );
        assert_ne!(exact, observed);
        eprintln!("KNOWN COMPATIBILITY GAP: {source} exact={{empty, p}}, clingo5.8.2={{p}}");
    }
}

#[test]
#[ignore = "requires independently installed clingo; bounded five-second/64-KiB subprocesses"]
fn generated_recursive_signed_extrema_and_nested_default_negation_match_clingo() {
    for (extremum, name) in EXTREMA {
        for (comparison, operator) in COMPARISONS {
            for weights in [[1, 1], [0, 2], [-1, 1], [-2, -1]] {
                for (bound, guard) in GUARDS {
                    for (negations, negative) in [(0, ""), (1, "not "), (2, "not not ")] {
                        compare(
                            &format!(
                                "q :- {negative}#{name} {{{},x:p; {},y:not q}} {operator} {guard}. p :- q.",
                                weights[0], weights[1]
                            ),
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
                            extremum,
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
}
