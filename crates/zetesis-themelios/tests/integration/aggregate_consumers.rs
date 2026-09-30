//! Finite substitutions keep each aggregate equality; proposals are not truth.
use crate::support::finite_bindings as reference;
use reference::{Models, exhaustive, external, holds, native, values};
use std::collections::BTreeSet;
use std::fmt::Write as _;
use themelios_base::source::SourceId;
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionFailure, ExpansionLimits, ExpansionResource,
    FormulaFailure, FormulaLimits, FormulaResource, FormulaWarning, admit_formula, prepare_formula,
};

use crate::support::objective_dependency_records as objective_dependencies;
use zetesis_reference_support::canonical;

const SOURCE: SourceId = SourceId::new(113);
fn options() -> AdmissionOptions {
    AdmissionOptions {
        source_id: SOURCE,
        ..Default::default()
    }
}
fn input(source: &str) -> AdmittedFormula {
    admit_formula(
        source.into(),
        options(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap_or_else(|error| panic!("{source}: {error}"))
}

// Ground references are authored independently of the runtime schedule. The
// correlated two-assignment case includes the unrealizable proposal rows.
const CASES: &[(&str, &str)] = &[
    ("N{p}:-N=#min{}.", "#sup{p}:-#sup=#min{}."),
    ("N{p}:-N=#max{}.", "#inf{p}:-#inf=#max{}."),
    ("p(2).q(N):-N=#count{},p(N+1).", "p(2)."),
    (
        "M{p}:-N=#count{},M=#count{}.",
        "0{p}:-0=#count{},0=#count{}.",
    ),
    (
        "d(1).N{a;b}N:-N=#count{X:d(X)}.",
        "d(1).0{a;b}0:-0=#count{1:d(1)}.1{a;b}1:-1=#count{1:d(1)}.",
    ),
    (
        "{d}.N{p(N)}N:-N=#count{1:d}.",
        "{d}.0{p(0)}0:-0=#count{1:d}.1{p(1)}1:-1=#count{1:d}.",
    ),
    (
        "{d}.N{a:N=1;b}N:-N=#count{1:d}.",
        "{d}.0{a:0=1;b}0:-0=#count{1:d}.1{a:1=1;b}1:-1=#count{1:d}.",
    ),
    (
        "{d}.N{a;b}N:-N=#count{1:d}.",
        "{d}.0{a;b}0:-0=#count{1:d}.1{a;b}1:-1=#count{1:d}.",
    ),
    (
        "{d;e}.N{a:b;b}N:-e,N=#count{1:d}.",
        "{d;e}.0{a:b;b}0:-e,0=#count{1:d}.1{a:b;b}1:-e,1=#count{1:d}.",
    ),
    (
        "{d;e}.N{a:b;b}N:-N=#count{1:d},e.",
        "{d;e}.0{a:b;b}0:-e,0=#count{1:d}.1{a:b;b}1:-e,1=#count{1:d}.",
    ),
    (
        "{d}.N+1{a;b}N+1:-N=#count{1:d}.",
        "{d}.1{a;b}1:-0=#count{1:d}.2{a;b}2:-1=#count{1:d}.",
    ),
    (
        "{d}.N{a;b}M:-N=#count{1:d},M=#sum{2:d}.",
        "{d}.0{a;b}0:-0=#count{1:d},0=#sum{2:d}.0{a;b}2:-0=#count{1:d},2=#sum{2:d}.1{a;b}0:-1=#count{1:d},0=#sum{2:d}.1{a;b}2:-1=#count{1:d},2=#sum{2:d}.",
    ),
    (
        "{d}.N{a}N:-N=#sum{-1:d}.",
        "{d}.0{a}0:-0=#sum{-1:d}.-1{a}-1:--1=#sum{-1:d}.",
    ),
    (
        "{d}.N{a;b}N:-N=#sum+{-1:d;2:d}.",
        "{d}.0{a;b}0:-0=#sum+{-1:d;2:d}.2{a;b}2:-2=#sum+{-1:d;2:d}.",
    ),
    ("N{a}N:-N=#count{}.", "0{a}0:-0=#count{}."),
    ("N{}N:-N=#count{}.", "0{}0:-0=#count{}."),
    (
        "N{a}N:-N=#count{1:a}.",
        "0{a}0:-0=#count{1:a}.1{a}1:-1=#count{1:a}.",
    ),
    (
        "{d}.N#count{1:a;2:b}N:-N=#count{1:d}.",
        "{d}.0#count{1:a;2:b}0:-0=#count{1:d}.1#count{1:a;2:b}1:-1=#count{1:d}.",
    ),
    (
        "p(1).q(N):-N=#count{},p(N+1).",
        "p(1).q(0):-0=#count{},p(1).",
    ),
    (
        "{p(1);p(2)}.q(N):-N=#count{1:p(1)},p(N+1).",
        "{p(1);p(2)}.q(0):-0=#count{1:p(1)},p(1).q(1):-1=#count{1:p(1)},p(2).",
    ),
    (
        "{p(1);p(2)}.q(N):-p(N+1),N=#count{1:p(1)}.",
        "{p(1);p(2)}.q(0):-0=#count{1:p(1)},p(1).q(1):-1=#count{1:p(1)},p(2).",
    ),
    (
        "{p(f(1));p(f(2))}.q(N):-N=#count{1:p(f(1))},p(f(N+1)).",
        "{p(f(1));p(f(2))}.q(0):-0=#count{1:p(f(1))},p(f(1)).q(1):-1=#count{1:p(f(1))},p(f(2)).",
    ),
    (
        "{-p(1);-p(2)}.q(N):-N=#count{1: -p(1)},-p(N+1).",
        "{-p(1);-p(2)}.q(0):-0=#count{1: -p(1)},-p(1).q(1):-1=#count{1: -p(1)},-p(2).",
    ),
    (
        "{p(1);p(2)}.q(Y):-N=#count{1:p(1)},Y=N+1,p(Y+1).",
        "{p(1);p(2)}.q(1):-0=#count{1:p(1)},p(2).",
    ),
    ("q(N):-N=#count{},p(N+1).", ""),
    (
        "p(1). n(N) :- N=#count{X:p(X)}, (N,0)=(1,0).",
        "p(1).n(1):-1=#count{1:p(1)}.",
    ),
    (
        "p(1). n(N,M) :- N=#count{X:p(X)}, M=N+1.",
        "p(1).n(0,1):-0=#count{1:p(1)}.n(1,2):-1=#count{1:p(1)}.",
    ),
    (
        "{a}.n(N):-N=#count{1:a},not N=0.",
        "{a}.n(1):-1=#count{1:a}.",
    ),
    (
        "{a}.n(N):-N=#count{1:a},not not 0<=N<=1.",
        "{a}.n(0):-0=#count{1:a}.n(1):-1=#count{1:a}.",
    ),
    ("p(f(N)):-N=#count{}.", "p(f(0)):-0=#count{}."),
    (
        "{p(1);p(2)}.q(N):-p(Y),Y=N+1,N=#count{}.",
        "{p(1);p(2)}.q(0):-p(1),0=#count{}.",
    ),
    (
        "{p(1);p(2)}.q(N):-N=#count{X:p(X)},N>0.",
        "{p(1);p(2)}.q(1):-1=#count{1:p(1);2:p(2)}.q(2):-2=#count{1:p(1);2:p(2)}.",
    ),
    (
        "{p(1);p(2)}.q(N+1):-N=#count{X:p(X)}.",
        "{p(1);p(2)}.q(1):-0=#count{1:p(1);2:p(2)}.q(2):-1=#count{1:p(1);2:p(2)}.q(3):-2=#count{1:p(1);2:p(2)}.",
    ),
    (
        "{p}.q(f((N,N+1))):-N=#count{1:p}.",
        "{p}.q(f((0,1))):-0=#count{1:p}.q(f((1,2))):-1=#count{1:p}.",
    ),
    (
        "{p}.q(Z):-N=#count{1:p},Y=N+1,Z=Y*2.",
        "{p}.q(2):-0=#count{1:p}.q(4):-1=#count{1:p}.",
    ),
    (
        "{p}.q(Z):-Z=Y*2,Y=N+1,N=#count{1:p}.",
        "{p}.q(2):-0=#count{1:p}.q(4):-1=#count{1:p}.",
    ),
    (
        "{p}.q(N):-N=#count{1:p},(N,1)!=(0,1).",
        "{p}.q(1):-1=#count{1:p}.",
    ),
    (
        "{p}.q(Y):-N=#count{1:p},(N,Y)=(1,N+2).",
        "{p}.q(3):-1=#count{1:p}.",
    ),
    (
        "{p(1);p(2)}.q(N):-N=#count{X:p(X)},0<N<=1.",
        "{p(1);p(2)}.q(1):-1=#count{1:p(1);2:p(2)}.",
    ),
    (
        "{p}.q(X):-N=#count{1:p},M=#sum{2:p},X=N+M.",
        "{p}.q(0):-0=#count{1:p},0=#sum{2:p}.q(2):-0=#count{1:p},2=#sum{2:p}.q(1):-1=#count{1:p},0=#sum{2:p}.q(3):-1=#count{1:p},2=#sum{2:p}.",
    ),
    (
        "{p;q}.r(N+1):-N=#count{1:p;1:q}.",
        "{p;q}.r(1):-0=#count{1:p;1:q}.r(2):-1=#count{1:p;1:q}.",
    ),
    (
        "{p}.q(f(N)):-N=#min{2:p}.",
        "{p}.q(f(#sup)):-#sup=#min{2:p}.q(f(2)):-2=#min{2:p}.",
    ),
    (
        "{p}.q(N+2):-N=#sum{-1:p}.",
        "{p}.q(2):-0=#sum{-1:p}.q(1):--1=#sum{-1:p}.",
    ),
    (
        "{p}.q(-f(N)):-N=#sum+{-1:p;2:p}.",
        "{p}.q(-f(0)):-0=#sum+{-1:p;2:p}.q(-f(2)):-2=#sum+{-1:p;2:p}.",
    ),
    ("q(f(N+1)):-N=#count{}.", "q(f(1)):-0=#count{}."),
    // These exact clingo-valid strings were formerly aggregate-profile refusals.
    ("r(N,M):-N=#count{},M=#count{},M>0.", ""),
    (
        "r(N,M):-N=#count{},M=#count{},(0,M)=(0,0).",
        "r(0,0):-0=#count{},0=#count{}.",
    ),
    (
        "r(N,M):-N=#count{},M=#count{},K=M+1.",
        "r(0,0):-0=#count{},0=#count{}.",
    ),
    ("n(N):-N=#count{},N>0.", ""),
    (
        "{p}.q(N):-N=#count{1:p},not N=0.",
        "{p}.q(1):-1=#count{1:p}.",
    ),
    (
        "{p}.q(N):-N=#count{1:p},not not N=1.",
        "{p}.q(1):-1=#count{1:p}.",
    ),
    // The nonbinding aggregate keeps the completed proposal's key identity.
    (
        "{a}.p(N):-N=#count{1:a},2=#count{N:a;0:a}.",
        "{a}.p(0):-0=#count{1:a},2=#count{0:a;0:a}.p(1):-1=#count{1:a},2=#count{1:a;0:a}.",
    ),
    // A local condition reads the same outer value, with signed atom identity.
    (
        "-q(0).q(1).{a}.p(N):-N=#count{1:a},1=#count{1: -q(N)}.",
        "-q(0).q(1).{a}.p(0):-0=#count{1:a},1=#count{1: -q(0)}.p(1):-1=#count{1:a},1=#count{1: -q(1)}.",
    ),
];

#[test]
fn models_match_finite_substitutions() {
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
fn frozen_truth_matches_finite_substitutions() {
    let mut pairs = 0;
    for &(source, expanded) in CASES {
        let left = input(source);
        let right = input(expanded);
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
            // All M/J pairs cover the required subset pairs and other contexts.
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
        assert_eq!(native(&input(source)), expected, "{source}");
        total += count;
    }
    println!("complete_sources={} full_models={total}", CASES.len());
}

#[test]
fn conditionals_consume_aggregate_descendants() {
    for (source, expanded) in [
        ("q(N):-N=#count{},Y=N+1,p(Y):d.", "q(0):-0=#count{},p(1):d."),
        (
            "q(N):-N=#count{},not p(N):d.",
            "q(0):-0=#count{},not p(0):d.",
        ),
    ] {
        assert_eq!(native(&input(source)), native(&input(expanded)));
        assert_eq!(input(source).source().text(), source);
    }
}

#[test]
fn dependency_cycles_have_a_scheduling_refusal() {
    let source = "q(N):-N=#count{X:p(X,Y)},Y=N+1.";
    let error = prepare_formula(
        source.into(),
        options(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap_err();
    assert!(
        matches!(error, FormulaFailure::CyclicValueInput { .. }),
        "{error}"
    );
    assert_eq!(error.diagnostics()[0].primary().location.source, SOURCE);
    assert!(error.to_string().contains("dependency"));
}

#[test]
fn captured_arguments_cannot_supply_missing_inputs() {
    let error = prepare_formula(
        "p(2).q(N,X):-N=#count{},p(X+1).".into(),
        options(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap_err();
    assert!(matches!(error, FormulaFailure::UnboundArgumentInput { .. }));
}

#[test]
fn aggregate_consumers_preserve_scored_answers() {
    for source in [
        "N{p}N:-N=#count{}.#minimize{1:p}.",
        "n(N):-N=#count{},N>0.#minimize{N:n(N)}.",
        "n(N,Y):-N=#count{},Y=N+1.#minimize{N,Y:n(N,Y)}.",
    ] {
        objective_dependencies::check(source);
    }
}

#[test]
fn empty_minimum_exceeds_every_finite_choice_count() {
    assert!(native(&input("N{p}:-N=#min{}.")).is_empty());
}

#[test]
fn empty_maximum_allows_every_finite_choice_count() {
    assert_eq!(
        native(&input("N{p}:-N=#max{}.")),
        Models::from([BTreeSet::new(), BTreeSet::from(["p".into()])])
    );
}

#[test]
fn undefined_consumers_refuse_the_complete_input() {
    let error = admit_formula(
        "q(N/0):-N=#count{}.".into(),
        options(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        FormulaFailure::Expansion(ExpansionFailure::Evaluation { .. })
    ));
}

#[test]
fn absent_relational_support_skips_value_evaluation() {
    assert_eq!(
        native(&input("q(N/0):-d,N=#count{}.")),
        Models::from([BTreeSet::new()])
    );
}

#[test]
fn mixed_consumers_keep_the_defined_family() {
    let source = "{p}.q(Y):-N=#count{1:p},N>0,Y=1/N.";
    let admitted = input(source);
    // N is generated, so N>0 does not exclude the zero proposal from the family.
    let [FormulaWarning::ZeroDivisor { location }] = admitted.warnings() else {
        panic!("one zero-divisor warning: {:?}", admitted.warnings());
    };
    assert_eq!(location.source, SOURCE);
    assert_eq!(
        admitted.source().slice(location.span).unwrap(),
        &source[4..]
    );
    let expected = Models::from([BTreeSet::new(), BTreeSet::from(["p".into(), "q(1)".into()])]);
    assert_eq!(native(&admitted), expected);
    assert_eq!(exhaustive(&admitted), expected);
}

#[test]
fn generated_values_obey_the_assignment_ceiling() {
    let error = admit_formula(
        "{p}.q(N+1):-N=#count{1:p}.".into(),
        options(),
        ExpansionLimits::default(),
        FormulaLimits {
            max_assignment_values: 1,
            ..Default::default()
        },
    )
    .unwrap_err();
    assert!(
        matches!(
            error,
            FormulaFailure::Limit {
                resource: FormulaResource::AssignmentValues,
                limit: 1,
                observed: 2,
                ..
            }
        ),
        "{error}"
    );
}

#[test]
fn generated_rows_obey_the_substitution_ceiling() {
    let error = admit_formula(
        "q(N+1):-N=#count{}.".into(),
        options(),
        ExpansionLimits::default(),
        FormulaLimits {
            max_substitutions: 0,
            ..Default::default()
        },
    )
    .unwrap_err();
    assert!(
        matches!(
            error,
            FormulaFailure::Limit {
                resource: FormulaResource::Substitutions,
                limit: 0,
                observed: 1,
                ..
            }
        ),
        "{error}"
    );
}

#[test]
fn preparation_limits_are_inclusive() {
    // Source parsing and the new plan share the cumulative expansion budget.
    // Locate its exact boundary without assuming a host usize/Vec layout.
    let source = "q(f(Y)):-N=#count{},Y=N+1.";
    for resource in [ExpansionResource::TermWork, ExpansionResource::ScalarBytes] {
        let limits = |ceiling| {
            let mut result = ExpansionLimits::default();
            match resource {
                ExpansionResource::TermWork => result.max_term_work = ceiling,
                ExpansionResource::ScalarBytes => result.max_scalar_bytes = ceiling,
                _ => unreachable!(),
            }
            result
        };
        let mut lower = 0;
        let mut upper = 16_384;
        assert!(
            prepare_formula(
                source.into(),
                options(),
                limits(upper),
                FormulaLimits::default()
            )
            .is_ok()
        );
        while lower < upper {
            let middle = lower + (upper - lower) / 2;
            if prepare_formula(
                source.into(),
                options(),
                limits(middle),
                FormulaLimits::default(),
            )
            .is_ok()
            {
                upper = middle;
            } else {
                lower = middle + 1;
            }
        }
        assert!(lower > 0);
        assert!(
            prepare_formula(
                source.into(),
                options(),
                limits(lower),
                FormulaLimits::default()
            )
            .is_ok()
        );
        let error = prepare_formula(
            source.into(),
            options(),
            limits(lower - 1),
            FormulaLimits::default(),
        )
        .unwrap_err();
        assert!(
            matches!(error, FormulaFailure::Expansion(ExpansionFailure::Limit {
            resource: actual, limit, observed, ..
        }) if actual == resource && limit == (lower-1) as u128 && observed == lower as u128),
            "{error}"
        );
        assert_eq!(error.diagnostics()[0].primary().location.source, SOURCE);
        println!("inclusive_preparation_{resource:?}={lower}");
    }
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config::with_cases(32))]
    #[test]
    fn affine_consumers_match_finite_substitutions(offset in -3_i32..4, factor in 1_i32..4) {
        let source = format!("{{p(1);p(2)}}.q(Z):-Z=Y*{factor},Y=N+({offset}),N=#count{{X:p(X)}}.");
        let mut ground = "{p(1);p(2)}.".to_owned();
        for count in 0..=2 {
            write!(ground,"q({}):-{count}=#count{{1:p(1);2:p(2)}}.", (count+offset)*factor).unwrap();
        }
        proptest::prop_assert_eq!(native(&input(&source)), native(&input(&ground)));
    }
}
