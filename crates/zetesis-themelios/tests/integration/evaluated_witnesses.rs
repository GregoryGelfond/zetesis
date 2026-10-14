//! Local witness rows distinguish captured inputs from closed evaluated values.
use crate::support::finite_bindings as reference;
use crate::support::witnesses::{self, limited};
use zetesis_reference_support::{canonical, formula, prepare, prepared, resource_limits};

use std::collections::BTreeSet;
use std::fmt::Write as _;

use reference::{Models, holds, native, values};
use zetesis_themelios::{
    ExpansionFailure, ExpansionLimits, FormulaFailure, FormulaLimits, FormulaResource,
    admit_formula,
};

// These independently written finite substitutions retain whole source atoms.
// Data equality selects alternatives; it contributes no logical support itself.
const CASES: &[(&str, &str)] = &[
    // Former syntax refusals now retain complete occurrence semantics.
    ("p(1,2).q:-p(X,X+(1;2)):#true.", "p(1,2).q:-p(1,2)."),
    ("p(1,2).q:-p(X,X+(1..2)):#true.", "p(1,2).q:-p(1,2)."),
    (
        "{p(1,2);p(1,3);p(2,5)}.q:-p(X,X+(1;2)):#true.",
        "{p(1,2);p(1,3);p(2,5)}.q:-p(1,2).q:-p(1,3).",
    ),
    (
        "{p(1,2);p(1,3);p(2,5)}.q:-p(X,X+(1..2)):#true.",
        "{p(1,2);p(1,3);p(2,5)}.q:-p(1,2).q:-p(1,3).",
    ),
    (
        "{p(1,2);p(2,4)}.q:-p(X,X+1):#true.",
        "{p(1,2);p(2,4)}.q:-p(1,2).",
    ),
    (
        "{p(2,1);p(4,2)}.q:-p(X+1,X):#true.",
        "{p(2,1);p(4,2)}.q:-p(2,1).",
    ),
    (
        "{p(f(1,2));p(f(2,4))}.q:-p(f(X,X+1)):#true.",
        "{p(f(1,2));p(f(2,4))}.q:-p(f(1,2)).",
    ),
    (
        "{p(f(g(2),1));p(f(g(4),2))}.q:-p(f(g(X+1),X)):#true.",
        "{p(f(g(2),1));p(f(g(4),2))}.q:-p(f(g(2),1)).",
    ),
    (
        "{-p(-f(1,2));-p(f(2,3));p(-f(3,4))}.q:- -p(-f(X,X+1)):#true.",
        "{-p(-f(1,2));-p(f(2,3));p(-f(3,4))}.q:- -p(-f(1,2)).",
    ),
    (
        "{p((1,2));p((2,4));p(f(3,4))}.q:-p((X,X+1)):#true.",
        "{p((1,2));p((2,4));p(f(3,4))}.q:-p((1,2)).",
    ),
    (
        "{p(f(1),2);p(f(2),4)}.q:-p(f(X),X+1):#true.",
        "{p(f(1),2);p(f(2),4)}.q:-p(f(1),2).",
    ),
    (
        "{p(1,2,2);p(2,3,4)}.q:-p(X,X+1,X+1):#true.",
        "{p(1,2,2);p(2,3,4)}.q:-p(1,2,2).",
    ),
    (
        "{p(f(1,1),2);p(f(2,3),3)}.q:-p(f(X,X),X+1):#true.",
        "{p(f(1,1),2);p(f(2,3),3)}.q:-p(f(1,1),2).",
    ),
    (
        "{p(f(1),g(2),3);p(f(2),g(1),9)}.q:-p(f(X),g(Y),X+Y):#true.",
        "{p(f(1),g(2),3);p(f(2),g(1),9)}.q:-p(f(1),g(2),3).",
    ),
    (
        "{p(f(1,2));p(f(2,3))}.q:-p(f(X,Y+1)):Y=1.",
        "{p(f(1,2));p(f(2,3))}.q:-p(f(1,2)).",
    ),
    (
        "d(1).{p(f(1,2),3);p(f(2,2),4)}.q(X):-d(X),p(f(X,Y),X+Y):#true.",
        "d(1).{p(f(1,2),3);p(f(2,2),4)}.q(1):-d(1),p(f(1,2),3).",
    ),
    (
        "{p(1,2,a);p(2,3,b);p(2,4,c)}.q:-p(X,X+1,Y):X=1..2.",
        "{p(1,2,a);p(2,3,b);p(2,4,c)}.q:-p(1,2,a),p(2,3,b).",
    ),
    (
        "p(1,2).s(2,3).q:-p(X,X+1):#true;s(X,X+1):#true.",
        "p(1,2).s(2,3).q:-p(1,2),s(2,3).",
    ),
    (
        "{p(1,2,a);p(1,2,b);p(2,4,c)}.q:-p(X,X+1,_):#true.",
        "{p(1,2,a);p(1,2,b);p(2,4,c)}.q:-p(1,2,a).q:-p(1,2,b).",
    ),
    (
        "{p(-2,2);p(-3,2)}.q:-p(X,|X|):#true.",
        "{p(-2,2);p(-3,2)}.q:-p(-2,2).",
    ),
    (
        "{p(1,2);p(2,4);c}.q:-p(X,X+1):c.",
        "{p(1,2);p(2,4);c}.q:-p(1,2):c.",
    ),
    (
        "{p(1,2);p(2,4);c}.q:-p(X,X+1):not c.",
        "{p(1,2);p(2,4);c}.q:-p(1,2):not c.",
    ),
    (
        "{p(1,2);p(2,4);c}.q:-p(X,X+1):not not c.",
        "{p(1,2);p(2,4);c}.q:-p(1,2):not not c.",
    ),
    (
        "{p(1,2)}.p(1,2):-q.q:-p(X,X+1):#true.",
        "{p(1,2)}.p(1,2):-q.q:-p(1,2).",
    ),
    (
        "{p(1,2);p(2,4);q}.not q:-p(X,X+1):#true.",
        "{p(1,2);p(2,4);q}.not q:-p(1,2).",
    ),
    ("{p(1,2);p(2,4)}.q:-p(X,X+1):#false.", "{p(1,2);p(2,4)}.q."),
];

