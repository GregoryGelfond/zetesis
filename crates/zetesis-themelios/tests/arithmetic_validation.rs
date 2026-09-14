//! Complete possible-positive bindings validate arithmetic before false filters
//! may remove their instances. Incomplete relational prefixes have no such duty.

#[path = "support/source_records.rs"]
mod source_records;
#[path = "support/source_oracle.rs"]
mod source_oracle;

use std::collections::BTreeSet;
use zetesis_themelios::{
    ExpansionFailure, FormulaFailure, FormulaLimits, observation::EvaluationError,
};

fn refused(source: &str, expected: &EvaluationError) {
    let failure = source_records::admit(source, &FormulaLimits::default()).unwrap_err();
    assert!(
        matches!(&failure, FormulaFailure::Expansion(ExpansionFailure::Evaluation { error, .. }) if error == expected),
        "{source}: {failure}"
    );
    let diagnostics = failure.diagnostics();
    assert!(!diagnostics.is_empty(), "{source}");
    for diagnostic in diagnostics {
        let span = diagnostic.primary().location.span;
        let start = usize::try_from(span.start().get()).unwrap();
        let end = usize::try_from(span.end().get()).unwrap();
        assert!(
            source
                .get(start..end)
                .is_some_and(|text| text.contains('/') || text.contains('+')),
            "{source}: {diagnostic:?}"
        );
    }
}

#[test]
fn false_comparisons_preserve_required_errors() {
    for operation in ["1/X=1", "X+2147483647<0"] {
        for condition in ["1=2", "X!=0"] {
            for body in [
                format!("d(X),{condition},{operation}"),
                format!("d(X),{operation},{condition}"),
            ] {
                let (facts, error) = if operation.starts_with("1/") {
                    ("d(0..2).", EvaluationError::Undefined)
                } else {
                    ("d(1..2).", EvaluationError::Overflow)
                };
                refused(&format!("{facts}p(X):-{body}."), &error);
            }
        }
    }
}

#[test]
fn late_relational_extensions_preserve_pending_errors() {
    for body in ["d(X),1=2,1/X=1,e(X,Y)", "e(X,Y),1/X=1,1=2,d(X)"] {
        refused(
            &format!("d(0).e(0,1).e(2,2).p(X,Y):-{body}."),
            &EvaluationError::Undefined,
        );
    }
}

const EMPTY_EXTENSIONS: &[&str] = &[
    "d(0).e(1,1).e(2,2).p(X,Y):-d(X),1/X=1,e(X,Y).",
    "d(0).e(1,1).e(2,2).p(X,Y):-d(X),1=2,1/X=1,e(X,Y).",
    "d(0).e(1,1).e(2,2).p(X,Y):-e(X,Y),1/X=1,1=2,d(X).",
    "d(0).p(X,Y):-d(X),1/X=1,e(X,Y).",
];

#[test]
fn incomplete_positive_prefixes_do_not_raise_errors() {
    for source in EMPTY_EXTENSIONS {
        let admitted = source_records::admit(source, &FormulaLimits::default()).unwrap();
        let original = source_records::admit(
            source.split("p(X,Y):-").next().unwrap(),
            &FormulaLimits::default(),
        )
        .unwrap();
        assert_eq!(
            source_records::exhaustive(&admitted),
            source_records::exhaustive(&original),
            "{source}"
        );
    }
}

#[test]
fn generated_rows_validate_before_false_filters() {
    for body in ["X=0,1=2,Y=1/X", "Y=1/X,1=2,X=0"] {
        refused(&format!("p(Y):-{body}."), &EvaluationError::Undefined);
    }
}

#[test]
fn constant_expressions_keep_required_validation() {
    for source in [
        "#const z=0. d(0).p(X):-d(X),1=2,1/(X+z)=1.",
        "#const z=0. d(0).p(X):-d(X),1/(X+z)=1,1=2.",
        "p:-1=2,1/0=1.",
        "p:-1/0=1,1=2.",
    ] {
        refused(source, &EvaluationError::Undefined);
    }
}

#[test]
fn objective_comparisons_validate_ignored_rows() {
    for field in ["1", "0", "symbol", "1@symbol"] {
        for body in ["d(X),1=2,1/X=1", "d(X),1/X=1,1=2"] {
            refused(
                &format!("d(0).:~{body}.[{field}]"),
                &EvaluationError::Undefined,
            );
        }
    }
}

const DEFINED: &[&str] = &[
    "d(1..2).p(X):-d(X),1=2,1/X=1.",
    "d(1..2).p(X):-d(X),1/X=1,1=2.",
    "d(1..2).p(X):-d(X),1/X=1.",
];

