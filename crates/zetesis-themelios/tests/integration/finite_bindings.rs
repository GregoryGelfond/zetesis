//! Finite candidate domains preserve complete guards, scopes and source evidence.

use crate::support::finite_bindings as reference;
use crate::support::finite_bindings::remap;

use std::collections::BTreeSet;

use themelios_base::source::SourceId;
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionFailure, ExpansionLimits, ExpansionResource,
    FormulaFailure, FormulaLimits, FormulaResource, admit_formula,
};

use reference::{Models, exhaustive, holds, native, values};

const SOURCE: SourceId = SourceId::new(113);

fn input_with(
    source: &str,
    expansion: ExpansionLimits,
    limits: &FormulaLimits,
) -> Result<AdmittedFormula, FormulaFailure> {
    admit_formula(
        source.into(),
        AdmissionOptions {
            source_id: SOURCE,
            ..Default::default()
        },
        expansion,
        *limits,
    )
}

fn input(source: &str) -> AdmittedFormula {
    input_with(
        source,
        ExpansionLimits::default(),
        &FormulaLimits::default(),
    )
    .unwrap_or_else(|error| panic!("{source}: {error}"))
}

fn model(atoms: impl IntoIterator<Item = String>) -> Models {
    BTreeSet::from([atoms.into_iter().collect()])
}

#[test]
fn separate_closed_bounds_match_independent_integer_substitutions() {
    for (source, expected) in bounded_domains() {
        let program = input(&source);
        assert_eq!(native(&program), expected, "{source}");
        assert_eq!(exhaustive(&program), expected, "{source}");
    }
}

fn bounded_domains() -> Vec<(String, Models)> {
    let mut cases = Vec::new();
    for lower in -2..=2 {
        for upper in -2..=2 {
            for (left, right) in [("<", "<"), ("<=", "<"), ("<", "<="), ("<=", "<=")] {
                let expected = model((-3..=3).filter_map(|value| {
                    let above = if left == "<" {
                        lower < value
                    } else {
                        lower <= value
                    };
                    let below = if right == "<" {
                        value < upper
                    } else {
                        value <= upper
                    };
                    (above && below).then(|| format!("p({value})"))
                }));
                for body in [
                    format!("{lower}{left}X,X{right}{upper}"),
                    format!("X{right}{upper},{lower}{left}X,{lower}{left}X"),
                ] {
                    let source = format!("p(X):-{body}.");
                    cases.push((source, expected.clone()));
                }
            }
        }
    }
    cases
}

fn expansions() -> Vec<(&'static str, &'static str)> {
    vec![
        ("p(X):-not not X=1..2.", "p(1).p(2)."),
        ("p(X):-not not 1..2=X.", "p(1).p(2)."),
        ("p(X):-not not X=2..1.", ""),
        ("d(2).p(X):-d(U),not not X=1..U.", "d(2).p(1).p(2)."),
        ("d(0;2).p(X):-d(X),not not X=1..2.", "d(0;2).p(2)."),
        ("p(X,Y):-not not (X,Y)=(1,2).", "p(1,2)."),
        ("d(1;2).p(X,Y):-d(Y),not not (X,Y)=(Y,1).", "d(1;2).p(1,1)."),
        ("d(1;2).p(X,Y):-d(Y),(X,Y)=(Y,1).", "d(1;2).p(1,1)."),
        ("p(X):-not not (X,X)=(1,2).", ""),
        ("p(X,Y):-(X,Y)=(1,2).", "p(1,2)."),
        ("p(X,Y):-0<X,X<3,0<Y,Y<2.", "p(1,1).p(2,1)."),
        (
            "{a}.p(X):-0<X,X<3,a.a:-p(1).",
            "{a}.p(1):-a.p(2):-a.a:-p(1).",
        ),
        (
            "{a}.p(X):-not not X=1..2,not a.",
            "{a}.p(1):-not a.p(2):-not a.",
        ),
        ("{p(X):0<X,X<3}.", "{p(1);p(2)}."),
        ("1{p(X):not not X=1..2}1.", "1{p(1);p(2)}1."),
        (
            "{a}.n(N):-N=#count{X:0<X,X<3,a}.",
            "{a}.n(N):-N=#count{1:a;2:a}.",
        ),
        ("{p(1);p(2)}.q:-p(X):0<X,X<3.", "{p(1);p(2)}.q:-p(1),p(2)."),
        ("{p(1)}.-p(X):-not not X=1..1.", "{p(1)}.-p(1)."),
    ]
}

