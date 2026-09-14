//! Independent closures reserve named capacity before entering the worker pool.

use std::num::NonZeroUsize;

use zetesis_core::{AdmissionLimits, AtomPattern, Predicate, Program, Seed, Template};
use zetesis_cpu::{BatchError, BatchOracle, Control, Limits};

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
    let required = limits.max_closure_bytes * 2;
    let pool = BatchOracle::new(NonZeroUsize::new(2).unwrap(), NonZeroUsize::new(2).unwrap())
        .unwrap()
        .with_closure_storage_limit(required - 1);
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
fn a_small_batch_reserves_only_active_owners() {
    let program = program();
    let seed = Seed::new(&program, []).unwrap();
    let limits = Limits::default();
    let pool = BatchOracle::new(NonZeroUsize::new(4).unwrap(), NonZeroUsize::new(4).unwrap())
        .unwrap()
        .with_closure_storage_limit(limits.max_closure_bytes);
    let results = pool
        .check_batch(&program, &[seed], limits, &Control::default())
        .unwrap();
    assert_eq!(results.len(), 1);
    assert!(results[0].as_ref().unwrap().accepted());
    let empty = pool
        .with_closure_storage_limit(0)
        .check_batch(&program, &[], limits, &Control::default())
        .unwrap();
    assert!(empty.is_empty());
}
