//! Conformance of the packed static oracle to the independent lazy join oracle.

use std::num::NonZeroUsize;
use std::time::Instant;

use zetesis_core::{
    AdmissionLimits, Atom, AtomPattern, Filter, GroundProgram, Predicate, Program, Seed,
    StaticLimits, Template, Term, Value,
};
use zetesis_cpu::{
    BatchError, BatchOracle, CandidateLimits, Candidates, Control, Limits, StaticCheck, Stop,
    check, check_static,
};

fn pattern(name: &str, terms: Vec<Term>) -> AtomPattern {
    AtomPattern::new(Predicate::new(name, terms.len()).unwrap(), terms).unwrap()
}

fn program(rules: Vec<Template>) -> Program {
    Program::new(rules, AdmissionLimits::default()).unwrap()
}

fn compile(source: &Program) -> GroundProgram {
    GroundProgram::compile(source, StaticLimits::default()).unwrap()
}

fn empty(source: &Program) -> Seed {
    Seed::new(source, []).unwrap()
}

fn run(graph: &GroundProgram, seed: &Seed) -> StaticCheck {
    check_static(graph, seed, Limits::default(), &Control::default()).unwrap()
}

fn compare(graph: &GroundProgram, seed: &Seed) {
    let dense = run(graph, seed);
    let lazy = check(
        graph.program(),
        seed,
        Limits::default(),
        &Control::default(),
    )
    .unwrap();
    assert_eq!(
        graph.model_from_words(dense.closure_words()).unwrap(),
        *lazy.closure()
    );
    assert_eq!(dense.accepted(), lazy.accepted());
    assert_eq!(dense.constraint_violated(), lazy.constraint_violated());
    assert_eq!(dense.seed_mismatch(), lazy.seed_mismatch());
    assert_eq!(
        dense.statistics().derived_atoms,
        lazy.closure().atoms().len()
    );
}

fn tiny_rule(head: usize, body: u32, mut gates: u32) -> Template {
    let atoms = [pattern("a", vec![]), pattern("b", vec![])];
    let positive = atoms
        .iter()
        .enumerate()
        .filter(|(index, _)| body & (1 << index) != 0)
        .map(|(_, atom)| atom.clone())
        .collect();
    let mut gate_true = Vec::new();
    let mut gate_false = Vec::new();
    for atom in &atoms {
        match gates % 3 {
            1 => gate_true.push(atom.clone()),
            2 => gate_false.push(atom.clone()),
            _ => {}
        }
        gates /= 3;
    }
    Template::new(
        head.checked_sub(1).map(|index| atoms[index].clone()),
        positive,
        gate_true,
        gate_false,
        vec![],
    )
}

fn compare_all(source: &Program) {
    let graph = compile(source);
    for seed in Candidates::new(source, CandidateLimits::default(), Control::default()) {
        compare(&graph, &seed.unwrap());
    }
}

#[test]
fn exhaustive_two_atom_programs_match_lazy_reduct_checks() {
    let rules: Vec<_> = (0..3)
        .flat_map(|head| {
            (0..4).flat_map(move |body| (0..9).map(move |gates| tiny_rule(head, body, gates)))
        })
        .collect();
    let mut count = 1;
    compare_all(&program(vec![]));
    for (index, first) in rules.iter().enumerate() {
        compare_all(&program(vec![first.clone()]));
        count += 1;
        for second in &rules[index..] {
            compare_all(&program(vec![first.clone(), second.clone()]));
            count += 1;
        }
    }
    assert_eq!(count, 5_995);
}

fn numbered(index: i32) -> AtomPattern {
    pattern("p", vec![Term::Constant(Value::Number(index))])
}

fn number_atom(index: i32) -> Atom {
    Atom::new(Predicate::new("p", 1).unwrap(), vec![Value::Number(index)]).unwrap()
}

fn choices(count: i32) -> Program {
    program(
        (0..count)
            .map(|index| {
                Template::new(
                    Some(numbered(index)),
                    vec![],
                    vec![numbered(index)],
                    vec![],
                    vec![],
                )
            })
            .collect(),
    )
}

#[test]
fn word_boundaries_and_tail_bits_match_exact_seed_closures() {
    for count in [1, 31, 32, 33, 63, 64, 65, 70] {
        let source = choices(count);
        let graph = compile(&source);
        let selected = [0, 31, 32, 63, 64, count - 1]
            .into_iter()
            .filter(|index| *index < count)
            .map(number_atom);
        for seed in [
            empty(&source),
            Seed::new(&source, selected).unwrap(),
            Seed::new(&source, (0..count).map(number_atom)).unwrap(),
        ] {
            compare(&graph, &seed);
            let dense = run(&graph, &seed);
            assert!(dense.accepted());
            assert_eq!(dense.closure_words(), graph.seed_words(&seed).unwrap());
        }
    }
}

