//! Closed structures remain complete semantic atom and contribution identities.
#[path = "support/finite_bindings.rs"]
mod reference;
use reference::{Models, exhaustive, native};
use std::collections::BTreeSet;
use zetesis_core::Model;
use zetesis_cpu::Control;
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionLimits, FormulaLimits, admit_formula,
};
fn input(source: &str) -> AdmittedFormula {
    admit_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap_or_else(|e| panic!("{source}: {e}"))
}
fn expected(models: &[&[&str]]) -> Models {
    models
        .iter()
        .map(|row| row.iter().map(|s| (*s).into()).collect())
        .collect()
}
fn cases() -> Vec<(&'static str, Models)> {
    vec![
        ("#const a=1.p(f(-a)).", expected(&[&["p(f(-1))"]])),
        ("#const a=f(1).p(-a).", expected(&[&["p(-f(1))"]])),
        ("#const f=1.p(g(f())).", expected(&[&["p(g(1))"]])),
        ("q:-z(1)<a(0,0).", expected(&[&["q"]])),
        (
            "p(f(1,g(2))).q(X):-p(X).",
            expected(&[&["p(f(1,g(2)))", "q(f(1,g(2)))"]]),
        ),
        ("p(f()).p(f).", expected(&[&["p(f)"]])),
        (
            "p(()).p((1,)).p((1,2)).",
            expected(&[&["p(())", "p((1,))", "p((1,2))"]]),
        ),
        ("p(-f(1)).p(-a).", expected(&[&["p(-f(1))", "p(-a)"]])),
        ("p(--f(1)).", expected(&[&["p(f(1))"]])),
        ("#const a=1.p(f(a)).", expected(&[&["p(f(1))"]])),
        ("#const a=f(1).p(g(a)).", expected(&[&["p(g(f(1)))"]])),
        ("p(f(1+1)).", expected(&[&["p(f(2))"]])),
        (
            "{p(f(1))}.q:-not p(f(1)).",
            expected(&[&["p(f(1))"], &["q"]]),
        ),
        ("p(f(1)):-not not p(f(1)).", expected(&[&[], &["p(f(1))"]])),
        (
            "p(f(1))|q((1,)).p(f(1)):-q((1,)).q((1,)):-p(f(1)).",
            expected(&[&["p(f(1))", "q((1,))"]]),
        ),
        (
            "1{p(f(1));p(f(2))}1.",
            expected(&[&["p(f(1))"], &["p(f(2))"]]),
        ),
        ("p(f(1)).-p(f(1)).", expected(&[])),
        ("p(f(1)).-p(f(2)).", expected(&[&["p(f(1))", "-p(f(2))"]])),
        (
            "p(f(1)).q(X):-p(X),X=f(1).",
            expected(&[&["p(f(1))", "q(f(1))"]]),
        ),
        (
            "p(f(1)).p((1,)).n(N):-N=#count{X:p(X)}.",
            expected(&[&["p(f(1))", "p((1,))", "n(2)"]]),
        ),
        (
            "p(f(1)).p(f(2)).n(N):-N=#sum{1,X:p(X)}.",
            expected(&[&["p(f(1))", "p(f(2))", "n(2)"]]),
        ),
        (
            "p(f(1)).q:-#count{f(1):p(f(1));f(1):p(f(1))}=1.",
            expected(&[&["p(f(1))", "q"]]),
        ),
        ("#const a=f(b).#const b=1.p(a).", expected(&[&["p(f(1))"]])),
        (
            "p((1,2)).q(X):-p(X),X=(1,2).",
            expected(&[&["p((1,2))", "q((1,2))"]]),
        ),
        ("p(X):-X=(1,2).", expected(&[&["p((1,2))"]])),
        ("p(-f()).p(-f).", expected(&[&["p(-f)"]])),
        ("p(f(#inf,#sup)).", expected(&[&["p(f(#inf,#sup))"]])),
        (
            "#const k=(1,2).p(k).q(X):-p(X).",
            expected(&[&["p((1,2))", "q((1,2))"]]),
        ),
        ("p(f(1)):-#count{f(1):p(f(1))}!=0.", expected(&[&[]])),
        ("q:-f(1)<f(2).", expected(&[&["q"]])),
        ("q:-f(2)<f(1).", expected(&[&[]])),
        ("q:-f(9)< -a(0).", expected(&[&["q"]])),
        ("q:- -a< \"s\".", expected(&[&["q"]])),
        ("q:-()<a.", expected(&[&["q"]])),
        ("q:-(2,)<f(1).", expected(&[&["q"]])),
        ("q:-f(1)<f(0,0).", expected(&[&["q"]])),
    ]
}
#[test]
fn closed_sources_match_independent_complete_models() {
    for (source, wanted) in cases() {
        let p = input(source);
        assert_eq!(native(&p), wanted, "{source}");
        assert_eq!(exhaustive(&p), wanted, "{source}");
        assert_eq!(p.source().text(), source);
    }
}
#[test]
fn full_structural_objective_keys_and_candidate_bounds_agree_for_every_model() {
    let p = input("{p(f(1));p(f(2));q((1,))}.#minimize{2@1,X:p(X);2@1,f(1):p(f(2));-1@0,X:q(X)}.");
    let plan = zetesis_themelios::objective_bound::ObjectivePlan::new(
        p.theory(),
        p.atoms(),
        p.objectives(),
        zetesis_themelios::objective_bound::ObjectivePlanLimits::default(),
        &Control::default(),
    )
    .unwrap();
    let scores: Vec<_> = (0..1usize << p.atoms().len())
        .map(|bits| {
            let model = Model::new(
                p.atoms()
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| bits & (1 << i) != 0)
                    .map(|(_, a)| a.clone()),
            );
            zetesis_objective::evaluate(
                p.objectives(),
                &model,
                zetesis_objective::Limits::default(),
                &Control::default(),
            )
            .unwrap()
            .score()
            .clone()
        })
        .collect();
    for incumbent in &scores {
        let bound = plan
            .bound(
                incumbent,
                zetesis_themelios::objective_bound::ObjectiveBoundLimits::default(),
                &Control::default(),
            )
            .unwrap();
        assert!(bound.original().same_instance(p.theory()));
        for (bits, score) in scores.iter().enumerate() {
            let m = zetesis_ferraris::Interpretation::new(
                bound.theory(),
                (0..p.atoms().len()).filter(|i| bits & (1 << i) != 0),
            )
            .unwrap();
            assert_eq!(
                zetesis_ferraris::models(
                    bound.theory(),
                    &m,
                    zetesis_ferraris::Limits::default(),
                    &Control::default()
                )
                .unwrap(),
                score.compare_costs(incumbent) != std::cmp::Ordering::Greater
            );
        }
    }
    assert_eq!(scores.len(), 8);
    assert_eq!(
        scores
            .iter()
            .map(|s| s.costs().to_vec())
            .collect::<Vec<_>>(),
        vec![
            vec![(1, 0), (0, 0)],
            vec![(1, 2), (0, 0)],
            vec![(1, 4), (0, 0)],
            vec![(1, 4), (0, 0)],
            vec![(1, 0), (0, -1)],
            vec![(1, 2), (0, -1)],
            vec![(1, 4), (0, -1)],
            vec![(1, 4), (0, -1)]
        ]
    );
}
#[test]
fn unsupported_structural_sources_remain_refused() {
    for source in [
        "p(a).p(f(X)):-p(X).",
        // Finite constructor matching is covered positively in function_patterns.rs.
        "p(f(1/0)).",
        "p(f(2147483647+1)).",
        "#const a=f(a).p(a).",
        "#const a=f(b).#const b=g(a).p(a).",
    ] {
        assert!(
            admit_formula(
                source.into(),
                AdmissionOptions::default(),
                ExpansionLimits::default(),
                FormulaLimits::default()
            )
            .is_err(),
            "{source}"
        );
    }
}
#[test]
#[ignore = "requires external clingo; original structural sources with exact full models"]
fn closed_structures_match_clingo() {
    for (source, wanted) in cases() {
        let out = reference::external(source, true);
        assert_eq!(out["Models"]["More"], "no");
        let models: Models = out["Call"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|c| c["Witnesses"].as_array().into_iter().flatten())
            .map(|w| {
                w["Value"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|a| a.as_str().unwrap().to_owned())
                    .collect::<BTreeSet<_>>()
            })
            .collect();
        assert_eq!(models, wanted, "{source}");
        assert_eq!(native(&input(source)), wanted);
    }
}
#[test]
fn arbitrary_public_structures_are_bounded_before_observation_and_retry_is_exact() {
    use zetesis_core::{Atom, Predicate, Sign, Value, ValueLimits, ValueNode};
    use zetesis_themelios::observation::{ErrorKind, Limits, Resource};
    let p = input("#show. #show seen(X):p(X).");
    let depth = 80;
    let mut nodes = vec![
        ValueNode::Function {
            name: "f".into(),
            sign: Sign::Negative,
            arity: 1
        };
        depth
    ];
    nodes.push(ValueNode::Number(i32::MIN));
    let value = Value::from_nodes(
        nodes,
        ValueLimits {
            max_depth: depth + 1,
            ..ValueLimits::default()
        },
    )
    .unwrap();
    let model =
        Model::new([Atom::new(Predicate::new("p", 1).unwrap(), vec![value.clone()]).unwrap()]);
    let observation = p.metadata().observations();
    let error = observation
        .render(
            &model,
            p.metadata().output(),
            Limits::default(),
            &Control::default(),
        )
        .unwrap_err();
    assert!(matches!(
        error.kind(),
        ErrorKind::Limit {
            resource: Resource::Depth,
            ..
        }
    ));
    let limits = Limits {
        max_symbol_depth: depth + 3,
        max_symbol_nodes: depth + 3,
        ..Limits::default()
    };
    let result = observation
        .render(&model, p.metadata().output(), limits, &Control::default())
        .unwrap();
    let Value::Structured(value) = value else {
        panic!("structure")
    };
    assert_eq!(result.text(), format!("seen({value})"));
    let control = Control::default();
    control.cancel();
    assert!(matches!(
        observation
            .render(&model, p.metadata().output(), limits, &control)
            .unwrap_err()
            .kind(),
        ErrorKind::Stopped(_)
    ));
    let exact = Limits {
        max_work: result.statistics().work,
        ..limits
    };
    assert_eq!(
        observation
            .render(&model, p.metadata().output(), exact, &Control::default())
            .unwrap()
            .text(),
        result.text()
    );
    assert!(matches!(
        observation
            .render(
                &model,
                p.metadata().output(),
                Limits {
                    max_work: exact.max_work - 1,
                    ..exact
                },
                &Control::default()
            )
            .unwrap_err()
            .kind(),
        ErrorKind::Limit {
            resource: Resource::Work,
            ..
        }
    ));
}
#[test]
fn compound_payload_admission_refuses_with_original_location() {
    let source = "% provenance\np(f(1,g(2))).";
    let failure = admit_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits {
            max_scalar_bytes: 1,
            ..ExpansionLimits::default()
        },
        FormulaLimits::default(),
    )
    .unwrap_err();
    assert!(matches!(
        &failure,
        zetesis_themelios::FormulaFailure::Expansion(zetesis_themelios::ExpansionFailure::Limit {
            resource: zetesis_themelios::ExpansionResource::ScalarBytes,
            limit: 1,
            ..
        })
    ));
    let location = failure.diagnostics()[0].primary().location;
    assert_eq!(location.source, AdmissionOptions::default().source_id);
    let p = input(source);
    assert!(p.source().slice(location.span).unwrap().contains("p("));
    assert_eq!(p.source().text(), source);
    assert_eq!(native(&p), expected(&[&["p(f(1,g(2)))"]]));
}

