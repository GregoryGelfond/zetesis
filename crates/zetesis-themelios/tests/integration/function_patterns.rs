//! Constructor patterns retain exact source atoms through finite extraction.
use crate::support::finite_bindings as reference;
use reference::{Models, exhaustive, external, holds, native, values};
use std::collections::BTreeSet;
use std::fmt::Write as _;
use zetesis_reference_support::{canonical, formula};
use zetesis_themelios::{
    AdmissionOptions, AnalysisBasis, ExpansionFailure, ExpansionLimits, ExpansionResource,
    FormulaFailure, FormulaLimits, FormulaResource, admit_formula, prepare_formula,
};

// The right sides are finite substitutions written independently of the matcher.
// They retain whole body atoms, including anonymous parts and explicit negation.
const CASES: &[(&str, &str)] = &[
    (
        "{q(f(1));q(f(2))}.p(X):-q(f(X)).",
        "{q(f(1));q(f(2))}.p(1):-q(f(1)).p(2):-q(f(2)).",
    ),
    (
        "{q(f(1));q(g(2))}.p(X):-q(f(X)).",
        "{q(f(1));q(g(2))}.p(1):-q(f(1)).",
    ),
    (
        "{q(f(1));q(-f(2));q(g(3))}.p(X):-q(-f(X)).",
        "{q(f(1));q(-f(2));q(g(3))}.p(2):-q(-f(2)).",
    ),
    (
        "{q(f(1));q(f(2,3));q(f)}.p(X):-q(f(X)).",
        "{q(f(1));q(f(2,3));q(f)}.p(1):-q(f(1)).",
    ),
    (
        "{q(f(g(1),(2,x)));q(f(g(3),(4,y)))}.p(X,Y):-q(f(g(X),(Y,_))).",
        "{q(f(g(1),(2,x)));q(f(g(3),(4,y)))}.p(1,2):-q(f(g(1),(2,x))).p(3,4):-q(f(g(3),(4,y))).",
    ),
    (
        "{q((-f(1),g(2)));q((f(3),g(4)))}.p(X,Y):-q((-f(X),g(Y))).",
        "{q((-f(1),g(2)));q((f(3),g(4)))}.p(1,2):-q((-f(1),g(2))).",
    ),
    (
        "{q(f(1));q(-f(2))}.p(X):-q(-(-f(X))).",
        "{q(f(1));q(-f(2))}.p(1):-q(f(1)).",
    ),
    (
        "{q(f(1,1));q(f(1,2));q(f(2,2))}.p(X):-q(f(X,X)).",
        "{q(f(1,1));q(f(1,2));q(f(2,2))}.p(1):-q(f(1,1)).p(2):-q(f(2,2)).",
    ),
    (
        "d(2).{q(f(1));q(f(2))}.p(X):-q(f(X)),d(X).",
        "d(2).{q(f(1));q(f(2))}.p(2):-q(f(2)),d(2).",
    ),
    (
        "{q(f(1),g(1));q(f(1),g(2))}.p(X):-q(f(X),g(X)).",
        "{q(f(1),g(1));q(f(1),g(2))}.p(1):-q(f(1),g(1)).",
    ),
    (
        "{q(f((),f));q(f(f,f))}.p(X):-q(f(X,f())).",
        "{q(f((),f));q(f(f,f))}.p(()):-q(f((),f)).p(f):-q(f(f,f)).",
    ),
    (
        "{q(f(1,g(2)));q(f(3,g(4)))}.p:-q(f(_,g(_))).",
        "{q(f(1,g(2)));q(f(3,g(4)))}.p:-q(f(1,g(2))).p:-q(f(3,g(4))).",
    ),
    ("p(X):-q(f(X)).", ""),
    (
        "{q(f(1));q(f(2))}.1{p(X):q(f(X))}1.",
        "{q(f(1));q(f(2))}.1{p(1):q(f(1));p(2):q(f(2))}1.",
    ),
    (
        "{q(f(1));q(f(2))}.n(N):-N=#count{X:q(f(X))}.",
        "{q(f(1));q(f(2))}.n(N):-N=#count{1:q(f(1));2:q(f(2))}.",
    ),
    (
        "{q(f(a));q(f(z))}.n(N):-N=#min{X:q(f(X))}.",
        "{q(f(a));q(f(z))}.n(N):-N=#min{a:q(f(a));z:q(f(z))}.",
    ),
    (
        "{-q(-f(1));-q(-f(2))}.p(X):--q(-f(X)).",
        "{-q(-f(1));-q(-f(2))}.p(1):--q(-f(1)).p(2):--q(-f(2)).",
    ),
    (
        "{q(f(1));q(f(2))}.p(X):-q(f(X)),not not p(X).",
        "{q(f(1));q(f(2))}.p(1):-q(f(1)),not not p(1).p(2):-q(f(2)),not not p(2).",
    ),
    (
        "{q(f(1));q(f(2));p(1);p(2)}.r:-p(X):q(f(X)).",
        "{q(f(1));q(f(2));p(1);p(2)}.r:-p(1):q(f(1));p(2):q(f(2)).",
    ),
    (
        "q(f(1)).p(X):-q(f(X)).q(f(2)):-p(1).",
        "q(f(1)).p(1):-q(f(1)).p(2):-q(f(2)).q(f(2)):-p(1).",
    ),
    (
        "q(f(1)).p(X;X+1):-q(f(X)).",
        "q(f(1)).p(1):-q(f(1)).p(2):-q(f(1)).",
    ),
];

