//! World-mask selection against independent per-seed reduct closure.

use zetesis_core::{
    AdmissionLimits, Atom, AtomPattern, Model, Predicate, Program, Seed, Template, Term, Value,
};
use zetesis_cpu::{Control, Limits, Stop, check, lazy};

fn pattern(name: &str, terms: Vec<Term>) -> AtomPattern {
    AtomPattern::new(Predicate::new(name, terms.len()).unwrap(), terms).unwrap()
}

fn constant(name: &str, value: i32) -> AtomPattern {
    pattern(name, vec![Term::Constant(Value::Number(value))])
}

fn atom(name: &str, value: i32) -> Atom {
    Atom::new(Predicate::new(name, 1).unwrap(), vec![Value::Number(value)]).unwrap()
}

fn rule(
    head: Option<AtomPattern>,
    positive: Vec<AtomPattern>,
    gates: Vec<AtomPattern>,
) -> Template {
    Template::new(head, positive, gates, vec![], vec![])
}

fn program(rules: Vec<Template>) -> Program {
    Program::new(rules, AdmissionLimits::default()).unwrap()
}

fn cartesian(size: i32) -> Program {
    let mut rules = Vec::new();
    for value in 0..size {
        let gate = constant("pick", value);
        rules.push(rule(Some(gate.clone()), vec![], vec![gate.clone()]));
        for name in ["a", "b", "c"] {
            rules.push(rule(
                Some(constant(name, value)),
                vec![],
                vec![gate.clone()],
            ));
        }
    }
    rules.push(rule(
        Some(pattern("triple", (0..3).map(Term::Variable).collect())),
        ["a", "b", "c"]
            .into_iter()
            .enumerate()
            .map(|(slot, name)| pattern(name, vec![Term::Variable(slot)]))
            .collect(),
        vec![],
    ));
    program(rules)
}

fn seeds(program: &Program, choices: &[Vec<i32>]) -> Vec<Seed> {
    choices
        .iter()
        .map(|choice| Seed::new(program, choice.iter().map(|value| atom("pick", *value))).unwrap())
        .collect()
}

fn run(program: &Program, seeds: &[Seed], selection: lazy::SourceSelection) -> lazy::Batch {
    lazy::check_with_source(
        program,
        seeds,
        lazy::Limits::default(),
        selection,
        &Control::default(),
        lazy::evaluate,
    )
    .unwrap()
}

fn assert_exact(program: &Program, seeds: &[Seed], batch: &lazy::Batch) {
    assert_eq!(batch.checks.len(), seeds.len());
    for (actual, seed) in batch.checks.iter().zip(seeds) {
        let expected = check(program, seed, Limits::default(), &Control::default()).unwrap();
        assert_eq!(actual.closure(), expected.closure());
        assert_eq!(actual.accepted(), expected.accepted());
        assert_eq!(actual.seed_mismatch(), expected.seed_mismatch());
        assert_eq!(actual.constraint_violated(), expected.constraint_violated());
    }
}

fn report_work(profile: &str, union: &lazy::Batch, masked: &lazy::Batch) {
    for (selection, batch) in [("union", union), ("worlds", masked)] {
        let progress = batch.progress;
        eprintln!(
            "source-work profile={profile} selection={selection} instances={} work={} mask_words={} peak_mask_bytes={} chunks={}",
            progress.instances,
            progress.source_work,
            progress.mask_words,
            progress.peak_mask_bytes,
            progress.chunks
        );
    }
}

#[test]
fn disjoint_worlds_preserve_scalar_closures() {
    let program = cartesian(4);
    let seeds = seeds(&program, &[vec![0], vec![1], vec![2], vec![3]]);
    assert_exact(
        &program,
        &seeds,
        &run(&program, &seeds, lazy::SourceSelection::Worlds),
    );
}

#[test]
fn empty_prefixes_reduce_offered_instances() {
    let program = cartesian(4);
    let seeds = seeds(&program, &[vec![0], vec![1], vec![2], vec![3]]);
    let union = run(&program, &seeds, lazy::SourceSelection::Union);
    let masked = run(&program, &seeds, lazy::SourceSelection::Worlds);
    report_work("disjoint-four", &union, &masked);
    assert!(masked.progress.pruned_prefixes > 0);
    assert!(masked.progress.instances < union.progress.instances);
    assert!(masked.progress.catalog_atoms < union.progress.catalog_atoms);
}

