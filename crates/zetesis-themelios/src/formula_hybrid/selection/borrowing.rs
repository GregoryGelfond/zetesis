//! Every checker borrows the streamed core's published selection: one catalog
//! index and one set of row positions per core, whichever thread prepares it.
//! The region search's routes, which prepare checkers on their own threads, are
//! tested with the streamed constraints in zetesis-stable.

use zetesis_algebra::{Cancellation, region::Region};

use crate::{ConstraintCheckLimits, ConstraintChecker, HybridFormula};

// The repeated condition adds no restriction: p(X) already entails not not
// p(X). Its signed source occurrence keeps the complete six-row constraint
// streamed, so this test exercises checker selection loans rather than native
// pair-exclusion materialization.
const STREAMED_AT_MOST_ONE: &str = "{p(1..4)}. :-p(X),p(Y),X<Y,not not p(X).";

/// The checker's prepared index and row positions are the core's published
/// ones, not copies.
fn assert_borrows(checker: &ConstraintChecker<'_>, owner: &HybridFormula) {
    let core = owner.core();
    let prepared = checker.prepared.as_ref().unwrap();
    assert!(std::ptr::eq(
        prepared.index.unwrap(),
        core.0.index.get().unwrap()
    ));
    let borrowed = &prepared.rows.as_ref().unwrap().predicates;
    let published = &core.0.rows.get().unwrap().predicates;
    assert_eq!(borrowed.len(), published.len());
    assert!(borrowed.iter().any(|row| !row.positions.is_empty()));
    for (row, positions) in borrowed.iter().zip(published) {
        assert!(std::ptr::eq(row.positions, positions.as_slice()));
    }
}

#[test]
fn checkers_on_several_threads_borrow_the_core_selection() {
    let owner = crate::test_support::hybrid(STREAMED_AT_MOST_ONE);
    assert_eq!(owner.streamed_templates(), 1);
    assert_eq!(owner.streamed_instances(), 6);
    let theory = owner.core_theory();
    let atoms = theory.atom_count();
    let mut held = Region::all_open(atoms);
    for atom in 0..atoms {
        held.hold(atom);
    }
    // Separate checkers keep one core: the first preparation publishes the
    // selection, and every later one, on any thread, borrows it.
    std::thread::scope(|scope| {
        for _ in 0..4 {
            scope.spawn(|| {
                let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
                let cancellation = Cancellation::default();
                checker
                    .check_region(theory, &Region::all_open(atoms), &cancellation)
                    .unwrap();
                assert_borrows(&checker, &owner);
                checker.check_region(theory, &held, &cancellation).unwrap();
                assert_borrows(&checker, &owner);
            });
        }
    });
}
