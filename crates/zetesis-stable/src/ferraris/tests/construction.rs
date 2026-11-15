//! Construction failures preserve the same typed error and settled prefix.
use super::*;

fn attempts(theory: &Theory, limits: Limits) -> Vec<ConstructionAttempt> {
    let workers = NonZeroUsize::new(2).unwrap();
    vec![
        StableModels::prepare(theory, limits, Cancellation::default()),
        StableModels::prepare_with_region_workers(theory, workers, limits, Cancellation::default()),
        StableModels::prepare_with_region_producers(
            theory,
            workers,
            limits,
            Cancellation::default(),
        ),
    ]
}

#[test]
fn arithmetic_refusal_retains_its_prefix_on_all_routes() {
    let owner = choice();
    let full = StableModels::prepare(&owner, Limits::default(), Cancellation::default());
    assert!(full.result.is_ok());
    let regions = full.statistics.regions.unwrap();
    let arithmetic = regions.parity.preparation_work
        + regions.cardinality.preparation_work
        + regions.images.preparation_work
        + regions.images.copying_work
        + regions.images.closure_work;
    assert!(arithmetic > 2);
    let extraction = full.statistics.search.work - arithmetic - regions.generator_order.work;
    let mut limits = Limits::default();
    limits.search.max_work = extraction + arithmetic / 2;
    for attempt in attempts(&owner, limits) {
        assert!(matches!(attempt.result, Err(Incomplete::WorkLimit)));
        let receipt = attempt.statistics.regions.unwrap();
        assert!(receipt.parity.preparation_work > 0);
        assert_eq!(receipt.parity.status, crate::PreparationStatus::Incomplete);
        assert_eq!(receipt.images, crate::RootImageStatistics::default());
        assert_eq!(receipt.counts.regions, 0);
        assert_eq!(receipt.counts.work, attempt.statistics.search.work);
        assert_eq!(
            attempt.statistics.search.work,
            extraction
                + receipt.parity.preparation_work
                + receipt.cardinality.preparation_work
                + receipt.images.preparation_work
                + receipt.images.copying_work
                + receipt.images.closure_work
        );
        assert!(attempt.statistics.search.work <= limits.search.max_work);
        assert_eq!(
            receipt.parity.phases.total(),
            receipt.parity.preparation_work
        );
        assert_eq!(
            receipt.cardinality.phases.total(),
            receipt.cardinality.preparation_work
        );
        assert_eq!(receipt.parity.retained_bytes, 0);
        assert_eq!(receipt.cardinality.retained_bytes, 0);
        assert_eq!(attempt.statistics.candidates, 0);
    }
}

#[test]
fn order_refusal_keeps_the_completed_arithmetic_prefix() {
    let owner = choice();
    let full = StableModels::prepare(&owner, Limits::default(), Cancellation::default());
    assert!(full.result.is_ok());
    let prepared = full.statistics.regions.unwrap();
    assert!(prepared.generator_order.work > 0);
    let mut limits = Limits::default();
    limits.search.max_work = full.statistics.search.work - 1;
    let attempt = StableModels::prepare(&owner, limits, Cancellation::default());
    assert!(matches!(attempt.result, Err(Incomplete::WorkLimit)));
    let receipt = attempt.statistics.regions.unwrap();
    assert_eq!(
        receipt.generator_order.status,
        GeneratorOrderStatus::Incomplete
    );
    assert_eq!(
        receipt.parity.preparation_work,
        prepared.parity.preparation_work
    );
    assert_eq!(
        receipt.cardinality.preparation_work,
        prepared.cardinality.preparation_work
    );
    assert_eq!(receipt.counts.work, attempt.statistics.search.work);
    assert_eq!(receipt.generator_order.retained_bytes, 0);
    assert_eq!(receipt.parity.retained_bytes, 0);
    assert_eq!(receipt.cardinality.retained_bytes, 0);
}

#[test]
fn a_successful_attempt_is_the_streams_initial_receipt() {
    for attempt in attempts(&choice(), Limits::default()) {
        let models = attempt.result.unwrap();
        assert_eq!(attempt.statistics, models.statistics());
        assert!(!models.exhausted());
    }
}

#[test]
fn early_cancellation_keeps_its_original_identity() {
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let attempt = StableModels::prepare(&choice(), Limits::default(), cancellation);
    assert!(matches!(attempt.result, Err(Incomplete::Cancelled)));
    assert_eq!(attempt.statistics.search.work, 0);
    assert_eq!(
        attempt.statistics.regions.unwrap().parity.status,
        crate::PreparationStatus::NotPrepared
    );
}
