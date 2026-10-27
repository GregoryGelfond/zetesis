//! Source formula normalization against explicit scalar facts.

use std::collections::BTreeSet;

use crate::support::admission;
use crate::support::atom_models::{self, Models, answer_sets};
use zetesis_reference_support::resource_limits;

use themelios_base::source::SourceId;
use themelios_base::span::{ByteOffset, Span};
use themelios_program::term::EvalError;
use zetesis_core::{Atom, Value, ValueLimits};
use zetesis_themelios::{
    AdmissionFailure, AdmissionOptions, AdmittedFormula, ExpansionFailure, ExpansionLimits,
    ExpansionResource, FormulaFailure, FormulaLimits, InputLimit, ProfileFeature, admit_formula,
};

fn expand(
    text: String,
    options: AdmissionOptions,
    limits: ExpansionLimits,
) -> Result<AdmittedFormula, ExpansionFailure> {
    match admit_formula(text, options, limits, FormulaLimits::default()) {
        Ok(input) => Ok(input),
        Err(FormulaFailure::Expansion(error)) => Err(error),
        Err(error) => panic!("expected a scalar normalization outcome: {error}"),
    }
}

fn extended(text: &str) -> AdmittedFormula {
    expand(
        text.to_owned(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
    )
    .unwrap_or_else(|error| panic!("extended fixture {text}: {error}"))
}

fn ground_heads(input: &AdmittedFormula) -> BTreeSet<Atom> {
    let atoms = input
        .atoms()
        .iter()
        .map(|atom| atom.to_atom(ValueLimits::default()).unwrap())
        .collect();
    assert_eq!(answer_sets(input), BTreeSet::from([atoms]));
    input
        .atoms()
        .iter()
        .map(|atom| atom.to_atom(ValueLimits::default()).unwrap())
        .collect()
}

fn same_facts(actual: &str, expected: &str) {
    assert_eq!(
        ground_heads(&extended(actual)),
        ground_heads(&extended(expected))
    );
}

#[test]
fn forward_constants_resolve_without_renaming_predicates_or_strings() {
    same_facts(
        "#const a=b+2. #const b=3. #const p=9. p(a,b,p,\"a\",unbound).",
        "p(5,3,9,\"a\",unbound).",
    );
    same_facts("#const a=\"hello\". #const b=a. p(b).", "p(\"hello\").");
}

#[test]
fn checked_ground_arithmetic_matches_explicit_scalars() {
    same_facts(
        "p(2+3*4, (2+3)*4, 2**5, -7/3, -7\\3, | -9 |, ~0, 6&3, 4?1, 7^3).",
        "p(14,20,32,-2,-1,9,-1,2,5,4).",
    );
    let minimum = extended("p(-2147483647-1).");
    assert_eq!(
        minimum
            .atoms()
            .at(0)
            .unwrap()
            .to_atom(ValueLimits::default())
            .unwrap()
            .values(),
        [Value::Number(i32::MIN)]
    );
}

#[test]
fn interval_cartesian_products_and_mixed_arity_pools_are_exact() {
    same_facts(
        "#const n=2. p(1..n,a;b,3..4). q((1;3),5..6). r.",
        "p(1,a). p(2,a). p(b,3). p(b,4). q(1,5). q(1,6). q(3,5). q(3,6). r.",
    );
    same_facts("p(a;b,c). p(2..1).", "p(a). p(b,c).");
}

#[test]
fn nonnumeric_fact_ranges_are_empty() {
    for source in [
        "p(a..b).",
        "p(1..a).",
        "p(a..1).",
        "p(\"a\"..\"b\").",
        "p(#inf..#sup).",
        "p(f(a)..f(b)).",
    ] {
        same_facts(source, "");
    }
}

#[test]
fn empty_ranges_preserve_other_fact_alternatives() {
    same_facts("p(a..b;2..3). q(1,a..2). r.", "p(2).p(3).r.");
}

#[test]
fn empty_ranges_preserve_endpoint_errors() {
    for source in ["p(a..(1/0)).", "p((1/0)..a).", "p(a..b,1/0)."] {
        assert!(
            matches!(
                expand(
                    source.to_owned(),
                    AdmissionOptions::default(),
                    ExpansionLimits::default(),
                ),
                Err(ExpansionFailure::Evaluation {
                    error: EvalError::Undefined,
                    ..
                }),
            ),
            "{source}"
        );
    }
}

#[test]
fn empty_ranges_preserve_variable_errors() {
    for source in ["p(a..b,X).", "p(a..X).", "p(X..a)."] {
        assert!(
            matches!(
                expand(
                    source.to_owned(),
                    AdmissionOptions::default(),
                    ExpansionLimits::default(),
                ),
                Err(ExpansionFailure::Evaluation {
                    error: EvalError::NotGround { .. },
                    ..
                }),
            ),
            "{source}"
        );
    }
}

#[test]
fn scalar_normalization_preserves_bindings_and_frozen_gates() {
    let expanded =
        extended("#const n=2. d(1..n). {p(X)} :- d(X), X != n+1, not q(n+1), not not r(n*2).");
    let explicit = extended("d(1). d(2). {p(X)} :- d(X), X != 3, not q(3), not not r(4).");
    assert_eq!(expanded.atoms(), explicit.atoms());
    assert_eq!(expanded.theory().nodes(), explicit.theory().nodes());
    assert_eq!(expanded.theory().operands(), explicit.theory().operands());
    assert_eq!(expanded.theory().roots(), explicit.theory().roots());
    let input = extended(
        "#const n=2. d(1..n). {q(n+1)}. {r(n*2)}. {p(X)} :- d(X), X != n+1, not q(n+1), not not r(n*2).",
    );
    let mut wanted: Models = [
        vec!["d(1)", "d(2)"],
        vec!["d(1)", "d(2)", "q(3)"],
        vec!["d(1)", "d(2)", "q(3)", "r(4)"],
    ]
    .iter()
    .map(|atoms| atoms.iter().map(|atom| atom_models::atom(atom)).collect())
    .collect();
    for mask in 0..4 {
        let mut atoms = BTreeSet::from([
            atom_models::atom("d(1)"),
            atom_models::atom("d(2)"),
            atom_models::atom("r(4)"),
        ]);
        for position in 0..2 {
            if mask & (1 << position) != 0 {
                atoms.insert(atom_models::atom(&format!("p({})", position + 1)));
            }
        }
        assert!(wanted.insert(atoms));
    }
    assert_eq!(wanted.len(), 7);
    assert_eq!(answer_sets(&input), wanted);
}

#[test]
fn expansion_retains_original_rule_spans_and_source_bytes() {
    let text = "#const n=2.\r\n% no regenerated source\r\np(1..n).\r\np(1..n).";
    let options = admission::options(SourceId::new(81));
    let (input, roots) = crate::support::root_emission::ground(
        zetesis_themelios::prepare_formula(
            text.to_owned(),
            options,
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .expect("located expansion"),
    );
    assert_eq!(input.source().unwrap().text(), text);
    assert_eq!(input.atoms().len(), 2);
    let authored = roots.indices(zetesis_themelios::GroundingPhase::RuleInstantiation);
    assert_eq!(authored.len(), 2);
    for index in authored {
        let origins = &input.formula_origins()[index];
        assert_eq!(
            origins.len(),
            2,
            "equal original rules retain both locations"
        );
        for site in origins {
            let location = site.location().unwrap();
            assert_eq!(location.source, SourceId::new(81));
            assert_eq!(
                input
                    .source()
                    .unwrap()
                    .slice(location.span)
                    .expect("original span"),
                "p(1..n)."
            );
        }
    }
}

#[test]
fn duplicate_definitions_and_policies_are_checked_before_canonicalization() {
    for text in [
        "#const n=2. #const n=2. p(n).",
        "#const n=2. #const n=3. p(n).",
    ] {
        assert!(matches!(
            expand(
                text.to_owned(),
                AdmissionOptions::default(),
                ExpansionLimits::default()
            ),
            Err(ExpansionFailure::DuplicateConstant { .. })
        ));
    }
    assert!(matches!(
        expand(
            "#const n=2. [default] p(n).".to_owned(),
            AdmissionOptions::default(),
            ExpansionLimits::default()
        ),
        Err(ExpansionFailure::ConstantPolicy { .. })
    ));
}

#[test]
fn definition_cycles_are_explicit_and_not_resolved_as_bare_symbols() {
    for text in [
        "#const a=a. p(a).",
        "#const a=b. #const b=c. #const c=a. p(a).",
    ] {
        let error = expand(
            text.to_owned(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
        )
        .expect_err("constant cycle");
        assert!(!error.diagnostics().is_empty());
        match error {
            ExpansionFailure::ConstantCycle { names, .. } => {
                assert_eq!(names.first(), names.last());
            }
            other => panic!("expected dependency cycle: {other}"),
        }
    }
}

#[test]
fn undefined_and_overflowing_arithmetic_refuse_the_whole_program() {
    for text in [
        "p(1/0). q.",
        "p(2**(-1)).",
        "p(2147483647+1).",
        "p(a+1).",
        "#const n=2147483647+1. q.",
        "p(|(-2147483647-1)|).",
    ] {
        assert!(
            matches!(
                expand(
                    text.to_owned(),
                    AdmissionOptions::default(),
                    ExpansionLimits::default()
                ),
                Err(ExpansionFailure::Evaluation { .. })
            ),
            "{text}"
        );
    }
    assert!(matches!(
        expand(
            "p(-2147483648).".to_owned(),
            AdmissionOptions::default(),
            ExpansionLimits::default()
        ),
        Err(ExpansionFailure::Admission(AdmissionFailure::Raise(_)))
    ));
}

#[test]
fn unsupported_forms_never_disappear_behind_empty_intervals() {
    for text in ["p(2..1,X).", "#program base(x). p.", r#"#include "x.lp"."#] {
        assert!(
            zetesis_reference_support::admit(text, &FormulaLimits::default()).is_err(),
            "{text}"
        );
    }
}

#[test]
fn giant_products_are_bounded_before_fact_materialization() {
    let limits = resource_limits::expansion(ExpansionResource::Templates, 5);
    let failure = expand(
        "p(1..3,1..3).".to_owned(),
        AdmissionOptions::default(),
        limits,
    )
    .expect_err("nine facts exceed five");
    assert!(matches!(
        failure,
        ExpansionFailure::Limit {
            resource: ExpansionResource::Templates,
            observed: 9,
            limit: 5,
            ..
        }
    ));
    assert!(matches!(
        expand(
            "p((-2147483647-1)..2147483647).".to_owned(),
            AdmissionOptions::default(),
            ExpansionLimits::default()
        ),
        Err(ExpansionFailure::Limit {
            resource: ExpansionResource::Templates,
            observed: 4_294_967_296,
            ..
        })
    ));
}

#[test]
fn independent_expansion_budgets_and_zero_limits_are_enforced() {
    for (text, expected, limit) in [
        ("#const n=2. p(n).", ExpansionResource::Constants, 0),
        ("p(1+2).", ExpansionResource::TermWork, 0),
        ("p(1..2).", ExpansionResource::Values, 3),
        (
            "#const s=\"abcd\". p(s;s;s).",
            ExpansionResource::ScalarBytes,
            3,
        ),
    ] {
        let limits = resource_limits::expansion(expected, limit);
        assert!(
            matches!(expand(text.to_owned(), AdmissionOptions::default(), limits), Err(ExpansionFailure::Limit { resource, .. }) if resource == expected)
        );
    }
    let empty = ExpansionLimits {
        max_constants: 0,
        max_term_work: 0,
        max_templates: 0,
        max_values: 0,
        max_family_bytes: 0,
        max_scalar_bytes: 0,
        max_origin_locations: 0,
        max_metadata_statements: 0,
    };
    assert!(expand(String::new(), AdmissionOptions::default(), empty).is_ok());
    assert!(
        expand(
            "p.".to_owned(),
            AdmissionOptions::default(),
            resource_limits::expansion(ExpansionResource::Values, 0)
        )
        .is_ok()
    );
    assert!(
        expand(
            "p(2..1).".to_owned(),
            AdmissionOptions::default(),
            resource_limits::expansion(ExpansionResource::Templates, 0)
        )
        .is_ok()
    );
}

#[test]
fn source_and_shared_template_limits_still_apply() {
    for text in ["#const n=2. p(n).", "p(1..2).", "p(1+2).", "p(a;b)."] {
        extended(text);
    }
    let options = resource_limits::admission(InputLimit::SourceBytes, 2);
    assert!(matches!(
        expand("p(1..2).".to_owned(), options, ExpansionLimits::default()),
        Err(ExpansionFailure::Admission(AdmissionFailure::Limit { .. }))
    ));
    // Expansion caps its template budget at the core ceiling, so it reports
    // the core limit as its own.
    let options = AdmissionOptions {
        core_limits: zetesis_core::AdmissionLimits {
            max_templates: 1,
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(matches!(
        expand("p(1..2).".to_owned(), options, ExpansionLimits::default()),
        Err(ExpansionFailure::Limit {
            resource: ExpansionResource::Templates,
            limit: 1,
            ..
        })
    ));
}

#[test]
fn duplicated_rule_evidence_is_bounded_before_per_fact_copying() {
    let limits = resource_limits::expansion(ExpansionResource::Origins, 3);
    assert!(matches!(
        expand(
            "p(1..2). p(1..2).".to_owned(),
            AdmissionOptions::default(),
            limits
        ),
        Err(ExpansionFailure::Limit {
            resource: ExpansionResource::Origins,
            observed: 4,
            limit: 3,
            ..
        })
    ));
}

#[test]
fn valid_closed_structures_do_not_make_an_empty_fact_product_nonempty() {
    for source in ["p(2..1,f(3)).", "p(2..1,(3,4))."] {
        let input = expand(
            source.to_owned(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
        )
        .unwrap();
        assert!(input.theory().roots().is_empty());
        assert_eq!(input.source().unwrap().text(), source);
    }
}

fn limit_failure(text: &str, limits: ExpansionLimits) -> (ExpansionResource, u128, u128, Span) {
    match expand(text.to_owned(), AdmissionOptions::default(), limits) {
        Err(ExpansionFailure::Limit {
            resource,
            limit,
            observed,
            location,
        }) => (
            resource,
            limit,
            observed,
            location.location().expect("source expansion location").span,
        ),
        other => panic!("{text}: expected a limit refusal, got {other:?}"),
    }
}

#[test]
fn a_term_work_ceiling_trips_at_the_node_the_normalization_reaches() {
    // The facts are normalized in their canonical order, node by node: the
    // second node of `e(3,4,5)` is the fifth unit, one over the ceiling, and
    // the refusal names the running total there and that statement's span.
    let (resource, limit, observed, span) = limit_failure(
        "e(0,1,2). e(3,4,5).",
        resource_limits::expansion(ExpansionResource::TermWork, 4),
    );
    assert_eq!(resource, ExpansionResource::TermWork);
    assert_eq!((limit, observed), (4, 5));
    assert_eq!(
        span,
        Span::new(ByteOffset::new(10), ByteOffset::new(19)).unwrap()
    );
}

#[test]
fn scalars_are_charged_head_first_and_then_in_argument_order() {
    // Four bytes in the head exceed a ceiling of three before the body's one
    // byte is reached; one byte in the first argument is within it and the
    // four of the second then make five.
    let limits = resource_limits::expansion(ExpansionResource::ScalarBytes, 3);
    let (resource, limit, observed, _) = limit_failure("q(\"aaaa\") :- p(\"b\").", limits);
    assert_eq!(
        (resource, limit, observed),
        (ExpansionResource::ScalarBytes, 3, 4)
    );
    let (resource, limit, observed, _) = limit_failure("p(\"b\",\"aaaa\").", limits);
    assert_eq!(
        (resource, limit, observed),
        (ExpansionResource::ScalarBytes, 3, 5)
    );
}

#[test]
fn a_definition_after_a_kept_leaf_preserves_both_values() {
    same_facts("#const n=2. p(1,n).", "p(1,2).");
}

#[test]
fn plain_rules_preserve_their_explicit_consequences() {
    let expanded = extended("e(0,1). r(0). r(Y) :- r(X), e(X,Y). q :- r(1), not p.");
    let expected = ground_heads(&extended("e(0,1).r(0).r(1).q."));
    assert_eq!(answer_sets(&expanded), BTreeSet::from([expected]));
    let sites = expanded
        .formula_origins()
        .iter()
        .flat_map(|sites| {
            assert!(!sites.is_empty());
            sites.iter().map(|site| {
                expanded
                    .source()
                    .unwrap()
                    .slice(site.location().unwrap().span)
                    .unwrap()
            })
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        sites,
        [
            "e(0,1).",
            "r(0).",
            "r(Y) :- r(X), e(X,Y).",
            "q :- r(1), not p."
        ]
        .into()
    );
}

#[test]
fn plain_fact_strings_with_nul_are_refused() {
    let error = expand(
        "p(\"bad\0value\").".into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
    )
    .unwrap_err();
    assert!(
        matches!(
            error,
            ExpansionFailure::Admission(AdmissionFailure::Profile {
                feature: ProfileFeature::NulString,
                ..
            })
        ),
        "{error}"
    );
}
