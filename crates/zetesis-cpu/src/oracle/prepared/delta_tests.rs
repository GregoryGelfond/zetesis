//! Delta scheduling preserves complete frozen-reduct results while avoiding rows.

use super::*;
use crate::oracle::Schedule;
use crate::oracle::test_support::rule;
use zetesis_core::{AdmissionLimits, Atom, Model, Seed, Template, Term, Value};
use zetesis_test_support::programs::{numbered as atom, pattern};

fn fact(name: &str, values: &[i32]) -> Template {
    Template::new(
        Some(pattern(
            name,
            values
                .iter()
                .copied()
                .map(Value::Number)
                .map(Term::Constant)
                .collect(),
        )),
        vec![],
        vec![],
        vec![],
        vec![],
    )
}

fn path(edges: i32, labels: i32) -> (Program, Model) {
    let (templates, expected) = path_input(edges, labels);
    (
        Program::new(templates, AdmissionLimits::default()).unwrap(),
        Model::new(expected).unwrap(),
    )
}

fn path_input(edges: i32, labels: i32) -> (Vec<Template>, Vec<Atom>) {
    let mut templates: Vec<_> = (0..edges).map(|x| fact("edge", &[x, x + 1])).collect();
    templates.push(fact("reach", &[0]));
    templates.push(rule(
        pattern("reach", vec![Term::Variable(1)]),
        vec![
            pattern("reach", vec![Term::Variable(0)]),
            pattern("edge", vec![Term::Variable(0), Term::Variable(1)]),
        ],
    ));
    let mut expected: Vec<_> = (0..edges).map(|x| atom("edge", &[x, x + 1])).collect();
    expected.extend((0..=edges).map(|x| atom("reach", &[x])));
    if labels > 0 {
        templates.extend((0..labels).map(|x| fact("label", &[x])));
        templates.push(rule(
            pattern("seen", vec![Term::Variable(0)]),
            vec![
                pattern("reach", vec![Term::Variable(0)]),
                pattern("label", vec![Term::Variable(1)]),
                pattern("label", vec![Term::Variable(2)]),
            ],
        ));
        expected.extend((0..labels).map(|x| atom("label", &[x])));
        expected.extend((0..=edges).map(|x| atom("seen", &[x])));
    }
    (templates, expected)
}

fn scheduled(
    prepared: &PreparedQueries,
    seed: SeedView<'_>,
    workspace: &mut ClosureWorkspace,
    schedule: Schedule,
    limits: Limits,
    cancellation: &Cancellation,
) -> Result<Check, Stop> {
    let mut work = Work::source(cancellation, limits.max_work);
    work.limits = limits;
    prepared.check_scheduled(seed, workspace, schedule, &mut work)
}

fn complete(program: &Program, schedule: Schedule) -> Check {
    let cancellation = Cancellation::default();
    let prepared =
        PreparedQueries::new(program, PreparationLimits::default(), &cancellation).unwrap();
    scheduled(
        &prepared,
        Seed::new(program, []).unwrap().view(),
        &mut ClosureWorkspace::default(),
        schedule,
        Limits::default(),
        &cancellation,
    )
    .unwrap()
}

fn assert_complete(check: &Check, expected: &Model, rounds: u64) {
    assert!(check.accepted());
    assert!(!check.constraint_violated());
    assert!(!check.seed_mismatch());
    assert_eq!(check.closure(), expected);
    let stats = check.statistics();
    assert_eq!(stats.derived_atoms, expected.atoms().len());
    assert_eq!(stats.rounds, rounds);
    assert!(stats.tuple_probes <= stats.work);
    assert!(stats.catalog_work <= stats.work);
    assert!(stats.peak_closure_bytes > 0);
    assert!(stats.peak_closure_bytes <= Limits::default().max_closure_bytes);
}

