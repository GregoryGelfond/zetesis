//! Comparison heads are nonbinding truth requirements on each completed row.

use crate::support::finite_bindings::{expected, native};
use zetesis_reference_support::{admit, formula};
use zetesis_themelios::{
    AdmittedFormula, ExpansionFailure, FormulaFailure, FormulaLimits, FormulaWarning,
    observation::EvaluationError,
};

/// The statement of `admitted`'s one warning, which reports a zero divisor
/// omitted from an instance of that statement.
fn omitted_statement(admitted: &AdmittedFormula) -> &str {
    let [FormulaWarning::ZeroDivisor { location }] = admitted.warnings() else {
        panic!("one zero-divisor warning: {:?}", admitted.warnings());
    };
    admitted
        .source()
        .expect("source input")
        .slice(location.location().expect("parsed source").span)
        .unwrap()
}

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
fn a_comparison_head_overflow_keeps_its_source_location() {
    let source = "d(1).X+2147483647=0:-d(X).";
    let expected = EvaluationError::Overflow;
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

#[test]
fn a_comparison_head_zero_divisor_warns_at_its_rule() {
    // The head is undefined at X = 0: that instance is omitted, not read as
    // a violated requirement, and the rule keeps the warning.
    let admitted = formula("d(0).1/X=1:-d(X).");
    assert_eq!(omitted_statement(&admitted), "1/X=1:-d(X).");
    assert_eq!(native(&admitted), expected(&[&["d(0)"]]));
}

#[test]
fn a_false_link_decides_a_chain_head_beside_an_undefined_one() {
    // The link 2<1 is false whatever X is, so the chain is false at X = 0
    // although the link 1<1/X divides by zero there: the head is a violated
    // requirement and no answer set remains. Nothing is omitted, so no
    // warning is kept.
    let admitted = formula("d(0).2<1<1/X:-d(X).");
    assert!(admitted.warnings().is_empty());
    assert_eq!(native(&admitted), expected(&[]));
}

#[test]
fn a_chain_head_with_no_false_link_is_omitted_where_a_link_is_undefined() {
    // The link 0<1 holds, so nothing decides the chain where 1<1/X divides
    // by zero: the instance is omitted with the rule's warning.
    let admitted = formula("d(0).0<1<1/X:-d(X).");
    assert_eq!(omitted_statement(&admitted), "0<1<1/X:-d(X).");
    assert_eq!(native(&admitted), expected(&[&["d(0)"]]));
}

#[test]
fn excluded_body_rows_do_not_reach_comparison_heads() {
    for source in ["d(0).1/X=1:-d(X),X!=0.", "d(0).1/X=1:-d(X),1=2."] {
        let admitted = formula(source);
        // A head that was reached would divide by zero and leave a warning.
        assert!(admitted.warnings().is_empty(), "{source}");
        assert_eq!(native(&admitted), expected(&[&["d(0)"]]), "{source}");
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
    assert_eq!(admitted.source().expect("source input").text(), source);
    assert!(admitted.formula_origins().iter().flatten().any(|origin| {
        admitted
            .source()
            .expect("source input")
            .slice(origin.location().expect("parsed source").span)
            .is_ok_and(|text| text.contains("X=1:-p(X)"))
    }));
}

#[test]
fn head_truth_does_not_hide_body_zero_divisors() {
    // The head's truth cannot act as an independent false body comparison:
    // whatever it is, the body's division is reached, the instance is
    // omitted and the rule keeps the warning.
    // Boolean controls establish the same body-validation boundary.
    for head in ["#true", "#false", "X=X", "X!=X", "not X=X", "not not X=X"] {
        let rule = format!("{head}:-d(X),1/X=1.");
        let admitted = formula(&format!("d(0).{rule}"));
        assert_eq!(omitted_statement(&admitted), rule);
        assert_eq!(native(&admitted), expected(&[&["d(0)"]]), "{rule}");
    }
}

#[test]
fn head_truth_does_not_hide_nested_aggregate_zero_divisors() {
    // The count's one element divides by zero and is omitted, leaving the
    // empty count 0: the body holds, so a false head leaves no answer set.
    // The rule keeps the warning whatever its head.
    for (head, holds) in [
        ("#true", true),
        ("#false", false),
        ("1=1", true),
        ("1!=1", false),
    ] {
        let rule = format!("{head}:-#count{{X:d(X),1/X=1}}=0.");
        let admitted = formula(&format!("d(0).{rule}"));
        assert_eq!(omitted_statement(&admitted), rule);
        let models = if holds {
            expected(&[&["d(0)"]])
        } else {
            expected(&[])
        };
        assert_eq!(native(&admitted), models, "{rule}");
    }
}

#[test]
fn head_truth_does_not_hide_nested_consequent_zero_divisors() {
    // The consequent divides by zero under its condition d(0), so it is
    // false there and the body fails: the answer set is the same whatever
    // the head. The rule keeps the warning.
    for head in ["#true", "#false", "1=1", "1!=1"] {
        let rule = format!("{head}:-1/X=1:d(X).");
        let admitted = formula(&format!("d(0).{rule}"));
        assert_eq!(omitted_statement(&admitted), rule);
        assert_eq!(native(&admitted), expected(&[&["d(0)"]]), "{rule}");
    }
}

#[test]
fn head_truth_preserves_nested_body_warnings() {
    // Neither body holds: the consequent is false under d(0), where it
    // divides by zero, and the count is 1, from d(1). So the answer set is
    // the facts whatever the head, and the rule keeps its warning.
    for head in ["#true", "#false", "1=1", "1!=1"] {
        for body in ["1/X=1:d(X)", "#count{X:d(X),1/X=1}=0"] {
            let rule = format!("{head}:-{body}.");
            let admitted = formula(&format!("d(0;1).{rule}"));
            assert_eq!(omitted_statement(&admitted), rule);
            assert_eq!(native(&admitted), expected(&[&["d(0)", "d(1)"]]), "{rule}");
        }
    }
}
