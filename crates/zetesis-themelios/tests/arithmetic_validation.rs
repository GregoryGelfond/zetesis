//! A comparison over the relationally bound variables that is defined and
//! false excludes the substitution, and nothing in an excluded substitution is
//! reached. Every other substitution validates its arithmetic in full: a
//! reached undefined or overflowing operation refuses the program. Incomplete
//! relational prefixes have no such duty.

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

fn admitted(source: &str) -> BTreeSet<(BTreeSet<String>, Option<Vec<i64>>)> {
    let admitted = source_records::admit(source, &FormulaLimits::default()).unwrap();
    source_records::exhaustive(&admitted)
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
    refused("d(0..2).p(X):-d(X),1/X=1.", &EvaluationError::Undefined);
    refused(
        "d(1..2).p(X):-d(X),X!=0,X+2147483647<0.",
        &EvaluationError::Overflow,
    );
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
        "d(0..1).e(1..2).p(X,Y):-d(X),e(Y),1/X=1,X!=3.",
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
fn reached_operations_differ_from_clingo() {
    // clingo drops the undefined instance X = 0 with a message and keeps
    // p(1); zetesis refuses the program, since nothing excludes the
    // substitution that reaches the operation.
    let source = "d(0..2).p(X):-d(X),1/X=1.";
    refused(source, &EvaluationError::Undefined);
    assert_eq!(
        source_oracle::records(source),
        BTreeSet::from([(
            ["d(0)", "d(1)", "d(2)", "p(1)"].map(str::to_owned).into(),
            None,
        )]),
        "{source}"
    );
}

#[test]
#[ignore = "requires independent clingo 5.8.2"]
fn excluded_substitutions_match_clingo() {
    // These sources once refused natively because a false comparison was not
    // permitted to hide an operation to its side; the excluded substitution is
    // now not reached, as in the reference.
    for source in [
        "d(0..2).p(X):-d(X),1=2,1/X=1.",
        "d(0..2).p(X):-d(X),1/X=1,1=2.",
        "d(0).p:-d(X),X!=0,1/X>0.",
        "a(0).b(1).p:-a(X),1/X>0,b(Y),Y=2.",
        "a.b.p(N):-N=#sum{2147483647:a;1:b},1=2.",
    ] {
        assert_eq!(admitted(source), source_oracle::records(source), "{source}");
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