#[test]
fn defined_filters_preserve_complete_families() {
    for (index, source) in DEFINED.iter().enumerate() {
        let admitted = source_records::admit(source, &FormulaLimits::default()).unwrap();
        let atoms = if index == 2 {
            ["d(1)", "d(2)", "p(1)"].as_slice()
        } else {
            ["d(1)", "d(2)"].as_slice()
        };
        assert_eq!(
            source_records::exhaustive(&admitted),
            BTreeSet::from([(atoms.iter().map(|atom| (*atom).to_owned()).collect(), None)]),
            "{source}"
        );
    }
}

#[test]
#[ignore = "requires independent clingo 5.8.2"]
fn defined_and_empty_join_families_match_clingo() {
    for source in DEFINED.iter().chain(EMPTY_EXTENSIONS) {
        let admitted = source_records::admit(source, &FormulaLimits::default()).unwrap();
        assert_eq!(
            source_records::exhaustive(&admitted),
            source_oracle::records(source),
            "{source}"
        );
    }
}

#[test]
#[ignore = "requires independent clingo 5.8.2"]
fn false_filter_error_policy_differs_from_clingo() {
    for source in [
        "d(0..2).p(X):-d(X),1=2,1/X=1.",
        "d(0..2).p(X):-d(X),1/X=1,1=2.",
    ] {
        refused(source, &EvaluationError::Undefined);
        assert_eq!(
            source_oracle::records(source),
            BTreeSet::from([(["d(0)", "d(1)", "d(2)"].map(str::to_owned).into(), None,)]),
            "{source}"
        );
    }
}

#[test]
#[ignore = "requires independent clingo 5.8.2"]
fn legacy_false_filters_retain_explicit_reference_differences() {
    // These exact legacy sources previously passed native admission because
    // false filters hid required body evaluation. Preserve the full external
    // records while requiring the deliberate native validation failures.
    for (source, error, atoms) in [
        (
            "d(0).p:-d(X),X!=0,1/X>0.",
            EvaluationError::Undefined,
            &["d(0)"][..],
        ),
        (
            "a(0).b(1).p:-a(X),1/X>0,b(Y),Y=2.",
            EvaluationError::Undefined,
            &["a(0)", "b(1)"][..],
        ),
        (
            "a.b.p(N):-N=#sum{2147483647:a;1:b},1=2.",
            EvaluationError::Overflow,
            &["a", "b"][..],
        ),
    ] {
        let failure = source_records::admit(source, &FormulaLimits::default()).unwrap_err();
        assert!(
            matches!(&failure, FormulaFailure::Expansion(ExpansionFailure::Evaluation { error: actual, .. }) if *actual == error),
            "{source}: {failure:?}"
        );
        assert_eq!(
            source_oracle::records(source),
            BTreeSet::from([(atoms.iter().map(|atom| (*atom).to_owned()).collect(), None)]),
            "{source}"
        );
    }
}

#[test]
fn scoped_objective_guards_validate_rejected_rows() {
    for body in ["d(X),1=2,#count{}>1/X", "d(X),#count{}>1/X,1=2"] {
        refused(&format!("d(0).:~{body}.[1]"), &EvaluationError::Undefined);
    }
}

#[test]
fn rule_scopes_validate_rejected_rows() {
    for head in ["p", "{p}", ""] {
        for body in ["d(X),1=2,#count{}>1/X", "d(X),#count{}>1/X,1=2"] {
            refused(
                &format!("d(0).{head}:-{body}."),
                &EvaluationError::Undefined,
            );
        }
    }
}

#[test]
fn rejected_scopes_do_not_add_theory_or_objectives() {
    let original = source_records::admit("d(1).", &FormulaLimits::default()).unwrap();
    for source in [
        "d(1).p:-d(X),1=2,#count{1:a}>1/X.",
        "d(1).:~d(X),1=2,#count{1:a}>1/X.[1]",
    ] {
        let admitted = source_records::admit(source, &FormulaLimits::default()).unwrap();
        assert_eq!(admitted.atoms(), original.atoms(), "{source}");
        assert_eq!(
            admitted.theory().nodes(),
            original.theory().nodes(),
            "{source}"
        );
        assert_eq!(
            admitted.theory().roots(),
            original.theory().roots(),
            "{source}"
        );
        assert_eq!(
            source_records::exhaustive(&admitted),
            source_records::exhaustive(&original),
            "{source}"
        );
        assert!(admitted.objectives().templates().is_empty(), "{source}");
    }
}

#[test]
fn missing_positive_extensions_skip_scoped_validation() {
    for tail in [
        "p(X,Y):-d(X),e(X,Y),1=2,#count{}>1/X.",
        ":~d(X),e(X,Y),1=2,#count{}>1/X.[1]",
    ] {
        let source = format!("d(0).e(1,1).e(2,2).{tail}");
        let admitted = source_records::admit(&source, &FormulaLimits::default()).unwrap();
        let original =
            source_records::admit("d(0).e(1,1).e(2,2).", &FormulaLimits::default()).unwrap();
        assert_eq!(
            source_records::exhaustive(&admitted),
            source_records::exhaustive(&original),
            "{source}"
        );
    }
}

