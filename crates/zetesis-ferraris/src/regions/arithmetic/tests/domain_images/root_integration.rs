//! Exact original-root publication and accounting for the two private policies.

use super::super::super::{
    PreparationAttempt, PreparationError, PreparationLimits, PreparationStatus, RootPreparation,
    images::Policy,
};
use super::super::budget::TypedRefusal;
use super::{contains, fixture};
use crate::{EvaluationLimits, EvaluationWorkspace, Region, Theory, test_support::world};
use zetesis_algebra::{Cancellation, Failure, Meter, Stop};

const SKIP: Policy = Policy {
    enabled: false,
    retain_on_decline: false,
};
const WHOLE: Policy = Policy {
    enabled: true,
    retain_on_decline: false,
};
const TRANSACTION: Policy = Policy {
    enabled: true,
    retain_on_decline: true,
};

fn limits() -> PreparationLimits {
    PreparationLimits {
        work: 100_000_000,
        bytes: usize::MAX,
    }
}

fn run(
    theory: &Theory,
    policy: Policy,
    limits: PreparationLimits,
) -> PreparationAttempt<Failure<Stop>> {
    let mut meter = Meter::local(u64::MAX);
    let attempt = RootPreparation::prepare_with_image_policy(
        theory,
        None,
        limits,
        &Cancellation::default(),
        &mut meter,
        policy,
    );
    exact(&attempt);
    assert_eq!(attempt.work, meter.charged());
    attempt
}

fn exact<E>(attempt: &PreparationAttempt<E>) {
    assert_eq!(
        attempt.parity.phases.total(),
        attempt.parity.preparation_work
    );
    assert_eq!(
        attempt.cardinality.phases.total(),
        attempt.cardinality.preparation_work
    );
    assert_eq!(
        attempt.work,
        attempt.parity.preparation_work
            + attempt.cardinality.preparation_work
            + attempt.images.preparation_work
            + attempt.images.copying_work
            + attempt.images.closure_work
    );
    assert!(attempt.peak_bytes >= attempt.images.peak_bytes);
}

fn shape(prepared: &RootPreparation) -> (bool, Option<&Region>) {
    (prepared.root_refuted(), prepared.seeded_root.as_ref())
}

fn owner() -> Theory {
    fixture(3, &[vec![0, 1]], &[(0, 2), (1, 2)])
}

#[test]
fn both_root_image_policies_publish_the_same_checked_cut() {
    let theory = owner();
    let before = run(&theory, SKIP, limits());
    let prior = before.result.unwrap().unwrap();
    assert!(!prior.root_refuted());
    assert!(
        prior
            .seeded_root
            .as_ref()
            .is_none_or(|root| !root.is_cut(2))
    );
    let whole = run(&theory, WHOLE, limits());
    let transaction = run(&theory, TRANSACTION, limits());
    assert_eq!(whole.images, transaction.images);
    assert_eq!(whole.images.status, PreparationStatus::Prepared);
    assert!(whole.images.groups > 0 && whole.images.pairs >= 2);
    assert!(whole.images.applied_cuts > 0 && whole.images.completed_rounds > 0);
    assert!(whole.images.copying_work > 0 && whole.images.closure_work > 0);
    assert_eq!(whole.parity, before.parity);
    assert_eq!(whole.cardinality, before.cardinality);
    let whole = whole.result.unwrap().unwrap();
    let transaction = transaction.result.unwrap().unwrap();
    assert_eq!(shape(&whole), shape(&transaction));
    assert!(whole.seeded_root.as_ref().unwrap().is_cut(2));
}

#[test]
fn root_images_preserve_every_original_model_in_tiny_relations() {
    let groups = [vec![0, 1], vec![2, 3]];
    let possible = [(0, 2), (0, 3), (1, 2), (1, 3)];
    for relation in 0..16 {
        let pairs: Vec<_> = possible
            .iter()
            .enumerate()
            .filter_map(|(index, &pair)| (relation & (1 << index) != 0).then_some(pair))
            .collect();
        let theory = fixture(4, &groups, &pairs);
        let mut evaluation = EvaluationWorkspace::default();
        let prepared = run(&theory, WHOLE, limits()).result.unwrap().unwrap();
        for model in 0..16 {
            let is_model = evaluation
                .evaluate(
                    &world(&theory, model),
                    EvaluationLimits::default(),
                    &Cancellation::default(),
                )
                .result
                .unwrap()
                .is_model();
            if is_model {
                assert!(!prepared.root_refuted());
                if let Some(root) = &prepared.seeded_root {
                    assert!(contains(root, model));
                }
            }
        }
    }
}

