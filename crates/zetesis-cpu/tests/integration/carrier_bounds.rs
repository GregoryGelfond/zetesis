//! The program's two closures bound the gate carrier: a gate atom no rule
//! derives under any gate assumption belongs to no answer set, and one the
//! gate-free rules derive belongs to every answer set, so the seed counter
//! omits the first and holds the second.

use std::collections::BTreeSet;
use zetesis_core::{Atom, Predicate, Program, Seed, Template, Term, Value};
use zetesis_cpu::{Cancellation, CandidateLimits, Candidates, Limits, check};

use zetesis_test_support::programs::{fact, number, pattern, program};

fn atom(name: &str, values: Vec<Value>) -> Atom {
    let predicate = Predicate::new(name, values.len()).expect("nonempty test name");
    Atom::new(predicate, values).expect("matching arity")
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

/// p :- not q.  q :- not p.  s.  r :- p, not s.
fn held_program() -> Program {
    let head = |name: &str| pattern(name, vec![]);
    program(vec![
        Template::new(Some(head("p")), vec![], vec![], vec![head("q")], vec![]),
        Template::new(Some(head("q")), vec![], vec![], vec![head("p")], vec![]),
        fact("s", vec![]),
        Template::new(
            Some(head("r")),
            vec![head("p")],
            vec![],
            vec![head("s")],
            vec![],
        ),
    ])
}

/// The seeds the narrowed counter offers for `program`, with the statistics.
fn narrowed(program: &Program) -> (Vec<Seed>, zetesis_cpu::CandidateStatistics) {
    let mut candidates =
        Candidates::new(program, CandidateLimits::default(), Cancellation::default());
    candidates.bounded(Limits::default());
    let seeds = candidates.by_ref().map(Result::unwrap).collect();
    (seeds, candidates.statistics())
}

fn accepted_models(program: &Program, bounded: bool) -> BTreeSet<Vec<Atom>> {
    let mut candidates =
        Candidates::new(program, CandidateLimits::default(), Cancellation::default());
    if bounded {
        candidates.bounded(Limits::default());
    }
    candidates
        .filter_map(|seed| {
            let checked = check(
                program,
                &seed.unwrap(),
                Limits::default(),
                &Cancellation::default(),
            )
            .unwrap();
            checked.accepted().then(|| {
                checked
                    .interpretation()
                    .atoms()
                    .iter()
                    .map(|atom| atom.to_atom(zetesis_core::ValueLimits::default()).unwrap())
                    .collect()
            })
        })
        .collect()
}

#[test]
fn an_omitted_negative_gate_is_false_not_a_missing_rule() {
    // No rule produces q; excluding q from true candidates must leave not q
    // satisfied in the unchanged rule p :- not q.
    let program = program(vec![Template::new(
        Some(pattern("p", vec![])),
        vec![],
        vec![],
        vec![pattern("q", vec![])],
        vec![],
    )]);
    let expected = BTreeSet::from([vec![atom("p", vec![])]]);
    assert_eq!(accepted_models(&program, true), expected);
    assert_eq!(accepted_models(&program, false), expected);
}

#[test]
fn possible_double_negation_does_not_establish_truth() {
    let p = pattern("p", vec![]);
    let program = program(vec![Template::new(
        Some(p.clone()),
        vec![],
        vec![p],
        vec![],
        vec![],
    )]);
    let expected = BTreeSet::from([vec![], vec![atom("p", vec![])]]);
    assert_eq!(accepted_models(&program, true), expected);
    assert_eq!(accepted_models(&program, false), expected);
}

#[test]
fn positive_cycles_supply_no_possible_support_without_a_fact() {
    // The negative reference makes p a gate. Candidate truth must not seed
    // the ordinary positive loop p :- p; only q belongs to the answer set.
    let p = pattern("p", vec![]);
    let program = program(vec![
        Template::new(Some(p.clone()), vec![p.clone()], vec![], vec![], vec![]),
        Template::new(Some(pattern("q", vec![])), vec![], vec![], vec![p], vec![]),
    ]);
    let expected = BTreeSet::from([vec![atom("q", vec![])]]);
    assert_eq!(accepted_models(&program, true), expected);
    assert_eq!(accepted_models(&program, false), expected);
}

#[test]
fn a_missing_positive_witness_cannot_produce_a_gate_atom() {
    let p = pattern("p", vec![]);
    let q = pattern("q", vec![]);
    let program = program(vec![
        Template::new(
            Some(p.clone()),
            vec![pattern("absent", vec![])],
            vec![],
            vec![q.clone()],
            vec![],
        ),
        Template::new(Some(q), vec![], vec![], vec![p], vec![]),
    ]);
    let expected = BTreeSet::from([vec![atom("q", vec![])]]);
    assert_eq!(accepted_models(&program, true), expected);
    assert_eq!(accepted_models(&program, false), expected);
}

#[test]
fn a_negative_constraint_still_refuses_an_unsupported_required_atom() {
    let program = program(vec![Template::new(
        None,
        vec![],
        vec![],
        vec![pattern("a", vec![])],
        vec![],
    )]);
    assert!(accepted_models(&program, true).is_empty());
    assert!(accepted_models(&program, false).is_empty());
}

#[test]
fn the_narrowing_never_offers_a_gate_atom_no_rule_derives() {
    // s waits on t, which nothing derives, so under no seed does a rule
    // derive s: the counter omits it, and the two seeds offered decide p
    // against q.
    let (seeds, statistics) = narrowed(&gated_program());
    assert_eq!(seeds.len(), 2);
    assert!(seeds.iter().all(|seed| !seed.contains(&atom("s", vec![]))));
    assert_eq!(statistics.cut_gate_atoms, 1);
}

#[test]
fn the_narrowing_holds_a_gate_atom_the_gate_free_rules_derive() {
    // s is a fact and a gate, so every answer set holds it: the counter
    // holds it in both seeds offered instead of counting it.
    let (seeds, statistics) = narrowed(&held_program());
    assert_eq!(seeds.len(), 2);
    assert!(seeds.iter().all(|seed| seed.contains(&atom("s", vec![]))));
    assert_eq!(statistics.held_gate_atoms, 1);
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
            let checked =
                check(program, &seed, Limits::default(), &Cancellation::default()).unwrap();
            (seed.atoms().len(), checked.accepted())
        })
        .collect()
}

