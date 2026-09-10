//! Finite affine chains preserve the complete correlated source guard.

#[path = "support/finite_bindings.rs"]
mod reference;

use proptest::prelude::*;
use reference::{Models, atom_text, exhaustive, holds, native, values};
use themelios_base::source::SourceId;
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionFailure, ExpansionLimits, ExpansionResource,
    FormulaFailure, FormulaLimits, FormulaResource, admit_formula,
};

fn input(source: &str) -> AdmittedFormula {
    admit_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap_or_else(|error| panic!("{source}: {error}"))
}

// Expected assignments are obtained by integer arithmetic, before source
// admission. Original clingo results corroborate these complete families.
const CASES: &[(&str, &[&str])] = &[
    (
        "p(X,Y,Z):-0<X+Y+Z<4,0<Y<2,0<Z<2.",
        &["p(-1,1,1)", "p(0,1,1)", "p(1,1,1)"],
    ),
    ("p(X,Y):-0<X+X<6,0<X<Y<4.", &["p(1,2)", "p(1,3)", "p(2,3)"]),
    ("p(X,Y):-0<X<Y<3,Y<X.", &[]),
    ("p(X,Y):-0<X<Y<3.", &["p(1,2)"]),
    ("p(X,Y):-0<X=Y<3.", &["p(1,1)", "p(2,2)"]),
    (
        "p(X,Y):-0<X<3>Y>0.",
        &["p(1,1)", "p(1,2)", "p(2,1)", "p(2,2)"],
    ),
    ("p(X,Y):-3>Y>X>0.", &["p(1,2)"]),
    ("p(X):-0<2*X<6.", &["p(1)", "p(2)"]),
    ("p(X):-0<-X<3.", &["p(-2)", "p(-1)"]),
    ("p(X,Y):-0<X<Y+1<3.", &["p(1,1)"]),
    ("p(X,Y):-0<2*X<3*Y<7.", &["p(1,1)", "p(1,2)", "p(2,2)"]),
    (
        "p(X,Y):-0<X+Y<3,0<Y<3.",
        &["p(-1,2)", "p(0,1)", "p(0,2)", "p(1,1)"],
    ),
    ("p(X,Y):-not not 0<X<Y<3.", &["p(1,2)"]),
    ("p(X,Y):-0<X<Y<3,Y<0.", &[]),
    ("p(X):-(-5)<2*X<0.", &["p(-2)", "p(-1)"]),
    ("p(X):-0<(-2)*X<5.", &["p(-2)", "p(-1)"]),
    ("p(X,Y):-0<X=Y+1<4.", &["p(1,0)", "p(2,1)", "p(3,2)"]),
    (
        "p(X,Y):-Y=1..2,0<X+Y<3.",
        &["p(-1,2)", "p(0,1)", "p(0,2)", "p(1,1)"],
    ),
    ("p(X,Y):-0<X<Y<3,Y=X+1.", &["p(1,2)"]),
    ("p(X,Y):-0<X<X<Y<3.", &[]),
    ("p(X,Y):-0<=X<=Y<=1.", &["p(0,0)", "p(0,1)", "p(1,1)"]),
    (
        "p(X,Y):-0<X<3,0<Y<3,X+Y<4.",
        &["p(1,1)", "p(1,2)", "p(2,1)"],
    ),
];

