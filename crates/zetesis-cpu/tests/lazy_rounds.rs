//! Independent closure comparisons and adversarial immutable-round protocols.
use zetesis_core::{
    AdmissionLimits, Atom, AtomPattern, GroundProgram, Model, Predicate, Program, Seed,
    StaticLimits, Template, Term, Value,
};
use zetesis_cpu::{Control, Limits, Stop, check, check_static, lazy, source};

fn pattern(name: &str, terms: Vec<Term>) -> AtomPattern {
    AtomPattern::new(Predicate::new(name, terms.len()).unwrap(), terms).unwrap()
}
fn nullary(name: &str) -> AtomPattern {
    pattern(name, vec![])
}
fn atom(name: &str) -> Atom {
    Atom::new(Predicate::new(name, 0).unwrap(), vec![]).unwrap()
}
fn rule(
    head: Option<AtomPattern>,
    positive: Vec<AtomPattern>,
    yes: Vec<AtomPattern>,
    no: Vec<AtomPattern>,
) -> Template {
    Template::new(head, positive, yes, no, vec![])
}
fn program(rules: Vec<Template>) -> Program {
    Program::new(rules, AdmissionLimits::default()).unwrap()
}
fn model(names: &[&str]) -> Model {
    Model::new(names.iter().map(|name| atom(name)))
}

fn separated_worlds() -> (Program, Vec<Seed>) {
    let program = program(vec![
        rule(Some(nullary("a")), vec![], vec![], vec![nullary("b")]),
        rule(Some(nullary("b")), vec![], vec![], vec![nullary("a")]),
        rule(Some(nullary("x")), vec![nullary("a")], vec![], vec![]),
        rule(Some(nullary("y")), vec![nullary("b")], vec![], vec![]),
        rule(
            Some(nullary("cross")),
            vec![nullary("x"), nullary("y")],
            vec![],
            vec![],
        ),
        rule(None, vec![nullary("cross")], vec![], vec![]),
    ]);
    let seeds = [&["a"][..], &["b"][..], &["a"][..]]
        .map(|names| Seed::new(&program, model(names).atoms().iter().cloned()).unwrap())
        .to_vec();
    (program, seeds)
}

#[test]
fn union_membership_never_establishes_world_truth() {
    let (program, seeds) = separated_worlds();
    let result = lazy::check_with(
        &program,
        &seeds,
        lazy::Limits::default(),
        &Control::default(),
        lazy::evaluate,
    )
    .unwrap();
    assert_eq!(result.checks.len(), 3);
    for (check, expected) in
        result
            .checks
            .iter()
            .zip([model(&["a", "x"]), model(&["b", "y"]), model(&["a", "x"])])
    {
        assert_eq!(*check.closure(), expected);
        assert!(check.accepted());
    }
    assert_eq!(result.progress.rounds, 3);
    assert!(result.progress.instances > 0);
}

#[test]
fn chunk_boundaries_preserve_exact_cpu_closures() {
    let (program, _) = separated_worlds();
    let seeds = [&[][..], &["a"], &["b"], &["a", "b"]]
        .map(|names| Seed::new(&program, model(names).atoms().iter().cloned()).unwrap());
    let graph = GroundProgram::compile(&program, StaticLimits::default()).unwrap();
    for chunk in [1, 2, 3, 5, 6, 7, 32] {
        let limits = lazy::Limits {
            max_chunk_rules: chunk,
            ..lazy::Limits::default()
        };
        let actual = lazy::check_with(
            &program,
            &seeds,
            limits,
            &Control::default(),
            lazy::evaluate,
        )
        .unwrap();
        for (actual, seed) in actual.checks.iter().zip(&seeds) {
            let expected = check(&program, seed, Limits::default(), &Control::default()).unwrap();
            let dense = check_static(&graph, seed, Limits::default(), &Control::default()).unwrap();
            assert_eq!(actual.closure(), expected.closure());
            assert_eq!(
                (
                    actual.accepted(),
                    actual.seed_mismatch(),
                    actual.constraint_violated()
                ),
                (
                    expected.accepted(),
                    expected.seed_mismatch(),
                    expected.constraint_violated()
                )
            );
            assert_eq!(
                actual.closure(),
                &graph.model_from_words(dense.closure_words()).unwrap()
            );
        }
    }
}

