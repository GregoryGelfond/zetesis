//! A comparison over the relationally bound variables that is defined and
//! false excludes the substitution, and nothing in an excluded substitution is
//! reached. Every other substitution validates its arithmetic in full: a
//! reached zero divisor is admitted only alongside a jointly defined family witness;
//! other reached arithmetic failures refuse the program. Incomplete
//! relational prefixes have no such duty.

use std::collections::BTreeSet;
use zetesis_clingo_support as oracle;
use zetesis_reference_support as reference;
use zetesis_themelios::{
    ExpansionFailure, FormulaFailure, FormulaLimits, observation::EvaluationError,
};

fn refused(source: &str, expected: &EvaluationError) {
    let Err(failure) = reference::admit(source, &FormulaLimits::default()) else {
        panic!("{source}: admission succeeded; expected {expected}");
    };
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

fn admitted(source: &str) -> BTreeSet<(BTreeSet<String>, Option<Vec<i64>>)> {
    let admitted = reference::admit(source, &FormulaLimits::default()).unwrap();
    reference::exhaustive(&admitted)
}

fn family(atoms: &[&str]) -> BTreeSet<(BTreeSet<String>, Option<Vec<i64>>)> {
    BTreeSet::from([(atoms.iter().map(|atom| (*atom).to_owned()).collect(), None)])
}

#[test]
fn excluded_substitutions_are_not_reached() {
    // X != 0 excludes X = 0, so 1/0 is never evaluated, whichever side of it
    // the comparison stands; 1 = 2 excludes every substitution.
    for body in [
        "d(X),X!=0,1/X=1",
        "d(X),1/X=1,X!=0",
        "d(X),1=2,1/X=1",
        "d(X),1/X=1,1=2",
    ] {
        let source = format!("d(0..2).p(X):-{body}.");
        let expected = if body.contains("X!=0") {
            family(&["d(0)", "d(1)", "d(2)", "p(1)"])
        } else {
            family(&["d(0)", "d(1)", "d(2)"])
        };
        assert_eq!(admitted(&source), expected, "{source}");
    }
    for body in ["d(X),1=2,X+2147483647<0", "d(X),X+2147483647<0,1=2"] {
        let source = format!("d(1..2).p(X):-{body}.");
        assert_eq!(admitted(&source), family(&["d(1)", "d(2)"]), "{source}");
    }
}

#[test]
fn reached_operations_refuse() {
    // Nothing excludes X = 0 here, so the division is reached; X != 0 over
    // d(1..2) excludes nothing, so the overflow is reached.
    refused("d(0).p(X):-d(X),1/X=1.", &EvaluationError::Undefined);
    refused(
        "d(1..2).p(X):-d(X),X!=0,X+2147483647<0.",
        &EvaluationError::Overflow,
    );
}

#[test]
fn body_generators_preserve_relational_comparison_failures() {
    // A body generator consumes an already complete relational binding;
    // it cannot discharge the division that binding reaches.
    for generator in ["Y=X+1", "Y=X..X+1"] {
        refused(
            &format!("d(0).p(X,Y):-d(X),1/X=1,{generator}."),
            &EvaluationError::Undefined,
        );
    }
}

#[test]
fn undefined_substitutions_do_not_discard_defined_ones() {
    let source = "d(0..3). p(X,Y) :- d(X), 1/X=1, Y=X+1.";
    assert_eq!(
        admitted(source),
        family(&["d(0)", "d(1)", "d(2)", "d(3)", "p(1,2)"]),
    );
}

#[test]
fn a_defined_false_substitution_keeps_the_rule_admissible() {
    let source = "d(0;2). p(X,Y) :- d(X), 1/X=1, Y=X+1.";
    assert_eq!(admitted(source), family(&["d(0)", "d(2)"]));
}

#[test]
fn an_exclusion_at_any_depth_hides_a_failure_at_another() {
    // 1/X fails for X = 0 once d(X) is bound; X + Y != Y is false for the
    // same X only once e(Y) is bound, deeper in the join. The substitution is
    // excluded, not refused, and the verdict does not depend on which is met
    // first.
    for body in ["d(X),e(Y),1/X=1,X+Y!=Y", "d(X),e(Y),X+Y!=Y,1/X=1"] {
        let source = format!("d(0..1).e(1..2).p(X,Y):-{body}.");
        assert_eq!(
            admitted(&source),
            family(&["d(0)", "d(1)", "e(1)", "e(2)", "p(1,1)", "p(1,2)"]),
            "{source}"
        );
    }
    // A comparison that excludes nothing leaves the failure a refusal.
    refused(
        "d(0).e(1..2).p(X,Y):-d(X),e(Y),1/X=1,X!=3.",
        &EvaluationError::Undefined,
    );
    // The same for a failure met before the relation that decides the exclusion.
    for body in ["d(X),1=2,1/X=1,e(X,Y)", "e(X,Y),1/X=1,1=2,d(X)"] {
        let source = format!("d(0).e(0,1).e(2,2).p(X,Y):-{body}.");
        assert_eq!(
            admitted(&source),
            family(&["d(0)", "e(0,1)", "e(2,2)"]),
            "{source}"
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
        let admitted = reference::admit(source, &FormulaLimits::default()).unwrap();
        let original = reference::admit(
            source.split("p(X,Y):-").next().unwrap(),
            &FormulaLimits::default(),
        )
        .unwrap();
        assert_eq!(
            reference::exhaustive(&admitted),
            reference::exhaustive(&original),
            "{source}"
        );
    }
}

#[test]
fn an_excluded_substitution_generates_nothing() {
    // 1 = 2 excludes the one substitution before any binder runs.
    for body in ["X=0,1=2,Y=1/X", "Y=1/X,1=2,X=0"] {
        let source = format!("p(Y):-{body}.");
        assert_eq!(admitted(&source), family(&[]), "{source}");
    }
    refused("p(Y):-X=0,Y=1/X.", &EvaluationError::Undefined);
}

#[test]
fn constant_comparisons_exclude_or_reach() {
    for source in [
        "#const z=0. d(0).p(X):-d(X),1=2,1/(X+z)=1.",
        "#const z=0. d(0).p(X):-d(X),1/(X+z)=1,1=2.",
    ] {
        assert_eq!(admitted(source), family(&["d(0)"]), "{source}");
    }
    // A closed undefined term is refused as written, during source
    // preparation, before any substitution exists to exclude.
    for source in ["p:-1=2,1/0=1.", "p:-1/0=1,1=2.", "p:-1/0=1."] {
        refused(source, &EvaluationError::Undefined);
    }
    refused(
        "#const z=0. d(0).p(X):-d(X),1/(X+z)=1.",
        &EvaluationError::Undefined,
    );
}

#[test]
fn objective_substitutions_follow_the_same_rule() {
    for field in ["1", "0", "symbol", "1@symbol"] {
        for body in ["d(X),1=2,1/X=1", "d(X),1/X=1,1=2"] {
            let source = format!("d(0).:~{body}.[{field}]");
            assert_eq!(admitted(&source), family(&["d(0)"]), "{source}");
        }
        refused(
            &format!("d(0).:~d(X),1/X=1.[{field}]"),
            &EvaluationError::Undefined,
        );
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
        let admitted = reference::admit(source, &FormulaLimits::default()).unwrap();
        let atoms = if index == 2 {
            ["d(1)", "d(2)", "p(1)"].as_slice()
        } else {
            ["d(1)", "d(2)"].as_slice()
        };
        assert_eq!(
            reference::exhaustive(&admitted),
            BTreeSet::from([(atoms.iter().map(|atom| (*atom).to_owned()).collect(), None)]),
            "{source}"
        );
    }
}

#[test]
#[ignore = "requires independent clingo 5.8.2"]
fn defined_and_empty_join_families_match_clingo() {
    for source in DEFINED.iter().chain(EMPTY_EXTENSIONS) {
        let admitted = reference::admit(source, &FormulaLimits::default()).unwrap();
        assert_eq!(
            reference::exhaustive(&admitted),
            oracle::records(source),
            "{source}"
        );
    }
}

#[test]
#[ignore = "requires independent clingo 5.8.2"]
fn all_undefined_families_differ_from_clingo() {
    // clingo drops the only undefined instance; the library refuses a
    // nonempty source family lacking any jointly defined substitution.
    let source = "d(0).p(X):-d(X),1/X=1.";
    refused(source, &EvaluationError::Undefined);
    assert_eq!(oracle::records(source), family(&["d(0)"]));
}

#[test]
#[ignore = "requires independent clingo 5.8.2"]
fn excluded_substitutions_match_clingo() {
    // A comparison that is defined and false excludes its substitution, so
    // an operation to its side is not reached, as in the reference.
    for source in [
        "d(0..2).p(X):-d(X),1=2,1/X=1.",
        "d(0..2).p(X):-d(X),1/X=1,1=2.",
        "d(0).p:-d(X),X!=0,1/X>0.",
        "a(0).b(1).p:-a(X),1/X>0,b(Y),Y=2.",
        "a.b.p(N):-N=#sum{2147483647:a;1:b},1=2.",
    ] {
        assert_eq!(admitted(source), oracle::records(source), "{source}");
    }
}

#[test]
fn scoped_guards_are_reached_only_on_substitutions_nothing_excludes() {
    for scope in [":~", "p:-", "{p}:-", ":-"] {
        for body in ["d(X),1=2,#count{}>1/X", "d(X),#count{}>1/X,1=2"] {
            let source = format!("d(0).{scope}{body}.[1]");
            let source = if scope == ":~" {
                source
            } else {
                source.trim_end_matches("[1]").to_owned()
            };
            assert_eq!(admitted(&source), family(&["d(0)"]), "{source}");
        }
        let source = format!("d(0).{scope}d(X),#count{{}}>1/X.[1]");
        let source = if scope == ":~" {
            source
        } else {
            source.trim_end_matches("[1]").to_owned()
        };
        refused(&source, &EvaluationError::Undefined);
    }
}

#[test]
fn rejected_scopes_do_not_add_theory_or_objectives() {
    let original = reference::admit("d(1).", &FormulaLimits::default()).unwrap();
    for source in [
        "d(1).p:-d(X),1=2,#count{1:a}>1/X.",
        "d(1).:~d(X),1=2,#count{1:a}>1/X.[1]",
    ] {
        let admitted = reference::admit(source, &FormulaLimits::default()).unwrap();
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
            reference::exhaustive(&admitted),
            reference::exhaustive(&original),
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
        let admitted = reference::admit(&source, &FormulaLimits::default()).unwrap();
        let original = reference::admit("d(0).e(1,1).e(2,2).", &FormulaLimits::default()).unwrap();
        assert_eq!(
            reference::exhaustive(&admitted),
            reference::exhaustive(&original),
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
    assert!(reference::admit(source, &limits).is_ok());
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
        let admitted = reference::admit(source, &FormulaLimits::default()).unwrap();
        let expanded = reference::admit(expanded, &FormulaLimits::default()).unwrap();
        assert_eq!(
            reference::exhaustive(&admitted),
            reference::exhaustive(&expanded),
            "{source}"
        );
    }
}

#[test]
fn body_values_keep_their_validation_scope() {
    // A gate or an aggregate is not a comparison over the substitution: it
    // excludes nothing at grounding, so the operations beside it are reached.
    for source in [
        "d(0).p(1/X):-d(X),not d(X).",
        "d(0).p(1/X):-d(X),#count{}=1.",
        "d(0).{p(1/X):d(X),not d(X)}.",
        "d(0).{p(1/X)}:-d(X),#count{}=1.",
    ] {
        refused(source, &EvaluationError::Undefined);
    }
    // A false comparison excludes the substitution, and with it the operation
    // in a gate, a head, a guard or an aggregate element beside it.
    for (source, expected) in [
        ("d(0).p:-d(X),1=2,not q(1/X).", &["d(0)"][..]),
        ("d(0).p:-d(X),not q(1/X),1=2.", &["d(0)"][..]),
        ("d(0).p(1/X):-d(X),1=2,#count{}=1/X.", &["d(0)"][..]),
        ("d(0).{p(1):d(X),1=2,not q(1/X)}.", &["d(0)"][..]),
        ("d(0).#sum{W:p(1):d(X),W=1/X,1=2}>=0.", &["d(0)"][..]),
    ] {
        assert_eq!(admitted(source), family(expected), "{source}");
    }
}

#[test]
#[ignore = "requires independent clingo 5.8.2"]
fn staged_head_families_match_clingo() {
    for (source, _) in STAGED_HEADS {
        let admitted = reference::admit(source, &FormulaLimits::default()).unwrap();
        assert_eq!(
            reference::exhaustive(&admitted),
            oracle::records(source),
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
        let admitted = reference::admit(source, &limits).unwrap();
        assert_eq!(
            reference::exhaustive(&admitted),
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
                reference::admit(source, &limits),
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

#[test]
fn independent_fatal_comparisons_are_not_hidden_by_zero_divisors() {
    for (fatal, expected) in [
        ("2147483647+Y=0", EvaluationError::Overflow),
        ("symbol+Y=0", EvaluationError::Undefined),
        ("2**(-Y)=0", EvaluationError::Undefined),
    ] {
        for comparisons in [format!("1/X=1,{fatal}"), format!("{fatal},1/X=1")] {
            refused(
                &format!("d(0;1).e(1).p(X):-d(X),e(Y),{comparisons}."),
                &expected,
            );
        }
    }
}

#[test]
fn independent_fatal_branches_are_not_hidden_inside_one_expression() {
    for expression in ["1/X-(2147483647+Y)", "(2147483647+Y)-1/X"] {
        refused(
            &format!("d(0).e(1).p(X):-d(X),e(Y),{expression}=0."),
            &EvaluationError::Overflow,
        );
    }
}

#[test]
fn every_arithmetic_expression_needs_the_same_defined_substitution() {
    // Each division separately has a defined witness; there is no binding
    // where both divisions are defined, so the whole family is refused.
    refused(
        "d(0;1).p(X):-d(X),1/X>0,1/(1-X)>0.",
        &EvaluationError::Undefined,
    );
}

#[test]
fn a_valid_outer_binding_does_not_rescue_an_undefined_local_family() {
    for rule in [
        "{p(K,X):d(K,X),1/X=1}:-k(K).",
        "p(K):-k(K),1=#count{X:d(K,X),1/X=1}.",
    ] {
        refused(
            &format!("k(0;1).d(0,0).d(1,1).{rule}"),
            &EvaluationError::Undefined,
        );
    }
}

#[test]
fn missing_local_facts_leave_an_empty_silent_family() {
    let source = "k(0;1).d(1,1).p(K):-k(K),1=#count{X:d(K,X),1/X=1}.";
    let result = reference::admit(source, &FormulaLimits::default()).unwrap();
    assert!(result.warnings().is_empty());
    assert_eq!(
        reference::exhaustive(&result),
        family(&["k(0)", "k(1)", "d(1,1)", "p(1)"])
    );
}

#[test]
fn each_local_element_is_a_separate_source_family() {
    refused(
        "d(0).e(1).{p(X):d(X),1/X=1;q(Y):e(Y),1/Y=1}.",
        &EvaluationError::Undefined,
    );
}

#[test]
fn source_arithmetic_uses_complete_family_evidence() {
    for (rule, expected) in [
        ("p(X,Y):-d(X),Y=1/X.", family(&["d(0)", "d(1)", "p(1,1)"])),
        ("p(1/X):-d(X).", family(&["d(0)", "d(1)", "p(1)"])),
    ] {
        let source = format!("d(0;1).{rule}");
        let result = reference::admit(&source, &FormulaLimits::default()).unwrap();
        assert_eq!(result.warnings().len(), 1);
        assert_eq!(reference::exhaustive(&result), expected);
    }
}

#[test]
fn independent_comparison_components_preserve_fatal_errors() {
    for comparison in [
        "1/X=2147483647+Y",
        "2147483647+Y=1/X",
        "(1/X,2147483647+Y)=(0,0)",
        "(2147483647+Y,1/X)=(0,0)",
        "not 1/X=2147483647+Y",
    ] {
        refused(
            &format!("d(0).e(1).p(X):-d(X),e(Y),{comparison}."),
            &EvaluationError::Overflow,
        );
    }
}

#[test]
fn false_tuple_shape_does_not_hide_a_separate_zero_divisor() {
    refused(
        "d(0).e(1).p(X):-d(X),e(Y),(X,)=(X,Y),1/X=1.",
        &EvaluationError::Undefined,
    );
}

#[test]
fn explicit_outer_guards_exclude_undefined_local_families() {
    let source = "d(0;1).e(0,0).e(1,1).p(K):-d(K),K!=0,#count{1/X:e(K,X)}>0.";
    let result = reference::admit(source, &FormulaLimits::default()).unwrap();
    assert!(result.warnings().is_empty());
    assert_eq!(
        reference::exhaustive(&result),
        family(&["d(0)", "d(1)", "e(0,0)", "e(1,1)", "p(1)"])
    );
}

#[test]
fn local_pool_alternatives_share_their_original_family() {
    let source = "d(0).p:-1=#count{X:d(X),1/(X;1)>0}.";
    let result = reference::admit(source, &FormulaLimits::default()).unwrap();
    assert_eq!(result.warnings().len(), 1);
    assert_eq!(reference::exhaustive(&result), family(&["d(0)", "p"]));
}

#[test]
fn pooled_choice_heads_share_their_original_local_family() {
    let source = "d(0).{p(1/X;1):d(X)}.";
    let result = reference::admit(source, &FormulaLimits::default()).unwrap();
    assert_eq!(result.warnings().len(), 1);
    assert_eq!(
        reference::exhaustive(&result),
        BTreeSet::from([
            (BTreeSet::from(["d(0)".to_owned()]), None),
            (BTreeSet::from(["d(0)".to_owned(), "p(1)".to_owned()]), None),
        ])
    );
}

#[test]
fn an_independent_fatal_branch_survives_an_unavailable_generated_input() {
    refused(
        "d(0).e(1).p(Y):-d(X),e(Z),Y=1/X,Y-(2147483647+Z)=0.",
        &EvaluationError::Overflow,
    );
}

#[test]
fn conditional_local_bindings_share_the_original_family() {
    let source = "d(0;1).q(1).p:-q(1/X):d(X).";
    let result = reference::admit(source, &FormulaLimits::default()).unwrap();
    assert_eq!(result.warnings().len(), 1);
    assert_eq!(
        reference::exhaustive(&result),
        family(&["d(0)", "d(1)", "q(1)", "p"])
    );
}

#[test]
fn conditional_condition_pool_alternatives_share_the_original_family() {
    let source = "d(0).q(0).p:-q(X):d(X),1/(X;1)>0.";
    let result = reference::admit(source, &FormulaLimits::default()).unwrap();
    assert_eq!(result.warnings().len(), 1);
    assert_eq!(
        reference::exhaustive(&result),
        family(&["d(0)", "q(0)", "p"])
    );
}

#[test]
fn separate_authored_local_elements_do_not_share_a_witness() {
    for rule in [
        "{p(1/X):d(X);p(1):d(X)}.",
        "p:-1=#count{X:d(X),1/X>0;1:d(X),1>0}.",
    ] {
        refused(&format!("d(0).{rule}"), &EvaluationError::Undefined);
    }
}
