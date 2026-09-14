//! Owned and shared true sets enter the same prepared or one-shot reduct checker.

use std::num::NonZeroUsize;
use std::sync::Arc;

use rayon::prelude::*;
use zetesis_core::{
    AdmissionLimits, Atom, AtomPattern, GroundProgram, Model, Predicate, Program, Seed,
    SeedSelection, Sign, StaticLimits, Template, Term, Value,
};
use zetesis_cpu::{
    BatchError, BatchOracle, Control, Limits, Stop, check, check_static, check_static_view,
    check_view, lazy,
};

fn atom(index: usize) -> Atom {
    let (name, sign) = match index {
        0 => ("a", Sign::Positive),
        1 => ("a", Sign::Negative),
        2 => ("consequence", Sign::Positive),
        _ => unreachable!(),
    };
    Atom::new(
        Predicate::with_sign(name, 1, sign).unwrap(),
        vec![Value::String("shared-value".repeat(128))],
    )
    .unwrap()
}

fn program() -> Program {
    let [a, b, c] = [0, 1, 2].map(|index| {
        let atom = atom(index);
        AtomPattern::new(
            atom.predicate().clone(),
            atom.values().iter().cloned().map(Term::Constant).collect(),
        )
        .unwrap()
    });
    Program::new(
        vec![
            Template::new(Some(a.clone()), vec![], vec![], vec![b.clone()], vec![]),
            Template::new(Some(b.clone()), vec![], vec![], vec![a.clone()], vec![]),
            Template::new(Some(c), vec![a.clone()], vec![], vec![], vec![]),
            Template::new(None, vec![a, b], vec![], vec![], vec![]),
        ],
        AdmissionLimits::default(),
    )
    .unwrap()
}

const MASKS: [u8; 6] = [2, 0, 1, 3, 2, 1];

fn selections(program: &Program) -> Vec<SeedSelection> {
    let atoms = [Arc::new(atom(0)), Arc::new(atom(1))];
    MASKS
        .iter()
        .map(|mask| {
            SeedSelection::new(
                program,
                atoms
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| mask & (1 << index) != 0)
                    .map(|(_, atom)| Arc::clone(atom)),
            )
            .unwrap()
        })
        .collect()
}

fn expected(mask: u8) -> (Model, bool, bool, bool) {
    // Freeze the two negative gates, then close only their selected rules.
    let indices: &[usize] = match mask {
        0 => &[0, 1, 2],
        1 => &[0, 2],
        2 => &[1],
        3 => &[],
        _ => unreachable!(),
    };
    (
        Model::new(indices.iter().copied().map(atom)),
        mask == 1 || mask == 2,
        mask == 0,
        mask == 0 || mask == 3,
    )
}

fn pool(capacity: usize) -> BatchOracle {
    BatchOracle::new(
        NonZeroUsize::new(3).unwrap(),
        NonZeroUsize::new(capacity).unwrap(),
    )
    .unwrap()
}

fn shared_limits() -> lazy::shared::Limits {
    lazy::shared::Limits {
        source: lazy::Limits {
            max_chunk_rules: 1,
            ..Default::default()
        },
        ..Default::default()
    }
}

#[test]
fn scalar_views_preserve_full_reduct_results() {
    let program = program();
    let selections = selections(&program);
    let graph = GroundProgram::compile(&program, StaticLimits::default()).unwrap();
    for (selection, mask) in selections.iter().zip(MASKS) {
        let owned = selection.to_seed();
        let reference = check(&program, &owned, Limits::default(), &Control::default()).unwrap();
        let dense = check_static(&graph, &owned, Limits::default(), &Control::default()).unwrap();
        for view in [owned.view(), selection.view()] {
            let actual =
                check_view(&program, view, Limits::default(), &Control::default()).unwrap();
            let packed =
                check_static_view(&graph, view, Limits::default(), &Control::default()).unwrap();
            let (closure, accepted, violated, mismatch) = expected(mask);
            assert_eq!(actual.closure(), &closure);
            assert_eq!(
                graph.model_from_words(packed.closure_words()).unwrap(),
                closure
            );
            assert_eq!(
                (
                    actual.accepted(),
                    actual.constraint_violated(),
                    actual.seed_mismatch()
                ),
                (accepted, violated, mismatch)
            );
            assert_eq!(
                (
                    packed.accepted(),
                    packed.constraint_violated(),
                    packed.seed_mismatch()
                ),
                (accepted, violated, mismatch)
            );
            assert_eq!(actual.statistics(), reference.statistics());
            assert_eq!(packed.statistics(), dense.statistics());
        }
    }
}