#[test]
fn rule_validation_uses_no_objective_scratch_allowance() {
    let limits = FormulaLimits {
        max_objective_formula_atoms: 0,
        max_objective_formula_nodes: 0,
        ..FormulaLimits::default()
    };
    let source = "d(1).p:-d(X),1=2,#count{1:a}>1/X.";
    assert!(source_records::admit(source, &limits).is_ok());
}

#[test]
fn tuple_shape_mismatch_preserves_required_errors() {
    for comparison in ["(1,1/X)=(1,)", "(1,)=(1,1/X)"] {
        refused(
            &format!("d(0).p(X):-d(X),{comparison}."),
            &EvaluationError::Undefined,
        );
    }
}

const STAGED_HEADS: &[(&str, &str)] = &[
    (
        "d(0;1).{p((X+1)**31):d(X),X=0,not absent}.",
        "d(0;1).{p(1)}.",
    ),
    ("d(0;1).#sum{1:p((X+1)**31):d(X),X=0}>=0.", "d(0;1).{p(1)}."),
    ("d(0;1).not p((X+1)**31):-d(X),X=0.", "d(0;1)."),
    ("d(0).e(1,1).{p(1/X):d(X),e(X,Y)}.", "d(0).e(1,1)."),
    ("d(0;1).{p((X+1)**31):d(X),X=0}.", "d(0;1).{p(1)}."),
    ("d(0..1).p(1/X):-d(X),X!=0.", "d(0..1).p(1)."),
    ("d(0..1).p(1/X)|q(1/X):-d(X),X!=0.", "d(0..1).p(1)|q(1)."),
    ("d(1).p(1/Y):-d(X),Y=X-1,1=2.", "d(1)."),
    ("q(1).p(X+1):-X=41,1=#count{Y:q(Y)}.", "q(1).p(42)."),
    ("q(1).p(X+1):-X=41,q(Y):q(Y).", "q(1).p(42)."),
];

#[test]
fn head_values_follow_scalar_body_selection() {
    for (source, expanded) in STAGED_HEADS {
        let admitted = source_records::admit(source, &FormulaLimits::default()).unwrap();
        let expanded = source_records::admit(expanded, &FormulaLimits::default()).unwrap();
        assert_eq!(
            source_records::exhaustive(&admitted),
            source_records::exhaustive(&expanded),
            "{source}"
        );
    }
}

#[test]
fn body_values_keep_their_validation_scope() {
    for source in [
        "d(0).p:-d(X),1=2,not q(1/X).",
        "d(0).p:-d(X),not q(1/X),1=2.",
        "d(0).p(1/X):-d(X),1=2,#count{}=1/X.",
        "d(0).p(1/X):-d(X),not d(X).",
        "d(0).p(1/X):-d(X),#count{}=1.",
        "d(0).{p(1/X):d(X),not d(X)}.",
        "d(0).{p(1/X)}:-d(X),#count{}=1.",
        "d(0).{p(1):d(X),1=2,not q(1/X)}.",
        "d(0).#sum{W:p(1):d(X),W=1/X,1=2}>=0.",
    ] {
        refused(source, &EvaluationError::Undefined);
    }
}

#[test]
#[ignore = "requires independent clingo 5.8.2"]
fn staged_head_families_match_clingo() {
    for (source, _) in STAGED_HEADS {
        let admitted = source_records::admit(source, &FormulaLimits::default()).unwrap();
        assert_eq!(
            source_records::exhaustive(&admitted),
            source_oracle::records(source),
            "{source}"
        );
    }
}

#[test]
fn head_generation_charges_only_selected_rows() {
    let limits = FormulaLimits {
        max_generated_values: 0,
        ..FormulaLimits::default()
    };
    for source in ["d(0).p(X+1):-d(X),1=2.", "d(0).{p(X+1):d(X),1=2}."] {
        let admitted = source_records::admit(source, &limits).unwrap();
        assert_eq!(
            source_records::exhaustive(&admitted),
            BTreeSet::from([(["d(0)".to_owned()].into(), None)]),
            "{source}"
        );
    }
    for source in [
        "d(0).p(X+1):-d(X).",
        "d(0).p(X+1):-d(X),not d(X).",
        "d(0).{p(X+1):d(X)}.",
    ] {
        assert!(
            matches!(
                source_records::admit(source, &limits),
                Err(FormulaFailure::Limit {
                    resource: zetesis_themelios::FormulaResource::GeneratedValues,
                    limit: 0,
                    observed: 1,
                    ..
                })
            ),
            "{source}"
        );
    }
}
