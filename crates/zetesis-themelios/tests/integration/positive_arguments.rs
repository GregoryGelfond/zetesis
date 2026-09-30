//! Evaluated positive arguments consume bindings and retain actual source atoms.
use crate::support::finite_bindings as reference;
use reference::{Models, exhaustive, external, holds, native, values};
use std::collections::BTreeSet;
use std::fmt::Write as _;
use zetesis_reference_support::{canonical, formula};
use zetesis_themelios::{
    AdmissionOptions, AnalysisBasis, ExpansionFailure, ExpansionLimits, ExpansionResource,
    FormulaFailure, FormulaLimits, FormulaResource, admit_formula, prepare_formula,
};

// Explicit finite substitutions retain each whole supporting atom. Arithmetic
// selects eligible substitutions; it supplies neither a premise nor an inverse.
const CASES: &[(&str, &str)] = &[
    // Former syntax refusals now retain complete occurrence semantics.
    (
        "d(1).p(2).q(X):-d(X),p(X+(1;2)).",
        "d(1).p(2).q(1):-d(1),p(2).",
    ),
    (
        "d(1).p(2).q(X):-d(X),p(X+(1..2)).",
        "d(1).p(2).q(1):-d(1),p(2).",
    ),
    (
        "d(1).{p(2);p(3);p(4)}.q(X):-d(X),p(X+(1;2)).",
        "d(1).{p(2);p(3);p(4)}.q(1):-d(1),p(2).q(1):-d(1),p(3).",
    ),
    (
        "d(1).{p(2);p(3);p(4)}.q(X):-d(X),p(X+(1..2)).",
        "d(1).{p(2);p(3);p(4)}.q(1):-d(1),p(2).q(1):-d(1),p(3).",
    ),
    ("d(1).p(2).q(X):-d(X),p(X+1).", "d(1).p(2).q(1):-d(1),p(2)."),
    ("d(1).p(2).q(X):-p(X+1),d(X).", "d(1).p(2).q(1):-p(2),d(1)."),
    ("p(2).q(X):-X=1,p(X+1).", "p(2).q(1):-p(2)."),
    (
        "p(2;3).q(X):-X=1..2,p(X+1).",
        "p(2;3).q(1):-p(2).q(2):-p(3).",
    ),
    (
        "{p(f(1),2);p(f(2),4)}.q(X):-p(f(X),X+1).",
        "{p(f(1),2);p(f(2),4)}.q(1):-p(f(1),2).",
    ),
    (
        "{p(f(1,2));p(f(2,4))}.q(X):-p(f(X,X+1)).",
        "{p(f(1,2));p(f(2,4))}.q(1):-p(f(1,2)).",
    ),
    (
        "d(2).{p(f(1),2);p(f(2),3)}.q(X):-p(f(X),X+1),d(X).",
        "d(2).{p(f(1),2);p(f(2),3)}.q(2):-p(f(2),3),d(2).",
    ),
    (
        "{p(f(1),g(2),3);p(f(2),g(1),9)}.q(X,Y):-p(f(X),g(Y),X+Y).",
        "{p(f(1),g(2),3);p(f(2),g(1),9)}.q(1,2):-p(f(1),g(2),3).",
    ),
    (
        "p(-1,f(2),3).q(X):-p(-(1),f(X),X+1).",
        "p(-1,f(2),3).q(2):-p(-1,f(2),3).",
    ),
    (
        "{p(-f(1,2));p(f(2,3))}.q(X):-p(-f(X,X+1)).",
        "{p(-f(1,2));p(f(2,3))}.q(1):-p(-f(1,2)).",
    ),
    (
        "p(f(g(2),1)).q(X):-p(f(g(X+1),X)).",
        "p(f(g(2),1)).q(1):-p(f(g(2),1)).",
    ),
    (
        "d(1).p(-1).q(X):-p(-X),d(X).",
        "d(1).p(-1).q(1):-p(-1),d(1).",
    ),
    (
        "d(-2).p(2).q(X):-d(X),p(|X|).",
        "d(-2).p(2).q(-2):-d(-2),p(2).",
    ),
    (
        "d(1).p(2).q(X+2):-d(X),p(X+1).",
        "d(1).p(2).q(3):-d(1),p(2).",
    ),
    (
        "d(1..2).p(2).{q(X):d(X),p(X+1)}.",
        "d(1..2).p(2).{q(1):d(1),p(2)}.",
    ),
    (
        "d(1..2).{p(2);p(3)}.n(N):-N=#count{X:d(X),p(X+1)}.",
        "d(1..2).{p(2);p(3)}.n(N):-N=#count{1:d(1),p(2);2:d(2),p(3)}.",
    ),
    (
        "d(1).{p(2);r(1)}.q:-r(X):d(X),p(X+1).",
        "d(1).{p(2);r(1)}.q:-r(1):d(1),p(2).",
    ),
    (
        "d(1).{p(2)}.q:-d(X),p(X+1),not r.r:-not q.",
        "d(1).{p(2)}.q:-d(1),p(2),not r.r:-not q.",
    ),
    (
        "d(1).{p(2);q(1)}.not q(X):-d(X),p(X+1).",
        "d(1).{p(2);q(1)}.not q(1):-d(1),p(2).",
    ),
    ("p(1).p(X+1):-p(X),p(X+0),X<2.", "p(1).p(2):-p(1),p(1)."),
    ("d(1).q(X):-d(X),p(X+1).", "d(1)."),
    ("q(X):-d(X),p(X/0).", ""),
    (
        "d(1..2).{p(2);p(3)}.1{q(X):d(X),p(X+1)}1.",
        "d(1..2).{p(2);p(3)}.1{q(1):d(1),p(2);q(2):d(2),p(3)}1.",
    ),
    (
        "d(1).{p(2,a);p(2,b)}.q(X):-d(X),p(X+1,_).",
        "d(1).{p(2,a);p(2,b)}.q(1):-d(1),p(2,a).q(1):-d(1),p(2,b).",
    ),
    (
        "{p(f(1),2,2);p(f(2),3,4)}.q(X):-p(f(X),X+1,X+1).",
        "{p(f(1),2,2);p(f(2),3,4)}.q(1):-p(f(1),2,2).",
    ),
];