#[test]
fn indexed_view_batches_preserve_occurrence_order() {
    let program = program();
    let selections = selections(&program);
    let graph = GroundProgram::compile(&program, StaticLimits::default()).unwrap();
    // Both doors start with equivalent empty caches. Reusing the same cache for
    // the second door would compare different allocation and lookup histories.
    let reference_pool = pool(selections.len());
    let pool = pool(selections.len());
    let owned: Vec<_> = selections.iter().map(SeedSelection::to_seed).collect();
    let reference = reference_pool
        .check_batch(&program, &owned, Limits::default(), &Control::default())
        .unwrap();
    let actual = pool
        .check_batch_views(
            &program,
            selections.par_iter().map(SeedSelection::view),
            Limits::default(),
            &Control::default(),
        )
        .unwrap();
    let dense = pool
        .check_static_batch_views(
            &graph,
            selections.par_iter().map(SeedSelection::view),
            Limits::default(),
            &Control::default(),
        )
        .unwrap();
    assert_eq!(actual.len(), MASKS.len());
    assert_eq!(dense.len(), MASKS.len());
    for (((actual, dense), reference), mask) in actual.iter().zip(&dense).zip(&reference).zip(MASKS)
    {
        let actual = actual.as_ref().unwrap();
        let dense = dense.as_ref().unwrap();
        assert_eq!(actual.closure(), &expected(mask).0);
        assert_eq!(
            graph.model_from_words(dense.closure_words()).unwrap(),
            expected(mask).0
        );
        assert_eq!(actual.accepted(), expected(mask).1);
        assert_eq!(dense.accepted(), expected(mask).1);
        assert_eq!(
            actual.statistics(),
            reference.as_ref().unwrap().statistics()
        );
    }
}

#[test]
fn shared_views_preserve_source_and_world_accounting() {
    let program = program();
    let selections = selections(&program);
    let owned: Vec<_> = selections.iter().map(SeedSelection::to_seed).collect();
    let pool = pool(selections.len());
    for mode in [lazy::SourceSelection::Union, lazy::SourceSelection::Worlds] {
        let reference = pool
            .check_shared(&program, &owned, shared_limits(), mode, &Control::default())
            .unwrap();
        let actual = pool
            .check_shared_views(
                &program,
                selections.iter().map(SeedSelection::view),
                shared_limits(),
                mode,
                &Control::default(),
            )
            .unwrap();
        assert_eq!(actual.statistics.source, reference.statistics.source);
        assert_eq!(actual.statistics.worlds, reference.statistics.worlds);
        assert_eq!(actual.statistics.submitted_candidates, MASKS.len());
        assert_eq!(actual.checks.len(), MASKS.len());
        for (actual, mask) in actual.checks.iter().zip(MASKS) {
            let (closure, accepted, violated, mismatch) = expected(mask);
            assert_eq!(actual.closure(), &closure);
            assert_eq!(
                (
                    actual.accepted(),
                    actual.constraint_violated(),
                    actual.seed_mismatch()
                ),
                (accepted, violated, mismatch)
            );
        }
    }
}

#[test]
fn scalar_view_work_limits_are_inclusive() {
    let program = program();
    let selections = selections(&program);
    let graph = GroundProgram::compile(&program, StaticLimits::default()).unwrap();
    let control = Control::default();
    for selection in &selections {
        let owned = selection.to_seed();
        let work = check(&program, &owned, Limits::default(), &control)
            .unwrap()
            .statistics()
            .work;
        let static_work = check_static(&graph, &owned, Limits::default(), &control)
            .unwrap()
            .statistics()
            .work;
        for view in [owned.view(), selection.view()] {
            for (maximum, succeeds) in [(work - 1, false), (work, true)] {
                let result = check_view(
                    &program,
                    view,
                    Limits {
                        max_work: maximum,
                        ..Default::default()
                    },
                    &control,
                );
                assert_eq!(result.is_ok(), succeeds);
                if !succeeds {
                    assert!(matches!(result, Err(Stop::WorkLimit)));
                }
            }
            for (maximum, succeeds) in [(static_work - 1, false), (static_work, true)] {
                let result = check_static_view(
                    &graph,
                    view,
                    Limits {
                        max_work: maximum,
                        ..Default::default()
                    },
                    &control,
                );
                assert_eq!(result.is_ok(), succeeds);
                if !succeeds {
                    assert!(matches!(result, Err(Stop::WorkLimit)));
                }
            }
        }
    }
}

