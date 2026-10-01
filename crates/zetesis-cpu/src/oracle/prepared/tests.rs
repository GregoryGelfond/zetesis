//! Reuse preserves full candidate results and admits actual retained capacity.

use zetesis_core::{
    AdmissionLimits, AtomPattern, Model, Predicate, Program, Seed, Template, Term, Value,
};
use zetesis_test_support::programs::unary as atom;

use super::*;

fn pattern(name: &str, term: Term) -> AtomPattern {
    AtomPattern::new(Predicate::new(name, 1).unwrap(), vec![term]).unwrap()
}

fn program() -> Program {
    let mut templates = [1, 2]
        .map(|value| {
            Template::new(
                Some(pattern("d", Term::Constant(Value::Number(value)))),
                vec![],
                vec![],
                vec![],
                vec![],
            )
        })
        .to_vec();
    templates.push(Template::new(
        Some(pattern("s", Term::Variable(0))),
        vec![pattern("d", Term::Variable(0))],
        vec![pattern("s", Term::Variable(0))],
        vec![],
        vec![],
    ));
    templates.push(Template::new(
        Some(pattern("q", Term::Variable(0))),
        vec![pattern("s", Term::Variable(0))],
        vec![],
        vec![],
        vec![],
    ));
    Program::new(templates, AdmissionLimits::default()).unwrap()
}

fn expected(value: i32) -> Model {
    Model::new([
        atom("d", 1),
        atom("d", 2),
        atom("s", value),
        atom("q", value),
    ])
    .unwrap()
}

fn ground_heads_program() -> Program {
    let mut templates = Vec::new();
    for (number, values) in [
        (1, vec![Value::Number(1), Value::String("v".into())]),
        (2, vec![Value::String("v".into()), Value::Number(1)]),
    ] {
        let gate = pattern("s", Term::Constant(Value::Number(number)));
        templates.push(Template::new(
            Some(gate.clone()),
            vec![],
            vec![gate.clone()],
            vec![],
            vec![],
        ));
        templates.push(Template::new(
            Some(
                AtomPattern::new(
                    Predicate::new("pair", 2).unwrap(),
                    values.into_iter().map(Term::Constant).collect(),
                )
                .unwrap(),
            ),
            vec![gate],
            vec![],
            vec![],
            vec![],
        ));
    }
    Program::new(templates, AdmissionLimits::default()).unwrap()
}

#[test]
fn ground_head_coordinates_preserve_frozen_truth() {
    let program = ground_heads_program();
    let cancellation = Cancellation::default();
    let prepared =
        PreparedQueries::new(&program, PreparationLimits::default(), &cancellation).unwrap();
    let mut ranked =
        PreparedQueries::new(&program, PreparationLimits::default(), &cancellation).unwrap();
    // The same immutable layouts and the ordinary per-binding ranking are
    // the reference; only ground coordinates are absent from this owner.
    ranked.heads = Heads::default();
    let mut workspace = ClosureWorkspace::default();
    let mut retained = Vec::new();
    for mask in [1, 2, 3, 0, 2, 1] {
        let seeds: Vec<_> = (1..=2)
            .filter(|number| mask & (1 << (number - 1)) != 0)
            .map(|number| atom("s", number))
            .collect();
        let seed = Seed::new(&program, seeds.clone()).unwrap();
        let check = prepared
            .check_view(
                seed.view(),
                &mut workspace,
                Limits::default(),
                &cancellation,
            )
            .unwrap();
        let reference = ranked
            .check_view(
                seed.view(),
                &mut ClosureWorkspace::default(),
                Limits::default(),
                &cancellation,
            )
            .unwrap();
        let mut atoms = seeds;
        for (number, values) in [
            (1, vec![Value::Number(1), Value::String("v".into())]),
            (2, vec![Value::String("v".into()), Value::Number(1)]),
        ] {
            if mask & (1 << (number - 1)) != 0 {
                atoms.push(
                    zetesis_core::Atom::new(Predicate::new("pair", 2).unwrap(), values).unwrap(),
                );
            }
        }
        let expected = Model::new(atoms).unwrap();
        assert!(check.accepted());
        assert_eq!(check.closure(), &expected);
        assert_eq!(check.closure(), reference.closure());
        assert_eq!(check.statistics().rounds, reference.statistics().rounds);
        assert_eq!(check.statistics().bindings, reference.statistics().bindings);
        retained.push((check, expected));
    }
    for (check, expected) in retained {
        assert_eq!(check.closure(), &expected);
    }
}