#[test]
fn models_match_explicit_substitution() {
    for &(source, expanded) in CASES {
        assert_eq!(
            native(&formula(source)),
            native(&formula(expanded)),
            "{source}"
        );
    }
}

#[test]
fn stability_matches_subset_enumeration() {
    for &(source, _) in CASES {
        let admitted = formula(source);
        assert_eq!(native(&admitted), exhaustive(&admitted), "{source}");
    }
}

#[test]
fn frozen_truth_matches_explicit_substitution() {
    let mut pairs = 0;
    for &(source, expanded) in CASES {
        let left = formula(source);
        let right = formula(expanded);
        let names: Vec<_> = left.atoms().iter().map(canonical).collect();
        let other: Vec<_> = right.atoms().iter().map(canonical).collect();
        assert_eq!(
            names.iter().collect::<BTreeSet<_>>(),
            other.iter().collect(),
            "{source}"
        );
        assert!(names.len() <= 7);
        let remap = |mask: usize| {
            names.iter().enumerate().fold(0, |result, (index, name)| {
                result
                    | (usize::from(mask & (1 << index) != 0)
                        << other.iter().position(|other| other == name).unwrap())
            })
        };
        for outer in 0..1 << names.len() {
            let a = values(left.theory(), outer, None);
            let b = values(right.theory(), remap(outer), None);
            assert_eq!(
                holds(left.theory(), &a),
                holds(right.theory(), &b),
                "{source}"
            );
            for inner in 0..1 << names.len() {
                assert_eq!(
                    holds(left.theory(), &values(left.theory(), inner, Some(&a))),
                    holds(
                        right.theory(),
                        &values(right.theory(), remap(inner), Some(&b))
                    ),
                    "{source}: M={outer} J={inner}"
                );
                pairs += 1;
            }
        }
    }
    println!("source_pairs={} frozen_pairs={pairs}", CASES.len());
}