#[test]
fn failed_final_chunk_never_publishes_a_batch() {
    let program = program(vec![rule(Some(nullary("a")), vec![], vec![], vec![])]);
    let seeds = [Seed::new(&program, []).unwrap()];
    let failure = lazy::check_with(
        &program,
        &seeds,
        lazy::Limits::default(),
        &Control::default(),
        |_| Err::<Vec<u32>, _>("device failure"),
    )
    .unwrap_err();
    assert!(matches!(
        failure.cause,
        lazy::Cause::Execution("device failure")
    ));
    assert_eq!(failure.progress.rounds, 0);
    assert_eq!(failure.progress.instances, 1);
    assert_eq!(failure.progress.chunks, 0);
}

#[test]
fn source_stop_discards_successful_pending_chunks() {
    let (program, seeds) = separated_worlds();
    let mut calls = 0;
    let control = Control::default();
    let failure = lazy::check_with(
        &program,
        &seeds,
        lazy::Limits {
            max_chunk_rules: 1,
            ..lazy::Limits::default()
        },
        &control,
        |chunk| {
            calls += 1;
            let result = lazy::evaluate(chunk);
            if calls == 1 {
                control.cancel();
            }
            result
        },
    )
    .unwrap_err();
    assert!(matches!(
        failure.cause,
        lazy::Cause::Source(Stop::Cancelled)
    ));
    assert_eq!(failure.progress.rounds, 0);
    assert_eq!(calls, 1);
}

#[test]
fn a_seed_atom_is_not_a_derived_fact() {
    let program = program(vec![rule(
        Some(nullary("a")),
        vec![nullary("a")],
        vec![nullary("a")],
        vec![],
    )]);
    let seeds = [Seed::new(&program, [atom("a")]).unwrap()];
    let result = lazy::check_with(
        &program,
        &seeds,
        lazy::Limits::default(),
        &Control::default(),
        lazy::evaluate,
    )
    .unwrap();
    assert_eq!(result.checks[0].closure(), &Model::default());
    assert!(result.checks[0].seed_mismatch());
    assert_eq!(result.progress.chunks, 0);
}

#[test]
fn fresh_rounds_revisit_newly_derived_rows() {
    let p = |term| pattern("p", vec![term]);
    let program = program(vec![
        rule(
            Some(p(Term::Constant(Value::Number(3)))),
            vec![],
            vec![],
            vec![],
        ),
        rule(
            Some(pattern("q", vec![Term::Variable(0)])),
            vec![p(Term::Variable(0))],
            vec![],
            vec![],
        ),
    ]);
    let seed = Seed::new(&program, []).unwrap();
    let result = lazy::check_with(
        &program,
        &[seed],
        lazy::Limits::default(),
        &Control::default(),
        lazy::evaluate,
    )
    .unwrap();
    assert_eq!(result.checks[0].closure().atoms().len(), 2);
    assert_eq!(result.progress.rounds, 3);
}

#[test]
fn foreign_seed_identity_is_refused_before_execution() {
    let (program, seeds) = separated_worlds();
    let other = Program::new(program.templates().to_vec(), AdmissionLimits::default()).unwrap();
    let error = lazy::check_with(
        &other,
        &seeds,
        lazy::Limits::default(),
        &Control::default(),
        |_| panic!("foreign input must not dispatch"),
    )
    .unwrap_err();
    assert!(matches!(
        error.cause,
        lazy::Cause::<Stop>::Source(Stop::WrongProgram)
    ));
}

