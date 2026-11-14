//! A comparison over the relationally bound variables that is defined and
//! false excludes the substitution, and nothing in an excluded substitution is
//! reached. Every other substitution validates its arithmetic in full: a
//! reached zero divisor omits its instance and leaves a warning on its
//! statement, whether or not its family has a defined instance; other
//! reached arithmetic failures refuse the program. Incomplete relational
//! prefixes have no such duty.

use crate::support::warnings::zero_divisor_statement;
use std::collections::BTreeSet;
use zetesis_clingo_support as oracle;
use zetesis_reference_support as reference;
use zetesis_reference_support::resource_limits;
use zetesis_themelios::{
    ExpansionFailure, FormulaFailure, FormulaLimits, FormulaResource, observation::EvaluationError,
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

/// The answer sets of `source`, admitted without a warning: no substitution
/// reached a zero divisor.
fn silent(source: &str) -> BTreeSet<(BTreeSet<String>, Option<Vec<i64>>)> {
    let admitted = reference::admit(source, &FormulaLimits::default())
        .unwrap_or_else(|failure| panic!("{source}: {failure}"));
    assert!(
        admitted.warnings().is_empty(),
        "{source}: {:?}",
        admitted.warnings()
    );
    reference::exhaustive(&admitted)
}

/// The answer sets of `source`, admitted with exactly one warning: a zero
/// divisor omitted from an instance of `statement`.
fn omitting(source: &str, statement: &str) -> BTreeSet<(BTreeSet<String>, Option<Vec<i64>>)> {
    let admitted = reference::admit(source, &FormulaLimits::default())
        .unwrap_or_else(|failure| panic!("{source}: {failure}"));
    assert_eq!(zero_divisor_statement(&admitted), statement, "{source}");
    reference::exhaustive(&admitted)
}

fn family(atoms: &[&str]) -> BTreeSet<(BTreeSet<String>, Option<Vec<i64>>)> {
    BTreeSet::from([(atoms.iter().map(|atom| (*atom).to_owned()).collect(), None)])
}

/// The family of these answer sets, none with costs.
fn answer_sets(models: &[&[&str]]) -> BTreeSet<(BTreeSet<String>, Option<Vec<i64>>)> {
    models.iter().flat_map(|atoms| family(atoms)).collect()
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
fn a_reached_zero_divisor_omits_its_instance() {
    // Nothing excludes X = 0 here, so the division is reached: the rule's
    // one instance is omitted and the rule keeps the warning.
    assert_eq!(
        omitting("d(0).p(X):-d(X),1/X=1.", "p(X):-d(X),1/X=1."),
        family(&["d(0)"])
    );
}

#[test]
fn a_reached_overflow_refuses() {
    // X != 0 over d(1..2) excludes nothing, so the overflow is reached.
    refused(
        "d(1..2).p(X):-d(X),X!=0,X+2147483647<0.",
        &EvaluationError::Overflow,
    );
}

#[test]
fn body_generators_do_not_hide_a_reached_zero_divisor() {
    // A body generator consumes an already complete relational binding; it
    // neither discharges the division that binding reaches nor generates
    // an instance from it.
    for generator in ["Y=X+1", "Y=X..X+1"] {
        let rule = format!("p(X,Y):-d(X),1/X=1,{generator}.");
        assert_eq!(
            omitting(&format!("d(0).{rule}"), &rule),
            family(&["d(0)"]),
            "{rule}"
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
fn a_defined_false_substitution_does_not_hide_an_omitted_one() {
    // The comparison divides by zero at X = 0 and is defined and false at
    // X = 2. Neither substitution derives an instance, and the false one does
    // not remove the warning the undefined one leaves on the rule.
    let rule = "p(X,Y) :- d(X), 1/X=1, Y=X+1.";
    assert_eq!(
        omitting(&format!("d(0;2). {rule}"), rule),
        family(&["d(0)", "d(2)"])
    );
}

#[test]
fn an_exclusion_at_any_depth_hides_a_failure_at_another() {
    // 1/X fails for X = 0 once d(X) is bound; X + Y != Y is false for the
    // same X only once e(Y) is bound, deeper in the join. The substitution is
    // excluded, not omitted with a warning, and the verdict does not depend
    // on which is met first.
    for body in ["d(X),e(Y),1/X=1,X+Y!=Y", "d(X),e(Y),X+Y!=Y,1/X=1"] {
        let source = format!("d(0..1).e(1..2).p(X,Y):-{body}.");
        assert_eq!(
            silent(&source),
            family(&["d(0)", "d(1)", "e(1)", "e(2)", "p(1,1)", "p(1,2)"]),
            "{source}"
        );
    }
    // A comparison that excludes nothing leaves the division reached: its
    // instances are omitted and the rule keeps the warning.
    let rule = "p(X,Y):-d(X),e(Y),1/X=1,X!=3.";
    assert_eq!(
        omitting(&format!("d(0).e(1..2).{rule}"), rule),
        family(&["d(0)", "e(1)", "e(2)"])
    );
    // An exclusion also hides a failure met before the relation that decides
    // the exclusion.
    for body in ["d(X),1=2,1/X=1,e(X,Y)", "e(X,Y),1/X=1,1=2,d(X)"] {
        let source = format!("d(0).e(0,1).e(2,2).p(X,Y):-{body}.");
        assert_eq!(
            silent(&source),
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
fn incomplete_positive_prefixes_raise_no_warning() {
    // No row completes the join, so the division in its prefix is under no
    // duty: nothing is omitted and no warning is kept.
    for source in EMPTY_EXTENSIONS {
        let admitted = reference::admit(source, &FormulaLimits::default()).unwrap();
        assert!(admitted.warnings().is_empty(), "{source}");
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
    // 1 = 2 excludes the one substitution before any binder runs, so no
    // division is reached and no warning is kept.
    for body in ["X=0,1=2,Y=1/X", "Y=1/X,1=2,X=0"] {
        let source = format!("p(Y):-{body}.");
        assert_eq!(silent(&source), family(&[]), "{source}");
    }
    // Without the exclusion the binder reaches 1/0: it generates nothing
    // either, and the rule keeps the warning.
    let rule = "p(Y):-X=0,Y=1/X.";
    assert_eq!(omitting(rule, rule), family(&[]));
}

#[test]
fn constant_comparisons_exclude_or_reach() {
    for source in [
        "#const z=0. d(0).p(X):-d(X),1=2,1/(X+z)=1.",
        "#const z=0. d(0).p(X):-d(X),1/(X+z)=1,1=2.",
    ] {
        assert_eq!(silent(source), family(&["d(0)"]), "{source}");
    }
    // A closed undefined term is refused as written, during source
    // preparation, before any substitution exists to exclude.
    for source in ["p:-1=2,1/0=1.", "p:-1/0=1,1=2.", "p:-1/0=1."] {
        refused(source, &EvaluationError::Undefined);
    }
    // With no comparison to exclude it, the substitution reaches the
    // division: its instance is omitted and the rule keeps the warning.
    assert_eq!(
        omitting(
            "#const z=0. d(0).p(X):-d(X),1/(X+z)=1.",
            "p(X):-d(X),1/(X+z)=1."
        ),
        family(&["d(0)"])
    );
}

#[test]
fn objective_substitutions_follow_the_same_rule() {
    for field in ["1", "0", "symbol", "1@symbol"] {
        for body in ["d(X),1=2,1/X=1", "d(X),1/X=1,1=2"] {
            let source = format!("d(0).:~{body}.[{field}]");
            assert_eq!(silent(&source), family(&["d(0)"]), "{source}");
        }
        let statement = format!(":~d(X),1/X=1.[{field}]");
        assert_eq!(
            omitting(&format!("d(0).{statement}"), &statement),
            family(&["d(0)"]),
            "{statement}"
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
#[ignore = "requires clingo: defined and empty join families match clingo"]
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
#[ignore = "requires clingo: all undefined families match clingo"]
fn all_undefined_families_match_clingo() {
    // clingo drops the only undefined instance with a note; the library
    // omits it with a warning, and the two families agree.
    let source = "d(0).p(X):-d(X),1/X=1.";
    assert_eq!(
        omitting(source, "p(X):-d(X),1/X=1."),
        oracle::records(source)
    );
}

#[test]
#[ignore = "requires clingo: excluded substitutions match clingo"]
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
            assert_eq!(silent(&source), family(&["d(0)"]), "{source}");
        }
        // Nothing excludes X = 0 here, so the guard's division is reached:
        // the instance is omitted and its statement keeps the warning.
        let statement = format!("{scope}d(X),#count{{}}>1/X.[1]");
        let statement = if scope == ":~" {
            statement
        } else {
            statement.trim_end_matches("[1]").to_owned()
        };
        assert_eq!(
            omitting(&format!("d(0).{statement}"), &statement),
            family(&["d(0)"]),
            "{statement}"
        );
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
            (admitted.theory().nodes(), admitted.theory().operands()),
            (original.theory().nodes(), original.theory().operands()),
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
        // The skipped guard divides by zero at X = 0, which only a warning
        // would show.
        assert!(admitted.warnings().is_empty(), "{source}");
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
fn tuple_shape_mismatch_does_not_hide_its_zero_divisor() {
    // The shapes differ whatever their fields hold, yet every field is
    // evaluated: the division is reached, its instance is omitted and the
    // rule keeps the warning.
    for comparison in ["(1,1/X)=(1,)", "(1,)=(1,1/X)"] {
        let rule = format!("p(X):-d(X),{comparison}.");
        assert_eq!(
            omitting(&format!("d(0).{rule}"), &rule),
            family(&["d(0)"]),
            "{rule}"
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
        // A head evaluated for a row its body does not select would divide
        // by zero in several of these sources, which only a warning would
        // show.
        assert!(admitted.warnings().is_empty(), "{source}");
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
    // The instance that divides by zero is omitted and its statement keeps
    // the warning.
    for source in [
        "d(0).p(1/X):-d(X),not d(X).",
        "d(0).p(1/X):-d(X),#count{}=1.",
        "d(0).{p(1/X):d(X),not d(X)}.",
        "d(0).{p(1/X)}:-d(X),#count{}=1.",
    ] {
        let statement = &source["d(0).".len()..];
        assert_eq!(omitting(source, statement), family(&["d(0)"]), "{source}");
    }
    // A false comparison excludes the substitution, and with it the operation
    // in a gate, a head, a guard or an aggregate element beside it: nothing
    // is reached, so no warning is kept.
    for (source, expected) in [
        ("d(0).p:-d(X),1=2,not q(1/X).", &["d(0)"][..]),
        ("d(0).p:-d(X),not q(1/X),1=2.", &["d(0)"][..]),
        ("d(0).p(1/X):-d(X),1=2,#count{}=1/X.", &["d(0)"][..]),
        ("d(0).{p(1):d(X),1=2,not q(1/X)}.", &["d(0)"][..]),
        ("d(0).#sum{W:p(1):d(X),W=1/X,1=2}>=0.", &["d(0)"][..]),
    ] {
        assert_eq!(silent(source), family(expected), "{source}");
    }
}

#[test]
#[ignore = "requires clingo: staged head families match clingo"]
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
    let limits = resource_limits::formula(FormulaResource::GeneratedValues, 0);
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
fn a_zero_divisor_in_any_expression_omits_the_instance() {
    // Each division is defined at one binding and undefined at the other, so
    // every instance divides by zero somewhere: both are omitted, under the
    // rule's one warning.
    let rule = "p(X):-d(X),1/X>0,1/(1-X)>0.";
    assert_eq!(
        omitting(&format!("d(0;1).{rule}"), rule),
        family(&["d(0)", "d(1)"])
    );
}

#[test]
fn a_defined_outer_binding_does_not_hide_a_local_omission() {
    // The local family under K = 0 holds one instance, which divides by zero
    // and is omitted. The family under K = 1 is defined: it neither removes
    // the warning nor restores that instance.
    for (rule, expected) in [
        (
            "{p(K,X):d(K,X),1/X=1}:-k(K).",
            &[
                &["k(0)", "k(1)", "d(0,0)", "d(1,1)"][..],
                &["k(0)", "k(1)", "d(0,0)", "d(1,1)", "p(1,1)"],
            ][..],
        ),
        (
            "p(K):-k(K),1=#count{X:d(K,X),1/X=1}.",
            &[&["k(0)", "k(1)", "d(0,0)", "d(1,1)", "p(1)"][..]][..],
        ),
    ] {
        assert_eq!(
            omitting(&format!("k(0;1).d(0,0).d(1,1).{rule}"), rule),
            answer_sets(expected),
            "{rule}"
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
fn a_defined_local_element_does_not_hide_an_omitted_one() {
    // The first element's one instance divides by zero and is omitted. The
    // second element is defined: it keeps its choice of q(1) and neither
    // removes the warning nor restores p(0).
    let choice = "{p(X):d(X),1/X=1;q(Y):e(Y),1/Y=1}.";
    assert_eq!(
        omitting(&format!("d(0).e(1).{choice}"), choice),
        answer_sets(&[&["d(0)", "e(1)"], &["d(0)", "e(1)", "q(1)"]])
    );
}

#[test]
fn a_binder_or_head_zero_divisor_omits_only_its_instance() {
    // The binder and the head divide by zero at X = 0 alone: that instance
    // is omitted with the rule's warning, and the instance at X = 1 remains.
    for (rule, expected) in [
        ("p(X,Y):-d(X),Y=1/X.", family(&["d(0)", "d(1)", "p(1,1)"])),
        ("p(1/X):-d(X).", family(&["d(0)", "d(1)", "p(1)"])),
    ] {
        assert_eq!(
            omitting(&format!("d(0;1).{rule}"), rule),
            expected,
            "{rule}"
        );
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
    // The tuple comparison is false for its shape alone and excludes
    // nothing, so the division beside it is reached: the instance is omitted
    // and the rule keeps the warning.
    let rule = "p(X):-d(X),e(Y),(X,)=(X,Y),1/X=1.";
    assert_eq!(
        omitting(&format!("d(0).e(1).{rule}"), rule),
        family(&["d(0)", "e(1)"])
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
fn local_pool_alternatives_warn_once_for_their_statement() {
    // The pool's alternative 1/X divides by zero at X = 0 and is omitted;
    // its alternative 1/1 keeps the element, so the count is 1. The one
    // warning is located at the rule as written, pool included.
    let rule = "p:-1=#count{X:d(X),1/(X;1)>0}.";
    assert_eq!(
        omitting(&format!("d(0).{rule}"), rule),
        family(&["d(0)", "p"])
    );
}

#[test]
fn pooled_choice_heads_warn_once_for_their_statement() {
    // The pooled head's alternative p(1/X) divides by zero at X = 0 and is
    // omitted; its alternative p(1) stays a choice. The one warning is
    // located at the statement as written, pool included.
    let choice = "{p(1/X;1):d(X)}.";
    assert_eq!(
        omitting(&format!("d(0).{choice}"), choice),
        answer_sets(&[&["d(0)"], &["d(0)", "p(1)"]])
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
fn an_assignment_overflow_refuses_beside_a_zero_divisor() {
    // An assignment aggregate's carrier is computed apart from the row's own
    // evaluation, whose last operation may have divided by zero: in an
    // earlier generator of the same row, or in the head, a choice bound or a
    // comparison head of the row before. The carrier's overflow is fatal
    // beside each, and is not read as that zero divisor.
    for source in [
        "d(0).{q}.h(Y,V):-d(X),Y=1/X,V=#sum{2147483647,a;1,b:q}.",
        "d(1;2).{q}.h(2/(X-1),V):-d(X),V=#sum{X*1073741824,a:q}.",
        "d(0;1;2).{q}.h(2/(X-1),V):-d(X),V=#sum{X*1073741824,a:q}.",
        "d(0;1).{q}.{c(X,V)}1/X:-d(X),V=#sum{X*2147483647+X,a:q}.",
        "d(1;2).{q}.2/(X-1)>0:-d(X),V=#sum{X*1073741824,a:q},V>=0.",
    ] {
        refused(source, &EvaluationError::Overflow);
    }
}

#[test]
fn an_assignment_exponent_failure_refuses_beside_a_zero_divisor() {
    refused(
        "d(1;2).{q}.h(2/(X-1),V):-d(X),V=#sum{2**(1-X),a:q}.",
        &EvaluationError::Undefined,
    );
}

#[test]
fn a_zero_divisor_beside_a_defined_assignment_omits_its_instance() {
    // The same rule over the one row whose head divides by zero and whose
    // carrier is defined: the instance is omitted with the rule's warning.
    let rule = "h(2/(X-1),V):-d(X),V=#sum{X*1073741824,a:q}.";
    assert_eq!(
        omitting(&format!("d(1).{{q}}.{rule}"), rule),
        answer_sets(&[&["d(1)"], &["d(1)", "q"]])
    );
}

#[test]
fn an_undefined_consequent_is_false_beside_a_defined_binding() {
    // The consequent divides by zero under the binding X = 0 and is defined
    // under X = 1: one warning, for the rule. A conditional literal keeps the
    // undefined binding, with a false consequent, so p needs d(0) false.
    let rule = "p:-q(1/X):d(X).";
    assert_eq!(
        omitting(&format!("d(0;1).q(1).{rule}"), rule),
        family(&["d(0)", "d(1)", "q(1)"])
    );
}

#[test]
fn pooled_conditional_conditions_warn_once_for_their_statement() {
    // The condition's alternative 1/X>0 divides by zero at X = 0 and has no
    // binding there; its alternative 1/1>0 keeps the binding, under which
    // q(0) holds. The one warning is located at the rule as written, pool
    // included.
    let rule = "p:-q(X):d(X),1/(X;1)>0.";
    assert_eq!(
        omitting(&format!("d(0).q(0).{rule}"), rule),
        family(&["d(0)", "q(0)", "p"])
    );
}

#[test]
fn elements_over_one_condition_are_omitted_separately() {
    // Both authored elements range over d(X). The one that divides by zero
    // is omitted with the statement's warning; the defined one beside it
    // still contributes its instance.
    for (rule, expected) in [
        (
            "{p(1/X):d(X);p(1):d(X)}.",
            &[&["d(0)"][..], &["d(0)", "p(1)"]][..],
        ),
        (
            "p:-1=#count{X:d(X),1/X>0;1:d(X),1>0}.",
            &[&["d(0)", "p"][..]][..],
        ),
    ] {
        assert_eq!(
            omitting(&format!("d(0).{rule}"), rule),
            answer_sets(expected),
            "{rule}"
        );
    }
}

/// Chains with a link that divides by zero beside a link that is defined
/// and false. The links are separate comparisons, so the false link decides
/// the chain, whatever its sign and wherever it stands: nothing is omitted
/// and no warning is kept.
const DECIDED_CHAINS: &[(&str, &[&[&str]])] = &[
    ("d(0).p:-d(X),not 2<1<1/X.", &[&["d(0)", "p"]]),
    ("d(0).p:-d(X),not 1/X<2<1.", &[&["d(0)", "p"]]),
    (
        "d(0;3).p(X):-d(X),not 2<X<6/X.",
        &[&["d(0)", "d(3)", "p(0)", "p(3)"]],
    ),
    ("d(0).p:-d(X),2<1<1/X.", &[&["d(0)"]]),
    ("d(0).p:-d(X),not not 2<1<1/X.", &[&["d(0)"]]),
    ("d(0).2<1<1/X:-d(X).", &[]),
    ("d(0;1).1<=X<=5/X:-d(X).", &[]),
    ("d(0).p:-2<1<1/X:d(X).", &[&["d(0)"]]),
    ("d(0).p:-not 2<1<1/X:d(X).", &[&["d(0)", "p"]]),
    (
        "{d(0)}.d(1).p:-not 2<X<1/X:d(X).",
        &[&["d(0)", "d(1)", "p"], &["d(1)", "p"]],
    ),
    (
        "d(0..2).p(X):-d(X),1<=X<=2/X.",
        &[&["d(0)", "d(1)", "d(2)", "p(1)"]],
    ),
    (
        "d(0..2).q(N):-N=#count{X:d(X),not 1<X<2/X}.",
        &[&["d(0)", "d(1)", "d(2)", "q(3)"]],
    ),
];

/// Chains with a link that divides by zero and no false link: the chain is
/// undefined, and its instance is omitted with the statement's warning. In a
/// conditional literal the consequent is then false under its condition.
const UNDEFINED_CHAINS: &[(&str, &str, &[&str])] = &[
    ("d(0).", "p:-d(X),not 0<1<1/X.", &["d(0)"]),
    ("d(0).", "0<1<1/X:-d(X).", &["d(0)"]),
    ("d(0).", "p:-not 0<1<1/X:d(X).", &["d(0)"]),
    (
        "d(0;1).",
        "p(X):-d(X),0<=X<=1/X.",
        &["d(0)", "d(1)", "p(1)"],
    ),
];

/// Chains with an interval among their terms, each of whose values has a
/// false link beside the link that divides by zero. An interval value is
/// one alternative of the chain and is read by its links as any chain is,
/// so the false link decides each: nothing is omitted and no warning is
/// kept.
const DECIDED_RANGED_CHAINS: &[(&str, &[&[&str]])] = &[
    (
        "d(0;1).p(X):-d(X),not 3<2<(1..2)/X.",
        &[&["d(0)", "d(1)", "p(0)", "p(1)"]],
    ),
    ("d(0).p:-d(X),not 2<(1..1)<1/X.", &[&["d(0)", "p"]]),
    ("d(0;1).p(X):-d(X),(1..2)/X<3<2.", &[&["d(0)", "d(1)"]]),
    (
        "d(0;1).{q(X):d(X),not 3<2<(1..2)/X}.",
        &[
            &["d(0)", "d(1)"],
            &["d(0)", "d(1)", "q(0)"],
            &["d(0)", "d(1)", "q(1)"],
            &["d(0)", "d(1)", "q(0)", "q(1)"],
        ],
    ),
    (
        "d(0;1).p:-2<=#count{X:d(X),not 3<2<(1..2)/X}.",
        &[&["d(0)", "d(1)", "p"]],
    ),
];

/// Chains with an interval one of whose values has no false link where a
/// link divides by zero. That value's alternative is omitted with the
/// statement's warning; a value with a false link is decided beside it.
const UNDEFINED_RANGED_CHAINS: &[(&str, &str, &[&[&str]])] = &[
    // At X = 0 the value 1 has the false link 1<1 and the value 2 has none.
    (
        "d(0;1).",
        "p(X):-d(X),not 1<(1..2)<=2/X.",
        &[&["d(0)", "d(1)", "p(0)", "p(1)"]],
    ),
    (
        "d(0;1).",
        "p(X):-d(X),1<(1..2)<=2/X.",
        &[&["d(0)", "d(1)", "p(1)"]],
    ),
    (
        "d(0;1).",
        "p:-not 1<(1..2)<=2/X:d(X).",
        &[&["d(0)", "d(1)", "p"]],
    ),
    ("d(0;1).", ":-not 1<(1..2)<=2/X:d(X).", &[]),
    // No value has a false link at X = 0.
    ("d(0).", "p:-d(X),not 0<(1..2)<3/X.", &[&["d(0)"]]),
    (
        "d(0;2).",
        "p(X):-d(X),not 1<(2..3)<6/X<4.",
        &[&["d(0)", "d(2)", "p(2)"]],
    ),
];

#[test]
fn a_false_link_decides_a_chain_beside_an_undefined_one() {
    for &(source, expected) in DECIDED_CHAINS {
        assert_eq!(silent(source), answer_sets(expected), "{source}");
    }
}

#[test]
fn a_chain_with_no_false_link_is_undefined_where_a_link_is() {
    for &(facts, statement, expected) in UNDEFINED_CHAINS {
        assert_eq!(
            omitting(&format!("{facts}{statement}"), statement),
            family(expected),
            "{statement}"
        );
    }
}

#[test]
fn a_false_link_decides_each_value_of_an_interval_in_a_chain() {
    for &(source, expected) in DECIDED_RANGED_CHAINS {
        assert_eq!(silent(source), answer_sets(expected), "{source}");
    }
}

#[test]
fn an_interval_value_with_no_false_link_is_omitted_where_a_link_is_undefined() {
    for &(facts, statement, expected) in UNDEFINED_RANGED_CHAINS {
        assert_eq!(
            omitting(&format!("{facts}{statement}"), statement),
            answer_sets(expected),
            "{statement}"
        );
    }
}

#[test]
fn a_fatal_link_refuses_beside_a_false_one() {
    // An overflow is no zero divisor: the false link 2<1 does not hide it,
    // in either order.
    for source in [
        "d(1).p:-d(X),not 2<1<X+2147483647.",
        "d(1).p:-d(X),not X+2147483647<2<1.",
        "d(1).2<1<X+2147483647:-d(X).",
    ] {
        refused(source, &EvaluationError::Overflow);
    }
}

#[test]
fn a_fatal_link_refuses_beside_a_false_link_and_a_zero_divisor() {
    // The false link 2<1 drops the zero divisor of 1/Y and must not make the
    // overflow of X+2147483647 read as one, in whatever order the links
    // stand. In the first two chains the link evaluated last divides by zero
    // and holds no overflow.
    for source in [
        "d(1,0).p:-d(X,Y),not X+2147483647<2<1<1/Y.",
        "d(1,0).X+2147483647<2<1<1/Y:-d(X,Y).",
        "d(1,0).p:-d(X,Y),not 1/Y<2<1<X+2147483647.",
        "d(1,0).p:-d(X,Y),not 2<1<X+2147483647<1/Y.",
        "d(1,0).p:-d(X,Y),not 2<1<1/Y<X+2147483647.",
    ] {
        refused(source, &EvaluationError::Overflow);
    }
}

/// Comparisons that read an interval with no value under the row `X = 0`,
/// an empty one or one whose bound divides by zero there, beside a term that
/// overflows at that row. The term stands before the interval and after it.
const VALUELESS_INTERVALS: &[&str] = &[
    // A comparison and a chain in a body, positive and negated.
    "d(0).p:-d(X),X+2147483647+1<(3..1).",
    "d(0).p:-d(X),(3..1)>X+2147483647+1.",
    "d(0).p:-d(X),2<X+2147483647+1<(3..1).",
    "d(0).p:-d(X),(3..1)<2<X+2147483647+1.",
    "d(0).p:-d(X),not X+2147483647+1<(3..1).",
    "d(0).p:-d(X),not not (3..1)>X+2147483647+1.",
    // The condition of a choice element and of an aggregate element.
    "d(0).{q(X):d(X),X+2147483647+1<(3..1)}.",
    "d(0).p:-0<#count{X:d(X),(3..1)>X+2147483647+1}.",
    // A conditional consequent, which no row of its condition reads.
    "d(0).p:-X+2147483647+1<(3..1):d(X).",
    "d(0).p:-(3..1)>X+2147483647+1:d(X).",
    "d(0).p:-X+2147483647+1<2<(1..2/X):d(X).",
    "d(0).p:-(1..2/X)>2>X+2147483647+1:d(X).",
    // The failing term reads a second interval, which has values.
    "d(0).p:-d(X),(1..2)+X+2147483647<(3..1).",
];

#[test]
fn a_fatal_term_refuses_beside_an_interval_without_a_value() {
    // The comparison has no instance under the row, so no instance evaluates
    // its other terms; their fatal failure refuses all the same, in either
    // order.
    for source in VALUELESS_INTERVALS {
        refused(source, &EvaluationError::Overflow);
    }
}

#[test]
fn a_zero_divisor_beside_an_empty_interval_omits_nothing() {
    // Where the comparison has no instance there is none to omit: no warning
    // is kept, in either order.
    for source in [
        "d(0).p:-d(X),1/X<(3..1).",
        "d(0).p:-d(X),(3..1)>1/X.",
        "d(0).p:-1/X<(3..1):d(X).",
    ] {
        assert_eq!(silent(source), family(&["d(0)"]), "{source}");
    }
}

#[test]
fn an_excluded_row_reaches_no_term_beside_an_empty_interval() {
    // X != 0 excludes the row, whichever side of the comparison it stands.
    for source in [
        "d(0).p:-d(X),X!=0,X+2147483647+1<(3..1).",
        "d(0).p:-d(X),X+2147483647+1<(3..1),X!=0.",
    ] {
        assert_eq!(silent(source), family(&["d(0)"]), "{source}");
    }
}

#[test]
fn a_term_over_an_absent_interval_value_is_not_read() {
    // The failing term itself reads the second interval, which is entered
    // only under a value of the empty first one: it has no value to fail at.
    let source = "d(0).p:-d(X),(3..1)>(1..2)+X+2147483647.";
    assert_eq!(silent(source), family(&["d(0)"]));
}

#[test]
fn a_comparison_without_an_interval_of_its_own_is_left_to_its_rows() {
    // The interval stands in another literal. Neither a chain that reads
    // nothing of it nor one that reads the variable it binds owns it: both
    // are evaluated under complete rows alone, and the rule has none.
    for source in [
        "d(0).p:-d(X),Y=3..1,1<X+2147483647+1<5.",
        "d(0).p:-d(X),Y=3..1,0<Y<X+2147483647+1.",
    ] {
        assert_eq!(silent(source), family(&["d(0)"]), "{source}");
    }
}

#[test]
#[ignore = "requires clingo: chains with an undefined link match clingo"]
fn chains_with_an_undefined_link_match_clingo() {
    for &(source, expected) in DECIDED_CHAINS {
        assert_eq!(oracle::records(source), answer_sets(expected), "{source}");
    }
    for &(facts, statement, expected) in UNDEFINED_CHAINS {
        assert_eq!(
            oracle::records(&format!("{facts}{statement}")),
            family(expected),
            "{statement}"
        );
    }
    for &(source, expected) in DECIDED_RANGED_CHAINS {
        assert_eq!(oracle::records(source), answer_sets(expected), "{source}");
    }
    for &(facts, statement, expected) in UNDEFINED_RANGED_CHAINS {
        assert_eq!(
            oracle::records(&format!("{facts}{statement}")),
            answer_sets(expected),
            "{statement}"
        );
    }
}

#[test]
fn fact_blocked_producers_preserve_downstream_overflow() {
    // Support is a source-admission carrier. The locally total h producer
    // remains in it even though no answer set can contain its head.
    refused(
        "blocked. d(2147483647). h(X):-d(X),not blocked. k(X+1):-h(X).",
        &EvaluationError::Overflow,
    );
}

#[test]
fn a_fact_blocked_producer_does_not_hide_a_zero_divisor() {
    // As with the overflow above, h(0) stays in the support that admission
    // reads, so the division downstream is reached: its instance is omitted
    // and its rule keeps the warning.
    assert_eq!(
        omitting(
            "blocked. d(0). h(X):-d(X),not blocked. k(1/X):-h(X).",
            "k(1/X):-h(X)."
        ),
        family(&["blocked", "d(0)"])
    );
}

#[test]
fn removing_a_blocked_producer_changes_overflow_admission() {
    assert_eq!(
        admitted("blocked. d(2147483647). k(X+1):-h(X)."),
        family(&["blocked", "d(2147483647)"]),
    );
}

#[test]
fn removing_a_blocked_producer_removes_the_zero_divisor_warning() {
    // Without a producer h has no support, so the division is never reached.
    assert_eq!(
        silent("blocked. d(0). k(1/X):-h(X)."),
        family(&["blocked", "d(0)"]),
    );
}