#[test]
fn ground_coordinates_remove_repeated_ranking_work() {
    let program = ground_heads_program();
    let cancellation = Cancellation::default();
    let prepared =
        PreparedQueries::new(&program, PreparationLimits::default(), &cancellation).unwrap();
    let mut ranked =
        PreparedQueries::new(&program, PreparationLimits::default(), &cancellation).unwrap();
    ranked.heads = Heads::default();
    let seed = Seed::new(&program, [atom("s", 1), atom("s", 2)]).unwrap();
    let check = |queries: &PreparedQueries| {
        queries
            .check_view(
                seed.view(),
                &mut ClosureWorkspace::default(),
                Limits::default(),
                &cancellation,
            )
            .unwrap()
    };
    let (prepared, ranked) = (check(&prepared), check(&ranked));
    assert_eq!(prepared.closure(), ranked.closure());
    assert_eq!(prepared.statistics().dense_heads, 4);
    assert_eq!(prepared.statistics().bindings, ranked.statistics().bindings);
    assert!(prepared.statistics().catalog_work < ranked.statistics().catalog_work);
    assert!(prepared.statistics().work < ranked.statistics().work);
}

#[test]
fn ground_coordinates_require_their_own_layout() {
    let program = ground_heads_program();
    let cancellation = Cancellation::default();
    let prepared =
        PreparedQueries::new(&program, PreparationLimits::default(), &cancellation).unwrap();
    assert_eq!(
        prepared.heads.position(0, usize::MAX),
        Err(Stop::InvalidProgram)
    );
}

/// With a width ceiling of three, a's arguments become unknown and the
/// target's bound includes b(0). At four, its bound contains only 1. The
/// optional block rule has equal source/head axes only in that latter plan.
fn changing_layout_program(block_rule: bool) -> (Program, Model) {
    let target = if block_rule { "q" } else { "p" };
    let mut facts: Vec<_> = [(1, 10), (3, 11), (4, 12), (5, 13)]
        .into_iter()
        .map(|(left, right)| {
            zetesis_core::Atom::new(
                Predicate::new("a", 2).unwrap(),
                vec![Value::Number(left), Value::Number(right)],
            )
            .unwrap()
        })
        .collect();
    facts.extend([atom("b", 0), atom("b", 1), atom(target, 1)]);
    if block_rule {
        facts.push(atom("p", 1));
    }
    let mut templates: Vec<_> = facts
        .iter()
        .map(|atom| {
            Template::new(
                Some(
                    AtomPattern::new(
                        atom.predicate().clone(),
                        atom.values().iter().cloned().map(Term::Constant).collect(),
                    )
                    .unwrap(),
                ),
                vec![],
                vec![],
                vec![],
                vec![],
            )
        })
        .collect();
    templates.push(Template::new(
        Some(pattern(target, Term::Variable(0))),
        vec![
            AtomPattern::new(
                Predicate::new("a", 2).unwrap(),
                vec![Term::Variable(0), Term::Variable(1)],
            )
            .unwrap(),
            pattern("b", Term::Variable(0)),
        ],
        vec![],
        vec![],
        vec![],
    ));
    if block_rule {
        templates.push(Template::new(
            Some(pattern("q", Term::Variable(0))),
            vec![pattern("p", Term::Variable(0))],
            vec![],
            vec![],
            vec![],
        ));
    }
    (
        Program::new(templates, AdmissionLimits::default()).unwrap(),
        Model::new(facts).unwrap(),
    )
}

fn differently_bounded(program: &Program, cancellation: &Cancellation) -> [PreparedQueries; 2] {
    [3, 4].map(|max_dense_atoms| {
        PreparedQueries::new(
            program,
            PreparationLimits {
                max_dense_atoms,
                ..PreparationLimits::default()
            },
            cancellation,
        )
        .unwrap()
    })
}

