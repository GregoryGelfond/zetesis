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
fn nonbase_delimiters_are_refused_before_raising() {
    for text in [
        "#program base(x).",
        "#program step(t).",
        "p. #program empty.",
        "#program empty. #program base. p.",
    ] {
        assert!(
            matches!(
                admit(text.to_owned(), AdmissionOptions::default()),
                Err(AdmissionFailure::Profile {
                    feature: ProfileFeature::ProgramPart,
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
    use themelios_base::source::Source;

    // Pin the raw relational-profile refusal, not an arbitrary parse/raise or
    // later admission failure. Duplicate elements and an explicit empty
    // condition must not disappear before this authored shape check.
    for (text, feature) in [
        ("{p;p}.", ProfileFeature::ChoiceCardinality),
        ("{p;q}.", ProfileFeature::ChoiceCardinality),
        ("{}.", ProfileFeature::ChoiceCardinality),
        ("1 {p}.", ProfileFeature::BoundedChoice),
        ("{p} 1.", ProfileFeature::BoundedChoice),
        ("{p:}.", ProfileFeature::ConditionalChoice),
        ("{p:q}.", ProfileFeature::ConditionalChoice),
    ] {
        let source_id = SourceId::new(220);
        let source = Source::new(source_id, text.into()).unwrap();
        let error = admit(
            text.into(),
            AdmissionOptions {
                source_id,
                ..AdmissionOptions::default()
            },
        )
        .unwrap_err();
        let AdmissionFailure::Profile {
            feature: actual,
            location,
        } = error
        else {
            panic!("{text}: {error}");
        };
        assert_eq!(actual, feature, "{text}");
        assert_eq!(location.source, source_id);
        assert_eq!(
            source.slice(location.span).unwrap(),
            text.strip_suffix('.').unwrap(),
            "the original complete choice head must be located: {text}"
        );
    }
}

#[test]
fn relational_refusals_preserve_the_profile_diagnostic() {
    use themelios_base::diagnostic::Severity;
    use themelios_base::source::Source;

    // These are refusals of the deliberately smaller relational admission
    // door. They do not assert that ordinary formula solving refuses the forms.
    for (text, feature, description, highlighted) in [
        (
            "{p;q}.",
            ProfileFeature::ChoiceCardinality,
            "choice with other than one element",
            "{p;q}",
        ),
        (
            "1{p}.",
            ProfileFeature::BoundedChoice,
            "bounded choice",
            "1{p}",
        ),
        (
            "{p:q}.",
            ProfileFeature::ConditionalChoice,
            "conditional choice element",
            "{p:q}",
        ),
        (
            "not p.",
            ProfileFeature::NegatedHead,
            "default-negated head",
            "not p.",
        ),
        (
            "p:-not 1=1.",
            ProfileFeature::NegatedComparison,
            "default-negated comparison",
            "p:-not 1=1.",
        ),
        (
            "p:-1=1=1.",
            ProfileFeature::ComparisonChain,
            "comparison chain",
            "p:-1=1=1.",
        ),
        (
            "p:-1<2.",
            ProfileFeature::ComparisonRelation,
            "comparison relation",
            "p:-1<2.",
        ),
        (
            "p:-#true.",
            ProfileFeature::BooleanLiteral,
            "Boolean literal",
            "p:-#true.",
        ),
    ] {
        let source_id = SourceId::new(219);
        let source = Source::new(source_id, text.into()).unwrap();
        let error = admit(
            text.into(),
            AdmissionOptions {
                source_id,
                ..AdmissionOptions::default()
            },
        )
        .unwrap_err();
        let AdmissionFailure::Profile {
            feature: actual,
            location,
        } = &error
        else {
            panic!("{text}: {error}");
        };
        assert_eq!(*actual, feature, "{text}");
        assert_eq!(location.source, source_id);
        assert_eq!(source.slice(location.span).unwrap(), highlighted, "{text}");
        let expected = format!("source profile does not admit {description}");
        assert_eq!(error.to_string(), expected);
        assert!(std::error::Error::source(&error).is_none());
        let diagnostics = error.diagnostics();
        let [diagnostic] = diagnostics.as_slice() else {
            panic!("one profile refusal must yield one diagnostic: {diagnostics:?}");
        };
        assert_eq!(diagnostic.severity(), Severity::Error);
        assert_eq!(diagnostic.message(), expected);
        assert_eq!(diagnostic.primary().location, *location);
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
}

#[test]
fn scalar_assignments_require_binding_analysis() {
    for source in ["p(X):-X=1.", "p(X):-X=Y.", "p :- r(_), _=1."] {
        assert!(
            matches!(
                admit(source.to_owned(), AdmissionOptions::default()),
                Err(AdmissionFailure::Profile {
                    feature: ProfileFeature::ScalarBinding,
                    ..
                })
            ),
            "{source}"
        );
    }
}

#[test]
fn anonymous_occurrences_have_distinct_slots() {
    let input = accepted("p(X) :- r(_,X,_).");
    let template = &input.program().templates()[0];
    let terms = template.positive()[0].terms();
    assert_ne!(terms[0], terms[2]);
}

#[test]
fn named_occurrences_share_slots() {
    let input = accepted("p(X) :- r(_,X,_).");
    let template = &input.program().templates()[0];
    let terms = template.positive()[0].terms();
    assert_eq!(template.head().expect("head").terms()[0], terms[1]);
}

#[test]
fn anonymous_head_slots_remain_unsafe() {
    assert!(matches!(
        admit("p(_) :- r(_).".into(), AdmissionOptions::default()),
        Err(AdmissionFailure::Core { .. })
    ));
}

#[test]
fn negated_anonymous_atoms_require_projection() {
    for source in ["p :- not r(_), s(_).", "p :- not not r(_)."] {
        assert!(matches!(
            admit(source.into(), AdmissionOptions::default()),
            Err(AdmissionFailure::Profile {
                feature: ProfileFeature::AnonymousProjection,
                ..
            })
        ));
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