#[test]
fn the_unbounded_counter_enumerates_the_whole_symbolic_carrier() {
    // The symbolic carrier holds blocked(1..4) and r(1..4): eight atoms and
    // 256 seeds, of which one is the answer.
    let program = blocked_program();
    let candidates = Candidates::new(
        &program,
        CandidateLimits::default(),
        Cancellation::default(),
    );
    let seeds = enumerate(&program, candidates);
    assert_eq!(seeds.len(), 256);
    assert_eq!(seeds.iter().filter(|(_, accepted)| *accepted).count(), 1);
}

#[test]
fn a_resource_stop_while_bounding_leaves_the_symbolic_carrier() {
    // One unit of work cannot compute a closure; the counter then runs over
    // all eight symbolic gate atoms and the receipt names the stop.
    let program = blocked_program();
    let mut candidates = Candidates::new(
        &program,
        CandidateLimits::default(),
        Cancellation::default(),
    );
    candidates.bounded(Limits {
        max_work: 1,
        ..Limits::default()
    });
    assert_eq!(candidates.by_ref().map(Result::unwrap).count(), 256);
    let statistics = candidates.statistics();
    assert_eq!(
        statistics.narrowing_stop,
        Some(zetesis_cpu::Stop::WorkLimit)
    );
    assert_eq!(statistics.cut_gate_atoms, 0);
    assert_eq!(statistics.held_gate_atoms, 0);
}

#[test]
fn a_cancelled_bound_stops_the_first_pull() {
    let program = blocked_program();
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let mut candidates = Candidates::new(&program, CandidateLimits::default(), cancellation);
    candidates.bounded(Limits::default());
    assert!(matches!(
        candidates.next(),
        Some(Err(zetesis_cpu::Stop::Cancelled))
    ));
}

#[test]
fn the_narrowing_decides_the_blocked_program_outright() {
    // The first pass holds blocked(2) and omits the other blocked atoms; the
    // second reads those decisions, so r(1), r(3) and r(4) are necessary and
    // r(2) underivable. Nothing is left to count: one seed, the answer.
    let program = blocked_program();
    let mut candidates = Candidates::new(
        &program,
        CandidateLimits::default(),
        Cancellation::default(),
    );
    candidates.bounded(Limits::default());
    let seeds = enumerate(&program, candidates);
    assert_eq!(seeds, [(4, true)]);
}

#[test]
fn a_program_without_gate_predicates_computes_no_closure_for_its_bounds() {
    // No work is admitted, yet no stop is recorded: with an empty carrier
    // the counter yields its one seed without asking for either closure.
    let program = program(vec![
        fact("d", vec![number(1)]),
        Template::new(
            Some(pattern("r", vec![Term::Variable(0)])),
            vec![pattern("d", vec![Term::Variable(0)])],
            vec![],
            vec![],
            vec![],
        ),
    ]);
    let mut candidates = Candidates::new(
        &program,
        CandidateLimits::default(),
        Cancellation::default(),
    );
    candidates.bounded(Limits {
        max_work: 0,
        ..Limits::default()
    });
    assert_eq!(candidates.by_ref().map(Result::unwrap).count(), 1);
    assert_eq!(candidates.statistics().narrowing_stop, None);
}