#[test]
#[ignore = "requires clingo: original sources match clingo full models"]
fn original_sources_match_clingo_full_models() {
    let mut total = 0;
    for &(source, _) in CASES {
        let result = external(source, true);
        assert_eq!(result["Models"]["More"], "no");
        let mut expected = Models::new();
        let mut count = 0;
        for call in result["Call"].as_array().unwrap() {
            if let Some(witnesses) = call["Witnesses"].as_array() {
                for witness in witnesses {
                    count += 1;
                    assert!(
                        expected.insert(
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
        assert_eq!(result["Models"]["Number"].as_u64(), Some(count));
        assert_eq!(native(&formula(source)), expected, "{source}");
        total += count;
    }
    println!("complete_sources={} full_models={total}", CASES.len());
}

#[test]
fn inverse_bindings_have_an_explicit_profile_refusal() {
    for source in [
        "p(2).q(X):-p(X+1).",
        "q(X):-p(X+1).",
        "q(f(2)).p(X):-q(f(X+1)).",
        "q(-1).p(X):-q(-X).",
        "p(2).q(X):-p(X+1),1=0.",
    ] {
        assert!(
            matches!(
                admit_formula(
                    source.into(),
                    AdmissionOptions::default(),
                    ExpansionLimits::default(),
                    FormulaLimits::default()
                ),
                Err(FormulaFailure::UnboundArgumentInput { .. })
            ),
            "{source}"
        );
    }
}

#[test]
fn anonymous_arithmetic_has_no_named_input() {
    assert!(matches!(
        admit_formula(
            "d(1).p(2).q:-d(X),p(X+_).".into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default()
        ),
        Err(FormulaFailure::UnboundArgumentInput { .. })
    ));
}

#[test]
fn input_refusal_explains_the_located_profile_boundary() {
    let error = admit_formula(
        "p(1).q(X):-p(X+1).".into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap_err();
    assert!(error.to_string().contains("independently bound input"));
    assert!(
        error
            .to_string()
            .contains("arithmetic inversion is unsupported")
    );
    assert_eq!(error.diagnostics().len(), 1);
}

#[test]
fn empty_support_cannot_supply_an_expression_input() {
    assert!(matches!(
        prepare_formula(
            "q(X):-p(X+1).".into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default()
        ),
        Err(FormulaFailure::UnboundArgumentInput { .. })
    ));
}

#[test]
fn local_inputs_do_not_make_an_outer_head_safe() {
    assert!(matches!(
        admit_formula(
            "d(1).p(2).q(X):-0<#count{Y:d(Y),p(Y+1)}.".into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default()
        ),
        Err(FormulaFailure::UnsafeVariable { .. })
    ));
}

#[test]
fn aggregate_proposals_must_match_captured_arguments() {
    assert_eq!(
        native(&formula("p(2).q(N):-N=#count{},p(N+1).")),
        native(&formula("p(2)."))
    );
}

#[test]
fn undefined_arithmetic_requires_a_complete_row() {
    for source in ["d(1).q(X):-d(X),p(X/0).", "p(1).q(X):-d(X),p(X/0)."] {
        assert!(
            admit_formula(
                source.into(),
                AdmissionOptions::default(),
                ExpansionLimits::default(),
                FormulaLimits::default()
            )
            .is_ok(),
            "{source}"
        );
    }
    assert!(matches!(
        admit_formula(
            "d(1).p(1).q(X):-d(X),p(X/0).".into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default()
        ),
        Err(FormulaFailure::Expansion(
            ExpansionFailure::Evaluation { .. }
        ))
    ));
}

#[test]
fn late_mismatch_preserves_the_next_row() {
    let source = "p(f(1,1),3).p(f(2,2),3).q(X):-p(f(X,X),X+1).";
    assert_eq!(
        native(&formula(source)),
        Models::from([BTreeSet::from([
            "p(f(1,1),3)".into(),
            "p(f(2,2),3)".into(),
            "q(2)".into()
        ])])
    );
}

#[test]
fn prepared_models_match_source_admission() {
    let source = "d(1).p(2).q(X):-p(X+1),d(X).";
    let prepared = prepare_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    assert_eq!(
        native(&prepared.ground().unwrap()),
        native(&formula(source))
    );
}

#[test]
fn analysis_retains_source_arithmetic() {
    use zetesis_themelios::logical::{
        program::{BodyElement, LiteralInner, Statement},
        term::Term,
    };
    let admitted = formula("d(1).p(2).q(X):-p(X+1),d(X).");
    assert_eq!(admitted.analysis_basis(), AnalysisBasis::NormalizedProgram);
    assert!(admitted.analyzed_program().statements().any(|statement| {
        let Statement::Rule(rule) = statement.get() else {
            return false;
        };
        rule.body().get().elements().any(|element| {
            let BodyElement::Literal(literal) = element.get() else {
                return false;
            };
            let LiteralInner::Atom(atom) = &literal.inner else {
                return false;
            };
            atom.get()
                .argument_terms()
                .any(|term| matches!(term, Term::BinaryOperation { .. }))
        })
    }));
}

#[test]
fn captured_rules_retain_source_origins() {
    let admitted = formula("d(1).p(2).q(X):-p(X+1),d(X).");
    assert!(
        admitted
            .formula_origins()
            .iter()
            .flatten()
            .any(|origin| admitted.source().slice(origin.span).unwrap() == "q(X):-p(X+1),d(X).")
    );
}

#[test]
fn captures_obey_the_variable_ceiling() {
    for (source, exact) in [("q(X):-d(X),p(X+1).", 2), ("q(X):-p(f(X,X+1)).", 3)] {
        for limit in [exact - 1, exact] {
            let mut options = AdmissionOptions::default();
            options.core_limits.max_variables_per_template = limit;
            let result = admit_formula(
                source.into(),
                options,
                ExpansionLimits::default(),
                FormulaLimits::default(),
            );
            if limit == exact {
                assert!(result.is_ok(), "{source}: {result:?}");
            } else {
                assert!(
                    matches!(result, Err(FormulaFailure::Limit { resource: FormulaResource::Variables, observed, .. }) if observed == exact as u128),
                    "{source}: {result:?}"
                );
            }
        }
    }
}

fn minimum_bytes(source: &str) -> usize {
    let prepare = |cap| {
        prepare_formula(
            source.into(),
            AdmissionOptions::default(),
            ExpansionLimits {
                max_scalar_bytes: cap,
                ..ExpansionLimits::default()
            },
            FormulaLimits::default(),
        )
    };
    let (mut lower, mut upper) = (0, ExpansionLimits::default().max_scalar_bytes);
    assert!(prepare(upper).is_ok());
    while lower < upper {
        let middle = lower + (upper - lower) / 2;
        if prepare(middle).is_ok() {
            upper = middle;
        } else {
            lower = middle + 1;
        }
    }
    assert!(prepare(lower).is_ok());
    assert!(matches!(
        prepare(lower - 1),
        Err(FormulaFailure::Expansion(ExpansionFailure::Limit {
            resource: ExpansionResource::ScalarBytes,
            ..
        }))
    ));
    lower
}

#[test]
fn empty_support_does_not_erase_expression_bytes() {
    // Authored plans are bounded before knowing whether a support row exists.
    let shallow = minimum_bytes("q(X):-d(X),p(X+1).");
    let deep = minimum_bytes("q(X):-d(X),p(X+1+2+3+4+5+6+7+8).");
    assert!(deep > shallow, "shallow={shallow} deep={deep}");
}

#[test]
fn work_prefixes_refuse_until_complete_admission() {
    let source = "d(1).p(2).q(X):-p(X+1),d(X).";
    let expected = native(&formula(source));
    // The bounded search checks every interrupted prefix, including comparison
    // readiness/evaluation and final formula emission; no prefix is an answer.
    for cap in 0..4096 {
        let result = admit_formula(
            source.into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits {
                max_work: cap,
                ..FormulaLimits::default()
            },
        );
        match result {
            Ok(admitted) => {
                assert!(cap > 0);
                assert_eq!(native(&admitted), expected);
                println!("complete_work={cap} refused_prefixes={cap}");
                return;
            }
            Err(FormulaFailure::Limit {
                resource: FormulaResource::Work,
                limit,
                observed,
                ..
            }) => {
                assert_eq!(limit, u128::from(cap));
                // Relation construction also charges bounded groups of cells.
                assert!(observed > limit);
            }
            other => panic!("work cap {cap}: {other:?}"),
        }
    }
    panic!("bounded fixture did not complete");
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config::with_cases(64))]
    #[test]
    fn structural_inputs_select_exact_arithmetic_rows(rows in proptest::collection::vec((-3_i32..=3,-3_i32..=3,proptest::bool::ANY),0..10)) {
        let mut source = String::from("q(X):-p(f(X),X+1).");
        let mut expected = BTreeSet::new();
        for (a,b,negative) in rows {
            let sign = if negative { "-" } else { "" };
            let atom = format!("p({sign}f({a}),{b})");
            write!(source,"{atom}.").unwrap();
            expected.insert(atom);
            if b == a + 1 && !negative { expected.insert(format!("q({a})")); }
        }
        proptest::prop_assert_eq!(native(&formula(&source)),Models::from([expected]));
    }
    #[test]
    fn independent_inputs_select_exact_support_rows(domain in proptest::collection::vec(-3_i32..=3,0..5), rows in proptest::collection::vec(-3_i32..=3,0..5), reverse in proptest::bool::ANY) {
        let mut source = String::from(if reverse { "q(X):-p(X+1),d(X)." } else { "q(X):-d(X),p(X+1)." });
        let mut expected = BTreeSet::new();
        for a in domain {
            write!(source,"d({a}).").unwrap();
            expected.insert(format!("d({a})"));
            if rows.contains(&(a+1)) { expected.insert(format!("q({a})")); }
        }
        for b in rows { write!(source,"p({b}).").unwrap(); expected.insert(format!("p({b})")); }
        proptest::prop_assert_eq!(native(&formula(&source)),Models::from([expected]));
    }
}
