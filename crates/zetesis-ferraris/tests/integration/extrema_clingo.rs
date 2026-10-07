//! Optional independent source oracle; clingo is never a production dependency.

use crate::support::aggregate_theories::push;
use crate::support::aggregate_theories::raw;
use crate::support::clingo_models::{Models, clingo};
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{
    AdmissionLimits, AggregateComparison as Comparison, AggregateElement as Element,
    AggregateExtremum as Extremum, AggregateLimits, ExtremumBound as Bound, Interpretation, Limits,
    Node, Theory, append_extremum, check,
};

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
    let mut nodes = raw(vec![
        Node::falsum(),
        Node::implies(0, 0),
        Node::atom(0),
        Node::atom(1),
        Node::implies(2, 0),
        Node::implies(3, 0),
        Node::or_pair([2, 4]),
    ]);
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
        let next = nodes.view().len();
        push(&mut nodes, zetesis_ferraris::NodeView::Implies(body, 0));
        body = next;
    }
    let mut roots = vec![nodes.view().len()];
    push(
        &mut nodes,
        zetesis_ferraris::NodeView::Implies(body, if cycle { 3 } else { 2 }),
    );
    if cycle {
        roots.push(nodes.view().len());
        push(&mut nodes, zetesis_ferraris::NodeView::Implies(3, 2));
    }
    let theory = Theory::new(2, nodes.into_parts(), roots, AdmissionLimits::default()).unwrap();
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
#[ignore = "requires clingo: empty extrema keep both infinite sentinels and all comparisons; bounded five-second/64-KiB subprocesses"]
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
#[ignore = "requires clingo: recursive not equal coalesced tuples and extreme values match clingo; bounded five-second/64-KiB subprocesses"]
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

/// The endpoint probes the README cites as evidence: each case's source, its
/// exact family, clingo 5.8.2's observed family and their classification.
const BOUNDARIES: &str = include_str!("../fixtures/extrema-clingo-5.8.2-boundaries.json");

#[test]
#[ignore = "requires clingo: characterizes six known endpoint mismatches; not equivalence passes"]
fn known_clingo_integer_endpoint_gaps_are_reported_separately() {
    let record: serde_json::Value = serde_json::from_str(BOUNDARIES).unwrap();
    let gaps: Vec<_> = record["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|case| case["classification"] == "known_mismatch")
        .collect();
    assert_eq!(gaps.len(), 6);
    assert_eq!(record["known_mismatches"], 6);
    for case in gaps {
        let source = case["source"].as_str().unwrap();
        let (extremum, _) = EXTREMA
            .into_iter()
            .find(|(_, name)| case["function"] == *name)
            .unwrap();
        let (comparison, _) = COMPARISONS
            .into_iter()
            .find(|(_, operator)| case["comparison"] == *operator)
            .unwrap();
        let weight = i32::try_from(case["weight"].as_i64().unwrap()).unwrap();
        let guard = i32::try_from(case["guard"].as_i64().unwrap()).unwrap();
        let exact = recorded(&case["exact_models"]);
        let observed = recorded(&case["observed_models"]);
        assert_eq!(
            native(
                &[Element {
                    weight,
                    condition: 4
                }],
                extremum,
                comparison,
                guard.into(),
                0,
                false
            ),
            exact,
            "{source}"
        );
        assert_eq!(
            clingo(source),
            observed,
            "The recorded oracle boundary changed; reassess this known compatibility gap: {source}"
        );
        assert_ne!(exact, observed);
        eprintln!("KNOWN COMPATIBILITY GAP: {source} exact={exact:?}, clingo5.8.2={observed:?}");
    }
}

/// A family of models as the boundary record spells it.
fn recorded(models: &serde_json::Value) -> Models {
    models
        .as_array()
        .unwrap()
        .iter()
        .map(|model| {
            model
                .as_array()
                .unwrap()
                .iter()
                .map(|atom| atom.as_str().unwrap().to_owned())
                .collect()
        })
        .collect()
}

#[test]
#[ignore = "requires clingo: generated recursive signed extrema and nested default negation match clingo; bounded five-second/64-KiB subprocesses"]
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