fn formula_values(
    theory: &zetesis_ferraris::Theory,
    bits: usize,
    frozen: Option<&[bool]>,
) -> Vec<bool> {
    use zetesis_ferraris::Node;
    let mut result = Vec::new();
    for (index, node) in theory.nodes().iter().enumerate() {
        let truth = match *node {
            Node::False => false,
            Node::Atom(atom) => bits & (1 << atom) != 0,
            Node::And(a, b) => result[a] && result[b],
            Node::Or(a, b) => result[a] || result[b],
            Node::Implies(a, b) => !result[a] || result[b],
        };
        result.push(truth && frozen.is_none_or(|mask| mask[index]));
    }
    result
}
#[test]
fn signed_structural_choices_match_manual_formulas_in_every_frozen_world() {
    use zetesis_ferraris::{Node, Theory};
    let p = input("{p(f(1));-p(f(1))}.");
    assert_eq!(p.atoms().len(), 2);
    let positive = p
        .atoms()
        .iter()
        .position(|a| a.predicate().sign() == zetesis_core::Sign::Positive)
        .unwrap();
    let negative = 1 - positive;
    let manual = Theory::new(
        2,
        vec![
            Node::False,
            Node::Atom(positive),
            Node::Atom(negative),
            Node::Implies(1, 0),
            Node::Implies(2, 0),
            Node::Or(1, 3),
            Node::Or(2, 4),
            Node::And(1, 2),
            Node::Implies(7, 0),
        ],
        vec![5, 6, 8],
        zetesis_ferraris::AdmissionLimits::default(),
    )
    .unwrap();
    let holds = |t: &Theory, values: &[bool]| t.roots().iter().all(|r| values[*r]);
    for outer in 0..4 {
        let actual_mask = formula_values(p.theory(), outer, None);
        let manual_mask = formula_values(&manual, outer, None);
        assert_eq!(
            holds(p.theory(), &actual_mask),
            holds(&manual, &manual_mask)
        );
        for inner in 0..4 {
            assert_eq!(
                holds(
                    p.theory(),
                    &formula_values(p.theory(), inner, Some(&actual_mask))
                ),
                holds(&manual, &formula_values(&manual, inner, Some(&manual_mask))),
                "M={outer} J={inner}"
            );
        }
    }
}
