//! Reuse preserves full candidate results and admits actual retained capacity.

use zetesis_core::{
    AdmissionLimits, Atom, AtomPattern, Model, Predicate, Program, Seed, Template, Term, Value,
};

use super::*;

fn atom(name: &str, value: i32) -> Atom {
    Atom::new(Predicate::new(name, 1).unwrap(), vec![Value::Number(value)]).unwrap()
}

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
}

#[test]
fn repeated_candidates_reuse_empty_query_capacity() {
    let program = program();
    let control = Control::default();
    let prepared = PreparedQueries::new(&program, PreparationLimits::default(), &control).unwrap();
    // Four templates and two positive occurrences; two passes of the bound
    // inference at sixteen each; three predicates offered a layout; the
    // row-step plan, a unit and a unit a term for each occurrence, four and
    // three.
    assert_eq!(prepared.statistics().work, 48);
    let mut workspace = ClosureWorkspace::default();
    let first = prepared
        .check_view(
            Seed::new(&program, [atom("s", 1)]).unwrap().view(),
            &mut workspace,
            Limits::default(),
            &control,
        )
        .unwrap();
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
                .check_view(seed.view(), &mut workspace, Limits::default(), &control)
                .unwrap();
            let fresh = crate::check(&program, &seed, Limits::default(), &control).unwrap();
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
    let control = Control::default();
    let prepared = PreparedQueries::new(&program, PreparationLimits::default(), &control).unwrap();
    let first_seed = Seed::new(&program, [atom("s", 1)]).unwrap();
    let mut workspace = ClosureWorkspace::default();
    let reference = prepared
        .check_view(
            first_seed.view(),
            &mut workspace,
            Limits::default(),
            &control,
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
            &control,
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
                    &control,
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
    let control = Control::default();
    let prepared = PreparedQueries::new(&program, PreparationLimits::default(), &control).unwrap();
    let cube = super::super::Cube::all_open();
    let reference = prepared
        .closure_of(
            super::super::Gates::Definite(&cube),
            &mut ClosureWorkspace::default(),
            Limits::default(),
            &control,
        )
        .unwrap();
    let empty = ClosureWorkspace::default().retained_bytes().unwrap();
    let mut workspace = ClosureWorkspace::default();
    let mut stopped = 0;
    for max_work in 0..64 {
        let result = prepared.closure_of(
            super::super::Gates::Definite(&cube),
            &mut workspace,
            Limits {
                max_work,
                ..Limits::default()
            },
            &control,
        );
        match result {
            Err(stop) => {
                stopped += 1;
                assert_eq!(stop, Stop::WorkLimit);
                assert_eq!(workspace.retained_bytes().unwrap(), empty);
                assert_eq!(workspace.catalogs.len(), 0);
                let next = prepared
                    .closure_of(
                        super::super::Gates::Possible(&cube),
                        &mut workspace,
                        Limits::default(),
                        &control,
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
    let control = Control::default();
    let prepared = PreparedQueries::new(&program, PreparationLimits::default(), &control).unwrap();
    let seed = Seed::new(&program, [atom("s", 1)]).unwrap();
    let mut workspace = ClosureWorkspace::default();
    let original = prepared
        .check_view(seed.view(), &mut workspace, Limits::default(), &control)
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
        &control,
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
    let control = Control::default();
    let prepared = PreparedQueries::new(&program, PreparationLimits::default(), &control).unwrap();
    assert!(prepared.program().same_instance(&program));
    let mut workspace = ClosureWorkspace::default();
    let first = prepared
        .check_view(
            Seed::new(&program, [atom("s", 2)]).unwrap().view(),
            &mut workspace,
            Limits::default(),
            &control,
        )
        .unwrap();
    let seed = Seed::new(&other, [atom("s", 1)]).unwrap();
    assert!(matches!(
        prepared.check_view(seed.view(), &mut workspace, Limits::default(), &control),
        Err(Stop::WrongProgram)
    ));
    let other_prepared =
        PreparedQueries::new(&other, PreparationLimits::default(), &control).unwrap();
    let completed = other_prepared
        .check_view(seed.view(), &mut workspace, Limits::default(), &control)
        .unwrap();
    assert!(completed.program().same_instance(&other));
    assert_eq!(completed.closure(), &expected(1));
    assert_eq!(first.closure(), &expected(2));
}

#[test]
fn preparation_refuses_its_own_work_boundary() {
    let program = program();
    let control = Control::default();
    let prepared = PreparedQueries::new(&program, PreparationLimits::default(), &control).unwrap();
    assert!(matches!(
        PreparedQueries::new(
            &program,
            PreparationLimits {
                max_work: prepared.statistics().work - 1,
                ..PreparationLimits::default()
            },
            &control
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
        &control,
    )
    .unwrap();
    assert_eq!(exact.statistics(), prepared.statistics());
}

#[test]
fn preparation_refuses_its_own_byte_boundary() {
    let program = program();
    let control = Control::default();
    let prepared = PreparedQueries::new(&program, PreparationLimits::default(), &control).unwrap();
    assert!(matches!(
        PreparedQueries::new(
            &program,
            PreparationLimits {
                max_bytes: prepared.statistics().retained_bytes - 1,
                ..PreparationLimits::default()
            },
            &control
        ),
        Err(Stop::StorageLimit)
    ));
}

#[test]
fn prepared_empty_checks_admit_their_retained_owners() {
    let program = Program::new(vec![], AdmissionLimits::default()).unwrap();
    let control = Control::default();
    let prepared = PreparedQueries::new(&program, PreparationLimits::default(), &control).unwrap();
    let seed = Seed::new(&program, []).unwrap();
    let mut workspace = ClosureWorkspace::default();
    let named = usize::try_from(workspace.retained_bytes().unwrap()).unwrap()
        + prepared.statistics().retained_bytes;
    assert!(matches!(
        prepared.check_view(
            seed.view(),
            &mut workspace,
            Limits {
                max_closure_bytes: 0,
                ..Limits::default()
            },
            &control
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
            &control,
        )
        .unwrap();
    assert_eq!(exact.closure(), &Model::default());
    assert!(exact.accepted());
    assert_eq!(exact.statistics().rounds, 0);
    assert_eq!(exact.statistics().peak_closure_bytes, named);
}
