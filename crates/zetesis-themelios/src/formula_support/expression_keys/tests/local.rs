use super::*;
use crate::test_support::atoms_named;
use zetesis_test_support::programs::numbered;

fn same_native_payload(left: &Compiled, right: &Compiled) {
    assert_same_program(left, right);
    let left = left.theory.parts();
    let right = right.theory.parts();
    assert_eq!(left.conditions(), right.conditions());
    assert_eq!(left.numeric_values(), right.numeric_values());
    assert_eq!(left.rows(), right.rows());
    assert_eq!(left.thresholds(), right.thresholds());
    assert_eq!(left.extremum_cuts(), right.extremum_cuts());
}

fn comparison(source: &str) -> (GroundingWork, GroundingWork) {
    let before = Receipts::default();
    let reference = controlled(true, || {
        ground(testing::prepare(source), Some(&before), None)
    })
    .unwrap();
    let after = Receipts::default();
    let selected = ground(testing::prepare(source), Some(&after), None).unwrap();
    same_native_payload(&reference, &selected);
    (
        before.emission.get().checked_sum(before.other.get()),
        after.emission.get().checked_sum(after.other.get()),
    )
}

#[test]
fn aggregate_ranges_keep_source_order_and_changing_bounds() {
    let source = include_str!("../../../../tests/fixtures/expression-keys/local-count.lp");
    let (before, after) = comparison(source);
    assert_eq!(before.range_probes, Some(0));
    assert!(after.range_probes.unwrap() > 0);
    assert!(after.numeric_index_builds.unwrap() > 0);
    assert!(after.join_rows.unwrap() < before.join_rows.unwrap());
}

#[test]
fn local_ranges_keep_signed_and_extremum_tuple_meanings() {
    for function in ["#count", "#sum", "#sum+", "#min", "#max"] {
        let source = format!(
            "b(0;2;4). {{p(3,-2;1,5;2,0)}}. \
             out(B,N) :- b(B), N={function}{{W,X:p(X,W),X<B}}."
        );
        let (before, after) = comparison(&source);
        assert_eq!(before.range_probes, Some(0), "{function}");
        assert!(after.range_probes.unwrap() > 0, "{function}");
        assert!(
            after.join_rows.unwrap() < before.join_rows.unwrap(),
            "{function}"
        );
    }
}

#[test]
fn local_ranges_keep_repeated_column_checks() {
    let source = "b(2;4). {p(3,3;1,1;2,0)}. out(B,N) :- b(B), N=#count{X:p(X,X),X<B}.";
    let (before, after) = comparison(source);
    assert_eq!(before.range_probes, Some(0));
    assert!(after.range_probes.unwrap() > 0);
}

#[test]
fn mixed_local_columns_decline_numeric_selection() {
    let source = "b(2). {p(1;a;3)}. out(B,N) :- b(B), N=#count{X:p(X),X<B}.";
    let (_, after) = comparison(source);
    assert!(after.numeric_index_declines.unwrap() > 0);
    assert_eq!(after.range_probes, Some(0));
}

#[test]
fn unsupported_local_scopes_keep_their_original_traversal() {
    for source in [
        "b(2). {p(1;3)}. {q(1)}. out(B,N) :- b(B), N=#count{X:p(X),X<B,not q(X)}.",
        "b(2). {p(f(1);f(3))}. out(B,N) :- b(B), N=#count{X:p(f(X)),X<B}.",
        "b(2). {p(1;3)}. out(B,N) :- b(B), N=#count{X:p(X),X*X<B}.",
    ] {
        let (_, after) = comparison(source);
        assert_eq!(after.range_probes, Some(0), "{source}");
    }
}

#[test]
fn local_arithmetic_failures_keep_their_error_and_site() {
    use crate::observation::EvaluationError;
    let source = "b(2). p(1). out(B,N) :- b(B), N=#count{X:p(X),X<B,(X+2147483647)/1 > 0}.";
    let before = controlled(true, || ground(testing::prepare(source), None, None)).unwrap_err();
    let receipts = Receipts::default();
    let after = ground(testing::prepare(source), Some(&receipts), None).unwrap_err();
    match (before, after) {
        (
            FormulaFailure::Expansion(crate::ExpansionFailure::Evaluation {
                error: before,
                location: before_site,
            }),
            FormulaFailure::Expansion(crate::ExpansionFailure::Evaluation {
                error: after,
                location: after_site,
            }),
        ) => {
            assert_eq!(before, EvaluationError::Overflow);
            assert_eq!(after, EvaluationError::Overflow);
            assert_eq!(before_site, after_site);
        }
        other => panic!("matching local arithmetic failures expected: {other:?}"),
    }
    assert_eq!(receipts.emission.get().range_probes, Some(0));
    assert_eq!(receipts.other.get().range_probes, Some(0));
}

#[test]
fn local_undefined_family_is_omitted_with_its_warning() {
    // The element's one instance divides by zero and is omitted, so the count
    // is over no tuple: `out(2,0)` and no `out(2,1)`.
    let source = "b(2). p(0). out(B,N) :- b(B), N=#count{X:p(X),X<B,1/X > 0}.";
    let reference = controlled(true, || ground(testing::prepare(source), None, None)).unwrap();
    assert_eq!(
        zero_divisor_statements(source, &reference.warnings),
        ["out(B,N) :- b(B), N=#count{X:p(X),X<B,1/X > 0}."]
    );
    let receipts = Receipts::default();
    let selected = ground(testing::prepare(source), Some(&receipts), None).unwrap();
    same_native_payload(&reference, &selected);
    assert_eq!(atoms_named(&selected, "out"), [numbered("out", &[2, 0])]);
    assert_eq!(
        answer_sets(&selected.theory, &selected.atoms),
        atom_sets([&[unary("b", 2), unary("p", 0), numbered("out", &[2, 0])]])
    );
    assert_eq!(receipts.emission.get().range_probes, Some(0));
    assert_eq!(receipts.other.get().range_probes, Some(0));
}

#[test]
fn local_defined_sibling_preserves_the_zero_divisor_warning() {
    let source = "b(2). p(0;1). out(B,N) :- b(B), N=#count{X:p(X),X<B,1/X>0}.";
    let reference = controlled(true, || ground(testing::prepare(source), None, None)).unwrap();
    assert!(matches!(
        reference.warnings.as_slice(),
        [crate::FormulaWarning::ZeroDivisor { .. }]
    ));
    let receipts = Receipts::default();
    let selected = ground(testing::prepare(source), Some(&receipts), None).unwrap();
    same_native_payload(&reference, &selected);
    assert_eq!(receipts.emission.get().range_probes, Some(0));
    assert_eq!(receipts.other.get().range_probes, Some(0));
}