// Ground arithmetic is normalized before pattern classification. The first
// group was supported by the original tuple-only matcher; the second composes
// that normalization with the newly supported function pattern.
const PRIOR_NUMERIC_ARGUMENTS: &[(&str, &[&str])] = &[
    ("q(-1).p:-q(-1).", &["q(-1)", "p"]),
    ("q(-1).p:-q(-(1)).", &["q(-1)", "p"]),
    ("q(-1).p:-q(-(-(-1))).", &["q(-1)", "p"]),
    ("q(-1,1).p(X):-q(-1,X).", &["q(-1,1)", "p(1)"]),
    ("q(-1,(2,x)).p(X):-q(-(1),(X,_)).", &["q(-1,(2,x))", "p(2)"]),
];
const MIXED_NUMERIC_ARGUMENTS: &[(&str, &[&str])] = &[
    ("q(-1,f(2)).p(X):-q(-1,f(X)).", &["q(-1,f(2))", "p(2)"]),
    ("q(-1,f(2)).p(X):-q(-(1),f(X)).", &["q(-1,f(2))", "p(2)"]),
    ("q(-1,-f(2)).p(X):-q(-1,-f(X)).", &["q(-1,-f(2))", "p(2)"]),
];

fn assert_single_model(source: &str, expected: &[&str]) {
    assert_eq!(
        native(&formula(source)),
        Models::from([expected.iter().map(|atom| (*atom).to_owned()).collect()]),
        "{source}"
    );
}

#[test]
fn ground_numeric_arguments_keep_prior_models() {
    for &(source, expected) in PRIOR_NUMERIC_ARGUMENTS {
        assert_single_model(source, expected);
    }
}

#[test]
fn numeric_arguments_compose_with_function_patterns() {
    for &(source, expected) in MIXED_NUMERIC_ARGUMENTS {
        assert_single_model(source, expected);
    }
}

