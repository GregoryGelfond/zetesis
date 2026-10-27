//! Source formula admission preserves typed values, scopes and refusal evidence.

use crate::support::admission;
use crate::support::atom_models::{self, Models, answer_sets};
use themelios_base::source::SourceId;
use zetesis_core::{Value, ValueLimits};
use zetesis_reference_support::resource_limits;
use zetesis_themelios::{
    AdmissionFailure, AdmissionOptions, AdmittedFormula, ExpansionFailure, ExpansionLimits,
    FormulaFailure, FormulaLimits, InputLimit, ProfileFeature, admit_formula,
};

fn admit(text: String, options: AdmissionOptions) -> Result<AdmittedFormula, FormulaFailure> {
    admit_formula(
        text,
        options,
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
}

fn accepted(text: &str) -> AdmittedFormula {
    admit(text.into(), AdmissionOptions::default()).unwrap()
}

fn expected(models: &[&[&str]]) -> Models {
    models
        .iter()
        .map(|atoms| atoms.iter().map(|atom| atom_models::atom(atom)).collect())
        .collect()
}

#[test]
fn positive_recursion_does_not_supply_its_own_support() {
    assert_eq!(answer_sets(&accepted("p :- q. q :- p.")), expected(&[&[]]));
    assert_eq!(
        answer_sets(&accepted("p :- q. q :- p. {p}.")),
        expected(&[&[], &["p", "q"]])
    );
}

#[test]
fn a_choice_supplies_both_frozen_gate_interpretations() {
    assert_eq!(answer_sets(&accepted("{p}.")), expected(&[&[], &["p"]]));
}

#[test]
fn default_and_double_default_negation_keep_opposite_polarities() {
    assert_eq!(
        answer_sets(&accepted("p :- not q, not not r.")),
        expected(&[&[]])
    );
    assert_eq!(
        answer_sets(&accepted("{q}. {r}. p :- not q, not not r.")),
        expected(&[&[], &["q"], &["p", "r"], &["q", "r"]])
    );
}

#[test]
fn constraints_preserve_frozen_gates_and_scalar_filters() {
    assert_eq!(
        answer_sets(&accepted(":- not p, not not q, 1 != 2.")),
        expected(&[&[]])
    );
    assert_eq!(
        answer_sets(&accepted("{p}. {q}. :- not p, not not q, 1 != 2.")),
        expected(&[&[], &["p"], &["p", "q"]])
    );
}

#[test]
fn scalar_values_keep_their_types_and_decode_strings() {
    let input = accepted("p(a, \"a\", 12, -7, \"a\\\"b\").");
    let atom = input
        .atoms()
        .at(0)
        .unwrap()
        .to_atom(ValueLimits::default())
        .unwrap();
    assert_eq!(
        atom.values(),
        [
            Value::Symbol("a".into()),
            Value::String("a".into()),
            Value::Number(12),
            Value::Number(-7),
            Value::String("a\"b".into())
        ]
    );
}

#[test]
fn numeral_boundaries_follow_the_actual_raiser() {
    accepted("p(2147483647, -2147483647).");
    for source in ["p(-2147483648).", "p(2147483648)."] {
        assert!(matches!(
            admit(source.into(), AdmissionOptions::default()),
            Err(FormulaFailure::Expansion(ExpansionFailure::Admission(
                AdmissionFailure::Raise(_)
            )))
        ));
    }
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
                admit(text.into(), AdmissionOptions::default()),
                Err(FormulaFailure::Expansion(ExpansionFailure::Admission(
                    AdmissionFailure::Profile {
                        feature: ProfileFeature::ProgramPart,
                        ..
                    }
                )))
            ),
            "{text}"
        );
    }
    accepted("p(\"#program step.\").");
}

#[test]
fn empty_body_scalar_filters_do_not_need_variable_bindings() {
    assert_eq!(
        answer_sets(&accepted("p :- 1=1. :- 1!=1.")),
        expected(&[&["p"]])
    );
}

#[test]
fn anonymous_occurrences_have_distinct_bindings() {
    accepted("p(X) :- r(_,X,_).");
    assert_eq!(
        answer_sets(&accepted("r(1,2,3). p(X) :- r(_,X,_).")),
        expected(&[&["r(1,2,3)", "p(2)"]])
    );
}

#[test]
fn named_occurrences_share_bindings() {
    accepted("p(X) :- r(_,X,_).");
    assert_eq!(
        answer_sets(&accepted("r(1,2,3). r(4,5,4). p(X) :- r(_,X,_).")),
        expected(&[&["r(1,2,3)", "r(4,5,4)", "p(2)", "p(5)"]])
    );
}

