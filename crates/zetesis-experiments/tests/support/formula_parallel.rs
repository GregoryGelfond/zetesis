//! Parallel execution preserves exact membership, input order and failure boundaries.

use std::collections::HashSet;
use std::num::NonZeroUsize;
use std::time::Instant;

use zetesis_cpu::Cancellation;
use zetesis_ferraris::{AdmissionLimits, Interpretation, Limits, Node, Theory, Verdict};
use zetesis_sat::Incomplete;

use super::FormulaPool;
use crate::formula_completion::{Membership, native_with_cancellation};
use crate::{FormulaBenchmarkError, FormulaFamily, FormulaFixture};

fn pool(workers: usize, candidates: usize) -> FormulaPool {
    FormulaPool::new(NonZeroUsize::new(workers).unwrap(), candidates).unwrap()
}

fn exhaustive(theory: &Theory, candidate: &Interpretation) -> Membership {
    match zetesis_ferraris::check(
        theory,
        candidate,
        Limits::default(),
        &Cancellation::default(),
    )
    .unwrap()
    .verdict()
    {
        Verdict::Stable => Membership::Stable,
        Verdict::NotModel { .. } => Membership::NotModel,
        Verdict::NonMinimal { .. } => Membership::NonMinimal,
    }
}

fn all_candidates(theory: &Theory) -> Vec<Interpretation> {
    (0..1usize << theory.atom_count())
        .map(|bits| {
            Interpretation::new(
                theory,
                (0..theory.atom_count()).filter(|atom| bits & (1 << atom) != 0),
            )
            .unwrap()
        })
        .collect()
}

#[test]
fn explicit_workers_check_all_tiny_worlds_against_exhaustive_ferraris() {
    for workers in [1, 2, 4] {
        let pool = pool(workers, 64);
        assert_eq!(pool.workers(), workers);
        let threads: HashSet<_> = pool
            .pool
            .broadcast(|_| std::thread::current().id())
            .into_iter()
            .collect();
        assert_eq!(threads.len(), workers);
        for family in [
            FormulaFamily::Choices,
            FormulaFamily::Cycle,
            FormulaFamily::Conjunction,
            FormulaFamily::Disjunction,
            FormulaFamily::MaskedImplication,
        ] {
            for atoms in 1..=5 {
                let fixture = FormulaFixture::new(family, atoms).unwrap();
                let theory = fixture.theory();
                let mut candidates = all_candidates(theory);
                // Interleaved duplicates and reversed order exercise indexed result
                // assembly without relying on a particular worker completion order.
                candidates.extend(candidates.clone().into_iter().rev());
                let expected: Vec<_> = candidates
                    .iter()
                    .map(|candidate| exhaustive(theory, candidate))
                    .collect();
                assert_eq!(
                    pool.check_batch(theory, &candidates, 100_000_000, &Cancellation::default())
                        .unwrap(),
                    expected,
                    "{workers}/{family:?}/{atoms}"
                );
            }
        }
    }
}

#[test]
fn zero_atoms_unused_atoms_and_empty_batches_have_exact_results() {
    let pool = pool(2, 8);
    for (atoms, nodes, roots) in [
        (0, vec![], vec![]),
        (0, vec![Node::False], vec![0]),
        (2, vec![], vec![]),
        (2, vec![Node::Atom(0)], vec![0]),
    ] {
        let theory = Theory::new(atoms, nodes, roots, AdmissionLimits::default()).unwrap();
        let candidates = all_candidates(&theory);
        let expected: Vec<_> = candidates
            .iter()
            .map(|candidate| exhaustive(&theory, candidate))
            .collect();
        assert_eq!(
            pool.check_batch(&theory, &candidates, 100_000_000, &Cancellation::default())
                .unwrap(),
            expected
        );
        assert!(
            pool.check_batch(&theory, &[], 0, &Cancellation::default())
                .unwrap()
                .is_empty()
        );
    }
}

#[test]
fn dimension_and_foreign_theory_refusals_leave_the_pool_reusable() {
    for (workers, candidates) in [(65, 1), (1, 0), (1, 4097)] {
        assert!(matches!(
            FormulaPool::new(NonZeroUsize::new(workers).unwrap(), candidates),
            Err(FormulaBenchmarkError::Dimensions)
        ));
    }
    let fixture = FormulaFixture::new(FormulaFamily::Cycle, 2).unwrap();
    let foreign = FormulaFixture::new(FormulaFamily::Cycle, 2).unwrap();
    let candidates = fixture.candidates(2, 0).unwrap();
    let pool = pool(2, 2);
    assert!(matches!(
        pool.check_batch(
            fixture.theory(),
            &fixture.candidates(3, 0).unwrap(),
            100_000_000,
            &Cancellation::default()
        ),
        Err(FormulaBenchmarkError::Dimensions)
    ));
    let mut mixed = candidates.clone();
    mixed[1] = foreign.candidates(2, 0).unwrap().remove(1);
    assert!(matches!(
        pool.check_batch(
            fixture.theory(),
            &mixed,
            100_000_000,
            &Cancellation::default()
        ),
        Err(FormulaBenchmarkError::Incomplete(Incomplete::WrongTheory))
    ));
    assert_eq!(
        pool.check_batch(
            fixture.theory(),
            &candidates,
            100_000_000,
            &Cancellation::default()
        )
        .unwrap(),
        [Membership::Stable, Membership::NonMinimal]
    );
}

#[test]
fn shared_cancellation_deadline_and_work_stops_never_return_partial_results() {
    let fixture = FormulaFixture::new(FormulaFamily::Cycle, 2).unwrap();
    let candidates = fixture.candidates(2, 0).unwrap();
    let pool = pool(2, 2);
    let cancelled = Cancellation::default();
    cancelled.cancel();
    for (cancellation, expected) in [
        (cancelled, Incomplete::Cancelled),
        (
            Cancellation::with_deadline(Instant::now()).unwrap(),
            Incomplete::Deadline,
        ),
    ] {
        for batch in [&candidates[..], &[]] {
            assert!(matches!(
                pool.check_batch(fixture.theory(), batch, 100_000_000, &cancellation),
                Err(FormulaBenchmarkError::Incomplete(actual)) if actual == expected
            ));
        }
        // The worker entry itself must use the caller's control, not silently
        // replace it with a fresh uncancelled control.
        assert!(matches!(
            native_with_cancellation(fixture.theory(), &candidates[1], 100_000_000, &cancellation),
            Err(FormulaBenchmarkError::Incomplete(actual)) if actual == expected
        ));
    }
    let cancellation = Cancellation::default();
    assert!(matches!(
        pool.check_batch(fixture.theory(), &candidates, 0, &cancellation),
        Err(FormulaBenchmarkError::Incomplete(_))
    ));
    assert!(
        cancellation.poll().is_ok(),
        "one failed job must not cancel its owner"
    );
    assert_eq!(
        pool.check_batch(fixture.theory(), &candidates, 100_000_000, &cancellation)
            .unwrap(),
        [Membership::Stable, Membership::NonMinimal]
    );
}