#[test]
fn another_preparation_retires_retained_layouts() {
    let (program, expected) = changing_layout_program(false);
    let cancellation = Cancellation::default();
    let prepared = differently_bounded(&program, &cancellation);
    let predicate = Predicate::new("p", 1).unwrap();
    assert_eq!(
        prepared[0]
            .layouts
            .get((&predicate).into())
            .unwrap()
            .positions(),
        2
    );
    assert_eq!(
        prepared[1]
            .layouts
            .get((&predicate).into())
            .unwrap()
            .positions(),
        1
    );
    let seed = Seed::new(&program, []).unwrap();
    for order in [[0, 1, 0, 1], [1, 0, 1, 0]] {
        let mut workspace = ClosureWorkspace::default();
        let mut retained = Vec::new();
        for at in order {
            let check = prepared[at]
                .check_view(
                    seed.view(),
                    &mut workspace,
                    Limits::default(),
                    &cancellation,
                )
                .unwrap();
            assert!(check.accepted());
            assert_eq!(check.closure(), &expected);
            assert!(Arc::ptr_eq(
                workspace.layouts.as_ref().unwrap(),
                &prepared[at].layouts
            ));
            retained.push(check);
        }
        for check in retained {
            assert_eq!(check.closure(), &expected);
        }
    }
}

#[test]
fn block_plans_follow_the_preparation_owner() {
    let (program, expected) = changing_layout_program(true);
    let cancellation = Cancellation::default();
    let prepared = differently_bounded(&program, &cancellation);
    let seed = Seed::new(&program, []).unwrap();
    let mut workspace = ClosureWorkspace::default();
    for at in [0, 1, 0, 1] {
        let check = prepared[at]
            .check_view(
                seed.view(),
                &mut workspace,
                Limits::default(),
                &cancellation,
            )
            .unwrap();
        assert!(check.accepted());
        assert_eq!(check.closure(), &expected);
        if at == 0 {
            assert_eq!(check.statistics().block_steps, 0);
        } else {
            assert!(check.statistics().block_steps > 0);
        }
    }
}

#[test]
fn refused_preparation_switch_retires_its_workspace() {
    let (program, expected) = changing_layout_program(false);
    let cancellation = Cancellation::default();
    let prepared = differently_bounded(&program, &cancellation);
    let seed = Seed::new(&program, []).unwrap();
    for (limits, stop) in [
        (
            Limits {
                max_work: 0,
                ..Limits::default()
            },
            Stop::WorkLimit,
        ),
        (
            Limits {
                max_closure_bytes: 0,
                ..Limits::default()
            },
            Stop::StorageLimit,
        ),
    ] {
        let mut workspace = ClosureWorkspace::default();
        let previous = prepared[0]
            .check_view(
                seed.view(),
                &mut workspace,
                Limits::default(),
                &cancellation,
            )
            .unwrap();
        let failure = prepared[1]
            .check_view(seed.view(), &mut workspace, limits, &cancellation)
            .unwrap_err();
        assert_eq!(failure, stop);
        assert!(workspace.layouts.is_none());
        assert_eq!(
            workspace.retained_bytes().unwrap(),
            ClosureWorkspace::default().retained_bytes().unwrap()
        );
        let recovered = prepared[1]
            .check_view(
                seed.view(),
                &mut workspace,
                Limits::default(),
                &cancellation,
            )
            .unwrap();
        assert!(recovered.accepted());
        assert_eq!(recovered.closure(), &expected);
        assert_eq!(previous.closure(), &expected);
    }
}

#[test]
fn preparation_admits_the_out_of_line_layout_header() {
    let program = ground_heads_program();
    let cancellation = Cancellation::default();
    let prepared =
        PreparedQueries::new(&program, PreparationLimits::default(), &cancellation).unwrap();
    // This ceiling holds the owner and every variable allocation, but omits
    // the separately allocated Layouts header that their Arc points to.
    let without_header = size_of::<PreparedQueries>() as u128
        + prepared.rules.bytes().unwrap()
        + prepared.block_steps.bytes().unwrap()
        + prepared.layouts.bytes().unwrap()
        + prepared.heads.bytes();
    let limits = PreparationLimits {
        max_bytes: usize::try_from(without_header).unwrap(),
        ..PreparationLimits::default()
    };
    assert!(matches!(
        PreparedQueries::new(&program, limits, &cancellation),
        Err(Stop::StorageLimit)
    ));
}

