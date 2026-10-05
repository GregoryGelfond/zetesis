//! Refusals preserve canonical identity and do not publish a replacement.

use super::support::{admitted, constant, family, name};
use std::collections::BTreeSet;
use themelios_macros::program;
use themelios_program::{
    AnswerSet, Origin, Program, Statement,
    program::{PartKey, Script},
};
use themelios_solve::{
    bridge::Door,
    contract::{Backend, Locus, Refused},
};
use zetesis_engine::Solver;

fn part_key(formals: &[&str]) -> PartKey {
    PartKey {
        name: name("step"),
        formals: formals.iter().map(|formal| name(formal)).collect(),
    }
}

fn constructed_part(key: PartKey) -> Program {
    let base = program! { q. };
    let step = program! { p(t). };
    Program::of_keyed_nodes([
        (
            base.base().key().clone(),
            base.base().statements().next().unwrap().clone(),
        ),
        (key, step.base().statements().next().unwrap().clone()),
    ])
}

#[test]
fn part_refusals_identify_the_exact_key() {
    // The same named part with and without formals has a different key.
    for (formals, text) in [
        (&[][..], include_str!("../fixtures/named-part.lp")),
        (&["t"][..], include_str!("../fixtures/part.lp")),
    ] {
        let key = part_key(formals);
        let program = constructed_part(key.clone());
        let source = admitted(text);
        for door in [Door::Program(&program), Door::Parsed(&source)] {
            let fault = Solver::default().lower(door).unwrap_err();
            assert_eq!(fault.locus(), Locus::Program);
            assert!(matches!(fault.refused(), Refused::Part(actual) if actual == &key));
        }
    }
}

#[test]
fn part_refusals_are_unlocated_through_both_doors() {
    let program = constructed_part(part_key(&["t"]));
    let source = admitted(include_str!("../fixtures/part.lp"));
    for door in [Door::Program(&program), Door::Parsed(&source)] {
        let fault = Solver::default().lower(door).unwrap_err();
        assert!(matches!(fault.refused(), Refused::Part(_)));
        assert!(fault.diagnostics().is_empty());
    }
}

#[test]
fn a_refused_replacement_preserves_the_previously_lowered_program() {
    let retained = program! { retained. };
    let refused = constructed_part(part_key(&["t"]));
    let parsed = admitted(include_str!("../fixtures/part.lp"));
    for door in [Door::Program(&refused), Door::Parsed(&parsed)] {
        let mut solver = Solver::default();
        solver.lower(Door::Program(&retained)).unwrap();
        assert!(solver.lower(door).is_err());
        assert_eq!(
            family(&mut solver),
            BTreeSet::from([AnswerSet::from([constant("retained")])])
        );
    }
}

#[test]
fn a_script_refusal_keeps_the_parsed_statement_location() {
    let source = admitted(include_str!("../fixtures/script.lp"));
    let statement = source
        .statements()
        .map(themelios_program::raise::StatementOccurrence::statement)
        .find(|statement| matches!(statement.get(), Statement::Script(_)))
        .unwrap();
    let fault = Solver::default().lower(Door::Parsed(&source)).unwrap_err();
    assert_eq!(fault.locus(), Locus::Program);
    let Refused::Statement(refused) = fault.refused() else {
        panic!("the script must be refused as a statement: {fault}");
    };
    assert_eq!(refused.get(), statement.get());
    let diagnostics = fault.diagnostics();
    let [diagnostic] = diagnostics.as_slice() else {
        panic!("expected one located script refusal: {diagnostics:?}");
    };
    assert!(
        statement
            .provenance()
            .origins()
            .any(|origin| origin == &Origin::Parsed(diagnostic.primary().location))
    );
}

#[test]
fn a_constructed_script_refusal_invents_no_location() {
    let script = Script::new(name("python"), "value = 1");
    let program = Program::of([Statement::from(script.clone())]);
    let fault = Solver::default()
        .lower(Door::Program(&program))
        .unwrap_err();
    assert_eq!(fault.locus(), Locus::Program);
    assert!(matches!(fault.refused(), Refused::Statement(statement)
        if statement.get() == &Statement::from(script)));
    assert!(fault.diagnostics().is_empty());
}