#[test]
fn dense_worlds_keep_every_source_instance() {
    let program = cartesian(3);
    let seeds = seeds(&program, &vec![vec![0, 1, 2]; 3]);
    let masked = run(&program, &seeds, lazy::SourceSelection::Worlds);
    let union = run(&program, &seeds, lazy::SourceSelection::Union);
    report_work("dense-three", &union, &masked);
    assert_exact(&program, &seeds, &masked);
    assert_eq!(masked.progress.pruned_prefixes, 0);
    assert_eq!(masked.progress.instances, union.progress.instances);
}

#[test]
fn one_world_keeps_every_source_instance() {
    let program = cartesian(3);
    let seeds = seeds(&program, &[vec![0, 1, 2]]);
    let masked = run(&program, &seeds, lazy::SourceSelection::Worlds);
    let union = run(&program, &seeds, lazy::SourceSelection::Union);
    report_work("single-three", &union, &masked);
    assert_exact(&program, &seeds, &masked);
    assert_eq!(masked.progress.pruned_prefixes, 0);
    assert_eq!(masked.progress.instances, union.progress.instances);
}

#[test]
fn duplicate_world_occurrences_keep_their_positions() {
    let program = cartesian(3);
    let seeds = seeds(&program, &[vec![2], vec![0], vec![2], vec![], vec![1]]);
    let masked = run(&program, &seeds, lazy::SourceSelection::Worlds);
    assert_exact(&program, &seeds, &masked);
    assert_eq!(masked.checks[0].closure(), masked.checks[2].closure());
    assert_eq!(masked.checks[3].closure(), &Model::default());
}

#[test]
fn world_word_boundaries_preserve_membership() {
    let program = cartesian(2);
    for count in [31, 32, 33, 63, 64, 65] {
        let choices: Vec<_> = (0..count).map(|index| vec![index % 2]).collect();
        let seeds = seeds(&program, &choices);
        assert_exact(
            &program,
            &seeds,
            &run(&program, &seeds, lazy::SourceSelection::Worlds),
        );
    }
}

#[test]
fn an_existing_atom_can_gain_another_world() {
    let mut rules = cartesian(2).templates().to_vec();
    rules.extend([
        rule(Some(constant("shared", 0)), vec![constant("a", 0)], vec![]),
        rule(Some(constant("later", 0)), vec![constant("b", 1)], vec![]),
        rule(
            Some(constant("shared", 0)),
            vec![constant("later", 0)],
            vec![],
        ),
        rule(
            Some(constant("observed", 0)),
            vec![constant("shared", 0), constant("b", 1)],
            vec![],
        ),
    ]);
    let program = program(rules);
    let seeds = seeds(&program, &[vec![0], vec![1]]);
    let result = run(&program, &seeds, lazy::SourceSelection::Worlds);
    assert_exact(&program, &seeds, &result);
    assert!(!result.checks[0].closure().contains(&atom("observed", 0)));
    assert!(result.checks[1].closure().contains(&atom("observed", 0)));
}

#[test]
fn source_masks_do_not_restrict_later_candidates() {
    let program = cartesian(3);
    let carrier = program.gate_atoms().collect::<Result<Vec<_>, _>>().unwrap();
    let first = seeds(&program, &[vec![0], vec![1], vec![2]]);
    run(&program, &first, lazy::SourceSelection::Worlds);
    assert_eq!(
        program.gate_atoms().collect::<Result<Vec<_>, _>>().unwrap(),
        carrier
    );
    let later = seeds(&program, &[vec![0, 1, 2]]);
    let result = run(&program, &later, lazy::SourceSelection::Worlds);
    assert_exact(&program, &later, &result);
    assert_eq!(result.checks[0].closure().atoms().len(), 39);
}