#[test]
fn repeated_candidates_reuse_empty_query_capacity() {
    let program = program();
    let cancellation = Cancellation::default();
    let prepared =
        PreparedQueries::new(&program, PreparationLimits::default(), &cancellation).unwrap();
    let mut workspace = ClosureWorkspace::default();
    let first = prepared
        .check_view(
            Seed::new(&program, [atom("s", 1)]).unwrap().view(),
            &mut workspace,
            Limits::default(),
            &cancellation,
        )
        .unwrap();
    // Canonical identity persists independently of truth. Warm both finite
    // candidates before asserting that repeated checks reuse their capacities;
    // a candidate's second check records the dense positions its first
    // admitted, so each seed is checked twice.
    for value in [1, 2, 2] {
        prepared
            .check_view(
                Seed::new(&program, [atom("s", value)]).unwrap().view(),
                &mut workspace,
                Limits::default(),
                &cancellation,
            )
            .unwrap();
    }
    let cursor = workspace.buffers.cursors.as_ptr();
    let undo = workspace.buffers.undo[0].as_ptr();
    let retained = workspace.retained_bytes().unwrap();
    assert!(retained > ClosureWorkspace::default().retained_bytes().unwrap());
    let mut steady_work = [None; 2];
    // Increasing completed candidate counts reuse actual allocations. They do
    // not establish faster wall time or shared final-result payload ownership.
    for count in [1, 2, 4, 8] {
        for index in 0..count {
            let value = if index % 2 == 0 { 2 } else { 1 };
            let seed = Seed::new(&program, [atom("s", value)]).unwrap();
            let reused = prepared
                .check_view(
                    seed.view(),
                    &mut workspace,
                    Limits::default(),
                    &cancellation,
                )
                .unwrap();
            let fresh = crate::check(&program, &seed, Limits::default(), &cancellation).unwrap();
            assert!(reused.accepted());
            assert_eq!(reused.closure(), &expected(value));
            assert_eq!(reused.closure(), fresh.closure());
            assert_eq!(reused.statistics().rounds, fresh.statistics().rounds);
            assert_eq!(reused.statistics().bindings, fresh.statistics().bindings);
            assert_eq!(workspace.catalogs.len(), 0);
            assert_eq!(workspace.buffers.cursors.as_ptr(), cursor);
            assert_eq!(workspace.buffers.undo[0].as_ptr(), undo);
            assert_eq!(workspace.retained_bytes().unwrap(), retained);
            // Retained empty extents change lookup and reset work. Allocation
            // reuse does not imply a lower catalog-work subtotal. The two
            // seeds' closures differ in where their dense rows sit, so each
            // seed has its own steady work.
            if let Some(previous) = steady_work[index % 2] {
                assert_eq!(reused.statistics().work, previous);
            }
            steady_work[index % 2] = Some(reused.statistics().work);
            assert_eq!(first.closure(), &expected(1));
        }
    }
}

#[test]
fn failed_candidates_cannot_retain_truth() {
    let program = program();
    let cancellation = Cancellation::default();
    let prepared =
        PreparedQueries::new(&program, PreparationLimits::default(), &cancellation).unwrap();
    let first_seed = Seed::new(&program, [atom("s", 1)]).unwrap();
    let mut workspace = ClosureWorkspace::default();
    let reference = prepared
        .check_view(
            first_seed.view(),
            &mut workspace,
            Limits::default(),
            &cancellation,
        )
        .unwrap();
    let ceiling = reference.statistics().work;
    let empty = ClosureWorkspace::default().retained_bytes().unwrap();
    for max_work in 0..ceiling {
        let result = prepared.check_view(
            first_seed.view(),
            &mut workspace,
            Limits {
                max_work,
                ..Limits::default()
            },
            &cancellation,
        );
        if let Err(stop) = result {
            assert_eq!(stop, Stop::WorkLimit);
            assert_eq!(workspace.retained_bytes().unwrap(), empty);
            assert_eq!(workspace.catalogs.len(), 0);
            let next = prepared
                .check_view(
                    Seed::new(&program, [atom("s", 2)]).unwrap().view(),
                    &mut workspace,
                    Limits::default(),
                    &cancellation,
                )
                .unwrap();
            assert!(next.accepted());
            assert_eq!(next.closure(), &expected(2));
            assert_eq!(reference.closure(), &expected(1));
        } else {
            assert_eq!(result.unwrap().closure(), &expected(1));
        }
    }
}