#[test]
fn path_delta_avoids_old_row_probes() {
    for (edges, rounds, full_bindings, delta_bindings, full_probes) in
        [(4, 6, 44, 9, 29), (8, 10, 134, 17, 89)]
    {
        let (program, expected) = path(edges, 0);
        let full = complete(&program, Schedule::Full);
        let delta = complete(&program, Schedule::Delta);
        assert_complete(&full, &expected, rounds);
        assert_complete(&delta, &expected, rounds);
        assert_eq!(full.statistics().bindings, full_bindings);
        assert_eq!(delta.statistics().bindings, delta_bindings);
        assert_eq!(full.statistics().tuple_probes, full_probes);
        // A round visits its new rows first. Every probe after the first
        // incremental round is a binding; that round also visits each new
        // edge as its own pivot, finding no earlier reach row.
        assert_eq!(
            delta.statistics().tuple_probes,
            delta_bindings + u64::try_from(edges).unwrap()
        );
    }
}

#[test]
fn an_incremental_round_selects_only_its_readers() {
    // Unrelated facts enter once at bootstrap. Their identities still change
    // catalog geometry, so inclusive work is not a producer-visit counter.
    for edges in [4, 8] {
        let unrelated = 12;
        let (chain, mut expected) = path_input(edges, 0);
        let mut templates: Vec<_> = (0..unrelated).map(|x| fact("unused", &[x])).collect();
        templates.extend(chain);
        expected.extend((0..unrelated).map(|x| atom("unused", &[x])));
        let program = Program::new(templates, AdmissionLimits::default()).unwrap();
        let recursive = program.templates().len() - 1;
        let cancellation = Cancellation::default();
        let prepared =
            PreparedQueries::new(&program, PreparationLimits::default(), &cancellation).unwrap();
        let seed = Seed::new(&program, []).unwrap();
        let mut workspace = ClosureWorkspace::default();
        let check = scheduled(
            &prepared,
            seed.view(),
            &mut workspace,
            Schedule::Delta,
            Limits::default(),
            &cancellation,
        )
        .unwrap();
        assert_complete(
            &check,
            &Model::new(expected).unwrap(),
            u64::try_from(edges + 2).unwrap(),
        );
        assert_eq!(
            check.statistics().bindings,
            u64::try_from(2 * edges + 1 + unrelated).unwrap()
        );
        // This is the actual template selection used by the final incremental
        // visit, with a new reach row and no remaining new edge/unused row.
        assert_eq!(workspace.buffers.rules, [recursive]);
    }
}

#[test]
fn repeated_occurrence_fanout_reduces_inclusive_work() {
    // Positive fanout dimensions preserve the additional seen-consequence round.
    // `rejected` counts the whole-row rejections the fixture's repeated label
    // occurrence produces under either schedule.
    for (edges, labels, rounds, full_bindings, delta_bindings, full_probes, rejected) in
        [(4, 3, 7, 254, 57, 298, 17), (8, 8, 11, 3695, 601, 4048, 73)]
    {
        let (program, expected) = path(edges, labels);
        let full = complete(&program, Schedule::Full);
        let delta = complete(&program, Schedule::Delta);
        assert_complete(&full, &expected, rounds);
        assert_complete(&delta, &expected, rounds);
        assert_eq!(full.statistics().bindings, full_bindings);
        assert_eq!(delta.statistics().bindings, delta_bindings);
        assert_eq!(full.statistics().tuple_probes, full_probes);
        // Beyond its bindings and rejections, the delta schedule probes each
        // new edge and each new label at both label occurrences once, in the
        // first incremental round, where they lead as pivots and find no
        // older row to join.
        assert_eq!(
            delta.statistics().tuple_probes,
            delta_bindings + rejected + u64::try_from(edges + 2 * labels).unwrap()
        );
        // These are complete candidate receipts: partition initialization,
        // reads/writes, canonical preparation, pending publication, extraction
        // and final gate comparison are all inside the charged total.
        assert!(
            delta.statistics().work < full.statistics().work,
            "full {:?}, delta {:?}",
            full.statistics(),
            delta.statistics()
        );
    }
}

