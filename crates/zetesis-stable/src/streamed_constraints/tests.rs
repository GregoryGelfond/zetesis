//! Real region traversals check an admitted program's streamed constraints on
//! every route: each prepared checker serves its walk or producer, and checks
//! run on the routes' own threads.

use crate::region_filter::tests::injection::{Source as InjectedSource, Worker as InjectedWorker};
use std::collections::BTreeSet;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use zetesis_algebra::region::Region;
use zetesis_ferraris::Theory;
use zetesis_themelios::{ConstraintAllowance, ConstraintCheckLimits, HybridFormula};

use super::StreamedConstraints;
use crate::{
    BatchLimits, BatchVerdict, Cancellation, CertificateLimits, CertificateOrder, Engagement,
    Incomplete, Limits, RegionFeasibility, RegionWalkEngagement, StableModels,
};

// The repeated condition adds no restriction: p(X) already entails not not
// p(X). Its signed source occurrence keeps the complete six-row constraint
// streamed, so the walks check it rather than its native pair exclusions.
const STREAMED_AT_MOST_ONE: &str = "{p(1..4)}. :-p(X),p(Y),X<Y,not not p(X).";

/// The streamed constraints, counting which threads their checks run on.
#[derive(Debug)]
struct Counting {
    constraints: StreamedConstraints,
    caller: std::thread::ThreadId,
    checks: AtomicUsize,
    parallel_checks: AtomicUsize,
}

impl Counting {
    fn new(owner: &HybridFormula) -> Arc<Self> {
        Arc::new(Self {
            constraints: StreamedConstraints::new(
                owner.core().clone(),
                ConstraintAllowance::new(ConstraintCheckLimits::default()),
            ),
            caller: std::thread::current().id(),
            checks: AtomicUsize::new(0),
            parallel_checks: AtomicUsize::new(0),
        })
    }
}

impl InjectedSource for Counting {
    fn worker(
        &self,
        theory: &Theory,
        cancellation: &Cancellation,
    ) -> Result<Box<dyn InjectedWorker + '_>, Incomplete> {
        Ok(Box::new(CountingWorker {
            counting: self,
            inner: self.constraints.worker(theory, cancellation)?,
        }))
    }
}

struct CountingWorker<'a> {
    counting: &'a Counting,
    inner: Box<super::Worker<'a>>,
}

impl InjectedWorker for CountingWorker<'_> {
    fn check(
        &mut self,
        theory: &Theory,
        region: &Region,
        cancellation: &Cancellation,
    ) -> Result<RegionFeasibility, Incomplete> {
        self.counting.checks.fetch_add(1, Ordering::Relaxed);
        if std::thread::current().id() != self.counting.caller {
            self.counting
                .parallel_checks
                .fetch_add(1, Ordering::Relaxed);
        }
        self.inner.check(theory, region, cancellation)
    }
}

fn subsets() -> BTreeSet<Vec<usize>> {
    BTreeSet::from([vec![], vec![0], vec![1], vec![2], vec![3]])
}

#[test]
fn producer_slates_check_the_streamed_constraints() {
    let owner = zetesis_reference_support::hybrid(STREAMED_AT_MOST_ONE);
    assert_eq!(owner.streamed_templates(), 1);
    assert_eq!(owner.streamed_instances(), 6);
    for count in [2, 4] {
        let counting = Counting::new(&owner);
        let mut search = StableModels::with_region_producers(
            owner.core_theory(),
            NonZeroUsize::new(count).unwrap(),
            Limits::default(),
            Cancellation::default(),
        )
        .unwrap();
        search.set_region_filter(counting.clone()).unwrap();
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
        assert_eq!(
            models
                .iter()
                .map(|model| model.atoms().collect::<Vec<_>>())
                .collect::<BTreeSet<_>>(),
            subsets()
        );
        assert!(search.batch_statistics().checker_calls >= 3);
        let statistics = search.statistics().region_filter.unwrap();
        assert!(
            (1..=count as u64).contains(&statistics.preparations),
            "joined slates retain their checkers, at most one per producer"
        );
        assert!(counting.checks.load(Ordering::Relaxed) > 0);
        assert!(counting.parallel_checks.load(Ordering::Relaxed) > 0);
        assert!(counting.constraints.take_failure().is_none());
    }
}

