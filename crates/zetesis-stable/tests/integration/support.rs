//! Helpers shared by the integration test modules.

pub(crate) mod arithmetic;
pub(crate) mod batching;
pub(crate) mod bounds;
pub(crate) mod choice_theories;
pub(crate) mod formula_theories;
pub(crate) mod interpretations;
pub(crate) mod routes;

/// Exact work conservation for these region-walk fixtures. They use no source
/// filter, restrictions, bounds, determined-candidate bypass or panicking
/// worker. A work ceiling can stop an indivisible charge before it executes.
/// Original evaluations use their separate verification allowance; nested
/// support and preparation counts are already included in their owner totals.
pub(crate) fn assert_region_work_accounted(statistics: &zetesis_stable::Statistics) {
    assert_eq!(statistics.candidate_restrictions, 0);
    assert!(statistics.region_filter.is_none());
    let certificate = statistics.certified.map_or(0, |receipt| {
        receipt
            .construction_work
            .checked_add(receipt.checking_work)
            .unwrap()
    });
    let accounted = [
        statistics
            .regions
            .expect("the fixture uses a region walk")
            .counts
            .work,
        statistics.reduct.regions.work,
        statistics.reduct.frozen_positive_work,
        statistics.search.decisions,
        certificate,
    ]
    .into_iter()
    .try_fold(0_u64, u64::checked_add)
    .expect("the finite fixture's work fits the receipt");
    assert_eq!(
        statistics.search.work, accounted,
        "the budget equals each performed owner's receipt exactly once: {statistics:?}"
    );
}
