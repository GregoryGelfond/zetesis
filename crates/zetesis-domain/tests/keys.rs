//! Keyed relations: exactly one value per key, from a program's choice rules.

use themelios_base::source::{Source, SourceId};
use themelios_program::program::{Arguments, Atom, Program};
use themelios_program::raise::raise;
use themelios_program::symbol::{Name, Sign, Signature, Symbol, VarName};
use themelios_syntax::{dialect::Dialect, parse::parse};
use zetesis_domain::{KeyWork, Limits, atom_signature, facts, keys};

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
    keys(&source(text), &mut KeyWork::new(Limits::default().max_work))
        .unwrap()
        .iter()
        .map(|key| (key.signature().clone(), key.value_position()))
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
fn the_value_is_bound_by_the_condition_not_the_body() {
    let program = "letter(a). digit(0..9). 1 { assign(L,D) : digit(D) } 1 :- letter(L), digit(D).";
    assert_eq!(keyed(program), [], "{program}");
}

#[test]
fn every_body_variable_is_a_key_argument() {
    let program = "letter(a). digit(0..9). 1 { assign(L,D) : digit(D) } 1 :- letter(L), digit(X).";
    assert_eq!(keyed(program), [], "{program}");
}

#[test]
fn the_relation_has_one_value_position() {
    let program =
        "letter(a). digit(0..9). 1 { assign(L,D,E) : digit(D), digit(E) } 1 :- letter(L).";
    assert_eq!(keyed(program), [], "{program}");
}

#[test]
fn every_key_argument_is_a_variable() {
    let program = "letter(a). digit(0..9). 1 { assign(f(L),D) : digit(D) } 1 :- letter(L).";
    assert_eq!(keyed(program), [], "{program}");
}

#[test]
fn a_negated_literal_in_the_condition_or_the_body_yields_no_key() {
    for program in [
        "letter(a). digit(0..9). 1 { assign(L,D) : digit(D), not odd(D) } 1 :- letter(L).",
        "letter(a). digit(0..9). 1 { assign(L,D) : digit(D) } 1 :- letter(L), not skip(L).",
    ] {
        assert_eq!(keyed(program), [], "{program}");
    }
}

#[test]
fn the_choice_has_one_element() {
    let program = "letter(a). digit(0..9). 1 { assign(L,D) : digit(D); other(L,D) : digit(D) } 1 :- letter(L).";
    assert_eq!(keyed(program), [], "{program}");
}

#[test]
fn the_relation_is_absent_from_its_own_body() {
    let program =
        "letter(a). digit(0..9). 1 { assign(L,D) : digit(D) } 1 :- letter(L), assign(L,0).";
    assert_eq!(keyed(program), [], "{program}");
}

const ONE_KEY: &str = "letter(a). digit(0..9). 1 { assign(L,D) : digit(D) } 1 :- letter(L).";

#[test]
fn the_key_names_its_variables() {
    let program = source(ONE_KEY);
    let keys = keys(&program, &mut KeyWork::new(Limits::default().max_work)).unwrap();
    let key = &keys[0];
    assert_eq!(key.key_variable(0).map(VarName::as_str), Some("L"));
    assert_eq!(key.key_variable(1), None);
    assert_eq!(key.value_variable().as_str(), "D");
}

#[test]
fn the_key_names_its_condition_and_body() {
    let program = source(ONE_KEY);
    let keys = keys(&program, &mut KeyWork::new(Limits::default().max_work)).unwrap();
    let key = &keys[0];
    assert_eq!(key.condition().literals().count(), 1);
    assert_eq!(key.body().elements().count(), 1);
}

#[test]
fn the_analysis_stops_within_its_work_limit() {
    let program = "letter(a). digit(0..9). 1 { assign(L,D) : digit(D) } 1 :- letter(L).";
    let mut work = KeyWork::new(3);
    let stop = keys(&source(program), &mut work).unwrap_err();
    assert_eq!(stop.limit, 3);
    assert!(stop.observed > 3);
    assert_eq!(work.steps(), 3);
}

#[test]
fn an_arity_beyond_a_signatures_width_has_no_signature() {
    let atom = Atom {
        sign: Sign::Positive,
        name: Name::new("p").unwrap(),
        arguments: Arguments::Single(vec![]),
    };
    assert_eq!(atom_signature(&atom, 2), Some(signature("p", 2)));
    assert_eq!(atom_signature(&atom, usize::MAX), None);
}

fn numbers(values: &[&Symbol]) -> Vec<i32> {
    values
        .iter()
        .map(|symbol| match symbol {
            Symbol::Number(number) => *number,
            other => panic!("{other:?}"),
        })
        .collect()
}

#[test]
fn the_facts_of_a_predicate_are_read_when_facts_are_all_that_produces_it() {
    let program = source("digit(0;1;2). letter(a). 1 { assign(L,D) : digit(D) } 1 :- letter(L).");
    let mut work = KeyWork::new(1_000);
    let values = facts(&program, &signature("digit", 1), 0, &mut work)
        .unwrap()
        .unwrap();
    assert_eq!(numbers(&values), [0, 1, 2]);
    assert_eq!(work.steps(), 3);
}

#[test]
fn a_predicate_with_a_rule_producer_has_no_facts_to_read() {
    let program = source("span(0;1). digit(D) :- span(D). digit(2).");
    let mut work = KeyWork::new(1_000);
    assert_eq!(
        facts(&program, &signature("digit", 1), 0, &mut work).unwrap(),
        None
    );
}

#[test]
fn a_predicate_produced_by_a_choice_has_no_facts_to_read() {
    let program = source("{ digit(1) }. digit(2).");
    let mut work = KeyWork::new(1_000);
    assert_eq!(
        facts(&program, &signature("digit", 1), 0, &mut work).unwrap(),
        None
    );
}

#[test]
fn reading_facts_stops_within_the_key_work() {
    let program = source("digit(0). digit(1). digit(2).");
    let mut work = KeyWork::new(2);
    let stop = facts(&program, &signature("digit", 1), 0, &mut work).unwrap_err();
    assert_eq!((stop.limit, work.steps()), (2, 2));
}