#[test]
fn moving_rank_and_repeated_predicate_have_one_first_new_binding() {
    let program = Program::new(
        vec![
            fact("p", &[2]),
            fact("step", &[1]),
            rule(
                pattern("p", vec![Term::Variable(0)]),
                vec![pattern("step", vec![Term::Variable(0)])],
            ),
            rule(
                pattern("hit", vec![Term::Variable(0)]),
                vec![
                    pattern("p", vec![Term::Variable(0)]),
                    pattern("p", vec![Term::Variable(0)]),
                ],
            ),
        ],
        AdmissionLimits::default(),
    )
    .unwrap();
    let expected = Model::new([
        atom("p", &[1]),
        atom("p", &[2]),
        atom("step", &[1]),
        atom("hit", &[1]),
        atom("hit", &[2]),
    ])
    .unwrap();
    let full = complete(&program, Schedule::Full);
    let delta = complete(&program, Schedule::Delta);
    assert_complete(&full, &expected, 4);
    assert_complete(&delta, &expected, 4);
    assert_eq!(full.statistics().bindings, 16);
    assert_eq!(delta.statistics().bindings, 5);
    assert!(delta.statistics().tuple_probes < full.statistics().tuple_probes);
}

#[test]
fn every_incomplete_delta_prefix_is_retired_before_reuse() {
    let (mut templates, mut expected) = path_input(3, 0);
    let gate = pattern("enabled", vec![]);
    let recursive = templates.pop().unwrap();
    templates.push(Template::new(
        recursive.head().cloned(),
        recursive.positive().to_vec(),
        vec![gate.clone()],
        vec![],
        vec![],
    ));
    templates.push(Template::new(
        Some(gate.clone()),
        vec![],
        vec![gate],
        vec![],
        vec![],
    ));
    let program = Program::new(templates, AdmissionLimits::default()).unwrap();
    expected.push(atom("enabled", &[]));
    let expected = Model::new(expected).unwrap();
    let cancellation = Cancellation::default();
    let prepared =
        PreparedQueries::new(&program, PreparationLimits::default(), &cancellation).unwrap();
    let seed = Seed::new(&program, [atom("enabled", &[])]).unwrap();
    let other_seed = Seed::new(&program, []).unwrap();
    let mut original = ClosureWorkspace::default();
    let complete = prepared
        .check_view(seed.view(), &mut original, Limits::default(), &cancellation)
        .unwrap();
    let reference = scheduled(
        &prepared,
        other_seed.view(),
        &mut ClosureWorkspace::default(),
        Schedule::Full,
        Limits::default(),
        &cancellation,
    )
    .unwrap();
    for max_work in 0..complete.statistics().work {
        let mut workspace = ClosureWorkspace::default();
        let Err(stop) = prepared.check_view(
            seed.view(),
            &mut workspace,
            Limits {
                max_work,
                ..Limits::default()
            },
            &cancellation,
        ) else {
            panic!("a strict prefix cannot establish completed source coverage");
        };
        assert_eq!(stop, Stop::WorkLimit);
        assert_eq!(
            workspace.retained_bytes().unwrap(),
            ClosureWorkspace::default().retained_bytes().unwrap()
        );
        let retry = prepared
            .check_view(
                other_seed.view(),
                &mut workspace,
                Limits::default(),
                &cancellation,
            )
            .unwrap();
        assert_eq!(retry.closure(), reference.closure());
        assert_eq!(retry.accepted(), reference.accepted());
        assert_eq!(retry.constraint_violated(), reference.constraint_violated());
        assert_eq!(retry.seed_mismatch(), reference.seed_mismatch());
        assert_eq!(complete.closure(), &expected);
    }
}