#[test]
fn malformed_output_cannot_be_a_completed_check() {
    let (program, seeds) = separated_worlds();
    let error = lazy::check_with(
        &program,
        &seeds,
        lazy::Limits::default(),
        &Control::default(),
        |_| Ok::<_, Stop>(vec![]),
    )
    .unwrap_err();
    assert!(matches!(error.cause, lazy::Cause::InvalidOutput));
}

#[test]
fn source_exhaustion_requires_successful_last_callback() {
    let program = program(vec![rule(Some(nullary("a")), vec![], vec![], vec![])]);
    let failure = source::scan(
        &program,
        &Model::default(),
        source::ScanLimits::default(),
        &Control::default(),
        |_| Err("last callback"),
    )
    .unwrap_err();
    assert!(matches!(
        failure.cause,
        source::ScanCause::Consumer("last callback")
    ));
    assert_eq!(failure.statistics.bindings, 1);
    assert!(failure.statistics.work > 0);
}

#[test]
fn exact_shared_work_ceiling_preserves_completion() {
    let (program, seeds) = separated_worlds();
    let limits = lazy::Limits::default();
    let expected = lazy::check_with(
        &program,
        &seeds,
        limits,
        &Control::default(),
        lazy::evaluate,
    )
    .unwrap();
    let exact = lazy::Limits {
        max_source_work: expected.progress.source_work,
        ..limits
    };
    assert!(lazy::check_with(&program, &seeds, exact, &Control::default(), lazy::evaluate).is_ok());
    let below = lazy::Limits {
        max_source_work: expected.progress.source_work - 1,
        ..limits
    };
    let error =
        lazy::check_with(&program, &seeds, below, &Control::default(), lazy::evaluate).unwrap_err();
    assert!(matches!(error.cause, lazy::Cause::Source(Stop::WorkLimit)));
    assert_eq!(error.progress.source_work, below.max_source_work);
}

#[test]
fn catalog_limit_counts_underived_atoms() {
    let (program, seeds) = separated_worlds();
    let error = lazy::check_with(
        &program,
        &seeds,
        lazy::Limits {
            max_atoms: 2,
            ..lazy::Limits::default()
        },
        &Control::default(),
        lazy::evaluate,
    )
    .unwrap_err();
    assert!(matches!(
        error.cause,
        lazy::Cause::Source(Stop::CarrierLimit)
    ));
}

#[test]
fn union_truth_mutation_changes_the_expected_models() {
    let (program, seeds) = separated_worlds();
    // Negative control: a deliberately wrong consumer ignores positive truth.
    let mutated = lazy::check_with(
        &program,
        &seeds,
        lazy::Limits::default(),
        &Control::default(),
        |chunk| {
            let mut output = lazy::evaluate(chunk)?;
            for offset in chunk.offsets() {
                let record = &chunk.records()[*offset as usize..];
                if record[0] > 0 && record[1] == 2 {
                    let head = (record[0] - 1) as usize;
                    for world in 0..chunk.worlds() {
                        if chunk.snapshots()[world * chunk.words() + head / 32] & (1 << (head % 32))
                            == 0
                        {
                            output[world * (chunk.words() + 1) + head / 32] |= 1 << (head % 32);
                        }
                    }
                }
            }
            Ok::<_, Stop>(output)
        },
    )
    .unwrap();
    assert!(
        mutated
            .checks
            .iter()
            .all(|check| check.closure().contains(&atom("cross")))
    );
    assert!(mutated.checks.iter().all(|check| !check.accepted()));
}

