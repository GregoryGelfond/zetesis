//! Completed nonbinding guards preserve comparison and default-negation semantics.

mod cases;
use crate::support::finite_bindings as reference;
mod truth;

use std::collections::BTreeSet;

use crate::support::objective_boundaries;
use cases::sources;
use reference::{Models, exhaustive, external, holds, native, values};
use themelios_base::source::SourceId;
use truth::Truth;
use zetesis_reference_support::canonical;
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionLimits, FormulaFailure, FormulaLimits,
    FormulaResource, FormulaWarning, admit_formula, prepare_formula,
};

const SOURCE: SourceId = SourceId::new(173);

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

#[test]
fn models_match_finite_substitutions() {
    for (source, expanded) in sources() {
        assert_eq!(
            native(&input(&source)),
            native(&input(&expanded)),
            "{source}"
        );
    }
}

#[test]
fn stability_matches_subset_enumeration() {
    for (source, _) in sources() {
        let admitted = input(&source);
        assert_eq!(native(&admitted), exhaustive(&admitted), "{source}");
    }
}

#[test]
fn frozen_truth_matches_finite_substitutions() {
    let mut pairs = 0;
    for (source, expanded) in sources() {
        let left = input(&source);
        let right = input(&expanded);
        let names: Vec<_> = left.atoms().iter().map(canonical).collect();
        let other: Vec<_> = right.atoms().iter().map(canonical).collect();
        assert_eq!(
            names.iter().collect::<BTreeSet<_>>(),
            other.iter().collect(),
            "{source}"
        );
        assert!(names.len() <= 7, "bounded exhaustive interpretation pairs");
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
    println!("source_pairs={} frozen_pairs={pairs}", sources().len());
}

#[test]
#[ignore = "requires clingo: original sources match clingo full models"]
fn original_sources_match_clingo_full_models() {
    let mut total = 0;
    for (source, _) in sources() {
        let result = external(&source, true);
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
        assert_eq!(native(&input(&source)), expected, "{source}");
        total += count;
    }
    println!("complete_sources={} full_models={total}", sources().len());
}

#[test]
fn comparison_complement_is_not_default_negation() {
    let empty = Models::from([BTreeSet::new()]);
    let optional = Models::from([BTreeSet::new(), BTreeSet::from(["p".into()])]);
    assert_eq!(native(&input("p:-N=#count{},N!=#count{1:p}.")), empty);
    assert_eq!(native(&input("p:-N=#count{},not N=#count{1:p}.")), optional);
}

#[test]
fn unrealized_proposals_do_not_establish_body_truth() {
    let source = "{p}.q(N):-N=#count{1:p},N<=#count{1:p}.";
    let expected = Models::from([
        BTreeSet::from(["q(0)".into()]),
        BTreeSet::from(["p".into(), "q(1)".into()]),
    ]);
    assert_eq!(native(&input(source)), expected);
    assert_ne!(
        native(&input("{p}.q(0):-0<=#count{1:p}.q(1):-1<=#count{1:p}.")),
        expected
    );
}

#[test]
fn body_order_preserves_completed_guard_values() {
    let literals = ["N=#count{1:p}", "Y=N+1", "Y<=#sum{2:p}"];
    let expected = Models::from([BTreeSet::new(), BTreeSet::from(["p".into(), "q(2)".into()])]);
    for order in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        let body = order.map(|index| literals[index]).join(",");
        assert_eq!(native(&input(&format!("{{p}}.q(Y):-{body}."))), expected);
    }
}

#[test]
fn repeated_outer_rows_recompute_guard_bounds() {
    let source = "i(0..2).q(I,Y):-i(I),N=#count{},Y=N+I,Y<=#count{1}.";
    assert_eq!(
        native(&input(source)),
        Models::from([BTreeSet::from([
            "i(0)".into(),
            "i(1)".into(),
            "i(2)".into(),
            "q(0,0)".into(),
            "q(1,1)".into(),
        ])])
    );
}

#[test]
fn guard_consumers_preserve_scored_answers() {
    for source in [
        // The unconsumed total assignment retains its established observer path.
        "q(N):-N=#count{}.#minimize{N:q(N)}.",
        "q(N):-N=#count{},N=#sum{}.#minimize{N:q(N)}.",
        "q(N):-N=#count{},not N!=#count{}.#minimize{N:q(N)}.",
        "{q}:-N=#count{},N<=#count{}.#minimize{1:q}.",
    ] {
        objective_boundaries::check(source);
    }
}

