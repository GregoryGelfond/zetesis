//! Independent closures reserve named capacity before entering the worker pool.

use std::num::NonZeroUsize;

use zetesis_core::{AdmissionLimits, AtomPattern, Predicate, Program, Seed, Template};
use zetesis_cpu::{BatchError, BatchOracle, Control, Limits, PreparationLimits, Stop};

fn program() -> Program {
    let head = AtomPattern::new(Predicate::new("a", 0).unwrap(), vec![]).unwrap();
    Program::new(
        vec![Template::new(Some(head), vec![], vec![], vec![], vec![])],
        AdmissionLimits::default(),
    )
    .unwrap()
}

#[test]
fn worker_reservations_obey_the_collective_ceiling() {
    let program = program();
    let seeds = [
        Seed::new(&program, []).unwrap(),
        Seed::new(&program, []).unwrap(),
    ];
    let limits = Limits::default();
    let pool =
        BatchOracle::new(NonZeroUsize::new(2).unwrap(), NonZeroUsize::new(2).unwrap()).unwrap();
    let initial = pool
        .check_batch(&program, &seeds, limits, &Control::default())
        .unwrap();
    assert!(
        initial
            .iter()
            .all(|result| result.as_ref().unwrap().accepted())
    );
    let required = usize::try_from(pool.query_statistics().unwrap().reserved_bytes).unwrap();
    assert!(required > usize::try_from(pool.query_statistics().unwrap().retained_bytes).unwrap());
    let pool = pool.with_closure_storage_limit(required - 1);
    assert!(matches!(
        pool.check_batch(&program, &seeds, limits, &Control::default()),
        Err(BatchError::ClosureStorage { required: observed, limit })
            if observed == required as u128 && limit == (required - 1) as u128
    ));
    // Refusal releases the pool's admission slot, with no published candidate.
    let pool = pool.with_closure_storage_limit(required);
    let results = pool
        .check_batch(&program, &seeds, limits, &Control::default())
        .unwrap();
    assert_eq!(results.len(), seeds.len());
    for result in results {
        let check = result.unwrap();
        assert!(check.accepted());
        assert_eq!(check.closure().atoms().len(), 1);
    }
}

#[test]
fn small_batches_account_for_their_retained_owner() {
    let program = program();
    let seed = Seed::new(&program, []).unwrap();
    let limits = Limits::default();
    let pool =
        BatchOracle::new(NonZeroUsize::new(4).unwrap(), NonZeroUsize::new(4).unwrap()).unwrap();
    let results = pool
        .check_batch(&program, &[seed], limits, &Control::default())
        .unwrap();
    assert_eq!(results.len(), 1);
    assert!(results[0].as_ref().unwrap().accepted());
    let statistics = pool.query_statistics().unwrap();
    assert_eq!(statistics.active_workspaces, 1);
    assert_eq!(statistics.retained_workspaces, 1);
    assert!(statistics.reserved_bytes < 4 * limits.max_closure_bytes as u128);
    let retained = statistics.retained_bytes;
    assert!(matches!(pool.with_closure_storage_limit(0)
        .check_batch(&program, &[], limits, &Control::default()),
        Err(BatchError::ClosureStorage { required, limit: 0 }) if required == retained));
}

#[test]
fn empty_batches_admit_only_retained_storage() {
    let program = program();
    let seed = Seed::new(&program, []).unwrap();
    let control = Control::default();
    let pool = BatchOracle::new(NonZeroUsize::MIN, NonZeroUsize::MIN).unwrap();
    let first = pool
        .check_batch(
            &program,
            std::slice::from_ref(&seed),
            Limits::default(),
            &control,
        )
        .unwrap();
    let before = pool.query_statistics().unwrap();
    let retained = usize::try_from(before.retained_bytes).unwrap();
    let pool = pool.with_closure_storage_limit(retained);
    let empty = pool
        .check_batch(
            &program,
            &[],
            Limits {
                max_work: 0,
                max_derived_atoms: 0,
                max_closure_bytes: 0,
            },
            &control,
        )
        .unwrap();
    assert!(empty.is_empty());
    let after = pool.query_statistics().unwrap();
    assert_eq!(after.preparation, before.preparation);
    assert_eq!(after.preparation_builds, before.preparation_builds);
    assert_eq!(after.retained_bytes, before.retained_bytes);
    assert_eq!(after.retained_workspaces, before.retained_workspaces);
    assert_eq!(after.active_workspaces, 0);
    assert_eq!(after.reused_workspaces, 0);
    assert_eq!(after.reserved_bytes, before.retained_bytes);
    // Reuse must still succeed when constructing another preparation cannot.
    let pool = pool
        .with_closure_storage_limit(BatchOracle::DEFAULT_CLOSURE_BYTES)
        .with_preparation_limits(PreparationLimits {
            max_work: 0,
            ..Default::default()
        });
    let next = pool
        .check_batch(&program, &[seed], Limits::default(), &control)
        .unwrap();
    assert_eq!(
        next[0].as_ref().unwrap().closure(),
        first[0].as_ref().unwrap().closure()
    );
    assert!(next[0].as_ref().unwrap().accepted());
}

#[test]
fn preparation_refusals_clear_submission_activity() {
    let program = program();
    let seed = Seed::new(&program, []).unwrap();
    let control = Control::default();
    let pool = BatchOracle::new(NonZeroUsize::MIN, NonZeroUsize::MIN).unwrap();
    let first = pool
        .check_batch(
            &program,
            std::slice::from_ref(&seed),
            Limits::default(),
            &control,
        )
        .unwrap();
    let before = pool.query_statistics().unwrap();
    assert_eq!(before.active_workspaces, 1);
    assert!(before.reserved_bytes > 0);
    let pool = pool.with_preparation_limits(PreparationLimits {
        max_bytes: before.preparation.unwrap().retained_bytes - 1,
        ..Default::default()
    });
    assert!(matches!(
        pool.check_batch(&program, &[seed], Limits::default(), &control),
        Err(BatchError::Preparation(Stop::StorageLimit))
    ));
    let after = pool.query_statistics().unwrap();
    assert_eq!(after.preparation, before.preparation);
    assert_eq!(after.preparation_builds, before.preparation_builds);
    assert_eq!(after.retained_bytes, before.retained_bytes);
    assert_eq!(after.active_workspaces, 0);
    assert_eq!(after.reused_workspaces, 0);
    assert_eq!(after.reserved_bytes, 0);
    assert!(first[0].as_ref().unwrap().accepted());
}
