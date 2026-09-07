//! Boundary laws against real themelios parsing and raising. These fixtures test
//! source admission and normalized structure, not solver outcomes.

use themelios_base::source::SourceId;
use zetesis_core::{Filter, Term, Value};
use zetesis_themelios::{
    AdmissionFailure, AdmissionOptions, Admitted, InputLimit, ProfileFeature, admit,
};

fn accepted(text: &str) -> Admitted {
    admit(text.to_owned(), AdmissionOptions::default()).expect("fixture is in S0")
}

#[test]
fn positive_recursion_remains_a_positive_dependency() {
    let input = accepted("p :- q. q :- p.");
    assert_eq!(input.program().templates().len(), 2);
    for template in input.program().templates() {
        assert_eq!(template.positive().len(), 1);
        assert!(template.gate_true().is_empty());
        assert!(template.gate_false().is_empty());
    }
}

#[test]
fn choice_head_is_a_frozen_true_gate_not_a_positive_antecedent() {
    let input = accepted("{p}.");
    let template = &input.program().templates()[0];
    assert!(template.positive().is_empty());
    assert_eq!(
        template.gate_true(),
        &[template.head().expect("choice head").clone()]
    );
    assert!(template.gate_false().is_empty());
}

#[test]
fn default_and_double_default_negation_have_opposite_gate_polarities() {
    let input = accepted("p :- not q, not not r.");
    let template = &input.program().templates()[0];
    assert_eq!(template.gate_false()[0].predicate().name(), "q");
    assert_eq!(template.gate_true()[0].predicate().name(), "r");
    assert!(template.positive().is_empty());
}

#[test]
fn constraints_share_gates_and_scalar_filters() {
    let input = accepted(":- not p, not not q, 1 != 2.");
    let template = &input.program().templates()[0];
    assert!(template.head().is_none());
    assert_eq!(template.gate_false().len(), 1);
    assert_eq!(template.gate_true().len(), 1);
    assert_eq!(
        template.filters(),
        &[Filter::Neq(
            Term::Constant(Value::Number(1)),
            Term::Constant(Value::Number(2))
        )]
    );
}

#[test]
fn scalar_values_keep_their_types_and_decode_strings() {
    let input = accepted("p(a, \"a\", 12, -7, \"a\\\"b\").");
    let terms = input.program().templates()[0]
        .head()
        .expect("fact head")
        .terms();
    assert_eq!(
        terms,
        &[
            Term::Constant(Value::Symbol("a".to_owned())),
            Term::Constant(Value::String("a".to_owned())),
            Term::Constant(Value::Number(12)),
            Term::Constant(Value::Number(-7)),
            Term::Constant(Value::String("a\"b".to_owned())),
        ]
    );
}

#[test]
fn numeral_boundaries_follow_the_actual_raiser() {
    accepted("p(2147483647, -2147483647).");
    assert!(matches!(
        admit("p(-2147483648).".to_owned(), AdmissionOptions::default()),
        Err(AdmissionFailure::Raise(_))
    ));
    assert!(matches!(
        admit("p(2147483648).".to_owned(), AdmissionOptions::default()),
        Err(AdmissionFailure::Raise(_))
    ));
}

#[test]
fn erased_program_delimiters_are_refused_in_the_source_tree() {
    for text in [
        "#program base.",
        "#program step(t).",
        "p. #program empty.",
        "#program empty. #program base. p.",
    ] {
        assert!(
            matches!(
                admit(text.to_owned(), AdmissionOptions::default()),
                Err(AdmissionFailure::Profile {
                    feature: ProfileFeature::Statement,
                    ..
                })
            ),
            "{text}"
        );
    }
    accepted("p(\"#program step.\").");
}

#[test]
fn raw_choice_shape_is_checked_before_set_canonicalization() {
    for text in [
        "{p;p}.", "{p;q}.", "{}.", "1 {p}.", "{p} 1.", "{p:}.", "{p:q}.",
    ] {
        assert!(
            admit(text.to_owned(), AdmissionOptions::default()).is_err(),
            "{text}"
        );
    }
}

#[test]
fn unsupported_language_families_never_become_partial_programs() {
    for text in [
        "p. #show p/0.",
        "#const n=1. p(n).",
        "#include \"missing.lp\".",
        "p | q.",
        "p :- #count{1:q} > 0.",
        "p :- q:r.",
        "not p.",
        "{not p}.",
        "p :- #true.",
        "#true.",
        "p(1..3).",
        "p(a;a).",
        "p((a;a)).",
        "p(1+2).",
        "p(~1).",
        "p(- -1).",
        "p(@f()).",
        "p(#inf).",
        "p :- 1 < 2.",
        "p :- 1 = 1 = 1.",
        "p :- not 1 = 1.",
    ] {
        assert!(
            admit(text.to_owned(), AdmissionOptions::default()).is_err(),
            "{text}"
        );
    }
}

