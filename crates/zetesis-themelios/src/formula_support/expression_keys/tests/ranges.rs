use super::*;

#[test]
fn range_probes_keep_exact_theory_and_selected_row_order() {
    for relation in ["<", "<=", ">", ">="] {
        let source = format!("{{p(3;1;2)}}. {{q(2)}}. :- q(Y),p(X),X {relation} Y+0.");
        let reference = controlled(true, || ground(testing::prepare(&source), None, None)).unwrap();
        let receipts = Receipts::default();
        let selected = ground(testing::prepare(&source), Some(&receipts), None).unwrap();
        assert_same_program(&reference, &selected);
        let work = receipts.emission.get();
        assert_eq!(work.range_probes, Some(1), "{relation}");
        assert_eq!(
            work.range_probe_rows,
            Some(if relation.len() == 1 { 1 } else { 2 }),
            "{relation}"
        );
        assert_eq!(work.numeric_index_builds, Some(1));
        assert_eq!(work.expression_key_probes, Some(0));
    }
}

#[test]
fn ranges_preserve_excluded_and_retained_independent_fatal_errors() {
    // The true q-only comparison gives q the same readiness as p's overflow
    // check; q's smaller population then puts it first. Keep the literal /1:
    // a variable divisor would classify a retained fatal row during family
    // evidence, before the constraint's emission selector can run.
    for head in [":-", "h(X,Y) :-"] {
        let excluded =
            format!("{{p(0;1)}}. {{q(0)}}. {head} q(Y),Y>=0,p(X),X<Y+1,(X+2147483647)/1>0.");
        let before = controlled(true, || ground(testing::prepare(&excluded), None, None)).unwrap();
        let receipts = Receipts::default();
        let after = ground(testing::prepare(&excluded), Some(&receipts), None).unwrap();
        assert_same_program(&before, &after);
        if head == ":-" {
            let work = receipts.emission.get();
            assert_eq!(
                work.range_probes,
                Some(2),
                "q guard and p selection both run"
            );
            assert_eq!(work.numeric_index_builds, Some(2));
            assert_eq!(work.range_probe_rows, Some(2));
        } else {
            assert!(receipts.other.get().range_probes.unwrap() >= 2);
        }
        let retained =
            format!("{{p(0;1)}}. {{q(0)}}. {head} q(Y),Y>=0,p(X),X>Y,(X+2147483647)/1>0.");
        let before =
            controlled(true, || ground(testing::prepare(&retained), None, None)).unwrap_err();
        let receipts = Receipts::default();
        let after = ground(testing::prepare(&retained), Some(&receipts), None).unwrap_err();
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
                assert_eq!(before, themelios_program::term::EvalError::Overflow);
                assert_eq!(before, after);
                assert_eq!(before_site, after_site);
            }
            other => panic!("matching retained scalar errors expected: {other:?}"),
        }
        if head == ":-" {
            let work = receipts.emission.get();
            assert_eq!(
                work.range_probes,
                Some(2),
                "q guard and p selection both run"
            );
            assert_eq!(work.numeric_index_builds, Some(2));
            assert_eq!(work.range_probe_rows, Some(2));
        } else {
            assert!(receipts.other.get().range_probes.unwrap() >= 2);
        }
    }
}

#[test]
fn hybrid_capture_does_not_select_ranges() {
    let source = "{p(1;2;3)}. {q(2)}. :- q(Y),p(X),X<Y+0.";
    let reference = controlled(true, || ground_hybrid(testing::prepare(source), None)).unwrap();
    let receipts = Receipts::default();
    let selected = ground_hybrid(testing::prepare(source), Some(&receipts)).unwrap();
    assert!(selected.1.is_some());
    assert_same_program(&reference.0, &selected.0);
    assert_eq!(receipts.emission.get().range_probes, Some(0));
    assert_eq!(receipts.emission.get().range_probe_rows, Some(0));
}

#[test]
fn mixed_column_ordering_keeps_asp_term_order() {
    let source = "{p(1;a)}. {q(0)}. :- q(Y),p(X),X>Y+0.";
    let before = controlled(true, || ground(testing::prepare(source), None, None)).unwrap();
    let receipts = Receipts::default();
    let after = ground(testing::prepare(source), Some(&receipts), None).unwrap();
    assert_same_program(&before, &after);
    assert_eq!(receipts.emission.get().numeric_index_declines, Some(1));
    assert_eq!(receipts.emission.get().range_probes, Some(0));
}
