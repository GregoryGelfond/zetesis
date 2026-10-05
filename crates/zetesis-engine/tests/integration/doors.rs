//! Canonical construction, display and the supported grounding profiles.

use super::support::{admitted, complete, constant, family, name, unary};
use std::{
    collections::{BTreeMap, BTreeSet},
    num::NonZeroUsize,
};
use themelios_macros::program;
use themelios_program::{AnswerSet, Sign, Symbol};
use themelios_solve::{bridge::Door, contract::Backend};
use zetesis_engine::{Config, Grounder, Solver};

#[test]
fn macro_and_parsed_doors_yield_the_same_complete_family() {
    let program = program! { p :- not q. q :- not p. };
    let source = admitted(include_str!("../fixtures/doors.lp"));
    let expected = BTreeSet::from([
        AnswerSet::from([constant("p")]),
        AnswerSet::from([constant("q")]),
    ]);
    let mut solver = Solver::default();
    for door in [Door::Program(&program), Door::Parsed(&source)] {
        solver.lower(door).unwrap();
        assert_eq!(family(&mut solver), expected);
    }
}

#[test]
fn display_terms_do_not_replace_full_answer_atoms() {
    let program = program! {
        hidden.
        { picked }.
        #show.
        #show 7 : picked.
    };
    let source = admitted(include_str!("../fixtures/observations.lp"));
    let expected = BTreeMap::from([
        (AnswerSet::from([constant("hidden")]), BTreeSet::new()),
        (
            AnswerSet::from([constant("hidden"), constant("picked")]),
            BTreeSet::from([Symbol::Number(7)]),
        ),
    ]);
    let mut solver = Solver::default();
    for door in [Door::Program(&program), Door::Parsed(&source)] {
        solver.lower(door).unwrap();
        let actual: BTreeMap<_, _> = complete(&mut solver)
            .into_iter()
            .map(|model| (model.atoms().clone(), model.shown().symbols().clone()))
            .collect();
        assert_eq!(actual, expected);
    }
}

#[test]
fn unscored_enumeration_preserves_complete_interpretations() {
    let source = admitted(include_str!("../fixtures/unscored.lp"));
    let expected = BTreeSet::from([
        AnswerSet::new(),
        AnswerSet::from([constant("p")]),
        AnswerSet::from([constant("q")]),
        AnswerSet::from([constant("p"), constant("q")]),
    ]);
    let mut solver = Solver::default();
    for door in [
        Door::Parsed(&source),
        Door::Program(Door::Parsed(&source).program()),
    ] {
        solver.lower(door).unwrap();
        assert_eq!(family(&mut solver), expected);
    }
}

#[test]
fn hybrid_constraints_preserve_the_eligible_answer_family() {
    let source = admitted(include_str!("../fixtures/hybrid.lp"));
    let expected = BTreeSet::from([
        AnswerSet::new(),
        AnswerSet::from([unary("picked", 1)]),
        AnswerSet::from([unary("picked", 2)]),
        AnswerSet::from([unary("picked", 3)]),
    ]);
    let mut solver = Solver::new(Config {
        grounder: Grounder::Hybrid,
        workers: NonZeroUsize::new(2).unwrap(),
        ..Config::default()
    });
    for door in [
        Door::Parsed(&source),
        Door::Program(Door::Parsed(&source).program()),
    ] {
        solver.lower(door).unwrap();
        assert_eq!(family(&mut solver), expected);
    }
}

#[test]
fn lazy_relations_preserve_the_recursive_answer_family() {
    let source = admitted(include_str!("../fixtures/lazy.lp"));
    let edges: AnswerSet = [(1, 2), (2, 3)]
        .map(|(left, right)| {
            Symbol::function(
                name("edge"),
                [Symbol::Number(left), Symbol::Number(right)],
                Sign::Positive,
            )
        })
        .into();
    let mut reachable = edges.clone();
    reachable.insert(unary("selected", 1));
    reachable.extend((1..=3).map(|value| unary("reach", value)));
    let expected = BTreeSet::from([edges, reachable]);
    let mut solver = Solver::new(Config {
        grounder: Grounder::Lazy,
        workers: NonZeroUsize::new(2).unwrap(),
        ..Config::default()
    });
    for door in [
        Door::Parsed(&source),
        Door::Program(Door::Parsed(&source).program()),
    ] {
        solver.lower(door).unwrap();
        assert_eq!(family(&mut solver), expected);
    }
}

#[test]
fn lazy_shown_terms_follow_recursive_reachability() {
    let source = admitted(include_str!("../fixtures/lazy.lp"));
    let mut solver = Solver::new(Config {
        grounder: Grounder::Lazy,
        workers: NonZeroUsize::new(2).unwrap(),
        ..Config::default()
    });
    for door in [
        Door::Parsed(&source),
        Door::Program(Door::Parsed(&source).program()),
    ] {
        solver.lower(door).unwrap();
        let models = complete(&mut solver);
        // Both cases must occur to exercise the conditional display.
        let selected = unary("selected", 1);
        assert_eq!(
            models
                .iter()
                .map(|model| model.atoms().contains(&selected))
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([false, true])
        );
        for model in models {
            let expected = if model.atoms().contains(&selected) {
                BTreeSet::from([1.into(), 2.into(), 3.into()])
            } else {
                BTreeSet::new()
            };
            assert_eq!(model.shown().symbols(), &expected);
        }
    }
}
