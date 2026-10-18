//! A partial worker lease is not a complete positive-kernel allowance.

use super::*;
use crate::search::{LocalQuota, SharedBudget};
use crate::{SearchLimits, SearchStatistics};

fn facts() -> Theory {
    let atoms = 2_048;
    Theory::new(
        atoms,
        FormulaParts::new((0..atoms).map(Node::atom).collect(), vec![]).unwrap(),
        (0..atoms).collect(),
        AdmissionLimits::default(),
    )
    .unwrap()
}

fn check(
    theory: &Theory,
    quota: impl Quota,
    max_work: u64,
) -> (Result<Check, Incomplete>, Statistics, SearchStatistics) {
    let cancellation = Cancellation::default();
    let index = IndexedTheory::for_membership_test(theory);
    let candidate = Interpretation::new(theory, 0..theory.atom_count()).unwrap();
    let limits = Limits {
        search: SearchLimits {
            max_work,
            ..SearchLimits::default()
        },
        ..Limits::default()
    };
    let mut budget = Budget {
        quota,
        limits: limits.search,
        cancellation: &cancellation,
        statistics: SearchStatistics::default(),
    };
    let mut state = State::new(SearchMethod::Regions);
    let mut statistics = Statistics::default();
    // The kernel starts with a partly consumed worker grant, as it does after
    // candidate narrowing. Reserving must settle that grant before waiting.
    let result = budget.tick().and_then(|()| {
        state.check(
            theory,
            Some(&index),
            &candidate,
            limits,
            &mut budget,
            &mut statistics,
        )
    });
    (result, statistics, budget.statistics)
}

fn grant_size() -> u64 {
    let cancellation = Cancellation::default();
    let shared = SharedBudget::new(SearchLimits::default(), SearchStatistics::default());
    let lease = shared.lease(&cancellation);
    let grant = lease.take_up_to(u64::MAX).unwrap();
    lease.give_back(grant);
    grant
}

fn shared_check(
    theory: &Theory,
    max_work: u64,
) -> (Result<Check, Incomplete>, Statistics, SearchStatistics) {
    let cancellation = Cancellation::default();
    let limits = SearchLimits {
        max_work,
        ..SearchLimits::default()
    };
    let shared = SharedBudget::new(limits, SearchStatistics::default());
    let (result, statistics, search) = check(theory, shared.lease(&cancellation), max_work);
    let mut joined = SearchStatistics::default();
    shared.record(&mut joined);
    assert_eq!(joined, search, "the whole reservation refunds unused work");
    (result, statistics, search)
}

#[test]
fn positive_closure_can_exceed_one_worker_grant() {
    let theory = facts();
    let (expected, local, local_work) = check(&theory, LocalQuota, u64::MAX);
    assert!(matches!(expected, Ok(Check::Stable)));
    assert!(local.reduct.frozen_positive_work > grant_size());
    let (result, actual, shared_work) = shared_check(&theory, u64::MAX);
    assert!(matches!(result, Ok(Check::Stable)), "{result:?}");
    assert_eq!(actual.countermodel_queries, 1);
    assert_eq!(actual.reduct.frozen_positive_attempts, 1);
    assert_eq!(actual.reduct.frozen_positive_completed, 1);
    assert_eq!(
        actual.reduct.frozen_positive_work,
        local.reduct.frozen_positive_work
    );
    assert_eq!(actual.reduct.regions.regions, 0);
    assert_eq!(shared_work, local_work);
    assert_eq!(shared_work.work, actual.reduct.frozen_positive_work + 1);
}

#[test]
fn positive_closure_keeps_exact_global_work_refusals() {
    let theory = facts();
    let (result, _, full) = check(&theory, LocalQuota, u64::MAX);
    assert!(matches!(result, Ok(Check::Stable)));
    for ceiling in [1, grant_size(), full.work - 1, full.work] {
        let (local, local_receipt, local_work) = check(&theory, LocalQuota, ceiling);
        let (shared, shared_receipt, shared_work) = shared_check(&theory, ceiling);
        for result in [local, shared] {
            if ceiling < full.work {
                assert!(matches!(result, Err(Incomplete::WorkLimit)), "{result:?}");
            } else {
                assert!(matches!(result, Ok(Check::Stable)), "{result:?}");
            }
        }
        assert_eq!(local_work, shared_work);
        assert_eq!(shared_work.work, ceiling);
        assert_eq!(local_receipt.countermodel_queries, 1);
        assert_eq!(shared_receipt.countermodel_queries, 1);
        assert_eq!(
            shared_receipt.reduct.frozen_positive_work,
            local_receipt.reduct.frozen_positive_work
        );
        assert_eq!(
            shared_receipt.reduct.frozen_positive_completed,
            u64::from(ceiling == full.work)
        );
    }
}