#[test]
fn lazy_view_failure_retains_the_same_progress() {
    let program = program();
    let selections = selections(&program);
    let owned: Vec<_> = selections.iter().map(SeedSelection::to_seed).collect();
    for mode in [lazy::SourceSelection::Union, lazy::SourceSelection::Worlds] {
        let completed = lazy::check_with_source(
            &program,
            &owned,
            shared_limits().source,
            mode,
            &Control::default(),
            lazy::evaluate,
        )
        .unwrap();
        for maximum in [
            completed.progress.source_work - 1,
            completed.progress.source_work,
        ] {
            let limits = lazy::Limits {
                max_source_work: maximum,
                ..shared_limits().source
            };
            let reference = lazy::check_with_source(
                &program,
                &owned,
                limits,
                mode,
                &Control::default(),
                lazy::evaluate,
            );
            let actual = lazy::check_with_source_views(
                &program,
                selections.iter().map(SeedSelection::view),
                limits,
                mode,
                &Control::default(),
                lazy::evaluate,
            );
            match (reference, actual) {
                (Ok(reference), Ok(actual)) => assert_eq!(actual.progress, reference.progress),
                (Err(reference), Err(actual)) => {
                    assert!(matches!(actual.cause, lazy::Cause::Source(Stop::WorkLimit)));
                    assert!(matches!(
                        reference.cause,
                        lazy::Cause::Source(Stop::WorkLimit)
                    ));
                    assert_eq!(actual.progress, reference.progress);
                }
                _ => panic!("view changed the exact source-work boundary"),
            }
        }
    }
}

#[test]
fn shared_view_world_failure_retains_ordered_progress() {
    let program = program();
    let selections = selections(&program);
    let owned: Vec<_> = selections.iter().map(SeedSelection::to_seed).collect();
    let pool = pool(selections.len());
    let limits = lazy::shared::Limits {
        max_world_work: 0,
        ..shared_limits()
    };
    let mode = lazy::SourceSelection::Worlds;
    let reference = pool
        .check_shared(&program, &owned, limits, mode, &Control::default())
        .unwrap_err();
    let actual = pool
        .check_shared_views(
            &program,
            selections.iter().map(SeedSelection::view),
            limits,
            mode,
            &Control::default(),
        )
        .unwrap_err();
    let (lazy::shared::Error::Incomplete(reference), lazy::shared::Error::Incomplete(actual)) =
        (reference, actual)
    else {
        panic!("shared world work must fail after admission")
    };
    assert_eq!(
        actual.cause,
        lazy::shared::Cause::World {
            index: 0,
            stop: Stop::WorkLimit
        }
    );
    assert_eq!(actual.statistics.source, reference.statistics.source);
    assert_eq!(actual.statistics.worlds, reference.statistics.worlds);
    assert_eq!(actual.statistics.worlds.len(), MASKS.len());
}

#[test]
fn views_preserve_foreign_program_rejection() {
    let program = program();
    let foreign = Program::new(program.templates().to_vec(), AdmissionLimits::default()).unwrap();
    let selections = selections(&foreign);
    let graph = GroundProgram::compile(&program, StaticLimits::default()).unwrap();
    let control = Control::default();
    for selection in &selections {
        assert!(matches!(
            check_view(&program, selection.view(), Limits::default(), &control),
            Err(Stop::WrongProgram)
        ));
        assert!(matches!(
            check_static_view(&graph, selection.view(), Limits::default(), &control),
            Err(Stop::WrongProgram)
        ));
    }
    let result = lazy::check_with_views(
        &program,
        selections.iter().map(SeedSelection::view),
        lazy::Limits::default(),
        &control,
        |_| -> Result<Vec<u32>, Stop> { panic!("foreign input must not reach evaluator") },
    )
    .unwrap_err();
    assert!(matches!(
        result.cause,
        lazy::Cause::Source(Stop::WrongProgram)
    ));
    assert_eq!(result.progress, lazy::Progress::default());
}

#[test]
fn view_batch_capacity_refuses_before_oracle_work() {
    let program = program();
    let selections = selections(&program);
    let pool = pool(selections.len() - 1);
    let cancelled = Control::default();
    cancelled.cancel();
    assert!(matches!(
        pool.check_batch_views(
            &program,
            selections.par_iter().map(SeedSelection::view),
            Limits::default(),
            &cancelled
        ),
        Err(BatchError::Capacity {
            limit: 5,
            actual: 6
        })
    ));
    assert!(matches!(
        pool.check_shared_views(
            &program,
            selections.iter().map(SeedSelection::view),
            shared_limits(),
            lazy::SourceSelection::Worlds,
            &cancelled
        ),
        Err(lazy::shared::Error::Admission(BatchError::Capacity {
            limit: 5,
            actual: 6
        }))
    ));
}

#[test]
fn empty_view_batch_performs_no_evaluation() {
    let program = program();
    let seeds: [Seed; 0] = [];
    let actual = lazy::check_with_views(
        &program,
        seeds.iter().map(Seed::view),
        lazy::Limits::default(),
        &Control::default(),
        |_| -> Result<Vec<u32>, Stop> { panic!("empty batch must not execute") },
    )
    .unwrap();
    assert!(actual.checks.is_empty());
    assert_eq!(actual.progress, lazy::Progress::default());
}