const LOCAL_CASES: &[(&str, &str)] = &[
    ("2#sum{2:p(X,Y):0<X<Y<3}2.", "2#sum{2:p(1,2)}2."),
    ("1#sum+{1:p(X,Y):0<X<Y<3}1.", "1#sum+{1:p(1,2)}1."),
    ("z#min{z:p(X,Y):0<X<Y<3}z.", "z#min{z:p(1,2)}z."),
    ("z#max{z:p(X,Y):0<X<Y<3}z.", "z#max{z:p(1,2)}z."),
    ("1{#true:0<X<3>Y>0}1.", "1{#true}1."),
    (
        "2#count{X,Y:#true:0<X=Y<3}2.",
        "2#count{1,1:#true;2,2:#true}2.",
    ),
    ("2{#true:0<X<Y<3;not p(1,2)}2.", "2{#true;not p(1,2)}2."),
    ("{p(X,Y):0<X<Y<3}.", "{p(1,2)}."),
    ("1{p(X,Y):0<X=Y<3}1.", "1{p(1,1);p(2,2)}1."),
    ("1#count{X,Y:p(X,Y):0<X<Y<3}1.", "1#count{1,2:p(1,2)}1."),
    (
        "{p(1,2)}.q:-#count{X,Y:p(X,Y),0<X<Y<3}=1.",
        "{p(1,2)}.q:-#count{1,2:p(1,2)}=1.",
    ),
    ("n(N):-N=#count{X,Y:0<X<Y<3}.", "n(N):-N=#count{1,2}."),
    ("{p(1,2)}.q:-p(X,Y):0<X<Y<3.", "{p(1,2)}.q:-p(1,2)."),
    ("{p(1,2)}.q:-not p(X,Y):0<X<Y<3.", "{p(1,2)}.q:-not p(1,2)."),
    (
        "d(0;1).{p(K,X,Y):0<X<Y<3}:-d(K).",
        "d(0;1).{p(K,1,2)}:-d(K).",
    ),
    ("d(1).{p(X,Y):0<X<Y<3}:-d(X).", "d(1).{p(1,2)}."),
    ("d(2).{p(X,Y):0<X<Y<3}:-d(X).", "d(2)."),
    ("p(X,Y)|q(X,Y):-0<2*X<3*Y<4.", "p(1,1)|q(1,1)."),
];

fn expected(atoms: &[&str]) -> Models {
    [atoms.iter().map(|atom| (*atom).to_owned()).collect()].into()
}

#[test]
fn finite_chains_generate_exact_integer_assignments() {
    for &(source, atoms) in CASES {
        let program = input(source);
        assert_eq!(native(&program), expected(atoms), "{source}");
        assert_eq!(exhaustive(&program), expected(atoms), "{source}");
    }
}

#[test]
fn coupled_endpoints_preserve_checked_integer_values() {
    // Independent finite arithmetic controls. Coupled endpoint originals have
    // no complete result from the bounded external oracle, so this proposition
    // does not claim their external source correspondence.
    let cases: &[(&str, &[&str])] = &[
        (
            "p(X,Y):-2147483646<=X<=Y<=2147483647.",
            &[
                "p(2147483646,2147483646)",
                "p(2147483646,2147483647)",
                "p(2147483647,2147483647)",
            ],
        ),
        ("p(X,Y):-2147483647<X<=Y<=2147483647.", &[]),
        (
            "p(X,Y):-(-2147483647-1)<=X<=Y<(-2147483647).",
            &["p(-2147483648,-2147483648)"],
        ),
    ];
    for &(source, atoms) in cases {
        let program = input(source);
        assert_eq!(native(&program), expected(atoms), "{source}");
        assert_eq!(exhaustive(&program), expected(atoms), "{source}");
    }
}

#[test]
fn correlated_chains_preserve_every_frozen_pair() {
    let source = "{a;b}.p(X,Y):-a,not b,0<X<Y<3.";
    let program = input(source);
    let expanded = input("{a;b}.p(1,2):-a,not b.");
    same_frozen(&program, &expanded);
}

fn same_frozen(program: &AdmittedFormula, expanded: &AdmittedFormula) {
    let names: Vec<_> = program.atoms().iter().map(atom_text).collect();
    let other: Vec<_> = expanded.atoms().iter().map(atom_text).collect();
    assert_eq!(
        names.iter().collect::<std::collections::BTreeSet<_>>(),
        other.iter().collect()
    );
    let remap = |mask: usize| {
        names.iter().enumerate().fold(0, |result, (index, name)| {
            result
                | if mask & (1 << index) == 0 {
                    0
                } else {
                    1 << other.iter().position(|item| item == name).unwrap()
                }
        })
    };
    assert!(names.len() <= 8, "bounded full frozen comparison");
    for outer in 0..1 << names.len() {
        let left = values(program.theory(), outer, None);
        let right = values(expanded.theory(), remap(outer), None);
        assert_eq!(
            holds(program.theory(), &left),
            holds(expanded.theory(), &right)
        );
        for inner in 0..1 << names.len() {
            assert_eq!(
                holds(
                    program.theory(),
                    &values(program.theory(), inner, Some(&left))
                ),
                holds(
                    expanded.theory(),
                    &values(expanded.theory(), remap(inner), Some(&right))
                ),
                "M={outer}, J={inner}",
            );
        }
    }
}

