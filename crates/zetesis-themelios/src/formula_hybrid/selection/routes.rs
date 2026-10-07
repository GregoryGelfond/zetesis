//! Real region traversals must borrow the streamed core's prepared rows.

use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use zetesis_cpu::{Cancellation, regions::Region};
use zetesis_ferraris::Theory;
use zetesis_sat::{
    BatchLimits, BatchVerdict, CertificateLimits, CertificateOrder, Incomplete, Limits,
    RegionFeasibility, RegionFilter, RegionFilterWorker, StableModels,
};

use crate::{
    AdmissionOptions, ConstraintCheckLimits, ConstraintChecker, ConstraintRegionVerdict,
    ExpansionLimits, FormulaLimits, HybridFormula, prepare_formula,
};

#[derive(Debug)]
struct Filter {
    owner: HybridFormula,
    workers: AtomicUsize,
    checks: AtomicUsize,
    caller: std::thread::ThreadId,
    parallel_checks: AtomicUsize,
}

impl Filter {
    fn new(source: &str) -> Self {
        let owner = prepare_formula(
            source.into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap()
        .ground_hybrid()
        .unwrap();
        assert!(owner.core().streamed_templates() > 0);
        Self {
            owner,
            workers: AtomicUsize::new(0),
            checks: AtomicUsize::new(0),
            caller: std::thread::current().id(),
            parallel_checks: AtomicUsize::new(0),
        }
    }
}

impl RegionFilter for Filter {
    fn worker(
        &self,
        theory: &Theory,
        _: &Cancellation,
    ) -> Result<Box<dyn RegionFilterWorker + '_>, Incomplete> {
        assert!(theory.same_instance(self.owner.core_theory()));
        self.workers.fetch_add(1, Ordering::Relaxed);
        Ok(Box::new(Worker {
            filter: self,
            checker: self
                .owner
                .checker(ConstraintCheckLimits::default())
                .unwrap(),
        }))
    }
}

struct Worker<'a> {
    filter: &'a Filter,
    checker: ConstraintChecker<'a>,
}

impl RegionFilterWorker for Worker<'_> {
    fn check(
        &mut self,
        theory: &Theory,
        region: &Region,
        cancellation: &Cancellation,
    ) -> Result<RegionFeasibility, Incomplete> {
        let verdict = self
            .checker
            .check_region(theory, region, cancellation)
            .unwrap();
        let core = self.filter.owner.core();
        let prepared = self.checker.prepared.as_ref().unwrap();
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
        self.filter.checks.fetch_add(1, Ordering::Relaxed);
        if std::thread::current().id() != self.filter.caller {
            self.filter.parallel_checks.fetch_add(1, Ordering::Relaxed);
        }
        Ok(match verdict {
            ConstraintRegionVerdict::NotRefuted => RegionFeasibility::NotRefuted,
            ConstraintRegionVerdict::Refuted { .. } => RegionFeasibility::Refuted,
        })
    }
}

#[test]
fn producer_rounds_borrow_the_core_selection() {
    for count in [2, 4] {
        let filter = Arc::new(Filter::new("{p(1..4)}. :-p(X),p(Y),X<Y."));
        let mut search = StableModels::with_region_producers(
            filter.owner.core_theory(),
            NonZeroUsize::new(count).unwrap(),
            Limits::default(),
            Cancellation::default(),
        )
        .unwrap();
        search.set_region_filter(filter.clone()).unwrap();
        let mut models = Vec::new();
        for _ in 0..8 {
            models.extend(
                search
                    .next_batch(
                        BatchLimits {
                            max_candidates: NonZeroUsize::new(2).unwrap(),
                            max_pending_bytes: 4096,
                        },
                        |_, candidates| {
                            Ok::<_, std::convert::Infallible>(vec![
                                BatchVerdict::Residual;
                                candidates.len()
                            ])
                        },
                    )
                    .unwrap(),
            );
            if search.exhausted() {
                break;
            }
        }
        assert!(search.exhausted());
        assert_eq!(models.len(), 5);
        assert!(search.batch_statistics().checker_calls >= 3);
        assert!(
            filter.workers.load(Ordering::Relaxed) >= 3,
            "more than one round created actual source workers"
        );
        assert!(filter.checks.load(Ordering::Relaxed) > 0);
        assert!(filter.parallel_checks.load(Ordering::Relaxed) > 0);
    }
}

#[test]
fn positive_checks_borrow_the_core_selection() {
    let filter = Arc::new(Filter::new("p(1..3). :-p(X),p(Y),X>Y+2."));
    // Separate traversals keep one core but create new source checkers.
    for _ in 0..2 {
        let mut search = StableModels::with_region_workers(
            filter.owner.core_theory(),
            NonZeroUsize::MIN,
            Limits::default(),
            Cancellation::default(),
        )
        .unwrap();
        assert!(
            search
                .enable_class_checking(
                    CertificateLimits::default(),
                    CertificateOrder::PositiveFirst,
                )
                .unwrap()
        );
        search.set_region_filter(filter.clone()).unwrap();
        let models: Vec<_> = search.by_ref().map(Result::unwrap).collect();
        assert_eq!(models.len(), 1);
        assert!(search.exhausted());
        assert_eq!(
            search.statistics().regions.unwrap().counts.regions,
            0,
            "the positive consequence set bypasses the candidate traversal"
        );
    }
    assert_eq!(filter.workers.load(Ordering::Relaxed), 2);
    assert_eq!(filter.checks.load(Ordering::Relaxed), 2);
    assert_eq!(filter.parallel_checks.load(Ordering::Relaxed), 0);
}

#[test]
fn persistent_workers_borrow_the_core_selection() {
    let filter = Arc::new(Filter::new("{p(1..4)}. :-p(X),p(Y),X<Y."));
    // A short search need not distribute work to every thread. Two traversals
    // guarantee distinct checkers without assuming a particular schedule.
    for _ in 0..2 {
        let mut search = StableModels::with_region_workers(
            filter.owner.core_theory(),
            NonZeroUsize::new(4).unwrap(),
            Limits::default(),
            Cancellation::default(),
        )
        .unwrap();
        search.set_region_filter(filter.clone()).unwrap();
        let models: Vec<_> = search.by_ref().map(Result::unwrap).collect();
        assert_eq!(models.len(), 5);
        assert!(search.exhausted());
    }
    assert!(filter.workers.load(Ordering::Relaxed) > 1);
    assert!(filter.checks.load(Ordering::Relaxed) > 0);
    assert!(filter.parallel_checks.load(Ordering::Relaxed) > 0);
}