#[test]
fn explicit_finite_instances_match_every_original_and_frozen_world() {
    for (source, specified) in expansions() {
        let generated = input(source);
        let explicit = input(specified);
        assert_eq!(generated.source().text(), source);
        assert_eq!(native(&generated), exhaustive(&explicit), "{source}");
        assert_eq!(exhaustive(&generated), exhaustive(&explicit), "{source}");
        assert_eq!(
            generated.atoms().iter().collect::<BTreeSet<_>>(),
            explicit.atoms().iter().collect(),
            "{source}"
        );
        assert!(generated.atoms().len() <= 8);
        for outer in 0..1 << generated.atoms().len() {
            let original = values(generated.theory(), outer, None);
            let specified_original = values(
                explicit.theory(),
                remap(outer, generated.atoms(), explicit.atoms()),
                None,
            );
            assert_eq!(
                holds(generated.theory(), &original),
                holds(explicit.theory(), &specified_original)
            );
            for inner in 0..1 << generated.atoms().len() {
                assert_eq!(
                    holds(
                        generated.theory(),
                        &values(generated.theory(), inner, Some(&original))
                    ),
                    holds(
                        explicit.theory(),
                        &values(
                            explicit.theory(),
                            remap(inner, generated.atoms(), explicit.atoms()),
                            Some(&specified_original)
                        )
                    ),
                    "{source}, M={outer}, J={inner}"
                );
            }
        }
        for location in generated.formula_origins().iter().flatten() {
            assert_eq!(location.source, SOURCE);
            assert!(generated.source().slice(location.span).is_ok());
        }
    }
}

fn unsafe_sources() -> [&'static str; 13] {
    [
        "p(X):-not 0<X,X<3.",
        "d(0,3).p(X):-d(L,U),L<X<U.",
        "p(X):-not not X=X+1.",
        "p(X,Y):-not not (X,Y)=(Y,X).",
        "p(X,Y):-not not (X,Y)=(Y,1).",
        "p(X,Y):-(X,Y)=(Y,1).",
        "p(X,Y):-not not (X,Y)=(1,X).",
        "p(X,Y):-(X,Y)=(1,X).",
        "p(X,Y):-not not (X,1)=(2,Y).",
        "p(X,Y):-(X,1)=(2,Y).",
        "p(X):-#count{Y:0<Y,Y<3}=1.",
        "p(X):-1{q(Y):0<Y,Y<3}1.",
        "p(X):-q(X):0<X,X<3.",
    ]
}

#[test]
fn scope_and_nonbinding_negation_do_not_acquire_accidental_domains() {
    for source in unsafe_sources() {
        let error = input_with(
            source,
            ExpansionLimits::default(),
            &FormulaLimits::default(),
        )
        .unwrap_err();
        assert!(
            matches!(error, FormulaFailure::UnsafeVariable { .. }),
            "{source}: {error}"
        );
        assert!(
            error
                .diagnostics()
                .iter()
                .all(|diagnostic| diagnostic.primary().location.source == SOURCE)
        );
    }
}

#[test]
fn active_undefined_operands_remain_located_failures() {
    for source in [
        "d(0).p(X):-d(D),not not X=1..1/D.",
        "d(1).p:-d(X),not not (X,1)=(1,2,1/0).",
        "p(X):-0<X,X<3,not not X=1>2<1/0.",
    ] {
        let error = input_with(
            source,
            ExpansionLimits::default(),
            &FormulaLimits::default(),
        )
        .unwrap_err();
        assert!(
            matches!(
                error,
                FormulaFailure::Expansion(ExpansionFailure::Evaluation { .. })
            ),
            "{source}: {error}"
        );
        assert!(
            error
                .diagnostics()
                .iter()
                .all(|diagnostic| diagnostic.primary().location.source == SOURCE)
        );
    }
}

#[test]
fn mixed_tuple_bindings_omit_only_undefined_substitutions() {
    let program = input("d(0;1).p(X):-d(D),not not (X,1/D)=(1,1).");
    let expected = native(&input("d(0;1).p(1)."));
    assert_eq!(program.warnings().len(), 1);
    assert_eq!(native(&program), expected);
    assert_eq!(exhaustive(&program), expected);
}

