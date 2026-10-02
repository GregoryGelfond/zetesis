//! Comparison heads are nonbinding truth requirements on each completed row.

use crate::support::finite_bindings::{expected, native};
use zetesis_reference_support::{admit, formula};
use zetesis_themelios::{
    ExpansionFailure, FormulaFailure, FormulaLimits, observation::EvaluationError,
};

#[test]
fn comparison_heads_do_not_bind_source_variables() {
    for source in ["X=1.", "d(1).X=Y:-d(X).", "not X=1.", "X=1:-p(X):d(X)."] {
        assert!(
            matches!(
                admit(source, &FormulaLimits::default()),
                Err(FormulaFailure::UnsafeVariable { .. })
            ),
            "{source}"
        );
    }
}

#[test]
fn comparison_head_failures_keep_source_locations() {
    for (source, expected) in [
        ("d(0).1/X=1:-d(X).", EvaluationError::Undefined),
        ("d(1).X+2147483647=0:-d(X).", EvaluationError::Overflow),
        ("d(0).2<1<1/X:-d(X).", EvaluationError::Undefined),
    ] {
        let Err(failure) = admit(source, &FormulaLimits::default()) else {
            panic!("{source}: expected {expected}");
        };
        assert!(
            matches!(&failure,
                FormulaFailure::Expansion(ExpansionFailure::Evaluation { error, .. })
                    if error == &expected),
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
}

#[test]
fn excluded_body_rows_do_not_reach_comparison_heads() {
    for source in ["d(0).1/X=1:-d(X),X!=0.", "d(0).1/X=1:-d(X),1=2."] {
        assert_eq!(native(&formula(source)), expected(&[&["d(0)"]]), "{source}");
    }
}

#[test]
fn defined_head_rows_preserve_undefined_family_evidence() {
    let source = "d(0;1).1/X=1:-d(X).";
    let admitted = formula(source);
    assert_eq!(admitted.warnings().len(), 1);
    assert_eq!(native(&admitted), expected(&[&["d(0)", "d(1)"]]));
}

#[test]
fn comparison_constraints_retain_authored_origins() {
    let source = "{p(1);p(2)}.X=1:-p(X).";
    let admitted = formula(source);
    assert_eq!(admitted.source().text(), source);
    assert!(admitted.formula_origins().iter().flatten().any(|origin| {
        admitted
            .source()
            .slice(origin.span)
            .is_ok_and(|text| text.contains("X=1:-p(X)"))
    }));
}

#[test]
fn head_truth_does_not_exclude_body_failures() {
    // The head's truth cannot act as an independent false body comparison.
    // Boolean controls establish the same body-validation boundary.
    for head in ["#true", "#false", "X=X", "X!=X", "not X=X", "not not X=X"] {
        let source = format!("d(0).{head}:-d(X),1/X=1.");
        assert!(
            matches!(
                admit(&source, &FormulaLimits::default()),
                Err(FormulaFailure::Expansion(ExpansionFailure::Evaluation {
                    error: EvaluationError::Undefined,
                    ..
                }))
            ),
            "{source}"
        );
    }
}

#[test]
fn head_truth_preserves_nested_body_failures() {
    for head in ["#true", "#false", "1=1", "1!=1"] {
        for body in ["1/X=1:d(X)", "#count{X:d(X),1/X=1}=0"] {
            let source = format!("d(0).{head}:-{body}.");
            assert!(
                matches!(
                    admit(&source, &FormulaLimits::default()),
                    Err(FormulaFailure::Expansion(ExpansionFailure::Evaluation {
                        error: EvaluationError::Undefined,
                        ..
                    }))
                ),
                "{source}"
            );
        }
    }
}

#[test]
fn head_truth_preserves_nested_body_warnings() {
    for head in ["#true", "#false", "1=1", "1!=1"] {
        for body in ["1/X=1:d(X)", "#count{X:d(X),1/X=1}=0"] {
            let source = format!("d(0;1).{head}:-{body}.");
            assert_eq!(formula(&source).warnings().len(), 1, "{source}");
        }
    }
}