#[test]
fn nonbinding_guards_cannot_supply_missing_inputs() {
    for source in [
        "q(N):-N<=#count{}.",
        "q(N):-not N=#count{}.",
        "q(N):-not not N=#count{}.",
    ] {
        let error = prepare_formula(
            source.into(),
            options(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap_err();
        assert!(
            matches!(error, FormulaFailure::UnsafeVariable { .. }),
            "{source}: {error}"
        );
        assert_eq!(error.diagnostics()[0].primary().location.source, SOURCE);
    }
}

#[test]
fn defined_false_guards_complete_the_mixed_family() {
    let source = "q:-N=#count{1},1/N<=#count{}.";
    let admitted = input(source);
    // N=1 gives a defined false guard, which witnesses definedness, not truth.
    let [FormulaWarning::ZeroDivisor { location }] = admitted.warnings() else {
        panic!("one zero-divisor warning: {:?}", admitted.warnings());
    };
    assert_eq!(location.source, SOURCE);
    assert_eq!(admitted.source().slice(location.span).unwrap(), source);
    let expected = Models::from([BTreeSet::new()]);
    assert_eq!(native(&admitted), expected);
    assert_eq!(exhaustive(&admitted), expected);
}

#[test]
fn empty_extremum_bounds_keep_their_logical_order() {
    let source = "q:-N=#min{},N<=#count{}.";
    assert_eq!(native(&input(source)), Models::from([BTreeSet::new()]));
}

#[test]
fn original_sources_remain_owned() {
    for (source, _) in sources() {
        assert_eq!(input(&source).source().text(), source);
    }
}

#[test]
fn grounding_limits_are_inclusive() {
    let source = "q(Y):-N=#count{},Y=N+1,Y<=#count{1}.";
    for resource in [
        FormulaResource::AssignmentValues,
        FormulaResource::Substitutions,
        FormulaResource::Work,
        FormulaResource::AggregateCacheRows,
        FormulaResource::AggregateCacheRoots,
    ] {
        let limits = |ceiling| {
            let mut limits = FormulaLimits::default();
            match resource {
                FormulaResource::AssignmentValues => {
                    limits.max_assignment_values = usize::try_from(ceiling).unwrap();
                }
                FormulaResource::Substitutions => limits.max_substitutions = ceiling,
                FormulaResource::Work => limits.max_work = ceiling,
                FormulaResource::AggregateCacheRows => {
                    limits.max_aggregate_cache_rows = usize::try_from(ceiling).unwrap();
                }
                FormulaResource::AggregateCacheRoots => {
                    limits.max_aggregate_cache_roots = usize::try_from(ceiling).unwrap();
                }
                _ => unreachable!(),
            }
            limits
        };
        let admit = |ceiling| {
            admit_formula(
                source.into(),
                options(),
                ExpansionLimits::default(),
                limits(ceiling),
            )
        };
        let mut lower = 0;
        let mut upper = 16_384;
        assert!(admit(upper).is_ok());
        while lower < upper {
            let middle = lower + (upper - lower) / 2;
            if admit(middle).is_ok() {
                upper = middle;
            } else {
                lower = middle + 1;
            }
        }
        assert!(lower > 0);
        assert_eq!(native(&admit(lower).unwrap()), native(&input(source)));
        let error = admit(lower - 1).unwrap_err();
        assert!(
            matches!(error,
            FormulaFailure::Limit { resource: actual, limit, observed, .. }
            if actual == resource && limit == u128::from(lower - 1) && observed == u128::from(lower)),
            "{error}"
        );
        let location = error.diagnostics()[0].primary().location;
        assert_eq!(location.source, SOURCE);
        assert_eq!(input(source).source().slice(location.span).unwrap(), source);
        println!("inclusive_{resource:?}={lower}");
    }
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config::with_cases(128))]
    #[test]
    fn singleton_guards_match_direct_frozen_formulas(
        offset in -3_i32..4,
        relation in proptest::sample::select(vec!["=", "!=", "<", "<=", ">", ">="]),
        negations in 0_usize..3,
    ) {
        let sign = ["", "not ", "not not "][negations];
        let source = format!("{{p}}.q(N):-N=#count{{1:p}},Y=N+({offset}),{sign}Y{relation}#count{{1:p}}.");
        let admitted = input(&source);
        let names: Vec<_> = admitted.atoms().iter().map(canonical).collect();
        proptest::prop_assert!(names.iter().all(|name| ["p", "q(0)", "q(1)"].contains(&name.as_str())));
        proptest::prop_assert!(names.iter().any(|name| name == "p"));
        for outer in 0..1 << names.len() {
            let original = values(admitted.theory(), outer, None);
            for inner in 0..1 << names.len() {
                // An identically false body may emit no head atom. Interpret
                // its absent symbol as false in both complete assignments.
                let atom = |name: &str| names.iter().position(|actual| actual == name)
                    .map_or(Truth::constant(false), |index| Truth::atom(outer & (1 << index) != 0, inner & (1 << index) != 0));
                let p = atom("p");
                let mut expected = p.or(p.negate());
                for proposed in 0..=1 {
                    let equality = if proposed == 0 { p.negate() } else { p };
                    let guard = truth::singleton(p, proposed + offset, relation, negations);
                    let body = equality.and(guard);
                    let head = atom(&format!("q({proposed})"));
                    // The admitted theory also carries candidate-only
                    // necessary support. Include that established boundary
                    // when comparing arbitrary (possibly unsupported) M/J.
                    let supported = head.implies(body).negate().negate();
                    expected = expected.and(body.implies(head)).and(supported);
                }
                proptest::prop_assert_eq!(holds(admitted.theory(), &original), expected.whole);
                proptest::prop_assert_eq!(
                    holds(admitted.theory(), &values(admitted.theory(), inner, Some(&original))),
                    expected.frozen,
                );
            }
        }
    }
}