// The expression 1+1 needs no witness binding. Projection selects only atoms
// whose evaluated position equals 2; its complete truth is then negated.
const CLOSED_NEGATIVE_CASES: &[(&str, &[&str])] = &[
    ("p(1,2).q:-not p(_,1+1):#true.", &["p(1,2)"]),
    ("p(1,2).q:-not not p(_,1+1):#true.", &["p(1,2)", "q"]),
    ("p(1,3).q:-not p(_,1+1):#true.", &["p(1,3)", "q"]),
    ("p(1,3).q:-not not p(_,1+1):#true.", &["p(1,3)"]),
    ("p(f(1,2)).q:-not p(f(_,1+1)):#true.", &["p(f(1,2))"]),
    (
        "p(f(1,2)).q:-not not p(f(_,1+1)):#true.",
        &["p(f(1,2))", "q"],
    ),
];

#[test]
fn closed_arithmetic_preserves_negative_projection() {
    for &(source, atoms) in CLOSED_NEGATIVE_CASES {
        let expected = Models::from([atoms.iter().map(|&atom| atom.to_owned()).collect()]);
        assert_eq!(native(&formula(source)), expected, "{source}");
    }
}

#[test]
fn models_match_finite_substitution() {
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
    reference::stability_matches_exhaustion(CASES.iter().map(|&(source, _)| source), formula);
}

