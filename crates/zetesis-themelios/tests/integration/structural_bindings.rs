//! Positive tuple matching preserves full source atoms and frozen semantics.
use crate::support::finite_bindings as reference;
use reference::{Models, exhaustive, external, holds, native, values};
use std::collections::BTreeSet;
use zetesis_reference_support::{canonical, formula};
use zetesis_themelios::{AdmissionOptions, ExpansionLimits, FormulaLimits, admit_formula};

const PROJECTION: &str = "q((1,x),2).\np(A) :- q((A,_),_).\np(B) :- q((A,_),B).\n";
const EXTREMA_SOURCE: &str = "q((#inf,a)).q((#sup,b)).q((\"#inf\",c)).p(X):-q((X,_)).";
fn models(rows: &[&[&str]]) -> Models {
    rows.iter()
        .map(|row| row.iter().map(|atom| (*atom).to_owned()).collect())
        .collect()
}
#[test]
fn projection_bug_retains_the_complete_model() {
    assert_eq!(
        native(&formula(PROJECTION)),
        models(&[&["q((1,x),2)", "p(1)", "p(2)"]])
    );
}

const CASES: &[(&str, &str)] = &[
    (
        "{q((\"a\",x));q((\"b\",y))}.p(X):-q((X,_)).",
        "{q((\"a\",x));q((\"b\",y))}.p(\"a\"):-q((\"a\",x)).p(\"b\"):-q((\"b\",y)).",
    ),
    (
        "{q((1,2),1);q((2,3),4)}.p(X,Y):-q((X,Y),X).",
        "{q((1,2),1);q((2,3),4)}.p(1,2):-q((1,2),1).",
    ),
    (
        "{q((1,x),2);q((2,x),3)}.p(A):-q((A,_),_).",
        "{q((1,x),2);q((2,x),3)}.p(1):-q((1,x),2).p(2):-q((2,x),3).",
    ),
    (
        "{q((1,1));q((1,2));q((2,2))}.p(X):-q((X,X)).",
        "{q((1,1));q((1,2));q((2,2))}.p(1):-q((1,1)).p(2):-q((2,2)).",
    ),
    (
        "{q(((1,x),y));q((2,z))}.p(X):-q((X,_)).",
        "{q(((1,x),y));q((2,z))}.p((1,x)):-q(((1,x),y)).p(2):-q((2,z)).",
    ),
    (
        "{q((1,x));q((2,y))}.p(X):-d(X),q((X,_)).d(2).",
        "{q((1,x));q((2,y))}.p(2):-d(2),q((2,y)).d(2).",
    ),
    (
        "{q((1,x));q((2,y))}.p(X):-q((X,_)),d(X).d(2).",
        "{q((1,x));q((2,y))}.p(2):-q((2,y)),d(2).d(2).",
    ),
    (
        "{q((1,x));q((2,y))}.p:-q((_,_)).",
        "{q((1,x));q((2,y))}.p:-q((1,x)).p:-q((2,y)).",
    ),
    (
        "{q((1,x));q((2,y))}.p(Y):-q((X,_)),Y=X+1.",
        "{q((1,x));q((2,y))}.p(2):-q((1,x)).p(3):-q((2,y)).",
    ),
    (
        "{q((1,x));q((2,y))}.p(Y):-Y=X+1,q((X,_)).",
        "{q((1,x));q((2,y))}.p(2):-q((1,x)).p(3):-q((2,y)).",
    ),
    (
        "{q((1,x));q((2,y))}.p(X):-q((X,_)),not b.b:-not p(1).",
        "{q((1,x));q((2,y))}.p(1):-q((1,x)),not b.p(2):-q((2,y)),not b.b:-not p(1).",
    ),
    (
        "{q((1,x));q((2,y))}.1{p(X):q((X,_))}1.",
        "{q((1,x));q((2,y))}.1{p(1):q((1,x));p(2):q((2,y))}1.",
    ),
    (
        "{q((1,x));q((2,y))}.n(N):-N=#count{X:q((X,_))}.",
        "{q((1,x));q((2,y))}.n(N):-N=#count{1:q((1,x));2:q((2,y))}.",
    ),
    (
        "{q((a,x));q((z,y))}.n(N):-N=#min{X:q((X,_))}.",
        "{q((a,x));q((z,y))}.n(N):-N=#min{a:q((a,x));z:q((z,y))}.",
    ),
    (
        "{-q((1,x));-q((2,y))}.p(X):--q((X,_)).",
        "{-q((1,x));-q((2,y))}.p(1):--q((1,x)).p(2):--q((2,y)).",
    ),
    (
        "{q((1,x));q((2,y))}.p(X):-q((X,_)),not not p(X).",
        "{q((1,x));q((2,y))}.p(1):-q((1,x)),not not p(1).p(2):-q((2,y)),not not p(2).",
    ),
];