#[test]
fn ground_negation_normalizes_before_matching() {
    use zetesis_themelios::logical::{
        program::{BodyElement, LiteralInner, Statement},
        symbol::Symbol,
        term::Term,
    };
    for &(source, _) in PRIOR_NUMERIC_ARGUMENTS
        .iter()
        .chain(MIXED_NUMERIC_ARGUMENTS)
    {
        let admitted = formula(source);
        let mut bodies = 0;
        for statement in admitted.analyzed_program().statements() {
            let Statement::Rule(rule) = statement.get() else {
                panic!("rule");
            };
            for element in rule.body().get().elements() {
                let BodyElement::Literal(literal) = element.get() else {
                    panic!("literal");
                };
                let LiteralInner::Atom(atom) = &literal.inner else {
                    panic!("atom");
                };
                assert!(
                    matches!(
                        atom.get().argument_terms().next(),
                        Some(Term::Symbolic(Symbol::Number(-1)))
                    ),
                    "{source}"
                );
                bodies += 1;
            }
        }
        assert_eq!(bodies, 1, "{source}");
    }
}

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
    let mut sources = 0;
    for source in CASES
        .iter()
        .map(|&(source, _)| source)
        .chain(PRIOR_NUMERIC_ARGUMENTS.iter().map(|&(source, _)| source))
        .chain(MIXED_NUMERIC_ARGUMENTS.iter().map(|&(source, _)| source))
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
        assert_eq!(native(&formula(source)), expected, "{source}");
        total += count;
        sources += 1;
    }
    println!("complete_sources={sources} full_models={total}");
}

#[test]
fn late_mismatch_leaves_the_next_row_unbound() {
    let admitted = formula("q(f(1,2),0).q(f(2,2),1).p(X,Z):-q(f(X,X),Z).");
    assert_eq!(
        native(&admitted),
        Models::from([BTreeSet::from([
            "q(f(1,2),0)".into(),
            "q(f(2,2),1)".into(),
            "p(2,1)".into(),
        ])])
    );
}

#[test]
fn inverse_consequent_remains_a_typed_refusal() {
    let source = "{p(f(1))}.q:-p(f(X+1)):#true.";
    // Evaluated witnesses now reach input-safety checking. Capturing f(1)
    // supplies the compared value, but cannot invert X+1 to bind X.
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

#[test]
fn negative_patterns_do_not_supply_bindings() {
    for source in ["p(X):-not q(f(X)).", "p(X):-not not q(f(X))."] {
        assert!(
            matches!(
                admit_formula(
                    source.into(),
                    AdmissionOptions::default(),
                    ExpansionLimits::default(),
                    FormulaLimits::default()
                ),
                Err(FormulaFailure::UnsafeVariable { .. })
            ),
            "{source}"
        );
    }
}

#[test]
fn extraction_does_not_bind_unmentioned_names() {
    assert!(matches!(
        admit_formula(
            "p(X):-q(f(Y)).".into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default()
        ),
        Err(FormulaFailure::UnsafeVariable { .. })
    ));
}

#[test]
fn prepared_models_match_source_admission() {
    let source = "q(f(1)).p(X):-q(f(X)).";
    let prepared = prepare_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    let admitted = prepared.ground().unwrap();
    assert_eq!(native(&admitted), native(&formula(source)));
}

#[test]
fn pattern_analysis_retains_function_terms() {
    use zetesis_themelios::logical::{
        program::{BodyElement, LiteralInner, Statement},
        term::Term,
    };
    let admitted = formula("q(f(1)).p(X):-q(f(X)).");
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
            atom.get().argument_terms().any(|term| {
                matches!(term,
                Term::Function { name, arguments } if name.as_str() == "f"
                    && matches!(arguments.as_slice(), [Term::Variable(_)]))
            })
        })
    }));
}

#[test]
fn captured_rules_retain_source_origins() {
    let admitted = formula("q(f(1)).p(X):-q(f(X)).");
    assert!(
        admitted
            .formula_origins()
            .iter()
            .flatten()
            .any(|origin| admitted.source().slice(origin.span).unwrap() == "p(X):-q(f(X)).")
    );
}