#[test]
fn anonymous_head_slots_remain_unsafe() {
    assert!(matches!(
        admit("p(_) :- r(_).".into(), AdmissionOptions::default()),
        Err(FormulaFailure::UnsafeVariable { .. })
    ));
}

#[test]
fn byte_limits_are_checked_before_syntax() {
    let options = resource_limits::admission(InputLimit::SourceBytes, 2);
    assert!(matches!(
        admit("invalid input".into(), options),
        Err(FormulaFailure::Expansion(ExpansionFailure::Admission(
            AdmissionFailure::Limit {
                resource: InputLimit::SourceBytes,
                ..
            }
        )))
    ));
    accepted("");
}

#[test]
fn body_limits_count_source_occurrences_before_deduplication() {
    let options = resource_limits::admission(InputLimit::BodyElements, 1);
    assert!(matches!(
        admit("p :- q,q.".into(), options),
        Err(FormulaFailure::Expansion(ExpansionFailure::Admission(
            AdmissionFailure::Limit {
                resource: InputLimit::BodyElements,
                ..
            }
        )))
    ));
}

#[test]
fn syntax_node_and_depth_limits_refuse_with_locations() {
    for (expected, limit) in [(InputLimit::SyntaxNodes, 0), (InputLimit::SyntaxDepth, 1)] {
        let options = resource_limits::admission(expected, limit);
        match admit("p(a).".into(), options) {
            Err(FormulaFailure::Expansion(ExpansionFailure::Admission(
                AdmissionFailure::Limit {
                    resource, location, ..
                },
            ))) => {
                assert_eq!(resource, expected);
                assert_eq!(location.location().unwrap().source, options.source_id);
            }
            other => panic!("expected traversal refusal: {other:?}"),
        }
    }
}

#[test]
fn syntax_refusal_preserves_dependency_diagnostics_and_source_identity() {
    let options = admission::options(SourceId::new(42));
    let error = admit("p(.".into(), options).expect_err("broken source");
    assert!(matches!(
        error,
        FormulaFailure::Expansion(ExpansionFailure::Admission(AdmissionFailure::Syntax(_)))
    ));
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
    let (input, roots) = crate::support::root_emission::source("p(a). p(a).");
    assert_eq!(input.atoms().len(), 1);
    let authored = roots.indices(zetesis_themelios::GroundingPhase::RuleInstantiation);
    assert_eq!(authored.len(), 1);
    assert_eq!(input.formula_origins()[authored[0]].len(), 2);
    assert_eq!(input.source().unwrap().text(), "p(a). p(a).");
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
        let input = accepted(source);
        assert_eq!(input.atoms().len(), 1);
        let atom = input.atoms().at(0).unwrap();
        assert_eq!(atom.values().len(), 1);
        let value = atom.values().at(0).unwrap();
        let actual = match value.descriptor() {
            zetesis_core::ValueNodeRef::Function { .. }
            | zetesis_core::ValueNodeRef::Tuple { .. } => value.to_string(),
            zetesis_core::ValueNodeRef::Symbol(value) => value.to_owned(),
            other => panic!("unexpected {other:?}"),
        };
        assert_eq!(actual, spelling);
    }
}

#[test]
fn formula_admission_reports_the_expansion_it_used() {
    let limits = ExpansionLimits::default();
    let admitted = admit_formula(
        "p(1..3). q(X) :- p(X).".into(),
        AdmissionOptions::default(),
        limits,
        FormulaLimits::default(),
    )
    .unwrap();
    let usage = admitted.expansion_usage();
    assert!(usage.term_work > 0);
    assert!(usage.term_work <= limits.max_term_work);
    assert!(usage.templates >= 4);
    assert!(usage.templates <= limits.max_templates);
    assert!(usage.values >= 3);
    assert!(usage.origin_locations >= 4);
}

#[test]
fn source_external_calls_and_unresolved_includes_remain_refused() {
    for text in ["p(@f()).", r#"#include "missing.lp"."#] {
        assert!(
            admit(text.into(), AdmissionOptions::default()).is_err(),
            "{text}"
        );
    }
}

#[test]
fn an_equality_of_unbound_variables_remains_unsafe() {
    assert!(matches!(
        admit("p(X):-X=Y.".into(), AdmissionOptions::default()),
        Err(FormulaFailure::UnsafeVariable { .. })
    ));
}
