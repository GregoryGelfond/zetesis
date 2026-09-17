//! The program's two closures bound the gate carrier: a gate atom no rule
//! derives under any gate assumption belongs to no answer set, and one the
//! gate-free rules derive belongs to every answer set, so the seed counter
//! omits the first and holds the second.

use zetesis_core::{
    AdmissionLimits, Atom, AtomPattern, Model, Predicate, Program, Template, Term, Value,
};
use zetesis_cpu::{
    CandidateLimits, Candidates, Control, Limits, check, lower_closure, upper_closure,
};

fn pattern(name: &str, terms: Vec<Term>) -> AtomPattern {
    let predicate = Predicate::new(name, terms.len()).expect("nonempty test name");
    AtomPattern::new(predicate, terms).expect("matching arity")
}
fn atom(name: &str, values: Vec<Value>) -> Atom {
    let predicate = Predicate::new(name, values.len()).expect("nonempty test name");
    Atom::new(predicate, values).expect("matching arity")
}
fn number(value: i32) -> Term {
    Term::Constant(Value::Number(value))
}
fn program(templates: Vec<Template>) -> Program {
    Program::new(templates, AdmissionLimits::default()).expect("safe finite test program")
}
fn fact(name: &str, values: Vec<Term>) -> Template {
    Template::new(Some(pattern(name, values)), vec![], vec![], vec![], vec![])
}
fn names(model: &Model) -> Vec<&str> {
    model
        .atoms()
        .iter()
        .map(|atom| atom.predicate().name())
        .collect()
}

/// p :- not q.  q :- not p.  r :- p, not s.  s :- t.  u.
fn gated_program() -> Program {
    let head = |name: &str| pattern(name, vec![]);
    program(vec![
        Template::new(Some(head("p")), vec![], vec![], vec![head("q")], vec![]),
        Template::new(Some(head("q")), vec![], vec![], vec![head("p")], vec![]),
        Template::new(
            Some(head("r")),
            vec![head("p")],
            vec![],
            vec![head("s")],
            vec![],
        ),
        Template::new(Some(head("s")), vec![head("t")], vec![], vec![], vec![]),
        fact("u", vec![]),
    ])
}

#[test]
fn the_upper_closure_treats_every_gate_as_possible() {
    // Every gate passes, so p, q and r are proposed; s waits on t, which
    // nothing proposes. Two seeds contradict each other's gates, but the
    // upper closure asks only whether some seed could enable each rule.
    let closure = upper_closure(&gated_program(), Limits::default(), &Control::default()).unwrap();
    assert_eq!(names(&closure), ["p", "q", "r", "u"]);
}

#[test]
fn the_lower_closure_fires_only_gate_free_rules() {
    // No gate passes, so only the fact u fires; s still waits on t.
    let closure = lower_closure(&gated_program(), Limits::default(), &Control::default()).unwrap();
    assert_eq!(names(&closure), ["u"]);
}

/// d(1..4). blocked(2). r(X) :- d(X), not blocked(X). :- not r(1).
fn blocked_program() -> Program {
    let mut templates: Vec<Template> = (1..=4)
        .map(|value| fact("d", vec![number(value)]))
        .collect();
    templates.push(fact("blocked", vec![number(2)]));
    templates.push(Template::new(
        Some(pattern("r", vec![Term::Variable(0)])),
        vec![pattern("d", vec![Term::Variable(0)])],
        vec![],
        vec![pattern("blocked", vec![Term::Variable(0)])],
        vec![],
    ));
    templates.push(Template::new(
        None,
        vec![],
        vec![],
        vec![pattern("r", vec![number(1)])],
        vec![],
    ));
    program(templates)
}

/// Every seed of `candidates` with its check, as (seed size, accepted).
fn enumerate(program: &Program, candidates: Candidates<'_>) -> Vec<(usize, bool)> {
    candidates
        .map(|seed| {
            let seed = seed.unwrap();
            let checked = check(program, &seed, Limits::default(), &Control::default()).unwrap();
            (seed.atoms().len(), checked.accepted())
        })
        .collect()
}