#[test]
fn local_chains_preserve_scoped_ground_formulas() {
    for &(source, expanded) in LOCAL_CASES {
        let program = input(source);
        let reference = input(expanded);
        assert_eq!(native(&program), native(&reference), "{source}");
        same_frozen(&program, &reference);
    }
}

const UNSAFE: &[&str] = &[
    "p(X,Y):-0<X+Y<3,0<X-Y<3.",
    "p(X,Y):-X+Y=2,X-Y=0.",
    "p(X,Y):-0<X<3,0<Y<X*X.",
    "p(X,Y):-0<X<3,0<Y<2**X.",
    "p(X,Y):-0<X<3,0<Y<X/2+2.",
    "p(X,Y):-0<X>Y<3.",
    "p(X,Y):-0<X+Y<3.",
    "d(1..2).p(X,Y):-d(Y),0<X+Y<3.",
    "p(X,Y):-0<X*Y<5,0<Y<3.",
    "p(X):-0<X*X<5.",
    "p(X):-0<X/2<3.",
    "p(X,Y):-not 0<X<Y<3.",
    "p(X,Y):-0<X-X+Y<3,0<Y<3.",
    "p(X):-q(X,Y):0<X<Y<3.",
];

#[test]
fn absent_finite_domains_remain_unsafe() {
    for source in UNSAFE {
        let result = admit_formula(
            (*source).into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        );
        assert!(
            matches!(result, Err(FormulaFailure::UnsafeVariable { .. })),
            "{source}: {result:?}"
        );
    }
}

#[test]
fn independently_bound_guards_bypass_affine_analysis() {
    for source in [
        "p(X,Y):-X=1..2,Y=1..2,0<X*Y<5.",
        "p(X):-X=0,0<2147483647*(2147483647*(2147483647*X))<2.",
    ] {
        let program = input(source);
        assert_eq!(native(&program), exhaustive(&program));
    }
}

#[test]
fn finite_chains_defer_irrelevant_coefficient_capacity() {
    let program = input("p(X,Y):-(-1)<X<Y<2,0=2147483647*(2147483647*(2147483647*X)).");
    let reference = input("p(0,1).");
    assert_eq!(native(&program), expected(&["p(0,1)"]));
    same_frozen(&program, &reference);
}

#[test]
fn deferred_coefficient_capacity_preserves_reached_arithmetic_errors() {
    let result = admit_formula(
        "p(X,Y):-0<X<Y<3,0=2147483647*(2147483647*(2147483647*X)).".into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    );
    assert!(matches!(
        result,
        Err(FormulaFailure::Expansion(
            ExpansionFailure::Evaluation { .. }
        ))
    ));
}

#[test]
fn coefficient_capacity_is_a_located_analysis_limit() {
    let result = admit_formula(
        "p(X):-0<2147483647*(2147483647*(2147483647*X))<2.".into(),
        AdmissionOptions {
            source_id: SourceId::new(101),
            ..Default::default()
        },
        ExpansionLimits::default(),
        FormulaLimits::default(),
    );
    assert!(
        matches!(result, Err(FormulaFailure::Limit {
        resource: FormulaResource::BindingCoefficientBits, limit: 64, observed: 94, location,
    }) if location.source == SourceId::new(101)),
        "{result:?}"
    );
}

#[test]
fn reached_chain_arithmetic_remains_an_error() {
    for source in [
        "p(X,Y):-0<X<Y<3,0=1,1/0=1.",
        "p(X,Y):-0<=X<Y<3,Y/X>1.",
        "p(X,Y):-0<X<Y<3,2147483647+X>0.",
    ] {
        let result = admit_formula(
            source.into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        );
        assert!(
            matches!(
                result,
                Err(FormulaFailure::Expansion(
                    ExpansionFailure::Evaluation { .. }
                ))
            ),
            "{source}: {result:?}"
        );
    }
}