#[test]
fn instance_scratch_is_reserved_before_catalog_growth() {
    let a = nullary("a");
    let program = program(vec![
        rule(None, vec![], vec![a.clone()], vec![]),
        rule(None, vec![], vec![a.clone(), a], vec![]),
    ]);
    let seeds = [Seed::new(&program, []).unwrap()];
    let atom_bytes = size_of::<Atom>() + 1;
    let fixed_bytes = (5 + 2 + 11 + 2) * size_of::<u32>() + size_of::<lazy::Check>();
    let limits = lazy::Limits {
        max_atoms: 1,
        max_chunk_rules: 2,
        max_chunk_words: 11,
        max_instance_bytes: 2 * atom_bytes,
        max_host_bytes: fixed_bytes + 6 * atom_bytes,
        ..Default::default()
    };
    let completed = lazy::check_with(
        &program,
        &seeds,
        limits,
        &Control::default(),
        lazy::evaluate,
    )
    .unwrap();
    assert_eq!(completed.progress.instances, 2);
    assert_eq!(completed.progress.catalog_atoms, 1);
    let below = lazy::Limits {
        max_host_bytes: limits.max_host_bytes - 1,
        ..limits
    };
    let failure = lazy::check_with(&program, &seeds, below, &Control::default(), |_| {
        panic!("catalog admission must fail before dispatch")
    })
    .unwrap_err();
    assert!(matches!(
        failure.cause,
        lazy::Cause::<Stop>::Source(Stop::Allocation)
    ));
    assert_eq!(failure.progress.instances, 1);
    assert_eq!(failure.progress.chunks, 0);
}

#[test]
fn failed_instance_copy_is_not_an_offered_binding() {
    let program = program(vec![rule(Some(nullary("a")), vec![], vec![], vec![])]);
    let failure = source::scan(
        &program,
        &Model::default(),
        source::ScanLimits {
            max_instance_atoms: 0,
            ..Default::default()
        },
        &Control::default(),
        |_| panic!("copy failed before a callback could be offered"),
    )
    .unwrap_err();
    assert!(matches!(
        failure.cause,
        source::ScanCause::<Stop>::Source(Stop::CarrierLimit)
    ));
    assert_eq!(failure.statistics.bindings, 0);
}

#[test]
fn final_check_metadata_is_reserved_before_execution() {
    let program = program(vec![]);
    let seeds = vec![Seed::new(&program, []).unwrap(); 16];
    let fixed_bytes = (5 * 16 + 2 * 16 + 4 + 1) * size_of::<u32>() + 16 * size_of::<lazy::Check>();
    let limits = lazy::Limits {
        max_atoms: 1,
        max_chunk_rules: 1,
        max_chunk_words: 4,
        max_instance_bytes: 0,
        max_host_bytes: fixed_bytes,
        ..Default::default()
    };
    let complete = lazy::check_with(
        &program,
        &seeds,
        limits,
        &Control::default(),
        lazy::evaluate,
    )
    .unwrap();
    assert_eq!(complete.checks.len(), 16);
    let failure = lazy::check_with(
        &program,
        &seeds,
        lazy::Limits {
            max_host_bytes: fixed_bytes - 1,
            ..limits
        },
        &Control::default(),
        lazy::evaluate,
    )
    .unwrap_err();
    assert!(matches!(
        failure.cause,
        lazy::Cause::Source(Stop::Allocation)
    ));
    assert_eq!(failure.progress, lazy::Progress::default());
}

#[test]
fn catalog_growth_preserves_each_world_interpretation() {
    let (base, _) = separated_worlds();
    let mut rules = base.templates().to_vec();
    rules.extend((0..65).map(|n| {
        rule(
            Some(pattern("fact", vec![Term::Constant(Value::Number(n))])),
            vec![],
            vec![],
            vec![],
        )
    }));
    let program = program(rules);
    let seeds = [
        Seed::new(&program, [atom("a")]).unwrap(),
        Seed::new(&program, [atom("b")]).unwrap(),
    ];
    let mut widths = std::collections::BTreeSet::new();
    let result = lazy::check_with(
        &program,
        &seeds,
        lazy::Limits {
            max_chunk_rules: 7,
            ..Default::default()
        },
        &Control::default(),
        |chunk| {
            widths.insert(chunk.words());
            lazy::evaluate(chunk)
        },
    )
    .unwrap();
    assert_eq!(widths, [1, 2, 4].into_iter().collect());
    for (actual, seed) in result.checks.iter().zip(&seeds) {
        let expected = check(&program, seed, Limits::default(), &Control::default()).unwrap();
        assert_eq!(actual.closure(), expected.closure());
        assert_eq!(actual.accepted(), expected.accepted());
    }
}