#[test]
fn the_bounded_counter_omits_gate_atoms_no_rule_derives_and_holds_the_necessary() {
    // The symbolic carrier holds blocked(1..4) and r(1..4): eight atoms and
    // 256 seeds. With every gate passing the upper closure derives blocked(2)
    // and all of r(1..4), r(2) included, since it reads no gate; the lower
    // closure derives blocked(2) alone, since r waits on a gate. Four free
    // gate atoms remain: sixteen seeds, each holding blocked(2).
    let program = blocked_program();
    let may = upper_closure(&program, Limits::default(), &Control::default()).unwrap();
    let must = lower_closure(&program, Limits::default(), &Control::default()).unwrap();
    assert!(may.contains(&atom("blocked", vec![Value::Number(2)])));
    assert!(!may.contains(&atom("blocked", vec![Value::Number(1)])));
    assert!(may.contains(&atom("r", vec![Value::Number(2)])));
    assert_eq!(names(&must), ["blocked", "d", "d", "d", "d"]);

    let mut candidates = Candidates::new(&program, CandidateLimits::default(), Control::default());
    candidates.within(may);
    candidates.requiring(&must).unwrap();
    assert_eq!(candidates.statistics().necessary_gate_atoms, 1);
    let seeds = enumerate(&program, candidates);
    assert_eq!(seeds.len(), 16);
    assert!(
        seeds.iter().all(|(size, _)| *size >= 1),
        "blocked(2) is held"
    );
    assert_eq!(
        seeds.iter().filter(|(_, accepted)| *accepted).count(),
        1,
        "d(1..4), blocked(2), r(1), r(3), r(4)"
    );
}

#[test]
fn the_unbounded_counter_enumerates_the_whole_symbolic_carrier() {
    let program = blocked_program();
    let candidates = Candidates::new(&program, CandidateLimits::default(), Control::default());
    let seeds = enumerate(&program, candidates);
    assert_eq!(seeds.len(), 256);
    assert_eq!(seeds.iter().filter(|(_, accepted)| *accepted).count(), 1);
}

#[test]
fn the_bounded_counter_reports_the_atoms_it_omitted() {
    // blocked(1), blocked(3) and blocked(4) are outside the upper closure;
    // the five remaining gate atoms give thirty-two seeds.
    let program = blocked_program();
    let may = upper_closure(&program, Limits::default(), &Control::default()).unwrap();
    let mut candidates = Candidates::new(&program, CandidateLimits::default(), Control::default());
    candidates.within(may);
    assert_eq!(candidates.by_ref().map(Result::unwrap).count(), 32);
    assert_eq!(candidates.statistics().underivable_gate_atoms, 3);
    assert_eq!(candidates.statistics().necessary_gate_atoms, 0);
}

#[test]
fn a_resource_stop_while_bounding_leaves_the_symbolic_carrier() {
    // One unit of work cannot compute a closure; the counter then runs over
    // all eight symbolic gate atoms and the receipt names the stop.
    let program = blocked_program();
    let mut candidates = Candidates::new(&program, CandidateLimits::default(), Control::default());
    candidates.bounded(Limits {
        max_work: 1,
        ..Limits::default()
    });
    assert_eq!(candidates.by_ref().map(Result::unwrap).count(), 256);
    let statistics = candidates.statistics();
    assert_eq!(statistics.bounds_stop, Some(zetesis_cpu::Stop::WorkLimit));
    assert_eq!(statistics.underivable_gate_atoms, 0);
    assert_eq!(statistics.necessary_gate_atoms, 0);
}

#[test]
fn a_cancelled_bound_stops_the_first_pull() {
    let program = blocked_program();
    let control = Control::default();
    control.cancel();
    let mut candidates = Candidates::new(&program, CandidateLimits::default(), control);
    candidates.bounded(Limits::default());
    assert!(matches!(
        candidates.next(),
        Some(Err(zetesis_cpu::Stop::Cancelled))
    ));
}

#[test]
fn the_bounded_counter_applies_both_closures_on_its_first_pull() {
    let program = blocked_program();
    let mut candidates = Candidates::new(&program, CandidateLimits::default(), Control::default());
    candidates.bounded(Limits::default());
    let seeds = enumerate(&program, candidates);
    assert_eq!(seeds.len(), 16);
    assert_eq!(seeds.iter().filter(|(_, accepted)| *accepted).count(), 1);
}
