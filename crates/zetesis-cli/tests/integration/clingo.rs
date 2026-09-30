//! Optional independent end-to-end comparison. clingo is a test dependency
//! supplied by the caller, never an execution path of the solver.
use clap::Parser;
use std::collections::BTreeSet;
use zetesis_cli::{Completion, Options, run};
use zetesis_clingo_support as oracle;
use zetesis_cpu::Cancellation;

fn canonical(line: &str) -> BTreeSet<String> {
    // This campaign uses only scalar atoms without strings containing spaces,
    // so each whitespace-delimited token is exactly one displayed atom.
    line.split_whitespace().map(str::to_owned).collect()
}
fn external(source: &str) -> BTreeSet<BTreeSet<String>> {
    let run = oracle::run(source, &["0", "--verbose=0"], oracle::Limits::default());
    let text = std::str::from_utf8(run.stdout()).unwrap();
    let mut lines: Vec<_> = text.lines().collect();
    let status = lines.pop().expect("clingo status line");
    assert!(matches!(status, "SATISFIABLE" | "UNSATISFIABLE"));
    lines.into_iter().map(canonical).collect()
}
fn compare(source: &str) {
    let expected = external(source);
    for search in ["closure", "countermodel"] {
        compare_native(source, search, &expected);
    }
}

fn compare_native(source: &str, search: &str, expected: &BTreeSet<BTreeSet<String>>) {
    let options = Options::try_parse_from([
        "zetesis",
        "--backend",
        "cpu",
        "--oracle",
        search,
        "--models",
        "0",
        "--workers",
        "2",
    ])
    .unwrap();
    let mut bytes = Vec::new();
    let report = run(
        source.into(),
        &options,
        &mut bytes,
        &Cancellation::default(),
    )
    .unwrap();
    assert_eq!(report.completion, Completion::Exhausted, "{source}");
    let text = String::from_utf8(bytes).unwrap();
    let mut lines = text.lines();
    let mut models = BTreeSet::new();
    while let Some(line) = lines.next() {
        if line.starts_with("Answer:") {
            assert!(models.insert(canonical(lines.next().unwrap())));
        }
    }
    assert_eq!(&models, expected, "search {search}, program: {source}");
}

#[test]
#[ignore = "requires an independently installed clingo executable"]
fn common_profile_matches_clingo() {
    let rules = [
        "a.",
        "b :- a.",
        "a :- b.",
        "a :- not b.",
        "b :- not a.",
        "{c}.",
        "c :- not not c.",
        ":- a,b.",
    ];
    for mask in 0..256u16 {
        let source = rules
            .iter()
            .enumerate()
            .filter(|(index, _)| mask & (1 << index) != 0)
            .map(|(_, rule)| *rule)
            .collect::<Vec<_>>()
            .join("\n");
        compare(&source);
    }
    for source in [
        "edge(1,2). edge(2,3). reach(X,Y) :- edge(X,Y). reach(X,Z) :- reach(X,Y), edge(Y,Z).",
        "node(1). node(2). {chosen(X)} :- node(X). :- chosen(1), chosen(2).",
        "d(1). d(2). pair(X,Y) :- d(X), d(Y), X != Y, not off(X,Y). {off(X,Y)} :- d(X), d(Y), X = Y.",
        "p(1,1). p(1,2). same(X) :- p(X,X). any :- p(_,_).",
        "a :- not b. b :- not a. :- not not a, not b.",
        "a :- not not b. b :- a. {b}.",
    ] {
        compare(source);
    }
}

#[test]
#[ignore = "requires an independently installed clingo executable"]
fn rule_excerpts_match_clingo() {
    for source in [
        include_str!("../fixtures/correctness/excerpts/shortest-path-reachable.lp"),
        include_str!("../fixtures/correctness/excerpts/shortest-path-disconnected-cycle-unsat.lp"),
        include_str!("../fixtures/correctness/excerpts/task-allocation-projections.lp"),
    ] {
        compare(source);
    }
}

#[test]
#[ignore = "requires an independently installed clingo executable"]
fn complete_network_repair_example_matches_clingo() {
    let source = include_str!("../../../../examples/network-repair.lp");
    assert_eq!(external(source).len(), 2);
    compare(source);
}