#[test]
fn exact_domain_ceilings_empty_intervals_and_retry_preserve_results() {
    for source in ["p(X):-0<X,X<4.", "p(X):-not not X=1..3."] {
        let error = input_with(
            source,
            ExpansionLimits::default(),
            &FormulaLimits {
                max_assignment_values: 2,
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(
            matches!(error, FormulaFailure::Limit { resource: FormulaResource::AssignmentValues, limit: 2, observed: 3, location } if location.source == SOURCE),
            "{error}"
        );
        let program = input_with(
            source,
            ExpansionLimits::default(),
            &FormulaLimits {
                max_assignment_values: 3,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            native(&program),
            model(["p(1)".into(), "p(2)".into(), "p(3)".into()])
        );
    }
    for source in [
        "p(X):-3<X,X<0.",
        "p(X):-2147483647<X,X<=2147483647.",
        "p(X):-(-2147483647-1)<=X,X<(-2147483647-1).",
        "p(X):-not not X=1..0.",
    ] {
        let program = input_with(
            source,
            ExpansionLimits::default(),
            &FormulaLimits {
                max_assignment_values: 0,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(native(&program), model([]), "{source}");
    }
}

#[test]
fn planner_work_refusal_is_transactional_at_the_exact_ceiling() {
    let source = "{p(X):0<X,X<3}.q:-p(Y):not not (Y,1)=(2,1).";
    let mut lower = 0;
    let mut upper = ExpansionLimits::default().max_term_work;
    while lower < upper {
        let limit = lower + (upper - lower) / 2;
        match input_with(
            source,
            ExpansionLimits {
                max_term_work: limit,
                ..Default::default()
            },
            &FormulaLimits::default(),
        ) {
            Ok(_) => upper = limit,
            Err(FormulaFailure::Expansion(ExpansionFailure::Limit {
                resource: ExpansionResource::TermWork,
                ..
            })) => lower = limit + 1,
            Err(error) => panic!("unexpected refusal: {error}"),
        }
    }
    assert!(lower > 0);
    let error = input_with(
        source,
        ExpansionLimits {
            max_term_work: lower - 1,
            ..Default::default()
        },
        &FormulaLimits::default(),
    )
    .unwrap_err();
    assert!(
        matches!(error, FormulaFailure::Expansion(ExpansionFailure::Limit { resource: ExpansionResource::TermWork, limit, observed, location }) if limit == (lower - 1) as u128 && observed > limit && location.source == SOURCE)
    );
    let program = input_with(
        source,
        ExpansionLimits {
            max_term_work: lower,
            ..Default::default()
        },
        &FormulaLimits::default(),
    )
    .unwrap();
    assert_eq!(native(&program), exhaustive(&input("{p(1);p(2)}.q:-p(2).")));
}

#[test]
#[ignore = "requires clingo: finite bindings match clingo; bounded original sources and full model records"]
fn finite_bindings_match_clingo() {
    let mut cases = bounded_domains();
    cases.extend(
        expansions()
            .into_iter()
            .map(|(source, specified)| (source.into(), exhaustive(&input(specified)))),
    );
    for (source, expected) in cases {
        let actual = reference::external(&source, true);
        assert_eq!(actual["Models"]["More"], "no", "{source}");
        let witnesses: Vec<_> = actual["Call"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|call| call["Witnesses"].as_array().into_iter().flatten())
            .collect();
        assert!(witnesses.iter().all(|witness| witness["Costs"].is_null()));
        let models: Models = witnesses
            .iter()
            .map(|witness| {
                witness["Value"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|atom| atom.as_str().unwrap().to_owned())
                    .collect()
            })
            .collect();
        assert_eq!(models.len(), witnesses.len(), "full models are unique");
        assert_eq!(
            actual["Models"]["Number"].as_u64().unwrap(),
            u64::try_from(models.len()).unwrap()
        );
        assert_eq!(models, expected, "{source}");
        assert_eq!(native(&input(&source)), expected, "{source}");
    }
    for source in unsafe_sources() {
        let actual = reference::external(source, false);
        assert_eq!(actual["Result"], "UNKNOWN");
    }
}