#[test]
fn empty_body_scalar_filters_do_not_need_variable_bindings() {
    let input = accepted("p :- 1=1. :- 1!=1.");
    assert_eq!(input.program().templates().len(), 2);
    assert!(
        input
            .program()
            .templates()
            .iter()
            .all(|template| template.positive().is_empty() && template.filters().len() == 1)
    );
    assert!(matches!(
        admit("p(X) :- X=1.".to_owned(), AdmissionOptions::default()),
        Err(AdmissionFailure::Core { .. })
    ));
}

#[test]
fn anonymous_occurrences_are_fresh_and_named_occurrences_are_shared() {
    let input = accepted("p(X) :- r(_,X,_).");
    let template = &input.program().templates()[0];
    let terms = template.positive()[0].terms();
    assert_ne!(terms[0], terms[2]);
    assert_eq!(template.head().expect("head").terms()[0], terms[1]);
    for text in ["p(_) :- r(_).", "p :- not r(_), s(_).", "p :- r(_), _=1."] {
        assert!(
            matches!(
                admit(text.to_owned(), AdmissionOptions::default()),
                Err(AdmissionFailure::Core { .. })
            ),
            "{text}"
        );
    }
}

#[test]
fn byte_limits_are_checked_before_syntax() {
    let options = AdmissionOptions {
        max_source_bytes: 2,
        ..AdmissionOptions::default()
    };
    assert!(matches!(
        admit("invalid input".to_owned(), options),
        Err(AdmissionFailure::Limit {
            resource: InputLimit::SourceBytes,
            ..
        })
    ));
    accepted("");
}

#[test]
fn body_limits_count_source_occurrences_before_deduplication() {
    let options = AdmissionOptions {
        max_body_elements: 1,
        ..AdmissionOptions::default()
    };
    assert!(matches!(
        admit("p :- q,q.".to_owned(), options),
        Err(AdmissionFailure::Limit {
            resource: InputLimit::BodyElements,
            ..
        })
    ));
}

#[test]
fn syntax_node_and_depth_limits_refuse_with_locations() {
    for (options, expected) in [
        (
            AdmissionOptions {
                max_syntax_nodes: 0,
                ..AdmissionOptions::default()
            },
            InputLimit::SyntaxNodes,
        ),
        (
            AdmissionOptions {
                max_syntax_depth: 1,
                ..AdmissionOptions::default()
            },
            InputLimit::SyntaxDepth,
        ),
    ] {
        match admit("p(a).".to_owned(), options) {
            Err(AdmissionFailure::Limit {
                resource, location, ..
            }) => {
                assert_eq!(resource, expected);
                assert_eq!(location.source, options.source_id);
            }
            other => panic!("expected traversal refusal: {other:?}"),
        }
    }
}

#[test]
fn core_limits_remain_an_independent_admission_door() {
    let mut options = AdmissionOptions::default();
    options.core_limits.max_templates = 1;
    assert!(matches!(
        admit("p. q.".to_owned(), options),
        Err(AdmissionFailure::Core { .. })
    ));
}

#[test]
fn syntax_refusal_preserves_dependency_diagnostics_and_source_identity() {
    let options = AdmissionOptions {
        source_id: SourceId::new(42),
        ..AdmissionOptions::default()
    };
    let error = admit("p(.".to_owned(), options).expect_err("broken source");
    assert!(matches!(error, AdmissionFailure::Syntax(_)));
    let diagnostics = error.diagnostics();
    assert!(!diagnostics.is_empty());
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.primary().location.source == options.source_id)
    );
}

#[test]
fn merged_rules_retain_all_source_origins() {
    let input = accepted("p(a). p(a).");
    assert_eq!(input.program().templates().len(), 1);
    assert_eq!(input.template_origins()[0].len(), 2);
    assert_eq!(input.source().text(), "p(a). p(a).");
}

#[test]
fn closed_function_and_tuple_facts_have_complete_distinct_values() {
    for (source, spelling) in [
        ("p(f(a)).", "f(a)"),
        ("p(f()).", "f"),
        ("p((a,b)).", "(a,b)"),
        ("p(()).", "()"),
        ("p(-a).", "-a"),
        ("p(-f()).", "-f"),
        ("p(-f(1)).", "-f(1)"),
    ] {
        let input = admit(source.to_owned(), AdmissionOptions::default()).unwrap();
        assert_eq!(input.program().domain().len(), 1);
        let value = &input.program().domain()[0];
        let actual = match value {
            zetesis_core::Value::Structured(value) => value.to_string(),
            zetesis_core::Value::Symbol(value) => value.clone(),
            other => panic!("unexpected {other:?}"),
        };
        assert_eq!(actual, spelling);
    }
}