#[test]
fn a_stopped_cube_closure_retires_the_workspace() {
    // Every work ceiling below a cube closure's need stops it with the
    // workspace retired, and the next closure on that workspace is the
    // closure a fresh one computes.
    let program = program();
    let cancellation = Cancellation::default();
    let prepared =
        PreparedQueries::new(&program, PreparationLimits::default(), &cancellation).unwrap();
    let cube = super::super::Cube::all_open();
    let reference = prepared
        .closure_of(
            super::super::Gates::Definite((&cube).into()),
            &mut ClosureWorkspace::default(),
            Limits::default(),
            &cancellation,
        )
        .unwrap();
    let empty = ClosureWorkspace::default().retained_bytes().unwrap();
    let mut workspace = ClosureWorkspace::default();
    let mut stopped = 0;
    for max_work in 0..64 {
        let result = prepared.closure_of(
            super::super::Gates::Definite((&cube).into()),
            &mut workspace,
            Limits {
                max_work,
                ..Limits::default()
            },
            &cancellation,
        );
        match result {
            Err(stop) => {
                stopped += 1;
                assert_eq!(stop, Stop::WorkLimit);
                assert_eq!(workspace.retained_bytes().unwrap(), empty);
                assert_eq!(workspace.catalogs.len(), 0);
                let next = prepared
                    .closure_of(
                        super::super::Gates::Possible((&cube).into()),
                        &mut workspace,
                        Limits::default(),
                        &cancellation,
                    )
                    .unwrap();
                assert!(next.atoms.atoms().len() >= reference.atoms.atoms().len());
            }
            Ok(completed) => assert_eq!(completed.atoms, reference.atoms),
        }
    }
    assert!(stopped > 0, "some ceiling stops the closure");
}

#[test]
fn changed_byte_limits_admit_retained_capacity_first() {
    let program = program();
    let cancellation = Cancellation::default();
    let prepared =
        PreparedQueries::new(&program, PreparationLimits::default(), &cancellation).unwrap();
    let seed = Seed::new(&program, [atom("s", 1)]).unwrap();
    let mut workspace = ClosureWorkspace::default();
    let original = prepared
        .check_view(
            seed.view(),
            &mut workspace,
            Limits::default(),
            &cancellation,
        )
        .unwrap();
    let limit = usize::try_from(workspace.retained_bytes().unwrap()).unwrap()
        + prepared.statistics().retained_bytes
        - 1;
    let Err(stop) = prepared.check_view(
        seed.view(),
        &mut workspace,
        Limits {
            max_closure_bytes: limit,
            ..Limits::default()
        },
        &cancellation,
    ) else {
        panic!("retained owners must be admitted before reuse");
    };
    assert_eq!(stop, Stop::StorageLimit);
    assert_eq!(
        workspace.retained_bytes().unwrap(),
        ClosureWorkspace::default().retained_bytes().unwrap()
    );
    assert_eq!(original.closure(), &expected(1));
}

#[test]
fn preparation_is_bound_to_the_exact_program() {
    let program = program();
    let other = self::program();
    let cancellation = Cancellation::default();
    let prepared =
        PreparedQueries::new(&program, PreparationLimits::default(), &cancellation).unwrap();
    assert!(prepared.program().same_instance(&program));
    let mut workspace = ClosureWorkspace::default();
    let first = prepared
        .check_view(
            Seed::new(&program, [atom("s", 2)]).unwrap().view(),
            &mut workspace,
            Limits::default(),
            &cancellation,
        )
        .unwrap();
    let seed = Seed::new(&other, [atom("s", 1)]).unwrap();
    assert!(matches!(
        prepared.check_view(
            seed.view(),
            &mut workspace,
            Limits::default(),
            &cancellation
        ),
        Err(Stop::WrongProgram)
    ));
    let other_prepared =
        PreparedQueries::new(&other, PreparationLimits::default(), &cancellation).unwrap();
    let completed = other_prepared
        .check_view(
            seed.view(),
            &mut workspace,
            Limits::default(),
            &cancellation,
        )
        .unwrap();
    assert!(completed.program().same_instance(&other));
    assert_eq!(completed.closure(), &expected(1));
    assert_eq!(first.closure(), &expected(2));
}

