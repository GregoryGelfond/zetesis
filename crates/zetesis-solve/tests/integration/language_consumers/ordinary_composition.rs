//! Hand-derived full answer families for interacting finite source constructs.

use super::{Case, Record, atom};
use zetesis_core::{Atom, Model, Sign, Value, ValueLimits, ValueNode};
use zetesis_test_support::programs::unary as number;

fn function(value: i32) -> Value {
    Value::from_nodes(
        vec![
            ValueNode::Function {
                name: "f".into(),
                sign: Sign::Positive,
                arity: 1,
            },
            ValueNode::Number(value),
        ],
        ValueLimits::default(),
    )
    .unwrap()
}

fn record(mut atoms: Vec<Atom>, cost: i64, display: String) -> Record {
    atoms.sort();
    Record {
        atoms: Model::new(atoms).unwrap(),
        costs: vec![(1, cost)],
        display,
    }
}

pub(super) fn cases() -> [Case; 5] {
    [
        disjuncts(),
        Case {
            source: include_str!("../../fixtures/language-consumers/pooled-objectives.lp"),
            file: "pooled-objectives.lp",
            reference_difference: None,
            // Both complete weight keys are active for the same selected row.
            answers: [1, 2]
                .map(|selected| {
                    record(
                        vec![
                            atom("d", vec![function(1)]),
                            atom("d", vec![function(2)]),
                            atom("pick", vec![function(selected)]),
                        ],
                        3,
                        format!("selected(f({selected}))"),
                    )
                })
                .into(),
        },
        Case {
            source: include_str!("../../fixtures/language-consumers/assignment-disjuncts.lp"),
            file: "assignment-disjuncts.lp",
            reference_difference: None,
            answers: [false, true]
                .into_iter()
                .flat_map(|selected| {
                    [false, true].map(move |head| {
                        let mut atoms = Vec::new();
                        if selected {
                            atoms.push(atom("p", vec![]));
                        }
                        let display = if head {
                            let count = i32::from(selected);
                            atoms.push(number("h", count));
                            format!("h({count})")
                        } else {
                            atoms.push(atom("z", vec![]));
                            "z".into()
                        };
                        record(atoms, 0, display)
                    })
                })
                .collect(),
        },
        Case {
            source: include_str!("../../fixtures/language-consumers/pooled-observations.lp"),
            file: "pooled-observations.lp",
            reference_difference: None,
            answers: [
                record(vec![number("n", 0)], 0, String::new()),
                record(
                    vec![number("n", 1), atom("p", vec![])],
                    0,
                    "captured(1)".into(),
                ),
            ]
            .into(),
        },
        body_conditions(),
    ]
}

fn disjuncts() -> Case {
    let mut answers = std::collections::BTreeSet::new();
    for selected in [&[][..], &[1][..], &[2][..], &[1, 2][..]] {
        let mut atoms = vec![number("d", 1), number("d", 2)];
        atoms.extend(selected.iter().map(|&value| number("p", value)));
        let mut fallback = atoms.clone();
        fallback.push(atom("z", vec![]));
        answers.insert(record(fallback, 0, "z".into()));
        // h(1) is eligible for either pool alternative; h(2) needs p(2).
        for value in [1, 2] {
            if (value == 1 && !selected.is_empty()) || selected.contains(&value) {
                let mut head = atoms.clone();
                head.push(number("h", value));
                answers.insert(record(head, 0, format!("h({value})")));
            }
        }
    }
    Case {
        source: include_str!("../../fixtures/language-consumers/pooled-disjuncts.lp"),
        file: "pooled-disjuncts.lp",
        reference_difference: None,
        answers,
    }
}

fn body_conditions() -> Case {
    Case {
        source: include_str!("../../fixtures/language-consumers/pooled-conditions.lp"),
        file: "pooled-conditions.lp",
        reference_difference: None,
        answers: [&[][..], &[1][..], &[2][..], &[1, 2][..]]
            .into_iter()
            .map(|selected| {
                let mut atoms = vec![number("d", 1), number("d", 2)];
                atoms.extend(
                    selected
                        .iter()
                        .map(|&value| atom("p", vec![function(value)])),
                );
                if selected.is_empty() {
                    record(atoms, 0, String::new())
                } else {
                    atoms.push(atom("hit", vec![]));
                    // Repeated pool rows retain the same two complete keys.
                    record(atoms, 3, "hit".into())
                }
            })
            .collect(),
    }
}
