//! Regions replace the flat count: a narrowed region is refuted by a definite
//! constraint, decided outright, split on its highest undecided atom, or
//! counted when its narrowing decided nothing beyond the split.

use zetesis_core::{Atom, Program, Seed, Template, Term};
use zetesis_cpu::{Cancellation, CandidateLimits, Candidates, Limits, Stop, check};

#[path = "support/programs.rs"]
mod programs;

use programs::{fact, number, pattern, program};

/// node(1..n). in(X) :- node(X), not out(X). out(X) :- node(X), not in(X).
/// With `path`, edge(i,i+1) and :- edge(X,Y), in(X), in(Y).
fn independent(nodes: i32, path: bool) -> Program {
    let mut templates: Vec<Template> = (1..=nodes)
        .map(|node| fact("node", vec![number(node)]))
        .collect();
    let x = Term::Variable(0);
    let y = Term::Variable(1);
    for (head, gate) in [("in", "out"), ("out", "in")] {
        templates.push(Template::new(
            Some(pattern(head, vec![x.clone()])),
            vec![pattern("node", vec![x.clone()])],
            vec![],
            vec![pattern(gate, vec![x.clone()])],
            vec![],
        ));
    }
    if path {
        for node in 1..nodes {
            templates.push(fact("edge", vec![number(node), number(node + 1)]));
        }
        templates.push(Template::new(
            None,
            vec![
                pattern("edge", vec![x.clone(), y.clone()]),
                pattern("in", vec![x]),
                pattern("in", vec![y]),
            ],
            vec![],
            vec![],
            vec![],
        ));
    }
    program(templates)
}

fn spelled(seed: &Seed) -> Vec<String> {
    let mut atoms: Vec<String> = seed
        .atoms()
        .iter()
        .map(|atom: &Atom| format!("{}{:?}", atom.predicate().name(), atom.values()))
        .collect();
    atoms.sort();
    atoms
}

/// Every seed the bounded counter offers, with its verdict.
fn offered(program: &Program) -> (Vec<(Vec<String>, bool)>, zetesis_cpu::CandidateStatistics) {
    let mut candidates =
        Candidates::new(program, CandidateLimits::default(), Cancellation::default());
    candidates.bounded(Limits::default());
    let seeds = candidates
        .by_ref()
        .map(|seed| {
            let seed = seed.unwrap();
            let checked =
                check(program, &seed, Limits::default(), &Cancellation::default()).unwrap();
            (spelled(&seed), checked.accepted())
        })
        .collect();
    (seeds, candidates.statistics())
}

#[test]
fn each_pair_is_decided_by_one_split() {
    // Deciding out(i) decides in(i) at once, so the tree has one split per
    // node and every leaf is an answer set: eight seeds, no count, no refutation.
    let program = independent(3, false);
    let (seeds, statistics) = offered(&program);
    assert_eq!(seeds.len(), 8);
    assert!(seeds.iter().all(|(_, accepted)| *accepted));
    assert_eq!(statistics.regions, 1 + 2 + 4 + 8);
    assert_eq!(statistics.regions_leaves, 8);
    assert_eq!(statistics.regions_counted, 0);
    assert_eq!(statistics.regions_refuted, 0);
    assert!(statistics.region_passes >= 14);
}

#[test]
fn an_edge_constraint_refutes_a_region_before_its_leaves() {
    // Two adjacent nodes held in fire the constraint in the region's lower
    // closure, so the region is refuted without a leaf: the five independent
    // sets of the path are the only seeds offered, and all are accepted.
    let program = independent(3, true);
    let (seeds, statistics) = offered(&program);
    assert_eq!(seeds.len(), 5);
    assert!(seeds.iter().all(|(_, accepted)| *accepted));
    assert!(statistics.regions_refuted >= 1);
    assert_eq!(statistics.regions_leaves, 5);
}

#[test]
fn leaves_come_in_the_counters_order() {
    // The split visits the out branch of the highest atom first, which is the
    // order in which the plain counter reaches the same accepted seeds.
    let program = independent(4, true);
    let (offered, _) = offered(&program);
    let counted: Vec<Vec<String>> = Candidates::new(
        &program,
        CandidateLimits::default(),
        Cancellation::default(),
    )
    .map(|seed| seed.unwrap())
    .filter(|seed| {
        check(&program, seed, Limits::default(), &Cancellation::default())
            .unwrap()
            .accepted()
    })
    .map(|seed| spelled(&seed))
    .collect();
    let leaves: Vec<Vec<String>> = offered.into_iter().map(|(seed, _)| seed).collect();
    assert_eq!(leaves, counted);
}

#[test]
fn a_region_the_narrowing_leaves_undecided_is_counted() {
    // p :- not q, not r.  q :- not p, not r.  r :- not p, not q.
    // Splitting on r: with r out, nothing becomes definite or impossible, so
    // that region is counted over p and q (four seeds); with r in, p and q
    // become impossible and the region is decided: the leaf {r}.
    let atom = |name: &str| pattern(name, vec![]);
    let program = program(vec![
        Template::new(
            Some(atom("p")),
            vec![],
            vec![],
            vec![atom("q"), atom("r")],
            vec![],
        ),
        Template::new(
            Some(atom("q")),
            vec![],
            vec![],
            vec![atom("p"), atom("r")],
            vec![],
        ),
        Template::new(
            Some(atom("r")),
            vec![],
            vec![],
            vec![atom("p"), atom("q")],
            vec![],
        ),
    ]);
    let (seeds, statistics) = offered(&program);
    assert_eq!(seeds.len(), 4 + 1);
    assert_eq!(
        seeds.iter().filter(|(_, accepted)| *accepted).count(),
        3,
        "{{p}}, {{q}} and {{r}}"
    );
    assert_eq!(statistics.regions, 3);
    assert_eq!(statistics.regions_counted, 1);
    assert_eq!(statistics.regions_leaves, 1);
}

#[test]
fn the_candidate_limit_counts_leaves_and_counted_seeds_alike() {
    let program = independent(3, false);
    let limits = CandidateLimits {
        max_candidates: 5,
        ..CandidateLimits::default()
    };
    let mut candidates = Candidates::new(&program, limits, Cancellation::default());
    candidates.bounded(Limits::default());
    let outcomes: Vec<_> = candidates.by_ref().map(|seed| seed.map(|_| ())).collect();
    assert_eq!(outcomes.len(), 6);
    assert!(outcomes[..5].iter().all(Result::is_ok));
    assert!(matches!(outcomes[5], Err(Stop::CandidateLimit)));
}