#[test]
fn every_optional_root_image_prefix_has_exact_receipts_and_no_partial_root() {
    let theory = owner();
    let baseline = run(&theory, SKIP, limits());
    let prior = baseline.result.unwrap().unwrap();
    let complete = run(&theory, WHOLE, limits());
    assert!(complete.work > baseline.work);
    let mut late_cut_refusals = 0;
    for work in baseline.work..complete.work {
        let whole = run(&theory, WHOLE, PreparationLimits { work, ..limits() });
        let transaction = run(&theory, TRANSACTION, PreparationLimits { work, ..limits() });
        assert!(whole.result.unwrap().is_none(), "whole allowance {work}");
        assert_eq!(whole.images.status, PreparationStatus::WorkDeclined);
        assert_eq!(whole.images, transaction.images);
        assert_eq!(whole.work, transaction.work);
        assert_eq!(whole.peak_bytes, transaction.peak_bytes);
        assert!(whole.work <= work);
        assert_eq!(whole.parity.retained_bytes, 0);
        assert_eq!(whole.cardinality.retained_bytes, 0);
        let retained = transaction.result.unwrap().unwrap();
        assert_eq!(
            shape(&retained),
            shape(&prior),
            "transaction allowance {work}"
        );
        assert_eq!(transaction.parity, baseline.parity);
        assert_eq!(transaction.cardinality, baseline.cardinality);
        if transaction.images.applied_cuts > 0 {
            late_cut_refusals += 1;
        }
    }
    assert!(
        late_cut_refusals > 0,
        "publication refusal must not leak a performed private cut"
    );
}

struct Quota {
    remaining: u64,
    charged: u64,
    refunded: u64,
    cancellation: Option<Cancellation>,
}
impl zetesis_algebra::Quota for Quota {
    type Error = TypedRefusal;
    fn reserve(&mut self, wanted: u64) -> Result<u64, Self::Error> {
        if self.remaining == 0 {
            if let Some(cancel) = &self.cancellation {
                cancel.cancel();
            }
            return Err(TypedRefusal::Storage {
                owner: Box::<str>::from("image quota"),
                required: 1,
                limit: 0,
            });
        }
        let granted = wanted.min(self.remaining);
        self.remaining -= granted;
        self.charged += granted;
        Ok(granted)
    }
    fn refund(&mut self, unspent: u64) {
        self.remaining += unspent;
        self.refunded += unspent;
    }
}

#[test]
fn image_mandatory_refusal_and_cancellation_never_publish_earlier_bundle() {
    let theory = owner();
    let complete = run(&theory, WHOLE, limits());
    for cancel in [false, true] {
        for policy in [WHOLE, TRANSACTION] {
            let cancellation = Cancellation::default();
            let mut quota = Quota {
                remaining: complete.work - 1,
                charged: 0,
                refunded: 0,
                cancellation: cancel.then(|| cancellation.clone()),
            };
            let attempt = RootPreparation::prepare_with_image_policy(
                &theory,
                None,
                limits(),
                &cancellation,
                &mut Meter::reserved(&mut quota),
                policy,
            );
            exact(&attempt);
            assert_eq!(attempt.work, quota.charged - quota.refunded);
            assert_eq!(attempt.images.status, PreparationStatus::Incomplete);
            assert_eq!(attempt.parity.retained_bytes, 0);
            assert_eq!(attempt.cardinality.retained_bytes, 0);
            if cancel {
                assert!(matches!(
                    attempt.result,
                    Err(PreparationError::Stopped(Failure::Stopped(Stop::Cancelled)))
                ));
            } else {
                assert!(matches!(
                    attempt.result,
                    Err(PreparationError::Stopped(Failure::Refused(TypedRefusal::Storage { owner, required: 1, limit: 0 }))) if owner.as_ref() == "image quota"
                ));
            }
        }
    }
}

#[test]
fn root_image_memory_ceiling_is_inclusive_and_preserves_transaction_on_decline() {
    let theory = owner();
    let complete = run(&theory, WHOLE, limits());
    let ceiling = usize::try_from(complete.peak_bytes).unwrap();
    let exact = run(
        &theory,
        WHOLE,
        PreparationLimits {
            bytes: ceiling,
            ..limits()
        },
    );
    assert_eq!(exact.images, complete.images);
    assert_eq!(exact.work, complete.work);
    let whole = run(
        &theory,
        WHOLE,
        PreparationLimits {
            bytes: ceiling - 1,
            ..limits()
        },
    );
    assert!(whole.result.unwrap().is_none());
    assert!(whole.peak_bytes > (ceiling - 1) as u128);
    let transaction = run(
        &theory,
        TRANSACTION,
        PreparationLimits {
            bytes: ceiling - 1,
            ..limits()
        },
    );
    if transaction.images.status == PreparationStatus::StorageDeclined {
        let prior = run(&theory, SKIP, limits()).result.unwrap().unwrap();
        assert_eq!(
            shape(transaction.result.as_ref().unwrap().as_ref().unwrap()),
            shape(&prior)
        );
    } else {
        // The earlier immutable preparation has the same indivisible byte contract.
        assert!(transaction.result.unwrap().is_none());
        assert_eq!(transaction.images.status, PreparationStatus::NotPrepared);
    }
}