#[test]
fn complete_delta_work_limit_is_inclusive() {
    let (program, expected) = path(4, 3);
    let cancellation = Cancellation::default();
    let seed = Seed::new(&program, []).unwrap();
    // One-shot work includes the same source preparation as the actual API.
    let reference = crate::check(&program, &seed, Limits::default(), &cancellation).unwrap();
    let exact = crate::check(
        &program,
        &seed,
        Limits {
            max_work: reference.statistics().work,
            ..Limits::default()
        },
        &cancellation,
    )
    .unwrap();
    assert_eq!(exact.closure(), &expected);
    assert_eq!(exact.statistics(), reference.statistics());
    let Err(stop) = crate::check(
        &program,
        &seed,
        Limits {
            max_work: reference.statistics().work - 1,
            ..Limits::default()
        },
        &cancellation,
    ) else {
        panic!("final completion work is required");
    };
    assert_eq!(stop, Stop::WorkLimit);
}

#[test]
fn delta_capacity_admits_the_complete_named_envelope() {
    let (program, expected) = path(4, 3);
    let cancellation = Cancellation::default();
    let seed = Seed::new(&program, []).unwrap();
    let reference = crate::check(&program, &seed, Limits::default(), &cancellation).unwrap();
    let exact = crate::check(
        &program,
        &seed,
        Limits {
            max_closure_bytes: reference.statistics().peak_closure_bytes,
            ..Limits::default()
        },
        &cancellation,
    )
    .unwrap();
    assert_eq!(exact.closure(), &expected);
    assert_eq!(exact.statistics(), reference.statistics());
    let Err(stop) = crate::check(
        &program,
        &seed,
        Limits {
            max_closure_bytes: reference.statistics().peak_closure_bytes - 1,
            ..Limits::default()
        },
        &cancellation,
    ) else {
        panic!("complete view and publication capacity is required");
    };
    assert_eq!(stop, Stop::StorageLimit);
}

#[test]
fn bootstrap_gates_and_latched_constraints_preserve_rejection() {
    let gate = pattern("enabled", vec![]);
    let program = Program::new(
        vec![
            // The ungated fact will disagree with the empty frozen seed. Neither
            // that mismatch nor the bootstrap constraint can truncate closure.
            fact("enabled", &[]),
            fact("a", &[]),
            Template::new(None, vec![], vec![], vec![], vec![]),
            Template::new(
                Some(pattern("gated", vec![])),
                vec![],
                vec![gate.clone()],
                vec![],
                vec![],
            ),
            rule(pattern("b", vec![]), vec![pattern("a", vec![])]),
            rule(pattern("c", vec![]), vec![pattern("b", vec![])]),
            Template::new(None, vec![pattern("c", vec![])], vec![], vec![], vec![]),
        ],
        AdmissionLimits::default(),
    )
    .unwrap();
    let expected = Model::new([
        atom("enabled", &[]),
        atom("a", &[]),
        atom("b", &[]),
        atom("c", &[]),
    ])
    .unwrap();
    let full = complete(&program, Schedule::Full);
    let delta = complete(&program, Schedule::Delta);
    for check in [&full, &delta] {
        assert_eq!(check.closure(), &expected);
        assert!(check.constraint_violated());
        assert!(check.seed_mismatch());
        assert!(!check.accepted());
        assert_eq!(check.statistics().rounds, 4);
    }
    // The second seed enables the zero-positive rule only at bootstrap. This
    // preserves its consequence even though it is not revisited in later rounds.
    let cancellation = Cancellation::default();
    let prepared =
        PreparedQueries::new(&program, PreparationLimits::default(), &cancellation).unwrap();
    let seed = Seed::new(&program, [atom("enabled", &[])]).unwrap();
    let expected =
        Model::new(["enabled", "a", "b", "c", "gated"].map(|name| atom(name, &[]))).unwrap();
    for schedule in [Schedule::Full, Schedule::Delta] {
        let check = scheduled(
            &prepared,
            seed.view(),
            &mut ClosureWorkspace::default(),
            schedule,
            Limits::default(),
            &cancellation,
        )
        .unwrap();
        assert_eq!(check.closure(), &expected);
        assert!(check.constraint_violated());
        assert!(!check.seed_mismatch());
        assert!(!check.accepted());
    }
}
