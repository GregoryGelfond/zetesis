//! Local witness rows distinguish captured inputs from closed evaluated values.
#[path = "support/finite_bindings.rs"]
mod reference;

use std::collections::BTreeSet;
use std::fmt::Write as _;

use reference::{Models, atom_text, exhaustive, external, holds, native, values};
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionFailure, ExpansionLimits, ExpansionResource,
    FormulaFailure, FormulaLimits, FormulaResource, admit_formula, prepare_formula,
};

fn limited(
    source: &str,
    expansion: ExpansionLimits,
    limits: &FormulaLimits,
) -> Result<AdmittedFormula, FormulaFailure> {
    admit_formula(
        source.into(),
        AdmissionOptions::default(),
        expansion,
        *limits,
    )
}

fn input(source: &str) -> AdmittedFormula {
    limited(
        source,
        ExpansionLimits::default(),
        &FormulaLimits::default(),
    )
    .unwrap_or_else(|error| panic!("{source}: {error}"))
}

// These independently written finite substitutions retain whole source atoms.
// Data equality selects alternatives; it contributes no logical support itself.
const CASES: &[(&str, &str)] = &[
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
        assert_eq!(native(&input(source)), expected, "{source}");
    }
}

#[test]
fn models_match_finite_substitution() {
    for &(source, expanded) in CASES {
        assert_eq!(native(&input(source)), native(&input(expanded)), "{source}");
    }
}

#[test]
fn stability_matches_subset_enumeration() {
    for &(source, _) in CASES {
        let admitted = input(source);
        assert_eq!(native(&admitted), exhaustive(&admitted), "{source}");
    }
}

#[test]
fn frozen_truth_matches_finite_substitution() {
    let mut pairs = 0;
    for &(source, expanded) in CASES {
        let (left, right) = (input(source), input(expanded));
        let names: Vec<_> = left.atoms().iter().map(atom_text).collect();
        let other: Vec<_> = right.atoms().iter().map(atom_text).collect();
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
#[ignore = "requires an independently installed clingo"]
fn original_sources_match_clingo_full_models() {
    let mut total = 0;
    for source in CASES
        .iter()
        .map(|&(source, _)| source)
        .chain(CLOSED_NEGATIVE_CASES.iter().map(|&(source, _)| source))
    {
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
        assert_eq!(native(&input(source)), expected, "{source}");
        total += count;
    }
    println!(
        "complete_sources={} full_models={total}",
        CASES.len() + CLOSED_NEGATIVE_CASES.len()
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
        prepare_formula(
            "q:-p(X,X+Y):#true.".into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default()
        ),
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
fn nested_alternatives_remain_explicit_refusals() {
    for source in [
        "p(1,2).q:-p(X,X+(1;2)):#true.",
        "p(1,2).q:-p(X,X+(1..2)):#true.",
    ] {
        assert!(
            matches!(
                limited(
                    source,
                    ExpansionLimits::default(),
                    &FormulaLimits::default()
                ),
                Err(FormulaFailure::Expansion(ExpansionFailure::Admission(
                    zetesis_themelios::AdmissionFailure::Profile {
                        feature: zetesis_themelios::ProfileFeature::Term,
                        ..
                    }
                )))
            ),
            "{source}"
        );
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
        native(&input("p(f(1,1),3).p(f(2,2),3).q:-p(f(X,X),X+1):#true.")),
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
        native(&input("p(1,2):-q.q:-p(X,X+1):#true.")),
        Models::from([BTreeSet::new()])
    );
}

#[test]
fn prepared_grounding_preserves_witness_models() {
    let source = CASES[0].0;
    let prepared = prepare_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    assert_eq!(native(&prepared.ground().unwrap()), native(&input(source)));
}

#[test]
fn compiled_checks_retain_original_source() {
    let source = CASES[0].0;
    let admitted = input(source);
    assert_eq!(admitted.source().text(), source);
    assert!(admitted.formula_origins().iter().flatten()
        .any(|location| admitted.source().slice(location.span).unwrap() == "q:-p(X,X+1):#true."));
}

#[test]
fn witness_captures_obey_the_variable_ceiling() {
    for (source, exact) in [("q:-p(X,X+1):#true.", 2), ("q:-p(f(X,X+1)):#true.", 3)] {
        for cap in [exact - 1, exact] {
            let mut options = AdmissionOptions::default();
            options.core_limits.max_variables_per_template = cap;
            let result = admit_formula(
                source.into(),
                options,
                ExpansionLimits::default(),
                FormulaLimits::default(),
            );
            if cap == exact {
                assert!(result.is_ok(), "{result:?}");
            } else {
                assert!(
                    matches!(result, Err(FormulaFailure::Limit {
                resource: FormulaResource::Variables, observed, ..
            }) if observed == exact as u128),
                    "{result:?}"
                );
            }
        }
    }
}

const BOUNDED: &str = "{p(f(1,2));p(f(2,4));p(f(3,4))}.q:-p(f(X,X+1)):#true.";

fn first_cap(
    mut run: impl FnMut(usize) -> Result<AdmittedFormula, FormulaFailure>,
    refused: impl Fn(&FormulaFailure) -> bool,
) -> usize {
    let (mut low, mut high) = (0, 1_000_000);
    assert!(run(high).is_ok());
    while low < high {
        let middle = low + (high - low) / 2;
        match run(middle) {
            Ok(_) => high = middle,
            Err(error) => {
                assert!(refused(&error), "{error}");
                low = middle + 1;
            }
        }
    }
    assert!(low > 0);
    assert!(refused(&run(low - 1).unwrap_err()));
    assert_eq!(native(&run(low).unwrap()), native(&input(BOUNDED)));
    low
}

#[test]
fn source_work_limit_is_inclusive() {
    let cap = first_cap(
        |cap| {
            limited(
                BOUNDED,
                ExpansionLimits {
                    max_term_work: cap,
                    ..Default::default()
                },
                &FormulaLimits::default(),
            )
        },
        |error| {
            matches!(
                error,
                FormulaFailure::Expansion(ExpansionFailure::Limit {
                    resource: ExpansionResource::TermWork,
                    ..
                })
            )
        },
    );
    println!("term_work={cap}");
}

#[test]
fn source_storage_limit_is_inclusive() {
    let cap = first_cap(
        |cap| {
            limited(
                BOUNDED,
                ExpansionLimits {
                    max_scalar_bytes: cap,
                    ..Default::default()
                },
                &FormulaLimits::default(),
            )
        },
        |error| {
            matches!(
                error,
                FormulaFailure::Expansion(ExpansionFailure::Limit {
                    resource: ExpansionResource::ScalarBytes,
                    ..
                })
            )
        },
    );
    println!("scalar_bytes={cap}");
}

#[test]
fn substitution_limit_is_inclusive() {
    let cap = first_cap(
        |cap| {
            limited(
                BOUNDED,
                ExpansionLimits::default(),
                &FormulaLimits {
                    max_substitutions: cap as u64,
                    ..Default::default()
                },
            )
        },
        |error| matches!(error, FormulaFailure::Limit { resource: FormulaResource::Substitutions, observed, limit, .. } if *observed == limit + 1),
    );
    println!("substitutions={cap}");
}

#[test]
fn work_prefixes_cannot_publish_partial_theories() {
    let expected = native(&input(CASES[0].0));
    for cap in 0..4096 {
        match limited(
            CASES[0].0,
            ExpansionLimits::default(),
            &FormulaLimits {
                max_work: cap,
                ..Default::default()
            },
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
                assert_eq!(observed, limit + 1);
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
        proptest::prop_assert_eq!(native(&input(&source)), Models::from([expected]));
    }
}