#[test]
fn preparation_refuses_its_own_work_boundary() {
    let program = program();
    let cancellation = Cancellation::default();
    let prepared =
        PreparedQueries::new(&program, PreparationLimits::default(), &cancellation).unwrap();
    assert!(matches!(
        PreparedQueries::new(
            &program,
            PreparationLimits {
                max_work: prepared.statistics().work - 1,
                ..PreparationLimits::default()
            },
            &cancellation
        ),
        Err(Stop::WorkLimit)
    ));
    let exact = PreparedQueries::new(
        &program,
        PreparationLimits {
            max_work: prepared.statistics().work,
            max_bytes: prepared.statistics().retained_bytes,
            ..PreparationLimits::default()
        },
        &cancellation,
    )
    .unwrap();
    assert_eq!(exact.statistics(), prepared.statistics());
}

#[test]
fn preparation_refuses_its_own_byte_boundary() {
    let program = program();
    let cancellation = Cancellation::default();
    let prepared =
        PreparedQueries::new(&program, PreparationLimits::default(), &cancellation).unwrap();
    assert!(matches!(
        PreparedQueries::new(
            &program,
            PreparationLimits {
                max_bytes: prepared.statistics().retained_bytes - 1,
                ..PreparationLimits::default()
            },
            &cancellation
        ),
        Err(Stop::StorageLimit)
    ));
}

#[test]
fn prepared_empty_checks_admit_their_retained_owners() {
    let program = Program::new(vec![], AdmissionLimits::default()).unwrap();
    let cancellation = Cancellation::default();
    let prepared =
        PreparedQueries::new(&program, PreparationLimits::default(), &cancellation).unwrap();
    let seed = Seed::new(&program, []).unwrap();
    let mut workspace = ClosureWorkspace::default();
    let baseline = prepared
        .check_view(
            seed.view(),
            &mut workspace,
            Limits::default(),
            &cancellation,
        )
        .unwrap();
    let named = baseline.statistics().peak_closure_bytes;
    workspace = ClosureWorkspace::default();
    assert!(matches!(
        prepared.check_view(
            seed.view(),
            &mut workspace,
            Limits {
                max_closure_bytes: 0,
                ..Limits::default()
            },
            &cancellation
        ),
        Err(Stop::StorageLimit)
    ));
    let exact = prepared
        .check_view(
            seed.view(),
            &mut workspace,
            Limits {
                max_closure_bytes: named,
                ..Limits::default()
            },
            &cancellation,
        )
        .unwrap();
    assert_eq!(exact.closure(), &Model::default());
    assert!(exact.accepted());
    assert_eq!(exact.statistics().rounds, 0);
    assert_eq!(exact.statistics().peak_closure_bytes, named);
}

fn check_value(
    prepared: &PreparedQueries,
    program: &Program,
    workspace: &mut ClosureWorkspace,
    value: i32,
    limits: Limits,
) -> Result<Check, Stop> {
    prepared.check_view(
        Seed::new(program, [atom("s", value)]).unwrap().view(),
        workspace,
        limits,
        &Cancellation::default(),
    )
}