#[test]
fn captures_obey_the_variable_ceiling() {
    for limit in [1, 2] {
        let mut options = AdmissionOptions::default();
        options.core_limits.max_variables_per_template = limit;
        let result = admit_formula(
            "q(f(1,2)).p(X):-q(f(X,_)).".into(),
            options,
            ExpansionLimits::default(),
            FormulaLimits::default(),
        );
        if limit == 2 {
            assert!(result.is_ok());
        } else {
            assert!(matches!(
                result,
                Err(FormulaFailure::Limit {
                    resource: FormulaResource::Variables,
                    observed: 2,
                    limit: 1,
                    ..
                })
            ));
        }
    }
}

#[test]
fn anonymous_nodes_need_only_the_whole_capture() {
    let mut options = AdmissionOptions::default();
    options.core_limits.max_variables_per_template = 1;
    assert!(
        admit_formula(
            "q(f(1,g(2,3))).p:-q(f(_,g(_,_))).".into(),
            options,
            ExpansionLimits::default(),
            FormulaLimits::default()
        )
        .is_ok()
    );
}

#[test]
fn pattern_nodes_obey_the_value_ceiling() {
    // No support rows or constants consume value allowances in these programs.
    // Sign wrappers remain charged source nodes even though the flat shape
    // records their combined sign in one function node.
    for (source, exact) in [
        ("p(X):-q(f(X)).", 2),
        ("p(X):-q(-f(X)).", 3),
        ("p(X):-q(-(-f(X))).", 4),
    ] {
        for limit in [exact - 1, exact] {
            let result = prepare_formula(
                source.into(),
                AdmissionOptions::default(),
                ExpansionLimits {
                    max_values: limit,
                    ..ExpansionLimits::default()
                },
                FormulaLimits::default(),
            );
            if limit == exact {
                assert!(result.is_ok(), "{source}: {result:?}");
            } else {
                assert!(
                    matches!(result, Err(FormulaFailure::Expansion(ExpansionFailure::Limit {
                resource: ExpansionResource::Values, observed, ..
            })) if observed == exact as u128),
                    "{source}"
                );
            }
        }
    }
}

fn minimum_preparation_bytes(source: &str) -> usize {
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
fn empty_support_does_not_erase_pattern_bytes() {
    let short = minimum_preparation_bytes("p(X):-q(f(X)).");
    let long = minimum_preparation_bytes(&format!("p(X):-q(f{}(X)).", "x".repeat(256)));
    // A pattern must own its longer function name even when no row can match.
    assert!(long >= short + 256, "short={short} long={long}");
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config::with_cases(64))]
    #[test]
    fn repeated_names_select_exactly_agreeing_rows(rows in proptest::collection::vec((-3_i32..=3,-3_i32..=3,proptest::bool::ANY),0..10)) {
        let mut source = String::from("p(X):-q(f(X,g(X))).");
        let mut expected = BTreeSet::new();
        for (a,b,negative) in rows {
            let sign = if negative { "-" } else { "" };
            let atom = format!("q({sign}f({a},g({b})))");
            write!(source,"{atom}.").unwrap();
            expected.insert(atom);
            if a == b && !negative { expected.insert(format!("p({a})")); }
        }
        proptest::prop_assert_eq!(native(&formula(&source)),Models::from([expected]));
    }
    #[test]
    fn prebound_names_select_exactly_matching_rows(rows in proptest::collection::vec((-3_i32..=3,-3_i32..=3),0..10), fixed in -3_i32..=3) {
        let mut source = format!("d({fixed}).p(X,Y):-d(X),q(f(X,(Y,))).");
        let mut expected = BTreeSet::from([format!("d({fixed})")]);
        for (a,b) in rows {
            write!(source,"q(f({a},({b},))).").unwrap();
            expected.insert(format!("q(f({a},({b},)))"));
            if a == fixed { expected.insert(format!("p({a},{b})")); }
        }
        proptest::prop_assert_eq!(native(&formula(&source)),Models::from([expected]));
    }
}