#[test]
fn filtered_out_gate_atoms_and_contradictory_gates_are_rejected_exactly() {
    let source = program(vec![
        Template::new(
            Some(numbered(0)),
            vec![],
            vec![numbered(1)],
            vec![],
            vec![Filter::Eq(
                Term::Constant(Value::Number(0)),
                Term::Constant(Value::Number(1)),
            )],
        ),
        Template::new(
            Some(numbered(0)),
            vec![],
            vec![numbered(0)],
            vec![numbered(0)],
            vec![],
        ),
    ]);
    compare_all(&source);
    let graph = compile(&source);
    let unsupported = Seed::new(&source, [number_atom(1)]).unwrap();
    assert!(run(&graph, &unsupported).seed_mismatch());
    assert!(run(&graph, &empty(&source)).accepted());
}

#[test]
fn constraints_preserve_full_closure_and_empty_constraint_rejects() {
    let source = program(vec![
        Template::new(None, vec![], vec![], vec![], vec![]),
        Template::new(Some(numbered(1)), vec![numbered(0)], vec![], vec![], vec![]),
        Template::new(Some(numbered(0)), vec![], vec![], vec![], vec![]),
    ]);
    let graph = compile(&source);
    compare(&graph, &empty(&source));
    let result = run(&graph, &empty(&source));
    assert!(result.constraint_violated());
    assert_eq!(result.statistics().derived_atoms, 2);
    assert_eq!(result.statistics().passes, 3);

    let source = program(vec![Template::new(None, vec![], vec![], vec![], vec![])]);
    let graph = compile(&source);
    let result = run(&graph, &empty(&source));
    assert!(result.constraint_violated());
    assert!(!result.accepted());
    assert!(result.closure_words().is_empty());
}

#[test]
fn limits_identity_and_control_remain_incomplete_stops() {
    let source = choices(33);
    let graph = compile(&source);
    let seed = Seed::new(&source, (0..33).map(number_atom)).unwrap();
    let limits = Limits::default();
    let completed = run(&graph, &seed);
    let exact = Limits {
        max_work: completed.statistics().work,
        max_derived_atoms: 33,
        ..Limits::default()
    };
    assert!(check_static(&graph, &seed, exact, &Control::default()).is_ok());
    assert_eq!(
        check_static(
            &graph,
            &seed,
            Limits {
                max_work: exact.max_work - 1,
                ..exact
            },
            &Control::default(),
        )
        .unwrap_err(),
        Stop::WorkLimit
    );
    assert_eq!(
        check_static(
            &graph,
            &seed,
            Limits {
                max_derived_atoms: 32,
                ..limits
            },
            &Control::default(),
        )
        .unwrap_err(),
        Stop::DerivedAtomLimit
    );
    let foreign = empty(&choices(33));
    assert_eq!(
        check_static(&graph, &foreign, limits, &Control::default()).unwrap_err(),
        Stop::WrongProgram
    );
    let cancelled = Control::default();
    cancelled.cancel();
    assert_eq!(
        check_static(&graph, &seed, limits, &cancelled).unwrap_err(),
        Stop::Cancelled
    );
    let expired = Control::with_deadline(Instant::now());
    assert_eq!(
        check_static(&graph, &seed, limits, &expired).unwrap_err(),
        Stop::Deadline
    );
    let source = program(vec![]);
    assert!(
        check_static(
            &compile(&source),
            &empty(&source),
            Limits {
                max_work: 0,
                max_derived_atoms: 0,
                ..Limits::default()
            },
            &Control::default(),
        )
        .unwrap()
        .accepted()
    );
}

#[test]
fn batch_preserves_input_order_and_individual_stops() {
    let source = choices(33);
    let graph = compile(&source);
    let seeds = vec![
        Seed::new(&source, [number_atom(32)]).unwrap(),
        empty(&source),
        empty(&choices(33)),
        Seed::new(&source, [number_atom(0), number_atom(31)]).unwrap(),
    ];
    let pool =
        BatchOracle::new(NonZeroUsize::new(2).unwrap(), NonZeroUsize::new(4).unwrap()).unwrap();
    let results = pool
        .check_static_batch(&graph, &seeds, Limits::default(), &Control::default())
        .unwrap();
    assert_eq!(results.len(), seeds.len());
    for (index, result) in results.into_iter().enumerate() {
        if index == 2 {
            assert_eq!(result.unwrap_err(), Stop::WrongProgram);
        } else {
            assert_eq!(
                result.unwrap().closure_words(),
                graph.seed_words(&seeds[index]).unwrap()
            );
        }
    }
    let over_capacity = vec![empty(&source); 5];
    assert!(matches!(
        pool.check_static_batch(
            &graph,
            &over_capacity,
            Limits::default(),
            &Control::default()
        ),
        Err(BatchError::Capacity {
            limit: 4,
            actual: 5
        })
    ));
}