fn prepared_program() -> (Program, PreparedQueries) {
    let program = program();
    let prepared = PreparedQueries::new(
        &program,
        PreparationLimits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    (program, prepared)
}

#[test]
fn a_single_check_records_no_discovered_positions() {
    // Every dense row of a first closure is a new identity; nothing repeats.
    let (program, prepared) = prepared_program();
    let mut workspace = ClosureWorkspace::default();
    check_value(&prepared, &program, &mut workspace, 1, Limits::default()).unwrap();
    assert_eq!(workspace.catalogs.discovered(), 0);
}

#[test]
fn a_repeated_check_records_its_dense_positions() {
    let (program, prepared) = prepared_program();
    let mut workspace = ClosureWorkspace::default();
    check_value(&prepared, &program, &mut workspace, 1, Limits::default()).unwrap();
    let repeated = check_value(&prepared, &program, &mut workspace, 1, Limits::default()).unwrap();
    assert_eq!(
        workspace.catalogs.discovered(),
        repeated.closure().atoms().len()
    );
}

#[test]
fn recorded_positions_answer_later_checks() {
    // The third check resolves its dense rows from recorded positions, so it
    // does less work than the check that recorded them, for the same closure.
    let (program, prepared) = prepared_program();
    let mut workspace = ClosureWorkspace::default();
    check_value(&prepared, &program, &mut workspace, 1, Limits::default()).unwrap();
    let recording = check_value(&prepared, &program, &mut workspace, 1, Limits::default()).unwrap();
    let answered = check_value(&prepared, &program, &mut workspace, 1, Limits::default()).unwrap();
    assert_eq!(answered.closure(), &expected(1));
    assert!(answered.statistics().work < recording.statistics().work);
}

#[test]
fn positions_first_seen_in_a_later_check_are_admitted() {
    // After `s(1)` repeats, a check deriving `s(2)` and `q(2)` admits them as
    // new identities; only the shared `d` rows are answered from records.
    let (program, prepared) = prepared_program();
    let mut workspace = ClosureWorkspace::default();
    for _ in 0..2 {
        check_value(&prepared, &program, &mut workspace, 1, Limits::default()).unwrap();
    }
    let recorded = workspace.catalogs.discovered();
    let later = check_value(&prepared, &program, &mut workspace, 2, Limits::default()).unwrap();
    assert_eq!(later.closure(), &expected(2));
    assert_eq!(workspace.catalogs.discovered(), recorded);
}

#[test]
fn passing_over_a_recorded_position_is_charged() {
    // Both workspaces admitted the same identities in the same order, but
    // only the first recorded `s(1)` and `q(1)`, whose rows precede those of
    // `s(2)` and `q(2)`; its check of `s(2)` passes over both records.
    let (program, prepared) = prepared_program();
    let mut passing = ClosureWorkspace::default();
    for value in [1, 1, 2, 2] {
        check_value(&prepared, &program, &mut passing, value, Limits::default()).unwrap();
    }
    let mut direct = ClosureWorkspace::default();
    for value in [1, 2, 2] {
        check_value(&prepared, &program, &mut direct, value, Limits::default()).unwrap();
    }
    assert_eq!(
        passing.catalogs.discovered(),
        direct.catalogs.discovered() + 2
    );
    let passed = check_value(&prepared, &program, &mut passing, 2, Limits::default()).unwrap();
    let answered = check_value(&prepared, &program, &mut direct, 2, Limits::default()).unwrap();
    assert_eq!(passed.closure(), answered.closure());
    assert_eq!(passed.statistics().work, answered.statistics().work + 2);
}

#[test]
fn a_failed_check_discards_recorded_positions() {
    let (program, prepared) = prepared_program();
    let mut workspace = ClosureWorkspace::default();
    for _ in 0..2 {
        check_value(&prepared, &program, &mut workspace, 1, Limits::default()).unwrap();
    }
    assert!(workspace.catalogs.discovered() > 0);
    let failed = check_value(
        &prepared,
        &program,
        &mut workspace,
        1,
        Limits {
            max_work: 0,
            ..Limits::default()
        },
    );
    assert_eq!(failed.err(), Some(Stop::WorkLimit));
    assert_eq!(workspace.catalogs.discovered(), 0);
}

#[test]
fn recorded_positions_stay_within_the_closure_ceiling() {
    // Whatever ceiling admits a repeated check, the workspace it leaves,
    // recorded positions included, fits that ceiling.
    let (program, prepared) = prepared_program();
    let mut warm = ClosureWorkspace::default();
    check_value(&prepared, &program, &mut warm, 1, Limits::default()).unwrap();
    let base = usize::try_from(warm.retained_bytes().unwrap()).unwrap()
        + prepared.statistics().retained_bytes;
    let mut recorded = false;
    for limit in base.saturating_sub(256)..base + 1024 {
        let mut workspace = ClosureWorkspace::default();
        check_value(&prepared, &program, &mut workspace, 1, Limits::default()).unwrap();
        let limits = Limits {
            max_closure_bytes: limit,
            ..Limits::default()
        };
        if let Ok(check) = check_value(&prepared, &program, &mut workspace, 1, limits) {
            assert_eq!(check.closure(), &expected(1));
            let retained = usize::try_from(workspace.retained_bytes().unwrap()).unwrap()
                + prepared.statistics().retained_bytes;
            assert!(retained <= limit);
            recorded |= workspace.catalogs.discovered() > 0;
        }
    }
    assert!(recorded);
}
