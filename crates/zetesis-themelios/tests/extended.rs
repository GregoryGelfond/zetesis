//! Opt-in source normalization against explicit scalar S0 programs.

use std::collections::BTreeSet;

use themelios_base::source::SourceId;
use themelios_program::term::EvalError;
use zetesis_core::{Atom, Term};
use zetesis_themelios::{
    AdmissionFailure, AdmissionOptions, Admitted, ExpansionFailure, ExpansionLimits,
    ExpansionResource, admit, admit_extended,
};

fn extended(text: &str) -> Admitted {
    admit_extended(
        text.to_owned(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
    )
    .unwrap_or_else(|error| panic!("extended fixture {text}: {error}"))
}

fn ground_heads(input: &Admitted) -> BTreeSet<Atom> {
    input
        .program()
        .templates()
        .iter()
        .map(|template| {
            assert!(template.positive().is_empty());
            assert!(template.gate_true().is_empty());
            assert!(template.gate_false().is_empty());
            assert!(template.filters().is_empty());
            let head = template.head().expect("fact");
            let values = head
                .terms()
                .iter()
                .map(|term| match term {
                    Term::Constant(value) => value.clone(),
                    Term::Variable(_) => panic!("ground fact"),
                })
                .collect();
            Atom::new(head.predicate().clone(), values).expect("head arity")
        })
        .collect()
}

fn same_facts(actual: &str, expected: &str) {
    let explicit =
        admit(expected.to_owned(), AdmissionOptions::default()).expect("explicit S0 facts");
    assert_eq!(ground_heads(&extended(actual)), ground_heads(&explicit));
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
        minimum.program().templates()[0]
            .head()
            .expect("head")
            .terms(),
        &[Term::Constant(zetesis_core::Value::Number(i32::MIN))]
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
fn scalar_normalization_preserves_relational_bindings_and_frozen_gates() {
    let expanded =
        extended("#const n=2. d(1..n). {p(X)} :- d(X), X != n+1, not q(n+1), not not r(n*2).");
    let explicit = admit(
        "d(1). d(2). {p(X)} :- d(X), X != 3, not q(3), not not r(4).".to_owned(),
        AdmissionOptions::default(),
    )
    .expect("explicit S0 program");
    assert_eq!(
        expanded.program().templates().len(),
        explicit.program().templates().len()
    );
    for template in expanded.program().templates() {
        assert!(explicit.program().templates().contains(template));
    }
}

#[test]
fn expansion_retains_original_rule_spans_and_source_bytes() {
    let text = "#const n=2.\r\n% no regenerated source\r\np(1..n).\r\np(1..n).";
    let options = AdmissionOptions {
        source_id: SourceId::new(81),
        ..AdmissionOptions::default()
    };
    let input = admit_extended(text.to_owned(), options, ExpansionLimits::default())
        .expect("located expansion");
    assert_eq!(input.source().text(), text);
    assert_eq!(input.program().templates().len(), 2);
    for origins in input.template_origins() {
        assert_eq!(
            origins.len(),
            2,
            "equal original rules retain both locations"
        );
        for location in origins {
            assert_eq!(location.source, SourceId::new(81));
            assert_eq!(
                input.source().slice(location.span).expect("original span"),
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
            admit_extended(
                text.to_owned(),
                AdmissionOptions::default(),
                ExpansionLimits::default()
            ),
            Err(ExpansionFailure::DuplicateConstant { .. })
        ));
    }
    assert!(matches!(
        admit_extended(
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
        let error = admit_extended(
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
                admit_extended(
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
        admit_extended(
            "p(-2147483648).".to_owned(),
            AdmissionOptions::default(),
            ExpansionLimits::default()
        ),
        Err(ExpansionFailure::Admission(AdmissionFailure::Raise(_)))
    ));
}

#[test]
fn unsupported_forms_never_disappear_behind_empty_intervals() {
    for text in [
        "p(2..1,X).",
        "p((1..2)+3).",
        "p(1..2) :- q.",
        "p(1;2) :- q.",
        "p(X+1) :- q(X).",
        "p :- X=1.",
        "#program base. p.",
        "#include \"x.lp\".",
        "{p;q}.",
        "p :- #count{X:q(X)}=1.",
    ] {
        assert!(
            admit_extended(
                text.to_owned(),
                AdmissionOptions::default(),
                ExpansionLimits::default()
            )
            .is_err(),
            "{text}"
        );
    }
    assert!(matches!(
        admit_extended(
            "p(X+1) :- q(X).".to_owned(),
            AdmissionOptions::default(),
            ExpansionLimits::default()
        ),
        Err(ExpansionFailure::Evaluation {
            error: EvalError::NotGround { .. },
            ..
        })
    ));
}

#[test]
fn giant_products_are_bounded_before_fact_materialization() {
    let limits = ExpansionLimits {
        max_templates: 5,
        ..ExpansionLimits::default()
    };
    let failure = admit_extended(
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
        admit_extended(
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
    for (text, limits, expected) in [
        (
            "#const n=2. p(n).",
            ExpansionLimits {
                max_constants: 0,
                ..ExpansionLimits::default()
            },
            ExpansionResource::Constants,
        ),
        (
            "p(1+2).",
            ExpansionLimits {
                max_term_work: 0,
                ..ExpansionLimits::default()
            },
            ExpansionResource::TermWork,
        ),
        (
            "p(1..2).",
            ExpansionLimits {
                max_values: 3,
                ..ExpansionLimits::default()
            },
            ExpansionResource::Values,
        ),
        (
            "#const s=\"abcd\". p(s;s;s).",
            ExpansionLimits {
                max_scalar_bytes: 3,
                ..ExpansionLimits::default()
            },
            ExpansionResource::ScalarBytes,
        ),
    ] {
        assert!(
            matches!(admit_extended(text.to_owned(), AdmissionOptions::default(), limits), Err(ExpansionFailure::Limit { resource, .. }) if resource == expected)
        );
    }
    let empty = ExpansionLimits {
        max_constants: 0,
        max_term_work: 0,
        max_templates: 0,
        max_values: 0,
        max_scalar_bytes: 0,
        max_origin_locations: 0,
        max_metadata_statements: 0,
    };
    assert!(admit_extended(String::new(), AdmissionOptions::default(), empty).is_ok());
    assert!(
        admit_extended(
            "p.".to_owned(),
            AdmissionOptions::default(),
            ExpansionLimits {
                max_values: 0,
                ..ExpansionLimits::default()
            }
        )
        .is_ok()
    );
    assert!(
        admit_extended(
            "p(2..1).".to_owned(),
            AdmissionOptions::default(),
            ExpansionLimits {
                max_templates: 0,
                ..ExpansionLimits::default()
            }
        )
        .is_ok()
    );
}

#[test]
fn extended_is_explicit_and_source_and_core_limits_still_apply() {
    for text in ["#const n=2. p(n).", "p(1..2).", "p(1+2).", "p(a;b)."] {
        assert!(admit(text.to_owned(), AdmissionOptions::default()).is_err());
        extended(text);
    }
    let options = AdmissionOptions {
        max_source_bytes: 2,
        ..AdmissionOptions::default()
    };
    assert!(matches!(
        admit_extended("p(1..2).".to_owned(), options, ExpansionLimits::default()),
        Err(ExpansionFailure::Admission(AdmissionFailure::Limit { .. }))
    ));
    let mut options = AdmissionOptions::default();
    options.core_limits.max_templates = 1;
    assert!(matches!(
        admit_extended("p(1..2).".to_owned(), options, ExpansionLimits::default()),
        Err(ExpansionFailure::Limit {
            resource: ExpansionResource::Templates,
            limit: 1,
            ..
        })
    ));
}

#[test]
fn duplicated_rule_evidence_is_bounded_before_per_fact_copying() {
    let limits = ExpansionLimits {
        max_origin_locations: 3,
        ..ExpansionLimits::default()
    };
    assert!(matches!(
        admit_extended(
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
        let input = admit_extended(
            source.to_owned(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
        )
        .unwrap();
        assert!(input.program().templates().is_empty());
        assert_eq!(input.source().text(), source);
    }
}
