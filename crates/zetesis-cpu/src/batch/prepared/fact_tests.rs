//! Collective cache accounting shares the actual immutable fact base once.

use super::*;
use zetesis_core::Seed;
use zetesis_test_support::programs::fact_tables;

#[test]
fn collective_cache_counts_one_closed_fact_base() {
    let [_, program] = fact_tables::interleaved();
    let cancellation = Cancellation::default();
    let mut cache = Cache::default();
    cache
        .prepare(
            &program,
            None,
            3,
            PreparationLimits::default(),
            usize::MAX,
            &cancellation,
        )
        .unwrap();
    let prepared = Arc::clone(cache.prepared.as_ref().unwrap());
    let seed = Seed::new(&program, []).unwrap();
    for workspace in &mut cache.workspaces {
        prepared
            .check_view(seed.view(), workspace, Limits::default(), &cancellation)
            .unwrap();
    }
    let shared = cache.shared_facts();
    assert!(shared > 0);
    assert!(
        cache
            .workspaces
            .iter()
            .all(|workspace| workspace.shared_fact_bytes() == shared)
    );
    let individual: u128 = cache
        .workspaces
        .iter()
        .map(|workspace| workspace.retained_bytes().unwrap())
        .sum();
    assert_eq!(
        cache.retained().unwrap(),
        cache.overhead() + individual - 2 * shared
    );
    let limits = Limits::default();
    let candidate =
        limits.max_closure_bytes as u128 - prepared.statistics().retained_bytes as u128 - shared;
    let idle = cache.workspaces[2].retained_bytes().unwrap() - shared;
    let expected = cache.overhead() + shared + 2 * candidate + idle;
    cache
        .admit(2, limits, usize::try_from(expected).unwrap())
        .unwrap();
    assert_eq!(cache.statistics().unwrap().reserved_bytes, expected);
    assert!(
        matches!(cache.admit(2, limits, usize::try_from(expected - 1).unwrap()), Err(BatchError::ClosureStorage {required, limit}) if required == expected && limit == expected - 1)
    );
}
