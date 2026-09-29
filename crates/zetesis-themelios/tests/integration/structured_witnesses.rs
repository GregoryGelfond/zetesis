//! Local witnesses retain complete atoms under structural selection.
use crate::support::finite_bindings as reference;
use crate::support::witnesses::limited;
use reference::{Models, exhaustive, external, holds, native, values};
use std::collections::BTreeSet;
use zetesis_reference_support::{canonical, formula};
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionFailure, ExpansionLimits, ExpansionResource,
    FormulaFailure, FormulaLimits, FormulaResource, admit_formula,
};

// Written finite substitutions retain full supporting atoms. A condition row
// quantifies universally; its compatible witness atoms form one disjunction.
const CASES: &[(&str, &str)] = &[
    (
        "{p(f(1));p(f(2));p(g(3))}.q:-p(f(X)):#true.",
        "{p(f(1));p(f(2));p(g(3))}.q:-p(f(1)).q:-p(f(2)).",
    ),
    (
        "{p(f(1,2));p(f(2,2))}.q:-p(f(X,X)):#true.",
        "{p(f(1,2));p(f(2,2))}.q:-p(f(2,2)).",
    ),
    (
        "{p(f(1,g(2,3)));p(f(1,g(4,5)));p(f(1,h(2,3)))}.q:-p(f(X,g(_,_))):#true.",
        "{p(f(1,g(2,3)));p(f(1,g(4,5)));p(f(1,h(2,3)))}.q:-p(f(1,g(2,3))).q:-p(f(1,g(4,5))).",
    ),
    (
        "{-p(-f(1));-p(f(2));p(-f(3))}.q:- -p(-f(X)):#true.",
        "{-p(-f(1));-p(f(2));p(-f(3))}.q:- -p(-f(1)).",
    ),
    (
        "{p((1,2));p((2,2));p(f(2,2))}.q:-p((X,X)):#true.",
        "{p((1,2));p((2,2));p(f(2,2))}.q:-p((2,2)).",
    ),
    (
        "{p(f(2,0));p(f(1,2));p(f(1,3))}.q:-p(f(X,Y)):X=1.",
        "{p(f(2,0));p(f(1,2));p(f(1,3))}.q:-p(f(1,2)).q:-p(f(1,3)).",
    ),
    (
        "d(1).{p(f(2,0));p(f(1,2))}.q(X):-d(X),p(f(X,Y)):#true.",
        "d(1).{p(f(2,0));p(f(1,2))}.q(1):-d(1),p(f(1,2)).",
    ),
    (
        "{p(f(1,a));p(f(1,b));p(f(2,c))}.q:-p(f(X,Y)):X=1..2.",
        "{p(f(1,a));p(f(1,b));p(f(2,c))}.q:-p(f(1,a)),p(f(2,c)).q:-p(f(1,b)),p(f(2,c)).",
    ),
    (
        "{p(f(1),2);p(f(2),2)}.q:-p(f(X),X):#true.",
        "{p(f(1),2);p(f(2),2)}.q:-p(f(2),2).",
    ),
    (
        "{p(1,f(2));p(2,f(2))}.q:-p(X,f(X)):#true.",
        "{p(1,f(2));p(2,f(2))}.q:-p(2,f(2)).",
    ),
    (
        "c(1).{p(f(2,0));p(f(1,2))}.q:-p(f(X,Y)):c(X).",
        "c(1).{p(f(2,0));p(f(1,2))}.q:-p(f(1,2)).",
    ),
    (
        "p(f(1)).s(g(2)).q:-p(f(X)):#true;s(g(X)):#true.",
        "p(f(1)).s(g(2)).q:-p(f(1)),s(g(2)).",
    ),
    (
        "{p(f(1));p(g(2));p(h(3))}.q:-p(f(X);g(X)):#true.",
        "{p(f(1));p(g(2));p(h(3))}.q:-p(f(1)).q:-p(g(2)).",
    ),
    (
        "{p(f(1,2));p(f(3,4))}.q:-p(f(_,_)):#true.",
        "{p(f(1,2));p(f(3,4))}.q:-p(f(1,2)).q:-p(f(3,4)).",
    ),
    (
        "{p(f(1));p(f(2))}.q:-p(f(X)):#false.",
        "{p(f(1));p(f(2))}.q.",
    ),
    (
        "{p(f(1));p(f(2))}.q:-p(f(X)):#true,not r.r:-not q.",
        "{p(f(1));p(f(2))}.q:-p(f(1);f(2)):#true,not r.r:-not q.",
    ),
    (
        "{p(f(1))}.p(f(1)):-q.q:-p(f(X)):#true.",
        "{p(f(1))}.p(f(1)):-q.q:-p(f(1)).",
    ),
    (
        "{p(f(1));p(f(2));q}.not q:-p(f(X)):#true.",
        "{p(f(1));p(f(2));q}.not q:-p(f(1)).not q:-p(f(2)).",
    ),
    (
        "{p(f(1));p(f(1,2));p(-f(1))}.q:-p(f(X)):#true.",
        "{p(f(1));p(f(1,2));p(-f(1))}.q:-p(f(1)).",
    ),
    (
        "{p(f(\"x\",(1,2)));p(f(\"y\",(3,4)))}.q:-p(f(\"x\",Y)):#true.",
        "{p(f(\"x\",(1,2)));p(f(\"y\",(3,4)))}.q:-p(f(\"x\",(1,2))).",
    ),
    (
        "{p(f(1),g(a));p(f(2),g(b))}.q:-p(f(X),g(a)):#true.",
        "{p(f(1),g(a));p(f(2),g(b))}.q:-p(f(1),g(a)).",
    ),
    (
        "{p(f(1),\"a\");p(f(2),\"b\")}.q:-p(f(X),\"b\"):#true.",
        "{p(f(1),\"a\");p(f(2),\"b\")}.q:-p(f(2),\"b\").",
    ),
    (
        "{p(f(1));p(f(2));p(g(3));c}.q:-p(f(X)):c.",
        "{p(f(1));p(f(2));p(g(3));c}.q:-p(f(1);f(2)):c.",
    ),
    (
        "{p(f(1));p(f(2));p(g(3));c}.q:-p(f(X)):not c.",
        "{p(f(1));p(f(2));p(g(3));c}.q:-p(f(1);f(2)):not c.",
    ),
    (
        "{p(f(1));p(f(2));p(g(3));c}.q:-p(f(X)):not not c.",
        "{p(f(1));p(f(2));p(g(3));c}.q:-p(f(1);f(2)):not not c.",
    ),
];

