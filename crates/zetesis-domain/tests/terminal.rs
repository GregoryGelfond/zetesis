//! Structural eligibility over exact original rule carriers; no solver dispatch.

use themelios_program::program::{Atom, Head, LiteralInner, Program, Rule, Statement};
use themelios_program::provenance::WithProvenance;
use themelios_program::symbol::{Name, Sign, Symbol};
use themelios_program::term::Term;
use zetesis_domain::terminal::{self, Analysis, Status, UnknownReason};
use zetesis_domain::{Context, Limits};

#[path = "terminal/support.rs"]
mod support;
use support::source;

fn head(source: &WithProvenance<Statement>) -> &Atom {
    let Statement::Rule(rule) = source.get() else {
        panic!("selected non-rule")
    };
    let Head::Literal(literal) = rule.head().get() else {
        panic!("selected non-normal head")
    };
    let LiteralInner::Atom(atom) = &literal.inner else {
        panic!("selected non-atom")
    };
    atom.get()
}

fn selected(result: &Analysis<'_>, name: &str, arity: usize) -> usize {
    result
        .definitions()
        .iter()
        .filter(|carrier| {
            let atom = head(carrier);
            atom.name.as_str() == name && atom.alternatives().any(|terms| terms.len() == arity)
        })
        .count()
}

#[test]
fn unrelated_isomorphic_choices_leave_only_the_terminal_definition_selected() {
    for (text, terminal, choice) in [
        (
            "seed(a). color(red). color(blue). 1 { picked(K,V):color(V) } 1 :- seed(K). receipt(K,V):-picked(K,V).",
            "receipt",
            "picked",
        ),
        (
            "ticket(7). state(9). state(11). 1 { allocated(A,B):state(B) } 1 :- ticket(A). ledger(A,B):-allocated(A,B).",
            "ledger",
            "allocated",
        ),
    ] {
        let program = source(text);
        let result = terminal::analyze(&program, Limits::default());
        assert_eq!(result.status(), Status::Complete);
        assert_eq!(result.definitions().len(), 1);
        assert_eq!(selected(&result, terminal, 2), 1);
        assert_eq!(
            selected(&result, choice, 2),
            0,
            "exact-one is not functional union support"
        );
    }
}

#[test]
fn optional_choices_can_supply_terminal_definitions() {
    // The base can choose neither, either, or both rows. Terminal eligibility
    // requires no exact-one property and establishes no truth for either row.
    let program = source("{ seed(1); seed(2) }. receipt(X):-seed(X).");
    let result = terminal::analyze(&program, Limits::default());
    assert_eq!(result.status(), Status::Complete);
    assert_eq!(result.definitions().len(), 1);
    assert_eq!(selected(&result, "receipt", 1), 1);
}

#[test]
fn all_distinct_normal_producers_are_returned_in_original_program_order() {
    let program = source("seed(1). seed(2). receipt(X):-seed(X). receipt(8). receipt(9).");
    let result = terminal::analyze(&program, Limits::default());
    let expected: Vec<_> = program
        .statements()
        .filter(|carrier| head(carrier).name.as_str() == "receipt")
        .collect();
    assert_eq!(result.status(), Status::Complete);
    assert_eq!(expected.len(), 3);
    assert_eq!(result.definitions().len(), expected.len());
    for (actual, expected) in result.definitions().iter().zip(expected) {
        assert!(std::ptr::eq(*actual, expected));
    }
}

#[test]
fn duplicate_rules_retain_the_original_merged_provenance_carrier() {
    let program = source("seed(1). receipt(X):-seed(X). receipt(X):-seed(X).");
    let result = terminal::analyze(&program, Limits::default());
    assert_eq!(result.definitions().len(), 1);
    let carrier = result.definitions()[0];
    assert!(
        program
            .statements()
            .any(|original| std::ptr::eq(original, carrier))
    );
    assert_eq!(carrier.provenance().origins().count(), 2);
}

#[test]
fn equal_content_does_not_authenticate_a_different_program_instance() {
    let program = source("completed.");
    let other = program.clone();
    let result = terminal::analyze(&program, Limits::default());
    assert!(std::ptr::eq(result.program(), &raw const program));
    assert!(result.belongs_to(&program));
    assert!(!result.belongs_to(&other));
}