#[test]
fn irregular_chunks_preserve_masked_closures() {
    let program = cartesian(3);
    let seeds = seeds(&program, &[vec![], vec![0], vec![1, 2], vec![0]]);
    for size in [1, 2, 5, 7, 32] {
        let batch = lazy::check_with_source(
            &program,
            &seeds,
            lazy::Limits {
                max_chunk_rules: size,
                ..Default::default()
            },
            lazy::SourceSelection::Worlds,
            &Control::default(),
            lazy::evaluate,
        )
        .unwrap();
        assert_exact(&program, &seeds, &batch);
    }
}

#[test]
fn catalog_stride_growth_keeps_the_source_snapshot() {
    for count in [31, 32, 33, 63, 64, 65] {
        let mut rules = cartesian(2).templates().to_vec();
        rules.extend((0..count).map(|value| {
            rule(
                Some(constant("chain", value)),
                if value == 0 {
                    vec![]
                } else {
                    vec![constant("chain", value - 1)]
                },
                vec![],
            )
        }));
        let program = program(rules);
        let seeds = seeds(&program, &[vec![1], vec![0], vec![1]]);
        assert_exact(
            &program,
            &seeds,
            &run(&program, &seeds, lazy::SourceSelection::Worlds),
        );
    }
}

#[test]
fn mask_work_uses_the_shared_source_quota() {
    let program = cartesian(3);
    let seeds = seeds(&program, &[vec![0], vec![1], vec![2]]);
    let baseline = run(&program, &seeds, lazy::SourceSelection::Worlds);
    assert!(baseline.progress.mask_words > 0);
    assert!(baseline.progress.mask_words < baseline.progress.source_work);
    let exact = lazy::Limits {
        max_source_work: baseline.progress.source_work,
        ..Default::default()
    };
    let repeat = lazy::check_with_source(
        &program,
        &seeds,
        exact,
        lazy::SourceSelection::Worlds,
        &Control::default(),
        lazy::evaluate,
    )
    .unwrap();
    assert_eq!(repeat.progress, baseline.progress);
    let below = lazy::Limits {
        max_source_work: exact.max_source_work - 1,
        ..exact
    };
    let failure = lazy::check_with_source(
        &program,
        &seeds,
        below,
        lazy::SourceSelection::Worlds,
        &Control::default(),
        lazy::evaluate,
    )
    .unwrap_err();
    assert!(matches!(
        failure.cause,
        lazy::Cause::Source(Stop::WorkLimit)
    ));
    assert_eq!(failure.progress.source_work, below.max_source_work);
}

#[test]
fn mask_preparation_can_stop_without_offering_instances() {
    let program = program(vec![]);
    let seeds = vec![Seed::new(&program, []).unwrap(); 33];
    let failure = lazy::check_with_source(
        &program,
        &seeds,
        lazy::Limits {
            max_source_work: 1,
            ..Default::default()
        },
        lazy::SourceSelection::Worlds,
        &Control::default(),
        lazy::evaluate,
    )
    .unwrap_err();
    assert!(matches!(
        failure.cause,
        lazy::Cause::Source(Stop::WorkLimit)
    ));
    assert_eq!(failure.progress.rounds, 0);
    assert_eq!(failure.progress.instances, 0);
    assert_eq!(failure.progress.mask_words, 1);
    assert_eq!(failure.progress.peak_mask_bytes, 2 * size_of::<u32>());
}

#[test]
fn root_membership_storage_has_an_inclusive_host_cap() {
    let program = program(vec![]);
    let seeds = vec![Seed::new(&program, []).unwrap(); 33];
    let fixed = (5 * 33 + 2 * 33 + 4 + 1) * size_of::<u32>() + 33 * size_of::<lazy::Check>();
    let masks = 2 * size_of::<u32>();
    let limits = lazy::Limits {
        max_atoms: 1,
        max_chunk_rules: 1,
        max_chunk_words: 4,
        max_instance_bytes: 0,
        max_host_bytes: fixed + masks,
        ..Default::default()
    };
    let complete = lazy::check_with_source(
        &program,
        &seeds,
        limits,
        lazy::SourceSelection::Worlds,
        &Control::default(),
        lazy::evaluate,
    )
    .unwrap();
    assert_eq!(complete.progress.peak_mask_bytes, masks);
    let failure = lazy::check_with_source(
        &program,
        &seeds,
        lazy::Limits {
            max_host_bytes: limits.max_host_bytes - 1,
            ..limits
        },
        lazy::SourceSelection::Worlds,
        &Control::default(),
        lazy::evaluate,
    )
    .unwrap_err();
    assert!(matches!(
        failure.cause,
        lazy::Cause::Source(Stop::Allocation)
    ));
    assert_eq!(failure.progress.rounds, 0);
    assert_eq!(failure.progress.peak_mask_bytes, 0);
}