// Anonymous structure introduces no enclosing name. The matching f witness
// makes its completed projection true before either default-negation sign.
const NEGATIVE_ANONYMOUS_CASES: &[(&str, &[&str])] = &[
    ("p(f(1)).q:-not p(f(_)):#true.", &["p(f(1))"]),
    ("p(f(1)).q:-not not p(f(_)):#true.", &["p(f(1))", "q"]),
    ("p(g(1)).q:-not p(f(_)):#true.", &["p(g(1))", "q"]),
    ("p(g(1)).q:-not not p(f(_)):#true.", &["p(g(1))"]),
];

#[test]
fn anonymous_structure_preserves_negative_projection() {
    for &(source, atoms) in NEGATIVE_ANONYMOUS_CASES {
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
    for &(source, _) in CASES {
        let admitted = formula(source);
        assert_eq!(native(&admitted), exhaustive(&admitted), "{source}");
    }
}

#[test]
fn frozen_truth_matches_finite_substitution() {
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

// A pair is truth at M and truth of the formula frozen at M, evaluated at J.
// These operations state the mathematical recurrence, independently of the
// source compiler and its formula DAG, support closure and native checker.
fn implication(left: (bool, bool), right: (bool, bool)) -> (bool, bool) {
    let original = !left.0 || right.0;
    (original, original && (!left.1 || right.1))
}

#[test]
fn conditional_truth_matches_quantified_witnesses() {
    let mut pairs = 0;
    for (polarity, sign) in ["", "not ", "not not "].into_iter().enumerate() {
        let source = format!("{{p(f(1));p(f(2));p(g(3));c}}.q:-p(f(X)):{sign}c.");
        let admitted = formula(&source);
        let names: Vec<_> = admitted.atoms().iter().map(canonical).collect();
        assert_eq!(names.len(), 5);
        let index = |name: &str| names.iter().position(|found| found == name).unwrap();
        let roots: Vec<_> = admitted
            .theory()
            .roots()
            .iter()
            .copied()
            .filter(|&root| {
                let zetesis_ferraris::Node::Implies(_, head) = admitted.theory().nodes()[root]
                else {
                    return false;
                };
                admitted.theory().nodes()[head] == zetesis_ferraris::Node::Atom(index("q"))
            })
            .collect();
        assert_eq!(
            roots.len(),
            1,
            "the original rule, excluding producer guards"
        );
        for outer in 0..1 << names.len() {
            let original = values(admitted.theory(), outer, None);
            for inner in 0..1 << names.len() {
                let atom = |name: &str| {
                    let original = outer & (1 << index(name)) != 0;
                    (original, original && inner & (1 << index(name)) != 0)
                };
                let mut condition = atom("c");
                for _ in 0..polarity {
                    condition = implication(condition, (false, false));
                }
                let (left, right) = (atom("p(f(1))"), atom("p(f(2))"));
                let consequent = (left.0 || right.0, left.1 || right.1);
                let expected = implication(implication(condition, consequent), atom("q"));
                assert_eq!(original[roots[0]], expected.0, "{source}: M={outer}");
                assert_eq!(
                    values(admitted.theory(), inner, Some(&original))[roots[0]],
                    expected.1,
                    "{source}: M={outer} J={inner}"
                );
                pairs += 1;
            }
        }
    }
    assert_eq!(pairs, 3072);
}

#[test]
#[ignore = "requires an independently installed clingo"]
fn original_sources_match_clingo_full_models() {
    let mut total = 0;
    for source in CASES
        .iter()
        .map(|&(source, _)| source)
        .chain(NEGATIVE_ANONYMOUS_CASES.iter().map(|&(source, _)| source))
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
    }
    println!(
        "complete_sources={} full_models={total}",
        CASES.len() + NEGATIVE_ANONYMOUS_CASES.len()
    );
}

#[test]
fn witness_names_cannot_establish_condition_safety() {
    for source in [
        "p(f(1)).q:-p(f(X)):X>0.",
        "p(f(1)).q:-p(f(X)):not d(X).",
        "p(f(1)).q:-p(f(X)):X>0,Y=2..1.",
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
fn local_witness_names_do_not_escape() {
    for source in [
        "p(f(1)).q(X):-p(f(X)):#true.",
        "p(f(1)).q:-p(f(X)):#true;X>0.",
        "p(f(1)).q:-p(f(X)):#false;X>0.",
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
fn negative_witnesses_cannot_supply_names() {
    for sign in ["not", "not not"] {
        let source = format!("p(f(1)).q:-{sign} p(f(X)):#true.");
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

#[test]
fn witness_arithmetic_requires_bound_inputs() {
    for source in ["p(f(2)).q:-p(f(X+1)):#true.", "p(2).q:-p(X+1):#true."] {
        assert!(
            matches!(
                limited(
                    source,
                    ExpansionLimits::default(),
                    &FormulaLimits::default()
                ),
                Err(FormulaFailure::UnboundArgumentInput { .. })
            ),
            "{source}"
        );
    }
}

#[test]
fn absent_support_cannot_make_a_witness_true() {
    assert_eq!(
        native(&formula("q:-p(f(X)):#true.")),
        Models::from([BTreeSet::new()])
    );
}

#[test]
fn a_witness_cannot_create_recursive_support() {
    assert_eq!(
        native(&formula("p(f(1)):-q.q:-p(f(X)):#true.")),
        Models::from([BTreeSet::new()])
    );
}

#[test]
fn late_mismatch_cannot_poison_the_next_witness() {
    let admitted = formula("p(f(1,2),0).p(f(2,2),1).q:-p(f(X,X),_):#true.");
    assert_eq!(
        native(&admitted),
        Models::from([BTreeSet::from([
            "p(f(1,2),0)".into(),
            "p(f(2,2),1)".into(),
            "q".into(),
        ])])
    );
}

#[test]
fn anonymous_positions_need_only_the_whole_capture() {
    let mut options = AdmissionOptions::default();
    options.core_limits.max_variables_per_template = 1;
    assert!(
        admit_formula(
            "p(f(1,g(2,3))).q:-p(f(_,g(_,_))):#true.".into(),
            options,
            ExpansionLimits::default(),
            FormulaLimits::default()
        )
        .is_ok()
    );
}

#[test]
fn witness_capture_limit_is_inclusive() {
    let mut options = AdmissionOptions::default();
    options.core_limits.max_variables_per_template = 2;
    let source = "p(f(1)).q:-p(f(X)):#true.";
    assert!(
        admit_formula(
            source.into(),
            options,
            ExpansionLimits::default(),
            FormulaLimits::default()
        )
        .is_ok()
    );
    options.core_limits.max_variables_per_template = 1;
    assert!(matches!(
        admit_formula(
            source.into(),
            options,
            ExpansionLimits::default(),
            FormulaLimits::default()
        ),
        Err(FormulaFailure::Limit {
            resource: FormulaResource::Variables,
            observed: 2,
            limit: 1,
            ..
        })
    ));
}

#[test]
fn witness_nodes_obey_the_value_ceiling() {
    // One complete condition selection and one selected consequent precede
    // pattern nodes. The flat control isolates those two occurrence units;
    // f(X) adds two nodes, and a source sign wrapper adds one even when folded.
    for (source, exact) in [
        ("q:-p(X):#true.", 2),
        ("q:-p(f(X)):#true.", 4),
        ("q:-p(-f(X)):#true.", 5),
    ] {
        for cap in [exact - 1, exact] {
            let result = zetesis_themelios::prepare_formula(
                source.into(),
                AdmissionOptions::default(),
                ExpansionLimits {
                    max_values: cap,
                    ..Default::default()
                },
                FormulaLimits::default(),
            );
            if cap == exact {
                assert!(result.is_ok(), "{source}: {result:?}");
            } else {
                assert!(
                    matches!(result, Err(FormulaFailure::Expansion(ExpansionFailure::Limit {
                    resource: ExpansionResource::Values, limit, observed, ..
                })) if limit == cap as u128 && observed == exact as u128),
                    "{source}"
                );
            }
        }
    }
}

#[test]
fn compiled_witnesses_retain_source_provenance() {
    let source = "p(f(1)).q:-p(f(X)):#true.";
    let admitted = formula(source);
    assert_eq!(admitted.source().text(), source);
    assert!(
        admitted
            .formula_origins()
            .iter()
            .flatten()
            .any(|location| admitted.source().slice(location.span).unwrap() == "q:-p(f(X)):#true.")
    );
}

// Admission is monotone in each isolated resource ceiling. Each iteration halves
// a finite interval; only the one varied typed refusal is accepted below it.
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
    assert_eq!(native(&run(low).unwrap()), native(&formula(BOUNDED)));
    low
}
const BOUNDED: &str = "{p(f(1,2));p(f(2,2));p(f(3,3))}.q:-p(f(X,X)):#true.";

#[test]
fn witness_work_limit_is_inclusive() {
    let cap = first_cap(
        |cap| {
            limited(
                BOUNDED,
                ExpansionLimits::default(),
                &FormulaLimits {
                    max_work: cap as u64,
                    ..Default::default()
                },
            )
        },
        // A relation operation can charge a bounded group of cells at once.
        |error| matches!(error, FormulaFailure::Limit { resource: FormulaResource::Work, observed, limit, .. } if observed > limit),
    );
    println!("formula_work={cap}");
}

#[test]
fn witness_substitution_limit_is_inclusive() {
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
fn witness_storage_limit_is_inclusive() {
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
fn witness_term_work_limit_is_inclusive() {
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