#[test]
fn positive_checks_use_a_checker_per_traversal() {
    let owner = zetesis_reference_support::hybrid("p(1..3). :-p(X),p(Y),X>Y+2.");
    let counting = Counting::new(&owner);
    // Separate traversals keep one core but create new checkers.
    for _ in 0..2 {
        let mut search = StableModels::with_region_workers(
            owner.core_theory(),
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
        search.set_region_filter(counting.clone()).unwrap();
        let models: Vec<_> = search.by_ref().map(Result::unwrap).collect();
        assert_eq!(models.len(), 1);
        assert!(search.exhausted());
        assert_eq!(
            search.statistics().regions.unwrap().counts.regions,
            0,
            "the positive consequence set bypasses the candidate traversal"
        );
        assert_eq!(search.statistics().region_filter.unwrap().preparations, 1);
    }
    assert_eq!(counting.checks.load(Ordering::Relaxed), 2);
    assert_eq!(counting.parallel_checks.load(Ordering::Relaxed), 0);
}

#[test]
fn persistent_workers_check_the_streamed_constraints() {
    let owner = zetesis_reference_support::hybrid(STREAMED_AT_MOST_ONE);
    let counting = Counting::new(&owner);
    // A short search need not distribute work to every thread. Two traversals
    // guarantee distinct checkers without assuming a particular schedule. The
    // workers start at the first pull: under the default engagement a walk
    // this short would stay on the calling thread.
    let mut preparations = 0;
    for _ in 0..2 {
        let mut search = StableModels::with_region_workers_engaging(
            owner.core_theory(),
            NonZeroUsize::new(4).unwrap(),
            Engagement::Immediate,
            Limits::default(),
            Cancellation::default(),
        )
        .unwrap();
        search.set_region_filter(counting.clone()).unwrap();
        let models: Vec<_> = search.by_ref().map(Result::unwrap).collect();
        assert_eq!(models.len(), 5);
        assert_eq!(
            models
                .iter()
                .map(|model| model.atoms().collect::<Vec<_>>())
                .collect::<BTreeSet<_>>(),
            subsets()
        );
        assert!(search.exhausted());
        assert!(matches!(
            search
                .statistics()
                .regions
                .unwrap()
                .workers
                .unwrap()
                .engagement,
            RegionWalkEngagement::Engaged { launched: 4, .. }
        ));
        preparations += search.statistics().region_filter.unwrap().preparations;
    }
    assert!(preparations > 1);
    assert!(counting.checks.load(Ordering::Relaxed) > 0);
    assert!(counting.parallel_checks.load(Ordering::Relaxed) > 0);
}

#[test]
fn the_public_installer_checks_the_streamed_constraints() {
    // The installer callers have: data in, the admitted program's own check.
    let owner = zetesis_reference_support::hybrid(STREAMED_AT_MOST_ONE);
    let constraints = Arc::new(StreamedConstraints::new(
        owner.core().clone(),
        ConstraintAllowance::new(ConstraintCheckLimits::default()),
    ));
    let mut search = StableModels::new(
        owner.core_theory(),
        Limits::default(),
        Cancellation::default(),
    )
    .unwrap();
    search
        .set_streamed_constraints(constraints.clone())
        .unwrap();
    let models: BTreeSet<Vec<usize>> = search
        .by_ref()
        .map(|model| model.unwrap().atoms().collect())
        .collect();
    assert_eq!(models, subsets());
    assert!(search.exhausted());
    let statistics = search.statistics().region_filter.unwrap();
    assert_eq!(statistics.preparations, 1);
    assert!(statistics.refuted > 0);
    assert!(constraints.take_failure().is_none());
    assert_eq!(
        search.set_streamed_constraints(constraints),
        Err(Incomplete::ClosedEnumerator)
    );
}

#[test]
fn constraints_of_another_program_stop_with_their_failure() {
    let owner = zetesis_reference_support::hybrid(STREAMED_AT_MOST_ONE);
    let other = zetesis_reference_support::hybrid(STREAMED_AT_MOST_ONE);
    let constraints = Arc::new(StreamedConstraints::new(
        other.core().clone(),
        ConstraintAllowance::new(ConstraintCheckLimits::default()),
    ));
    let mut search = StableModels::new(
        owner.core_theory(),
        Limits::default(),
        Cancellation::default(),
    )
    .unwrap();
    search
        .set_streamed_constraints(constraints.clone())
        .unwrap();
    assert!(matches!(search.next(), Some(Err(Incomplete::RegionFilter))));
    assert!(!search.exhausted());
    assert!(matches!(
        constraints.take_failure().map(|failure| failure.cause),
        Some(zetesis_themelios::ConstraintCheckCause::WrongProgram)
    ));
}