#[test]
fn execution_failure_discards_masked_pending_deltas() {
    let program = cartesian(3);
    let seeds = seeds(&program, &[vec![0], vec![1]]);
    let mut calls = 0;
    let failure = lazy::check_with_source(
        &program,
        &seeds,
        lazy::Limits {
            max_chunk_rules: 1,
            ..Default::default()
        },
        lazy::SourceSelection::Worlds,
        &Control::default(),
        |chunk| {
            calls += 1;
            if calls == 3 {
                Err("executor interrupted")
            } else {
                Ok(lazy::evaluate(chunk).unwrap())
            }
        },
    )
    .unwrap_err();
    assert!(matches!(
        failure.cause,
        lazy::Cause::Execution("executor interrupted")
    ));
    assert_eq!(failure.progress.rounds, 0);
    assert_eq!(failure.progress.chunks, 2);
    assert!(failure.progress.mask_words > 0);
}

#[test]
fn cancellation_discards_masked_pending_deltas() {
    let program = cartesian(3);
    let seeds = seeds(&program, &[vec![0], vec![1]]);
    let control = Control::default();
    let mut calls = 0;
    let failure = lazy::check_with_source(
        &program,
        &seeds,
        lazy::Limits {
            max_chunk_rules: 1,
            ..Default::default()
        },
        lazy::SourceSelection::Worlds,
        &control,
        |chunk| {
            calls += 1;
            let output = lazy::evaluate(chunk);
            control.cancel();
            output
        },
    )
    .unwrap_err();
    assert!(matches!(
        failure.cause,
        lazy::Cause::Source(Stop::Cancelled)
    ));
    assert_eq!(failure.progress.rounds, 0);
    // Cancellation precedes readback validation, so the returned executor
    // output has not established a successfully evaluated protocol chunk.
    assert_eq!(calls, 1);
    assert_eq!(failure.progress.chunks, 0);
}

#[test]
fn default_selection_matches_explicit_union() {
    let program = cartesian(2);
    let seeds = seeds(&program, &[vec![0], vec![1]]);
    let old = lazy::check_with(
        &program,
        &seeds,
        lazy::Limits::default(),
        &Control::default(),
        lazy::evaluate,
    )
    .unwrap();
    let union = run(&program, &seeds, lazy::SourceSelection::Union);
    assert_eq!(old.progress, union.progress);
    assert_eq!(old.progress.mask_words, 0);
    assert_eq!(old.progress.pruned_prefixes, 0);
    assert_eq!(old.progress.peak_mask_bytes, 0);
}

#[test]
fn frozen_gates_remain_world_local() {
    let mut rules = cartesian(2).templates().to_vec();
    rules.extend([
        Template::new(
            Some(constant("enabled", 0)),
            vec![constant("a", 0)],
            vec![constant("pick", 0)],
            vec![constant("pick", 1)],
            vec![],
        ),
        rule(None, vec![constant("a", 0), constant("b", 1)], vec![]),
    ]);
    let program = program(rules);
    let seeds = seeds(&program, &[vec![0], vec![1], vec![0, 1]]);
    let result = run(&program, &seeds, lazy::SourceSelection::Worlds);
    assert_exact(&program, &seeds, &result);
    assert!(result.checks[0].closure().contains(&atom("enabled", 0)));
    assert!(!result.checks[2].closure().contains(&atom("enabled", 0)));
}