#[test]
fn complete_models_match_explicit_ground_rules() {
    for &(source, expanded) in CASES {
        assert_eq!(
            native(&formula(source)),
            native(&formula(expanded)),
            "{source}"
        );
    }
}
#[test]
fn stable_models_match_independent_subset_enumeration() {
    for &(source, _) in CASES {
        let input = formula(source);
        assert_eq!(native(&input), exhaustive(&input), "{source}");
    }
}
#[test]
fn every_frozen_pair_matches_explicit_ground_rules() {
    for &(source, expanded) in CASES {
        let original = formula(source);
        let reference = formula(expanded);
        let left: Vec<_> = original.atoms().iter().map(canonical).collect();
        let right: Vec<_> = reference.atoms().iter().map(canonical).collect();
        assert_eq!(
            left.iter().collect::<BTreeSet<_>>(),
            right.iter().collect(),
            "{source}"
        );
        assert!(left.len() <= 7);
        let remap = |mask: usize| {
            left.iter().enumerate().fold(0, |result, (index, atom)| {
                result
                    | if mask & (1 << index) == 0 {
                        0
                    } else {
                        1 << right.iter().position(|other| other == atom).unwrap()
                    }
            })
        };
        for outer in 0..1 << left.len() {
            let frozen = values(original.theory(), outer, None);
            let other = values(reference.theory(), remap(outer), None);
            assert_eq!(
                holds(original.theory(), &frozen),
                holds(reference.theory(), &other),
                "{source}: M={outer}"
            );
            for inner in 0..1 << left.len() {
                assert_eq!(
                    holds(
                        original.theory(),
                        &values(original.theory(), inner, Some(&frozen))
                    ),
                    holds(
                        reference.theory(),
                        &values(reference.theory(), remap(inner), Some(&other))
                    ),
                    "{source}: M={outer} J={inner}"
                );
            }
        }
    }
}
#[test]
#[ignore = "requires clingo: original sources match clingo full models"]
fn original_sources_match_clingo_full_models() {
    for source in [PROJECTION, EXTREMA_SOURCE]
        .into_iter()
        .chain(CASES.iter().map(|&(source, _)| source))
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
    }
}

#[test]
fn a_late_mismatch_does_not_bind_the_next_row() {
    let admitted = formula("q((1,2),0).q((2,2),1).p(X,Z):-q((X,X),Z).");
    assert_eq!(
        native(&admitted),
        models(&[&["q((1,2),0)", "q((2,2),1)", "p(2,1)"]])
    );
}
#[test]
fn tuple_shape_distinguishes_scalar_function_and_arity() {
    let admitted = formula(
        "q(1).q((1,)).q((1,2)).q(f(1)).q(((),x)).q((#inf,#sup)).q((-f(1),\"s\")).p(X):-q((X,_)).",
    );
    assert_eq!(
        native(&admitted),
        models(&[&[
            "q(1)",
            "q((1,))",
            "q((1,2))",
            "q(f(1))",
            "q(((),x))",
            "q((#inf,#sup))",
            "q((-f(1),\"s\"))",
            "p(1)",
            "p(())",
            "p(#inf)",
            "p(-f(1))"
        ]])
    );
}
#[test]
fn nested_constants_require_exact_value_identity() {
    let admitted = formula("q((f(1),(2,))).q((-f(1),(3,))).q((f(1),(4,5))).p(X):-q((f(1),(X,))).");
    assert_eq!(
        native(&admitted),
        models(&[&[
            "q((f(1),(2,)))",
            "q((-f(1),(3,)))",
            "q((f(1),(4,5)))",
            "p(2)"
        ]])
    );
}
#[test]
fn anonymous_nodes_have_independent_existential_values() {
    let admitted = formula("q((1,2),3).p:-q((_,_),_).");
    assert_eq!(native(&admitted), models(&[&["q((1,2),3)", "p"]]));
}
#[test]
fn structural_bindings_can_feed_later_support_rounds() {
    let admitted = formula("q((1,2)).p(X):-q((X,_)).r(X):-p(X).p(X):-r(X).");
    assert_eq!(native(&admitted), models(&[&["q((1,2))", "p(1)", "r(1)"]]));
}
#[test]
fn a_negative_tuple_occurrence_cannot_make_a_name_safe() {
    assert!(
        admit_formula(
            "p(X):-not q((X,_)).".into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default()
        )
        .is_err()
    );
}
#[test]
fn an_arithmetic_tuple_pattern_is_not_a_producer() {
    assert!(
        admit_formula(
            "q((2,x)).p(X):-q((X+1,_)).".into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default()
        )
        .is_err()
    );
}
#[test]
fn a_named_pattern_retains_the_complete_source_atom() {
    assert_eq!(
        native(&formula("q(f(1)).p(X):-q(f(X)).")),
        models(&[&["q(f(1))", "p(1)"]])
    );
}
#[test]
fn nested_anonymous_nodes_do_not_consume_variable_slots() {
    let mut options = AdmissionOptions::default();
    options.core_limits.max_variables_per_template = 1;
    assert!(
        admit_formula(
            "q((1,2)).p:-q((_,_)).".into(),
            options,
            ExpansionLimits::default(),
            FormulaLimits::default()
        )
        .is_ok()
    );
}
#[test]
fn a_whole_argument_capture_obeys_the_variable_ceiling() {
    let mut options = AdmissionOptions::default();
    options.core_limits.max_variables_per_template = 1;
    let error = admit_formula(
        "q((1,2)).p(X):-q((X,_)).".into(),
        options,
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        zetesis_themelios::FormulaFailure::Limit {
            resource: zetesis_themelios::FormulaResource::Variables,
            observed: 2,
            limit: 1,
            ..
        }
    ));
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config::with_cases(64))]
    #[test]
    fn repeated_names_project_exactly_the_agreeing_rows(rows in proptest::collection::vec((-4_i32..=4,-4_i32..=4,-4_i32..=4),0..12)) {
        use std::fmt::Write;
        let mut source=String::from("p(X,Z):-q((X,X),Z).");
        let mut expected=BTreeSet::new();
        for (a,b,c) in rows {
            write!(source,"q(({a},{b}),{c}).").unwrap();
            expected.insert(format!("q(({a},{b}),{c})"));
            if a==b { expected.insert(format!("p({a},{c})")); }
        }
        proptest::prop_assert_eq!(native(&formula(&source)),BTreeSet::from([expected]));
    }
    #[test]
    fn prebound_names_project_exactly_the_matching_rows(rows in proptest::collection::vec((-4_i32..=4,-4_i32..=4),0..12), fixed in -4_i32..=4) {
        use std::fmt::Write;
        let mut source=format!("d({fixed}).p(X,Y):-d(X),q((X,Y)).");
        let mut expected=BTreeSet::from([format!("d({fixed})")]);
        for (a,b) in rows {
            write!(source,"q(({a},{b})).").unwrap();
            expected.insert(format!("q(({a},{b}))"));
            if a==fixed { expected.insert(format!("p({a},{b})")); }
        }
        proptest::prop_assert_eq!(native(&formula(&source)),BTreeSet::from([expected]));
    }
}

