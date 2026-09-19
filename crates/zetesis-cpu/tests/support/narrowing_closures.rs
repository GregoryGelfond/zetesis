//! The narrowing keeps its closures' preparation and workspace with the
//! iterator, so that every closure after the first runs on what the first
//! prepared and reserved.

use super::*;
use zetesis_core::{AdmissionLimits, AtomPattern, Predicate, Template, Term, Value};

/// node(1..n). in(X) :- node(X), not out(X). out(X) :- node(X), not in(X).
fn independent(nodes: i32) -> Program {
    let pattern = |name: &str, terms: Vec<Term>| {
        AtomPattern::new(Predicate::new(name, terms.len()).unwrap(), terms).unwrap()
    };
    let mut templates: Vec<Template> = (1..=nodes)
        .map(|node| {
            Template::new(
                Some(pattern("node", vec![Term::Constant(Value::Number(node))])),
                vec![],
                vec![],
                vec![],
                vec![],
            )
        })
        .collect();
    let x = Term::Variable(0);
    for (head, gate) in [("in", "out"), ("out", "in")] {
        templates.push(Template::new(
            Some(pattern(head, vec![x.clone()])),
            vec![pattern("node", vec![x.clone()])],
            vec![],
            vec![pattern(gate, vec![x.clone()])],
            vec![],
        ));
    }
    Program::new(templates, AdmissionLimits::default()).unwrap()
}

#[test]
fn the_narrowing_keeps_the_capacity_its_closures_reserved() {
    // Fifteen regions, each narrowed by two closures on the one workspace,
    // which retains the capacity the closures reserved instead of freeing it
    // after each.
    let program = independent(3);
    let mut candidates = Candidates::new(&program, CandidateLimits::default(), Control::default());
    candidates.bounded(Limits::default());
    assert_eq!(candidates.by_ref().map(Result::unwrap).count(), 8);
    assert_eq!(candidates.statistics().regions, 15);
    let BoundsState::Applied(closures) = &candidates.bounds else {
        panic!("the bounds are applied");
    };
    let empty = ClosureWorkspace::default().retained_bytes().unwrap();
    assert!(closures.workspace.retained_bytes().unwrap() > empty);
}

#[test]
fn a_stopped_preparation_is_the_narrowings_stop() {
    // No unit of work prepares a program with gate atoms: the narrowing
    // reports the preparation's stop and offers the whole symbolic carrier.
    let program = independent(2);
    let mut candidates = Candidates::new(&program, CandidateLimits::default(), Control::default());
    candidates.bounded(Limits {
        max_work: 0,
        ..Limits::default()
    });
    assert_eq!(candidates.by_ref().map(Result::unwrap).count(), 16);
    assert_eq!(candidates.statistics().bounds_stop, Some(Stop::WorkLimit));
    assert!(matches!(candidates.bounds, BoundsState::Unavailable));
}