#[test]
fn empty_positive_constraints_remain_visible() {
    let mut rules = cartesian(2).templates().to_vec();
    rules.push(rule(None, vec![], vec![constant("pick", 0)]));
    let program = program(rules);
    let seeds = seeds(&program, &[vec![0], vec![1]]);
    let result = run(&program, &seeds, lazy::SourceSelection::Worlds);
    assert_exact(&program, &seeds, &result);
    assert!(result.checks[0].constraint_violated());
    assert!(!result.checks[1].constraint_violated());
}

#[test]
fn masks_never_borrow_truth_from_a_seed() {
    let p = constant("p", 0);
    let program = program(vec![rule(Some(p.clone()), vec![p.clone()], vec![p])]);
    let seeds = [Seed::new(&program, [atom("p", 0)]).unwrap()];
    let result = run(&program, &seeds, lazy::SourceSelection::Worlds);
    assert_eq!(result.checks[0].closure(), &Model::default());
    assert!(result.checks[0].seed_mismatch());
    assert_eq!(result.progress.instances, 0);
}

#[test]
fn bound_prefix_windows_select_the_correct_row_masks() {
    use zetesis_core::Filter;
    let mut rules = cartesian(3).templates().to_vec();
    rules.push(Template::new(
        Some(pattern("same", vec![Term::Variable(0)])),
        vec![
            pattern("a", vec![Term::Variable(0)]),
            pattern("b", vec![Term::Variable(0)]),
            pattern("c", vec![Term::Variable(0)]),
        ],
        vec![],
        vec![],
        vec![Filter::Neq(
            Term::Variable(0),
            Term::Constant(Value::Number(1)),
        )],
    ));
    let program = program(rules);
    let seeds = seeds(&program, &[vec![0], vec![1], vec![2], vec![0, 1, 2]]);
    let result = run(&program, &seeds, lazy::SourceSelection::Worlds);
    assert_exact(&program, &seeds, &result);
    assert!(result.checks[2].closure().contains(&atom("same", 2)));
    assert!(!result.checks[1].closure().contains(&atom("same", 1)));
}

#[test]
fn catalog_growth_keeps_live_mask_storage_reserved() {
    let rules = (0..33)
        .map(|n| {
            let head = pattern(&format!("a{n:02}"), vec![]);
            rule(Some(head), vec![], vec![])
        })
        .collect();
    let program = program(rules);
    let seeds = [Seed::new(&program, []).unwrap()];
    let atom_bytes = size_of::<Atom>() + 3;
    let base = (2 + 4 + 1) * size_of::<u32>() + size_of::<lazy::Check>() + atom_bytes;
    let root_mask = size_of::<u32>();
    let growth_peak = base + 6 * 2 * size_of::<u32>() + 33 * 4 * atom_bytes + root_mask;
    let limits = lazy::Limits {
        max_rounds: 1,
        max_chunk_rules: 1,
        max_chunk_words: 4,
        max_instance_bytes: atom_bytes,
        max_host_bytes: growth_peak,
        ..Default::default()
    };
    // Only the first round is permitted. At its inclusive growth budget it
    // completes; the separate round ceiling then prevents a final closure.
    let exact = lazy::check_with_source(
        &program,
        &seeds,
        limits,
        lazy::SourceSelection::Worlds,
        &Control::default(),
        lazy::evaluate,
    )
    .unwrap_err();
    assert!(matches!(exact.cause, lazy::Cause::Source(Stop::WorkLimit)));
    assert_eq!(exact.progress.rounds, 1);
    let below = lazy::check_with_source(
        &program,
        &seeds,
        lazy::Limits {
            max_host_bytes: growth_peak - 1,
            ..limits
        },
        lazy::SourceSelection::Worlds,
        &Control::default(),
        lazy::evaluate,
    )
    .unwrap_err();
    assert!(matches!(below.cause, lazy::Cause::Source(Stop::Allocation)));
    assert_eq!(below.progress.rounds, 0);
    assert_eq!(below.progress.catalog_atoms, 32);
    assert!(below.progress.chunks > 0);
    assert_eq!(below.progress.peak_mask_bytes, root_mask);
}