/// Eight nodes with a bad third one, as in the stratified family: blocked
/// and reach are gate predicates with eight symbolic atoms each.
fn stratified_program() -> Program {
    let mut templates: Vec<Template> = (1..=8)
        .map(|node| fact("node", vec![number(node)]))
        .collect();
    templates.push(fact("bad", vec![number(3)]));
    for node in 1..8 {
        templates.push(fact("e", vec![number(node), number(node + 1)]));
        templates.push(fact("next", vec![number(node), number(node + 1)]));
    }
    for node in 1..7 {
        templates.push(fact("e", vec![number(node), number(node + 2)]));
    }
    let (x, y) = (Term::Variable(0), Term::Variable(1));
    templates.push(Template::new(
        Some(pattern("blocked", vec![y.clone()])),
        vec![
            pattern("bad", vec![x.clone()]),
            pattern("next", vec![x.clone(), y.clone()]),
        ],
        vec![],
        vec![],
        vec![],
    ));
    templates.push(fact("reach", vec![number(1)]));
    templates.push(Template::new(
        Some(pattern("reach", vec![y.clone()])),
        vec![
            pattern("reach", vec![x.clone()]),
            pattern("e", vec![x, y.clone()]),
        ],
        vec![],
        vec![pattern("blocked", vec![y])],
        vec![],
    ));
    templates.push(Template::new(
        None,
        vec![],
        vec![],
        vec![pattern("reach", vec![number(8)])],
        vec![],
    ));
    program(templates)
}

#[test]
fn the_narrowing_decides_a_stratified_program_in_three_passes() {
    // Pass one: blocked(4) and reach(1) are necessary, seven blocked atoms
    // underivable. Pass two reads those decisions: reach(2), reach(3) and
    // reach(5..8) are necessary and reach(4) is underivable. Pass three
    // changes nothing, and the one seed is the answer.
    let program = stratified_program();
    let mut candidates = Candidates::new(
        &program,
        CandidateLimits::default(),
        Cancellation::default(),
    );
    candidates.bounded(Limits::default());
    let seeds = enumerate(&program, candidates);
    assert_eq!(seeds, [(8, true)]);
}

/// The statistics of the stratified program's narrowing, after its one seed.
fn stratified_statistics() -> zetesis_cpu::CandidateStatistics {
    let program = stratified_program();
    let mut candidates = Candidates::new(
        &program,
        CandidateLimits::default(),
        Cancellation::default(),
    );
    candidates.bounded(Limits::default());
    assert_eq!(candidates.by_ref().map(Result::unwrap).count(), 1);
    candidates.statistics()
}

#[test]
fn the_narrowing_reports_its_passes() {
    let statistics = stratified_statistics();
    assert_eq!(statistics.narrowing_passes, 3);
    assert!(!statistics.root_refuted);
    assert_eq!(statistics.narrowing_stop, None);
}

#[test]
fn the_narrowing_reports_its_decisions() {
    let statistics = stratified_statistics();
    assert_eq!(statistics.held_gate_atoms, 8);
    assert_eq!(statistics.cut_gate_atoms, 8);
}

#[test]
fn a_definite_constraint_refutes_the_whole_carrier() {
    // d(1..3). :- not d(4).  No rule derives d(4), so after the first pass
    // the constraint's gate holds under every seed and it fires in the lower
    // closure: no seed is offered, and no answer set exists.
    let mut templates: Vec<Template> = (1..=3)
        .map(|value| fact("d", vec![number(value)]))
        .collect();
    templates.push(Template::new(
        None,
        vec![],
        vec![],
        vec![pattern("d", vec![number(4)])],
        vec![],
    ));
    let program = program(templates);
    let mut candidates = Candidates::new(
        &program,
        CandidateLimits::default(),
        Cancellation::default(),
    );
    candidates.bounded(Limits::default());
    assert_eq!(candidates.by_ref().count(), 0);
    let statistics = candidates.statistics();
    assert!(statistics.root_refuted);
    assert_eq!(statistics.narrowing_passes, 1);
    // The unbounded counter finds the same absence the long way.
    let unbounded = Candidates::new(
        &program,
        CandidateLimits::default(),
        Cancellation::default(),
    );
    assert!(
        enumerate(&program, unbounded)
            .iter()
            .all(|(_, accepted)| !accepted)
    );
}