struct ImageAttempt {
    result: Result<Option<RootPreparation>, PreparationError<Failure<Stop>>>,
    statistics: super::super::super::RootImageStatistics,
    before: Option<Region>,
}

/// Isolate the image byte ceiling from earlier preparation's larger scratch.
/// Fixture construction/snapshotting is not part of this runtime receipt.
fn isolated_image(theory: &Theory, policy: Policy, bytes: usize) -> ImageAttempt {
    use super::super::super::{budget::Budget, images};
    let baseline = run(theory, SKIP, limits());
    let prior = baseline.result.unwrap().unwrap();
    let before = prior.seeded_root.clone();
    let mut parity = baseline.parity;
    let mut cardinality = baseline.cardinality;
    let mut statistics = images::RootImageStatistics::default();
    let mut meter = Meter::local(u64::MAX);
    let cancellation = Cancellation::default();
    let mut budget = Budget::new(&mut meter, u64::MAX, bytes, &cancellation);
    let result = images::apply(
        Ok(Some(prior)),
        theory,
        policy,
        &mut parity,
        &mut cardinality,
        &mut statistics,
        &mut budget,
    );
    budget.finish();
    assert_eq!(
        budget.work(),
        statistics.preparation_work + statistics.copying_work + statistics.closure_work
    );
    assert_eq!(budget.peak_bytes(), statistics.peak_bytes);
    ImageAttempt {
        result,
        statistics,
        before,
    }
}

#[test]
fn image_only_storage_decline_retains_the_exact_previous_seed() {
    let theory = owner();
    let complete = isolated_image(&theory, WHOLE, usize::MAX);
    let bytes = usize::try_from(complete.statistics.peak_bytes).unwrap();
    assert_eq!(complete.statistics.status, PreparationStatus::Prepared);
    let inclusive = isolated_image(&theory, WHOLE, bytes);
    assert_eq!(inclusive.statistics, complete.statistics);
    assert_eq!(
        shape(inclusive.result.as_ref().unwrap().as_ref().unwrap()),
        shape(complete.result.as_ref().unwrap().as_ref().unwrap())
    );
    let whole = isolated_image(&theory, WHOLE, bytes - 1);
    let transaction = isolated_image(&theory, TRANSACTION, bytes - 1);
    assert_eq!(whole.statistics.status, PreparationStatus::StorageDeclined);
    assert_eq!(whole.statistics, transaction.statistics);
    assert!(whole.result.unwrap().is_none());
    let retained = transaction.result.unwrap().unwrap();
    assert!(!retained.root_refuted());
    assert_eq!(retained.seeded_root, transaction.before);
}

#[test]
fn completed_image_proof_survives_later_publication_refusal_without_a_root() {
    // Three mandatory finite domains with strict increase across each edge;
    // two values cannot supply an increasing assignment of length three.
    let theory = fixture(
        6,
        &[vec![0, 1], vec![2, 3], vec![4, 5]],
        &[(0, 2), (1, 2), (1, 3), (2, 4), (3, 4), (3, 5)],
    );
    let baseline = run(&theory, SKIP, limits());
    let prior = baseline.result.unwrap().unwrap();
    assert!(!prior.root_refuted());
    let complete = run(&theory, WHOLE, limits());
    assert_eq!(complete.images.status, PreparationStatus::Prepared);
    assert!(complete.images.root_refuted);
    assert!(!complete.parity.root_refuted && !complete.cardinality.root_refuted);
    assert!(complete.result.unwrap().unwrap().root_refuted());

    // The final charged operation is publication of the image-refuted flag.
    // Both variants have already completed the same semantic proof at this limit.
    let work = complete.work - 1;
    let whole = run(&theory, WHOLE, PreparationLimits { work, ..limits() });
    let transaction = run(&theory, TRANSACTION, PreparationLimits { work, ..limits() });
    assert_eq!(whole.images, transaction.images);
    assert_eq!(whole.work, work);
    assert_eq!(transaction.work, work);
    assert_eq!(whole.images.status, PreparationStatus::WorkDeclined);
    assert!(whole.images.root_refuted);
    assert!(whole.result.unwrap().is_none());
    let retained = transaction.result.unwrap().unwrap();
    assert!(!retained.root_refuted());
    assert_eq!(shape(&retained), shape(&prior));
    assert_eq!(transaction.parity, baseline.parity);
    assert_eq!(transaction.cardinality, baseline.cardinality);
}