#[test]
fn any_nonqualifying_producer_blocks_the_entire_signature() {
    for producer in [
        "{ receipt(2) }.",
        "receipt(2) | alternative.",
        "receipt(X):seed(X).",
        "#count { X : receipt(X) } = 1 :- seed(X).",
        "receipt(X+1):-seed(X).",
        "receipt(1..3).",
        "receipt(f(X)):-seed(X).",
        "receipt(2):-not absent.",
    ] {
        let program = source(&format!("seed(1). receipt(0). {producer}"));
        let result = terminal::analyze(&program, Limits::default());
        assert_eq!(result.status(), Status::Complete, "{producer}");
        assert_eq!(selected(&result, "receipt", 1), 0, "{producer}");
    }
}

#[test]
fn every_head_variable_needs_a_whole_positive_body_binder() {
    for rule in [
        "receipt(X):-seed(_).",
        "receipt(_):-seed(X).",
        "receipt(X,Y):-seed(X).",
        "receipt(X):-seed(f(X)).",
        "receipt(X):-X=1.",
    ] {
        let program = source(&format!("seed(1). {rule}"));
        let result = terminal::analyze(&program, Limits::default());
        assert_eq!(result.status(), Status::Complete, "{rule}");
        assert!(
            result
                .definitions()
                .iter()
                .all(|carrier| head(carrier).name.as_str() != "receipt"),
            "{rule}"
        );
    }
}

#[test]
fn repeated_variables_remain_eligible() {
    let program = source("receipt(X,X):-seed(X),pair(X,X).");
    let result = terminal::analyze(&program, Limits::default());
    assert_eq!(result.status(), Status::Complete);
    assert_eq!(selected(&result, "receipt", 2), 1);
}

#[test]
fn anonymous_body_positions_remain_eligible() {
    let program = source("receipt(X):-seed(X,_).");
    let result = terminal::analyze(&program, Limits::default());
    assert_eq!(result.status(), Status::Complete);
    assert_eq!(selected(&result, "receipt", 1), 1);
}

#[test]
fn semantic_reads_block_selection_in_every_supported_local_context() {
    for consumer in [
        ":- receipt(1).",
        ":- not receipt(1).",
        ":- not not receipt(1).",
        "other:-receipt(X):seed(X).",
        "other:-seed(X):receipt(X).",
        "{ other(X):receipt(X) }.",
        "other(X):receipt(X) | alternative.",
        "#count { X : other(X) : receipt(X) } = 1.",
        ":- #count{X:receipt(X)} > 0.",
        ":- #sum{1,X:receipt(X)} > 0.",
        ":- 1 { receipt(X):seed(X) }.",
    ] {
        let program = source(&format!("seed(1). receipt(X):-seed(X). {consumer}"));
        let result = terminal::analyze(&program, Limits::default());
        assert_eq!(result.status(), Status::Complete, "{consumer}");
        assert_eq!(selected(&result, "receipt", 1), 0, "{consumer}");
    }
}

#[test]
fn definition_dependency_cycles_prevent_selection() {
    for text in [
        "receipt(X):-receipt(X).",
        "seed(1). receipt(X):-seed(X). seed(X):-receipt(X).",
    ] {
        let program = source(text);
        let result = terminal::analyze(&program, Limits::default());
        assert_eq!(result.status(), Status::Complete);
        assert_eq!(selected(&result, "receipt", 1), 0);
    }
}

#[test]
fn complementary_strong_signs_block_implicit_coherence_dependencies() {
    for other in [
        "-receipt(2).",
        ":- -receipt(2).",
        "{ -receipt(X):seed(X) }.",
    ] {
        let program = source(&format!("seed(1). receipt(X):-seed(X). {other}"));
        let result = terminal::analyze(&program, Limits::default());
        assert_eq!(result.status(), Status::Complete);
        assert_eq!(selected(&result, "receipt", 1), 0, "{other}");
    }
}

