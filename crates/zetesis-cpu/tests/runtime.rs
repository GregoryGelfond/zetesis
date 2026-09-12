//! Resource, cancellation, batching, and sparse-first behavior independent of
//! the exhaustive semantic comparison in conformance.rs.

use std::num::NonZeroUsize;
use std::time::Instant;

use zetesis_core::{
    AdmissionLimits, Atom, AtomPattern, Predicate, Program, Seed, Template, Term, Value,
};
use zetesis_cpu::{
    BatchError, BatchOracle, CandidateLimits, Candidates, Control, Limits, Stop, check,
};

fn pattern(name: &str, terms: Vec<Term>) -> AtomPattern {
    AtomPattern::new(Predicate::new(name, terms.len()).expect("name"), terms).expect("arity")
}

fn fact(name: &str, terms: Vec<Term>) -> Template {
    Template::new(
        Some(pattern(name, terms)),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
}

fn choice(name: &str) -> Template {
    let head = pattern(name, Vec::new());
    Template::new(
        Some(head.clone()),
        Vec::new(),
        vec![head],
        Vec::new(),
        Vec::new(),
    )
}

fn program(templates: Vec<Template>) -> Program {
    Program::new(templates, AdmissionLimits::default()).expect("admitted templates")
}

fn empty_seed(program: &Program) -> Seed {
    Seed::new(program, []).expect("empty seed")
}

#[test]
fn empty_program_completes_with_zero_work_and_storage() {
    let program = program(Vec::new());
    let result = check(
        &program,
        &empty_seed(&program),
        Limits {
            max_work: 0,
            max_derived_atoms: 0,
        },
        &Control::default(),
    )
    .expect("vacuous coverage");
    assert!(result.accepted());
    assert!(result.closure().atoms().is_empty());
    assert_eq!(result.statistics().work, 0);
    assert_eq!(result.statistics().rounds, 0);
}

#[test]
fn stops_never_return_a_partially_accepted_result() {
    let program = program(vec![fact("p", Vec::new())]);
    let seed = empty_seed(&program);
    for (limits, expected) in [
        (
            Limits {
                max_work: 0,
                ..Limits::default()
            },
            Stop::WorkLimit,
        ),
        (
            Limits {
                max_derived_atoms: 0,
                ..Limits::default()
            },
            Stop::DerivedAtomLimit,
        ),
    ] {
        assert!(
            matches!(check(&program, &seed, limits, &Control::default()), Err(stop) if stop == expected)
        );
    }
    let cancelled = Control::default();
    cancelled.clone().cancel();
    assert!(matches!(
        check(&program, &seed, Limits::default(), &cancelled),
        Err(Stop::Cancelled)
    ));
    assert!(matches!(
        check(
            &program,
            &seed,
            Limits::default(),
            &Control::with_deadline(Instant::now())
        ),
        Err(Stop::Deadline)
    ));
}

#[test]
fn equal_syntax_does_not_authorize_a_foreign_seed() {
    let left = program(vec![choice("p")]);
    let right = program(vec![choice("p")]);
    assert!(matches!(
        check(
            &left,
            &empty_seed(&right),
            Limits::default(),
            &Control::default()
        ),
        Err(Stop::WrongProgram)
    ));
}

#[test]
fn candidate_enumeration_streams_the_binary_powerset_and_finishes_exactly() {
    let program = program(vec![choice("a"), choice("b")]);
    let mut candidates = Candidates::new(
        &program,
        CandidateLimits {
            max_candidates: 4,
            max_carrier_atoms: 2,
        },
        Control::default(),
    );
    let first = candidates.next().expect("first").expect("empty candidate");
    assert!(first.atoms().is_empty());
    assert_eq!(candidates.discovered_atoms(), 0);
    let mut names = vec![Vec::<String>::new()];
    for seed in candidates.by_ref() {
        names.push(
            seed.expect("within exact bounds")
                .atoms()
                .iter()
                .map(|atom| atom.predicate().name().to_owned())
                .collect(),
        );
    }
    assert_eq!(
        names,
        vec![
            vec![],
            vec!["a".to_owned()],
            vec!["b".to_owned()],
            vec!["a".to_owned(), "b".to_owned()]
        ]
    );
    assert_eq!(candidates.discovered_atoms(), 2);
    assert!(candidates.next().is_none());
}

#[test]
fn candidate_limits_and_cancellation_are_terminal_once() {
    let program = program(vec![choice("p")]);
    for (limits, expected) in [
        (
            CandidateLimits {
                max_candidates: 1,
                max_carrier_atoms: 1,
            },
            Stop::CandidateLimit,
        ),
        (
            CandidateLimits {
                max_candidates: 2,
                max_carrier_atoms: 0,
            },
            Stop::CarrierLimit,
        ),
    ] {
        let mut candidates = Candidates::new(&program, limits, Control::default());
        assert!(candidates.next().expect("empty").is_ok());
        assert!(matches!(candidates.next(), Some(Err(stop)) if stop == expected));
        assert!(candidates.next().is_none());
    }
    let control = Control::default();
    let mut candidates = Candidates::new(&program, CandidateLimits::default(), control.clone());
    assert!(candidates.next().expect("empty").is_ok());
    control.cancel();
    assert!(matches!(candidates.next(), Some(Err(Stop::Cancelled))));
    assert!(candidates.next().is_none());
}

#[test]
fn a_large_symbolic_carrier_does_not_delay_the_first_check() {
    let gate_arity = 24;
    let mut templates = vec![
        fact("d", vec![Term::Constant(Value::Number(0))]),
        fact("d", vec![Term::Constant(Value::Number(1))]),
    ];
    templates.push(Template::new(
        None,
        Vec::new(),
        vec![pattern(
            "large",
            vec![Term::Constant(Value::Number(0)); gate_arity],
        )],
        Vec::new(),
        Vec::new(),
    ));
    let program = program(templates);
    let mut candidates = Candidates::new(
        &program,
        CandidateLimits {
            max_candidates: 1,
            max_carrier_atoms: 0,
        },
        Control::default(),
    );
    let seed = candidates
        .next()
        .expect("empty")
        .expect("no carrier needed");
    assert_eq!(candidates.discovered_atoms(), 0);
    // Two source rounds inspect this gate and the two unary fact heads.
    // Key argument scans are charged separately from the existing work budget;
    // none of the exponential symbolic carrier is enumerated.
    let key_scan_work = 2 * u64::try_from(gate_arity + 2).unwrap();
    let result = check(
        &program,
        &seed,
        Limits {
            max_work: 100 + key_scan_work,
            max_derived_atoms: 2,
        },
        &Control::default(),
    )
    .expect("sparse check");
    assert!(result.accepted());
    assert_eq!(result.closure().atoms().len(), 2);
}

#[test]
fn gates_prune_after_their_arguments_are_bound_before_a_cartesian_join() {
    let size = 64_i32;
    let mut templates: Vec<_> = (0..size)
        .map(|value| fact("d", vec![Term::Constant(Value::Number(value))]))
        .collect();
    let selected = pattern("selected", vec![Term::Variable(0)]);
    templates.push(Template::new(
        Some(selected.clone()),
        vec![pattern("d", vec![Term::Variable(0)])],
        vec![selected.clone()],
        Vec::new(),
        Vec::new(),
    ));
    templates.push(Template::new(
        Some(pattern("pair", vec![Term::Variable(0), Term::Variable(1)])),
        vec![
            pattern("d", vec![Term::Variable(0)]),
            pattern("d", vec![Term::Variable(1)]),
        ],
        vec![selected],
        Vec::new(),
        Vec::new(),
    ));
    let program = program(templates);
    let seed = Seed::new(
        &program,
        [Atom::new(
            Predicate::new("selected", 1).expect("name"),
            vec![Value::Number(0)],
        )
        .expect("arity")],
    )
    .expect("carrier");
    let result =
        check(&program, &seed, Limits::default(), &Control::default()).expect("complete check");
    assert!(result.accepted());
    assert_eq!(result.closure().atoms().len(), 129);
    // Compare non-catalog work with the unpruned Cartesian row population.
    // This includes joins, rounds, gates and emission. Catalog construction
    // and comparisons now have their own charged schedule.
    let non_catalog_work = result.statistics().work - result.statistics().catalog_work;
    assert!(
        non_catalog_work < u64::try_from(size * size).unwrap(),
        "{:?}",
        result.statistics()
    );
}

#[test]
fn bounded_rayon_batch_preserves_input_order_and_individual_stops() {
    let program = program(vec![choice("p")]);
    let seeds: Vec<_> = Candidates::new(&program, CandidateLimits::default(), Control::default())
        .collect::<Result<_, _>>()
        .expect("two seeds");
    let workers = NonZeroUsize::new(2).expect("nonzero");
    let batch = BatchOracle::new(workers, workers).expect("owned pool");
    let results = batch
        .check_batch(&program, &seeds, Limits::default(), &Control::default())
        .expect("within batch bound");
    assert_eq!(
        results
            .iter()
            .map(|result| result.as_ref().expect("complete").closure().atoms().len())
            .collect::<Vec<_>>(),
        [0, 1]
    );
    let too_small =
        BatchOracle::new(workers, NonZeroUsize::new(1).expect("nonzero")).expect("owned pool");
    assert!(matches!(
        too_small.check_batch(&program, &seeds, Limits::default(), &Control::default()),
        Err(BatchError::Capacity { .. })
    ));
    let control = Control::default();
    control.cancel();
    let stopped = batch
        .check_batch(&program, &seeds, Limits::default(), &control)
        .expect("admitted batch");
    assert!(
        stopped
            .iter()
            .all(|result| matches!(result, Err(Stop::Cancelled)))
    );
}