#[test]
fn frozen_truth_matches_finite_substitution() {
    let mut pairs = 0;
    for &(source, expanded) in CASES {
        let (left, right) = (formula(source), formula(expanded));
        let names: Vec<_> = left.atoms().iter().map(canonical).collect();
        let other: Vec<_> = right.atoms().iter().map(canonical).collect();
        assert_eq!(
            names.iter().collect::<BTreeSet<_>>(),
            other.iter().collect()
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
    reference::original_sources_match_clingo_full_models(
        CASES
            .iter()
            .map(|&(source, _)| source)
            .chain(CLOSED_NEGATIVE_CASES.iter().map(|&(source, _)| source)),
        formula,
    );
}

#[test]
fn witness_inputs_cannot_establish_condition_safety() {
    for source in [
        "p(1,2).q:-p(X,X+1):X>0.",
        "p(1,2).q:-p(X,X+1):not d(X).",
        "p(1,2).q:-p(X,X+1):X>0,Y=2..1.",
    ] {
        assert!(
            matches!(
                limited(
                    source,
                    ExpansionLimits::default(),
                    &FormulaLimits::default()
                ),
                Err(FormulaFailure::UnsafeVariable { .. })
            ),
            "{source}"
        );
    }
}

#[test]
fn witness_inputs_cannot_escape_the_alternative() {
    for source in [
        "p(1,2).q(X):-p(X,X+1):#true.",
        "p(1,2).q:-p(X,X+1):#true;X>0.",
        "p(1,2).q:-p(X,X+1):#false;X>0.",
    ] {
        assert!(
            matches!(
                limited(
                    source,
                    ExpansionLimits::default(),
                    &FormulaLimits::default()
                ),
                Err(FormulaFailure::UnsafeVariable { .. })
            ),
            "{source}"
        );
    }
}

#[test]
fn arithmetic_cannot_invert_a_captured_value() {
    for source in [
        "p(2).q:-p(X+1):#true.",
        "p(f(2)).q:-p(f(X+1)):#true.",
        "p(1,2).q:-p(X,Y+1):#true.",
        "q:-p(X,X+Y):#false.",
        "q:-p(X,X+_):#true.",
    ] {
        let error = limited(
            source,
            ExpansionLimits::default(),
            &FormulaLimits::default(),
        )
        .unwrap_err();
        assert!(
            matches!(error, FormulaFailure::UnboundArgumentInput { .. }),
            "{source}: {error}"
        );
        assert!(
            error
                .to_string()
                .contains("arithmetic inversion is unsupported")
        );
        assert_eq!(error.diagnostics().len(), 1);
    }
}

#[test]
fn preparation_requires_independently_bound_inputs() {
    // Preparation must refuse before any support traversal, including when the
    // current finite support carrier would be empty.
    assert!(matches!(
        prepare("q:-p(X,X+Y):#true."),
        Err(FormulaFailure::UnboundArgumentInput { .. })
    ));
}

#[test]
fn negative_witnesses_cannot_supply_inputs() {
    for sign in ["not", "not not"] {
        for term in ["X,X+1", "f(X,X+1)"] {
            let source = format!("p(1,2).q:-{sign} p({term}):#true.");
            assert!(
                matches!(
                    limited(
                        &source,
                        ExpansionLimits::default(),
                        &FormulaLimits::default()
                    ),
                    Err(FormulaFailure::UnsafeVariable { .. })
                ),
                "{source}"
            );
        }
    }
}

#[test]
fn undefined_arithmetic_requires_a_complete_row() {
    for source in ["q:-p(X,X/0):#true.", "p(g(1,2)).q:-p(f(X,X/0)):#true."] {
        assert!(
            limited(
                source,
                ExpansionLimits::default(),
                &FormulaLimits::default()
            )
            .is_ok(),
            "{source}"
        );
    }
    assert!(matches!(
        limited(
            "p(1,2).q:-p(X,X/0):#true.",
            ExpansionLimits::default(),
            &FormulaLimits::default()
        ),
        Err(FormulaFailure::Expansion(
            ExpansionFailure::Evaluation { .. }
        ))
    ));
}

#[test]
fn checked_arithmetic_preserves_failure_provenance() {
    for expression in ["X+1", "X/0", "X+f(1)"] {
        let source = format!("p(2147483647,0).q:-p(X,{expression}):#true.");
        let error = limited(
            &source,
            ExpansionLimits::default(),
            &FormulaLimits::default(),
        )
        .unwrap_err();
        assert!(
            matches!(
                error,
                FormulaFailure::Expansion(ExpansionFailure::Evaluation { .. })
            ),
            "{error}"
        );
        assert!(!error.diagnostics().is_empty());
    }
}

#[test]
fn rejected_values_preserve_the_next_row() {
    assert_eq!(
        native(&formula("p(f(1,1),3).p(f(2,2),3).q:-p(f(X,X),X+1):#true.")),
        Models::from([BTreeSet::from([
            "p(f(1,1),3)".into(),
            "p(f(2,2),3)".into(),
            "q".into()
        ])])
    );
}

#[test]
fn local_evaluation_cannot_create_recursive_support() {
    assert_eq!(
        native(&formula("p(1,2):-q.q:-p(X,X+1):#true.")),
        Models::from([BTreeSet::new()])
    );
}

#[test]
fn prepared_grounding_preserves_witness_models() {
    let source = CASES[0].0;
    let prepared = prepared(source);
    assert_eq!(
        native(&prepared.ground().unwrap()),
        native(&formula(source))
    );
}

#[test]
fn compiled_checks_retain_original_source() {
    let source = "p(1,2).q:-p(X,X+(1;2)):#true.";
    let admitted = formula(source);
    assert_eq!(admitted.source().expect("source input").text(), source);
    assert!(admitted.formula_origins().iter().flatten().any(|location| {
        admitted
            .source()
            .expect("source input")
            .slice(location.location().expect("parsed source").span)
            .unwrap()
            == "q:-p(X,X+(1;2)):#true."
    }));
}

#[test]
fn witness_captures_obey_the_variable_ceiling() {
    for (source, exact) in [("q:-p(X,X+1):#true.", 2), ("q:-p(f(X,X+1)):#true.", 3)] {
        for cap in [exact - 1, exact] {
            let result = admit_formula(
                source.into(),
                resource_limits::admission_formula(FormulaResource::Variables, cap),
                ExpansionLimits::default(),
                FormulaLimits::default(),
            );
            if cap == exact {
                assert!(result.is_ok(), "{result:?}");
            } else {
                assert!(
                    matches!(result, Err(FormulaFailure::Limit {
                resource: FormulaResource::Variables, observed, ..
            }) if observed == u128::from(exact)),
                    "{result:?}"
                );
            }
        }
    }
}

const BOUNDED: &str = "{p(f(1,2));p(f(2,4));p(f(3,4))}.q:-p(f(X,X+1)):#true.";

#[test]
fn source_work_admissions_obey_their_limits() {
    println!("term_work={}", witnesses::term_work_admissions(BOUNDED));
}

#[test]
fn source_storage_limit_is_inclusive() {
    println!("scalar_bytes={}", witnesses::scalar_bytes_ceiling(BOUNDED));
}

#[test]
fn substitution_limit_is_inclusive() {
    println!("substitutions={}", witnesses::substitution_ceiling(BOUNDED));
}

#[test]
fn work_prefixes_cannot_publish_partial_theories() {
    let expected = native(&formula(CASES[0].0));
    for cap in 0..4096 {
        match limited(
            CASES[0].0,
            ExpansionLimits::default(),
            &resource_limits::formula(FormulaResource::Work, cap),
        ) {
            Ok(admitted) => {
                assert!(cap > 0);
                assert_eq!(native(&admitted), expected);
                println!("complete_work={cap} refused_prefixes={cap}");
                return;
            }
            Err(FormulaFailure::Limit {
                resource: FormulaResource::Work,
                observed,
                limit,
                ..
            }) => {
                assert_eq!(limit, u128::from(cap));
                // Relation construction charges bounded groups of copied cells.
                assert!(observed > limit);
            }
            other => panic!("work cap {cap}: {other:?}"),
        }
    }
    panic!("bounded witness did not complete");
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config::with_cases(64))]
    #[test]
    fn arithmetic_selects_exact_complete_witnesses(
        rows in proptest::collection::vec((-3_i32..=3, -3_i32..=3, proptest::bool::ANY), 0..10),
        reverse in proptest::bool::ANY,
    ) {
        let consequent = if reverse { "p(f(X+1,X))" } else { "p(f(X,X+1))" };
        let mut source = format!("q:-{consequent}:#true.");
        let mut expected = BTreeSet::new();
        for (a, b, negative) in rows {
            let sign = if negative { "-" } else { "" };
            let atom = format!("p({sign}f({a},{b}))");
            write!(source, "{atom}.").unwrap();
            expected.insert(atom);
            let satisfies = if reverse { a == b+1 } else { b == a+1 };
            if satisfies && !negative { expected.insert("q".into()); }
        }
        proptest::prop_assert_eq!(native(&formula(&source)), Models::from([expected]));
    }
}