#[test]
fn duplicate_included_patterns_retain_original_spans() {
    use zetesis_themelios::{
        BundleAdmissionOptions, BundleLimits, SourceBundle, admit_bundle_formula,
    };
    let bundle = SourceBundle::load(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/structural-bindings/entry.lp"),
        BundleLimits::default(),
    )
    .unwrap();
    let admitted = admit_bundle_formula(
        bundle,
        BundleAdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    let mut found = false;
    for origins in admitted.formula_origins() {
        let matching: Vec<_> = origins
            .iter()
            .filter(|origin| {
                admitted
                    .bundle()
                    .get(origin.source)
                    .unwrap()
                    .source()
                    .slice(origin.span)
                    .unwrap()
                    == "p(X):-q((X,_))."
            })
            .collect();
        // Synthetic support guards combine producer witnesses. The duplicated
        // source rule itself retains both parsed occurrences.
        if !matching.is_empty() && matching.len() == origins.len() {
            assert_eq!(
                matching
                    .iter()
                    .map(|origin| origin.source)
                    .collect::<BTreeSet<_>>()
                    .len(),
                2
            );
            found = true;
        }
    }
    assert!(found);
}

#[test]
fn extracted_extrema_keep_their_complete_value_identity() {
    assert_eq!(
        native(&formula(EXTREMA_SOURCE)),
        models(&[&[
            "q((#inf,a))",
            "q((#sup,b))",
            "q((\"#inf\",c))",
            "p(#inf)",
            "p(#sup)",
            "p(\"#inf\")"
        ]])
    );
}
#[test]
fn extracted_extrema_preserve_every_frozen_fact_consequence() {
    // All three q atoms are facts. Their corresponding p atoms are mandatory,
    // so the independent formula is the conjunction of these six atoms.
    let admitted = formula(EXTREMA_SOURCE);
    assert_eq!(admitted.atoms().len(), 6);
    let full = (1 << 6) - 1;
    for outer in 0..=full {
        let frozen = values(admitted.theory(), outer, None);
        assert_eq!(holds(admitted.theory(), &frozen), outer == full);
        for inner in 0..=full {
            assert_eq!(
                holds(
                    admitted.theory(),
                    &values(admitted.theory(), inner, Some(&frozen))
                ),
                outer == full && inner == full
            );
        }
    }
}
