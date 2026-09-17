//! Keyed relations: exactly one value per key, from a program's choice rules.

use themelios_base::source::{Source, SourceId};
use themelios_program::program::Program;
use themelios_program::raise::raise;
use themelios_program::symbol::{Name, Sign, Signature, VarName};
use themelios_syntax::{dialect::Dialect, parse::parse};
use zetesis_domain::{Limits, keys};

fn source(text: &str) -> Program {
    let source = Source::new(SourceId::new(17), text.to_owned()).unwrap();
    let parsed = parse(&source, Dialect::Clingo);
    assert!(parsed.diagnostics().is_empty(), "{text}");
    let raised = raise(&parsed);
    assert!(raised.diagnostics().is_empty(), "{text}");
    raised.program().clone()
}

fn signature(name: &str, arity: u32) -> Signature {
    Signature {
        sign: Sign::Positive,
        name: Name::new(name).unwrap(),
        arity,
    }
}

fn keyed(text: &str) -> Vec<(Signature, usize)> {
    keys(&source(text), &Limits::default())
        .unwrap()
        .iter()
        .map(|key| (key.signature().clone(), key.value()))
        .collect()
}

#[test]
fn an_exactly_one_choice_keys_its_relation_on_the_body_variables() {
    // Every letter is assigned exactly one digit and nothing else assigns.
    let program = "letter(a;b). digit(0..9). 1 { assign(L,D) : digit(D) } 1 :- letter(L).";
    assert_eq!(keyed(program), [(signature("assign", 2), 1)]);
    let program = "idx(1..4). val(0;1). 1 { carry(V,I) : val(V) } 1 :- idx(I).";
    assert_eq!(keyed(program), [(signature("carry", 2), 0)]);
}

#[test]
fn the_bounds_must_be_exactly_one() {
    for program in [
        "letter(a). digit(0..9). { assign(L,D) : digit(D) } :- letter(L).",
        "letter(a). digit(0..9). 1 { assign(L,D) : digit(D) } :- letter(L).",
        "letter(a). digit(0..9). { assign(L,D) : digit(D) } 1 :- letter(L).",
        "letter(a). digit(0..9). 1 { assign(L,D) : digit(D) } 2 :- letter(L).",
        "letter(a). digit(0..9). 2 { assign(L,D) : digit(D) } 2 :- letter(L).",
    ] {
        assert_eq!(keyed(program), [], "{program}");
    }
    for program in [
        "letter(a). digit(0..9). 1 <= { assign(L,D) : digit(D) } <= 1 :- letter(L).",
        "letter(a). digit(0..9). { assign(L,D) : digit(D) } = 1 :- letter(L).",
        "letter(a). digit(0..9). 1 = { assign(L,D) : digit(D) } :- letter(L).",
    ] {
        assert_eq!(keyed(program), [(signature("assign", 2), 1)], "{program}");
    }
}

#[test]
fn another_producer_of_the_relation_removes_the_key() {
    // A fact, a rule, a second choice or an external each can add an atom the
    // choice did not select, so the value is no longer a function of the key.
    for program in [
        "letter(a). digit(0..9). 1 { assign(L,D) : digit(D) } 1 :- letter(L). assign(b,3).",
        "letter(a). digit(0..9). 1 { assign(L,D) : digit(D) } 1 :- letter(L). assign(L,0) :- letter(L).",
        "letter(a). digit(0..9). 1 { assign(L,D) : digit(D) } 1 :- letter(L). 1 { assign(b,D) : digit(D) } 1.",
        "letter(a). digit(0..9). 1 { assign(L,D) : digit(D) } 1 :- letter(L). #external assign(b,3).",
    ] {
        assert_eq!(keyed(program), [], "{program}");
    }
}

#[test]
fn the_key_is_exactly_the_body_and_the_value_is_bound_by_the_condition() {
    for program in [
        // The value variable is bound by the body, not the condition.
        "letter(a). digit(0..9). 1 { assign(L,D) : digit(D) } 1 :- letter(L), digit(D).",
        // A body variable is not a key argument.
        "letter(a). digit(0..9). 1 { assign(L,D) : digit(D) } 1 :- letter(L), digit(X).",
        // Two value positions.
        "letter(a). digit(0..9). 1 { assign(L,D,E) : digit(D), digit(E) } 1 :- letter(L).",
        // A key argument that is a term, not a variable.
        "letter(a). digit(0..9). 1 { assign(f(L),D) : digit(D) } 1 :- letter(L).",
        // A negated condition or body literal.
        "letter(a). digit(0..9). 1 { assign(L,D) : digit(D), not odd(D) } 1 :- letter(L).",
        "letter(a). digit(0..9). 1 { assign(L,D) : digit(D) } 1 :- letter(L), not skip(L).",
        // Two elements.
        "letter(a). digit(0..9). 1 { assign(L,D) : digit(D); other(L,D) : digit(D) } 1 :- letter(L).",
        // The relation appears in its own body.
        "letter(a). digit(0..9). 1 { assign(L,D) : digit(D) } 1 :- letter(L), assign(L,0).",
    ] {
        assert_eq!(keyed(program), [], "{program}");
    }
}

#[test]
fn the_key_names_its_variables_and_owner() {
    let program = source("letter(a). digit(0..9). 1 { assign(L,D) : digit(D) } 1 :- letter(L).");
    let keys = keys(&program, &Limits::default()).unwrap();
    let key = &keys[0];
    assert_eq!(key.key_variable(0).map(VarName::as_str), Some("L"));
    assert_eq!(key.key_variable(1), None);
    assert_eq!(key.value_variable().as_str(), "D");
    assert!(std::ptr::eq(
        key.statement(),
        program.statements().nth(2).unwrap()
    ));
    assert_eq!(key.condition().literals().count(), 1);
    assert_eq!(key.body().elements().count(), 1);
}

#[test]
fn the_analysis_stops_within_its_work_limit() {
    let program = "letter(a). digit(0..9). 1 { assign(L,D) : digit(D) } 1 :- letter(L).";
    let limits = Limits {
        max_work: 3,
        ..Limits::default()
    };
    let stop = keys(&source(program), &limits).unwrap_err();
    assert_eq!(stop.limit, 3);
    assert!(stop.observed > 3);
}