#[test]
fn same_spelling_at_another_arity_is_a_different_signature() {
    let program = source("receipt(1). -receipt(2,3). :- receipt(1,2,3).");
    let result = terminal::analyze(&program, Limits::default());
    assert_eq!(result.status(), Status::Complete);
    assert_eq!(selected(&result, "receipt", 1), 1);
}

#[test]
fn show_declarations_preserve_selection() {
    for declaration in [
        "",
        "#show receipt/1.",
        "#show other/0.",
        "#show X:receipt(X).",
    ] {
        let program = source(&format!("seed(1). receipt(X):-seed(X). {declaration}"));
        let result = terminal::analyze(&program, Limits::default());
        assert_eq!(result.status(), Status::Complete);
        assert_eq!(result.definitions().len(), 1);
        assert_eq!(selected(&result, "receipt", 1), 1);
    }
}

#[test]
fn defined_declarations_preserve_selection() {
    let program = source("seed(1). receipt(X):-seed(X). #defined receipt/1.");
    let result = terminal::analyze(&program, Limits::default());
    assert_eq!(result.status(), Status::Complete);
    assert_eq!(result.definitions().len(), 1);
    assert_eq!(selected(&result, "receipt", 1), 1);
}

#[test]
fn global_unsupported_context_clears_all_partial_selection() {
    for (suffix, reason) in [
        ("#minimize{1:completed}.", UnknownReason::Objectives),
        (":~ completed. [1@0]", UnknownReason::Objectives),
        ("#project completed/0.", UnknownReason::Projection),
        (
            "#const n=2.",
            UnknownReason::Source(zetesis_domain::UnknownReason::Constants),
        ),
        (
            "#external open.",
            UnknownReason::Source(zetesis_domain::UnknownReason::External),
        ),
        (
            "#include \"absent.lp\".",
            UnknownReason::Source(zetesis_domain::UnknownReason::Include),
        ),
        (
            "#program step(t). later(t).",
            UnknownReason::Source(zetesis_domain::UnknownReason::ProgramPart),
        ),
        (
            "&a{}.",
            UnknownReason::Source(zetesis_domain::UnknownReason::Theory),
        ),
    ] {
        let program = source(&format!("completed. {suffix}"));
        if reason == UnknownReason::Source(zetesis_domain::UnknownReason::ProgramPart) {
            // Raising retains parts with statements, not an empty directive.
            assert!(
                program
                    .parts()
                    .any(|part| part.key().name.as_str() == "step")
            );
        }
        let result = terminal::analyze(&program, Limits::default());
        assert_eq!(result.status(), Status::Unknown(reason), "{suffix}");
        assert!(result.definitions().is_empty());
        match result.context().unwrap() {
            Context::Part(part) => {
                assert!(program.parts().any(|original| std::ptr::eq(original, part)));
            }
            Context::Statement(carrier) => assert!(
                program
                    .statements()
                    .any(|original| std::ptr::eq(original, carrier))
            ),
        }
    }
}

#[test]
fn already_closed_compound_symbols_are_borrowed_without_source_evaluation() {
    let symbol = Symbol::Function {
        name: Name::new("parcel").unwrap(),
        arguments: vec![Symbol::Number(7), Symbol::String("fragile".into())],
        sign: Sign::Negative,
    };
    let program = Program::of([Rule::fact(Atom::new(
        Name::new("receipt").unwrap(),
        [Term::Symbolic(symbol)],
    ))]);
    let result = terminal::analyze(&program, Limits::default());
    assert_eq!(result.status(), Status::Complete);
    assert_eq!(result.definitions().len(), 1);
    assert!(
        program
            .statements()
            .any(|original| std::ptr::eq(original, result.definitions()[0]))
    );
}

#[test]
fn unchanged_einstein_selects_only_its_terminal_definition() {
    let program = source(include_str!("../../../examples/einstein-riddle.lp"));
    let result = terminal::analyze(&program, Limits::default());
    assert_eq!(result.status(), Status::Complete);
    assert_eq!(result.definitions().len(), 1);
    assert_eq!(selected(&result, "solution", 6), 1);
    // None of the exact-one choice relations is classified as a definition.
    for name in ["lives_in", "drinks", "owns", "smokes", "painted"] {
        assert_eq!(selected(&result, name, 2), 0);
    }
}
