//! Ordered lazy probes compared with independently materialized static closure.

use zetesis_core::{
    AdmissionLimits, Atom, AtomPattern, Filter, GroundProgram, Predicate, Program, Seed,
    StaticLimits, Template, Term, Value,
};
use zetesis_cpu::{Cancellation, Limits, Stop, check, check_static};

fn pattern(name: &str, terms: Vec<Term>) -> AtomPattern {
    AtomPattern::new(Predicate::new(name, terms.len()).unwrap(), terms).unwrap()
}

fn number(n: i32) -> Term {
    Term::Constant(Value::Number(n))
}

fn fact(name: &str, terms: Vec<Term>) -> Template {
    Template::new(Some(pattern(name, terms)), vec![], vec![], vec![], vec![])
}

fn source(rows: u8, selectors: u8) -> Program {
    let x = Term::Variable(0);
    let y = Term::Variable(1);
    let mut rules = vec![fact("d", vec![number(0)]), fact("d", vec![number(1)])];
    for a in 0..2 {
        if selectors & (1 << a) != 0 {
            rules.push(fact("s", vec![number(a)]));
        }
        for b in 0..2 {
            if rows & (1 << (2 * a + b)) != 0 {
                rules.push(fact("r", vec![number(a), number(b)]));
            }
        }
    }
    rules.extend([
        Template::new(
            Some(pattern("off", vec![x.clone()])),
            vec![pattern("d", vec![x.clone()])],
            vec![pattern("off", vec![x.clone()])],
            vec![],
            vec![],
        ),
        Template::new(
            Some(pattern("different", vec![x.clone(), y.clone()])),
            vec![
                pattern("s", vec![x.clone()]),
                pattern("r", vec![x.clone(), y.clone()]),
            ],
            vec![],
            vec![pattern("off", vec![y.clone()])],
            vec![Filter::Neq(x.clone(), y.clone())],
        ),
        Template::new(
            Some(pattern("diagonal", vec![x.clone()])),
            vec![
                pattern("s", vec![x.clone()]),
                pattern("r", vec![x.clone(), x.clone()]),
            ],
            vec![],
            vec![],
            vec![],
        ),
        Template::new(
            Some(pattern("via", vec![y.clone()])),
            vec![
                pattern("s", vec![x.clone()]),
                pattern("r", vec![x, y.clone()]),
                pattern("r", vec![y, number(0)]),
            ],
            vec![],
            vec![],
            vec![],
        ),
        Template::new(
            None,
            vec![pattern("different", vec![number(0), number(1)])],
            vec![],
            vec![pattern("off", vec![number(0)])],
            vec![],
        ),
    ]);
    Program::new(rules, AdmissionLimits::default()).unwrap()
}

fn seed(source: &Program, bits: u8) -> Seed {
    let atoms = (0..2)
        .filter(|n| bits & (1 << n) != 0)
        .map(|n| Atom::new(Predicate::new("off", 1).unwrap(), vec![Value::Number(n)]).unwrap());
    Seed::new(source, atoms).unwrap()
}

#[test]
fn ordered_joins_preserve_frozen_closures() {
    let mut checked = 0;
    for rows in 0..16 {
        for selectors in 0..4 {
            let source = source(rows, selectors);
            let graph = GroundProgram::compile(&source, StaticLimits::default()).unwrap();
            for bits in 0..4 {
                let seed = seed(&source, bits);
                let lazy =
                    check(&source, &seed, Limits::default(), &Cancellation::default()).unwrap();
                let dense =
                    check_static(&graph, &seed, Limits::default(), &Cancellation::default())
                        .unwrap();
                assert_eq!(
                    lazy.closure(),
                    &graph.model_from_words(dense.closure_words()).unwrap(),
                    "rows={rows}, selectors={selectors}, seed={bits}"
                );
                assert_eq!(lazy.constraint_violated(), dense.constraint_violated());
                assert_eq!(lazy.seed_mismatch(), dense.seed_mismatch());
                assert_eq!(lazy.accepted(), dense.accepted());
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 256);
}

#[test]
fn lookup_stops_do_not_publish_partial_closures() {
    let source = source(15, 3);
    let seed = seed(&source, 2);
    let checked = check(&source, &seed, Limits::default(), &Cancellation::default()).unwrap();
    let work = checked.statistics().work;
    for max_work in [0, 1, work / 2, work - 1] {
        let result = check(
            &source,
            &seed,
            Limits {
                max_work,
                ..Limits::default()
            },
            &Cancellation::default(),
        );
        assert!(matches!(result, Err(Stop::WorkLimit)));
    }
    let exact = check(
        &source,
        &seed,
        Limits {
            max_work: work,
            ..Limits::default()
        },
        &Cancellation::default(),
    )
    .unwrap();
    assert_eq!(exact.closure(), checked.closure());
    assert_eq!(exact.statistics(), checked.statistics());
}

#[test]
fn a_failed_suffix_cannot_poison_later_prefixes() {
    let x = Term::Variable(0);
    let y = Term::Variable(1);
    let source = Program::new(
        vec![
            fact("s", vec![number(0)]),
            fact("s", vec![number(1)]),
            fact("r", vec![number(0), number(0), number(1)]),
            fact("r", vec![number(0), number(1), number(1)]),
            fact("r", vec![number(1), number(0), number(0)]),
            Template::new(
                Some(pattern("keep", vec![x.clone(), y.clone()])),
                vec![
                    pattern("s", vec![x.clone()]),
                    pattern("r", vec![x, y.clone(), y]),
                ],
                vec![],
                vec![],
                vec![],
            ),
        ],
        AdmissionLimits::default(),
    )
    .unwrap();
    let checked = check(
        &source,
        &Seed::new(&source, []).unwrap(),
        Limits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    let actual: Vec<_> = checked
        .closure()
        .atoms()
        .iter()
        .filter(|atom| atom.predicate().name() == "keep")
        .map(|atom| atom.values().to_vec())
        .collect();
    assert_eq!(
        actual,
        vec![
            vec![Value::Number(0), Value::Number(1)],
            vec![Value::Number(1), Value::Number(0)]
        ]
    );
}