#[test]
fn a_large_catalog_ceiling_does_not_allocate_its_carrier() {
    let program = program(vec![rule(Some(nullary("a")), vec![], vec![], vec![])]);
    let seeds = vec![Seed::new(&program, []).unwrap(); 64];
    let result = lazy::check_with(
        &program,
        &seeds,
        lazy::Limits {
            max_atoms: 1_000_000,
            max_host_bytes: 32 * 1024 * 1024,
            ..Default::default()
        },
        &Control::default(),
        |chunk| {
            assert_eq!(chunk.words(), 1);
            lazy::evaluate(chunk)
        },
    )
    .unwrap();
    assert_eq!(result.checks.len(), 64);
    assert_eq!(result.progress.catalog_atoms, 1);
}

#[test]
fn demanded_ids_preserve_closure_across_word_boundaries() {
    for count in [31, 32, 33, 63, 64, 65] {
        let program = program(
            (0..count)
                .map(|n| {
                    rule(
                        Some(pattern("p", vec![Term::Constant(Value::Number(n))])),
                        if n == 0 {
                            vec![]
                        } else {
                            vec![pattern("p", vec![Term::Constant(Value::Number(n - 1))])]
                        },
                        vec![],
                        vec![],
                    )
                })
                .collect(),
        );
        let seeds = vec![Seed::new(&program, []).unwrap(); 3];
        let result = lazy::check_with(
            &program,
            &seeds,
            lazy::Limits {
                max_chunk_rules: 1,
                ..Default::default()
            },
            &Control::default(),
            lazy::evaluate,
        )
        .unwrap();
        assert_eq!(
            result.progress.catalog_atoms,
            usize::try_from(count).unwrap()
        );
        for (actual, seed) in result.checks.iter().zip(&seeds) {
            let expected = check(&program, seed, Limits::default(), &Control::default()).unwrap();
            assert_eq!(actual.closure(), expected.closure());
        }
    }
}

#[test]
fn failed_growth_discards_already_evaluated_deltas() {
    let program = program(
        (0..33)
            .map(|n| rule(Some(nullary(&format!("a{n:02}"))), vec![], vec![], vec![]))
            .collect(),
    );
    let seeds = [Seed::new(&program, []).unwrap()];
    let atom_bytes = size_of::<Atom>() + 3;
    // At the first 32-to-64-bit growth, three old and three new vectors
    // coexist. The conservative six-new-vector preflight is an explicit cap.
    let base = (2 + 4 + 1) * size_of::<u32>() + size_of::<lazy::Check>() + atom_bytes;
    let peak = base + 6 * 2 * size_of::<u32>() + 33 * 4 * atom_bytes;
    let limits = lazy::Limits {
        max_chunk_rules: 1,
        max_chunk_words: 4,
        max_instance_bytes: atom_bytes,
        max_host_bytes: peak,
        ..Default::default()
    };
    let completed = lazy::check_with(
        &program,
        &seeds,
        limits,
        &Control::default(),
        lazy::evaluate,
    )
    .unwrap();
    assert_eq!(completed.checks[0].closure().atoms().len(), 33);
    let failure = lazy::check_with(
        &program,
        &seeds,
        lazy::Limits {
            max_host_bytes: peak - 1,
            ..limits
        },
        &Control::default(),
        lazy::evaluate,
    )
    .unwrap_err();
    assert!(matches!(
        failure.cause,
        lazy::Cause::Source(Stop::Allocation)
    ));
    assert_eq!(failure.progress.rounds, 0);
    assert_eq!(failure.progress.catalog_atoms, 32);
    assert!(failure.progress.chunks > 0);
}