#[test]
fn envelope_storage_and_work_limits_are_inclusive() {
    for resource in [ExpansionResource::TermWork, ExpansionResource::ScalarBytes] {
        let run = |limit| {
            let mut expansion = ExpansionLimits::default();
            match resource {
                ExpansionResource::TermWork => expansion.max_term_work = limit,
                ExpansionResource::ScalarBytes => expansion.max_scalar_bytes = limit,
                _ => unreachable!("selected envelope resources"),
            }
            admit_formula(
                "p(X,Y):-0<2*X<3*Y<7.".into(),
                AdmissionOptions::default(),
                expansion,
                FormulaLimits::default(),
            )
        };
        let mut lower = 0;
        let mut upper = 1_048_576;
        assert!(run(upper).is_ok());
        while lower < upper {
            let middle = lower + (upper - lower) / 2;
            if run(middle).is_ok() {
                upper = middle;
            } else {
                lower = middle + 1;
            }
        }
        assert!(run(lower).is_ok());
        assert!(
            matches!(run(lower - 1), Err(FormulaFailure::Expansion(ExpansionFailure::Limit { resource: actual, .. })) if actual == resource)
        );
    }
}

#[test]
fn comparison_order_preserves_finite_admission() {
    let comparisons = ["0<X+Y", "X+Y<4", "0<Y", "Y<2"];
    let expected = input("p(0,1).p(1,1).p(2,1).");
    for first in 0..4 {
        for second in 0..4 {
            for third in 0..4 {
                for fourth in 0..4 {
                    let order = [first, second, third, fourth];
                    if order
                        .iter()
                        .collect::<std::collections::BTreeSet<_>>()
                        .len()
                        != 4
                    {
                        continue;
                    }
                    let body = order.map(|index| comparisons[index]).join(",");
                    let program = input(&format!("p(X,Y):-{body}."));
                    same_frozen(&program, &expected);
                }
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]
    #[test]
    fn affine_envelopes_preserve_correlated_reducts(
        left in 1_i32..=3, right in 1_i32..=3, upper in 2_i32..=4,
        negative_left in any::<bool>(), negative_right in any::<bool>(),
    ) {
        let left = if negative_left { -left } else { left };
        let right = if negative_right { -right } else { right };
        let source = format!("{{a;b}}.p(X,Y):-a,not b,0<({left})*X<({right})*Y<{upper}.");
        let reversed = format!("{{a;b}}.p(X,Y):-a,not b,{upper}>({right})*Y>({left})*X>0.");
        let mut expanded = String::from("{a;b}.");
        for x in -3..=3 {
            for y in -3..=3 {
                if 0 < left*x && left*x < right*y && right*y < upper {
                    use std::fmt::Write;
                    write!(expanded, "p({x},{y}):-a,not b.").unwrap();
                }
            }
        }
        let reference = input(&expanded);
        let program = input(&source);
        prop_assert_eq!(native(&program), native(&reference));
        same_frozen(&program, &reference);
        same_frozen(&input(&reversed), &reference);
    }
}

#[test]
#[ignore = "requires an independently installed clingo"]
fn original_chains_match_complete_clingo_answers() {
    for &(source, atoms) in CASES {
        let record = reference::external(source, true);
        let models: Models = record["Call"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|call| call["Witnesses"].as_array().into_iter().flatten())
            .map(|witness| {
                witness["Value"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|atom| atom.as_str().unwrap().to_owned())
                    .collect()
            })
            .collect();
        assert_eq!(models, expected(atoms), "{source}");
        assert_eq!(native(&input(source)), models, "{source}");
    }
    for &(source, expanded) in LOCAL_CASES {
        let record = reference::external(source, true);
        let native_record = reference::external(expanded, true);
        let answers = |record: &serde_json::Value| -> Models {
            record["Call"]
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|call| call["Witnesses"].as_array().into_iter().flatten())
                .map(|witness| {
                    witness["Value"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|atom| atom.as_str().unwrap().to_owned())
                        .collect()
                })
                .collect()
        };
        assert_eq!(answers(&record), answers(&native_record), "{source}");
        assert_eq!(native(&input(source)), answers(&record), "{source}");
    }
}
