use zetesis_cpu::{Cancellation, Stop};

use super::super::{PreparationError, PreparationLimits, PreparationStatus, RootPreparation};
use crate::test_support::{constructed_theory, push};
use crate::{NarrowingQuota, NodeView, RegionLimits, Theory, producers};

struct Quota {
    ceiling: u64,
    held: u64,
}

impl NarrowingQuota for Quota {
    fn reserve(&mut self, wanted: u64) -> Result<u64, Stop> {
        let granted = wanted.min(self.ceiling - self.held);
        if granted == 0 {
            return Err(Stop::WorkLimit);
        }
        self.held += granted;
        Ok(granted)
    }
    fn refund(&mut self, unspent: u64) {
        self.held -= unspent;
    }
}

fn limits() -> PreparationLimits {
    PreparationLimits {
        work: PreparationLimits::DEFAULT_WORK,
        bytes: usize::MAX,
    }
}

fn cycle(atoms: usize) -> Theory {
    constructed_theory(atoms, |nodes| {
        let falsum = push(nodes, NodeView::False);
        let negative: Vec<_> = (0..atoms)
            .map(|atom| push(nodes, NodeView::Implies(atom, falsum)))
            .collect();
        let mut roots = Vec::new();
        for left in 0..atoms {
            let right = (left + 1) % atoms;
            for terms in [[left, right], [negative[left], negative[right]]] {
                let body = push(nodes, NodeView::And(&terms));
                roots.push(push(nodes, NodeView::Implies(body, falsum)));
            }
        }
        roots
    })
}

#[test]
fn odd_cycle_refutation_skips_cardinality_preparation() {
    let theory = cycle(3);
    let mut quota = Quota {
        ceiling: u64::MAX,
        held: 0,
    };
    let result = RootPreparation::prepare_reserved(
        &theory,
        None,
        limits(),
        &Cancellation::default(),
        &mut quota,
    );
    assert_eq!(result.parity.status, PreparationStatus::Prepared);
    assert_eq!(result.cardinality.status, PreparationStatus::NotPrepared);
    assert!(result.parity.root_refuted);
    assert_eq!(result.parity.rows, 3);
    assert_eq!(result.parity.two, 3);
    assert_eq!(result.work, result.parity.preparation_work);
    assert_eq!(result.parity.phases.total(), result.parity.preparation_work);
    assert_eq!(result.work, quota.held);
    assert!(result.result.unwrap().unwrap().root_refuted());
}

#[test]
fn even_cycle_retains_one_owner_and_no_echelon_units() {
    let theory = cycle(4);
    let foreign = cycle(4);
    let mut quota = Quota {
        ceiling: u64::MAX,
        held: 0,
    };
    let result = RootPreparation::prepare_reserved(
        &theory,
        None,
        limits(),
        &Cancellation::default(),
        &mut quota,
    );
    assert_eq!(result.work, quota.held);
    assert_eq!(
        result.work,
        result.parity.preparation_work + result.cardinality.preparation_work
    );
    assert_eq!(result.parity.phases.total(), result.parity.preparation_work);
    assert_eq!(
        result.cardinality.phases.total(),
        result.cardinality.preparation_work
    );
    assert_eq!(result.parity.rank, 3);
    let prepared = result.result.unwrap().unwrap();
    assert!(!prepared.root_refuted());
    assert_eq!(prepared.parity.units().count(), 0);
    assert!(prepared.parity.belongs_to(&theory));
    assert!(!prepared.parity.belongs_to(&foreign));
    let cardinality = prepared.cardinality.unwrap();
    assert!(cardinality.belongs_to(&theory));
    assert!(!cardinality.belongs_to(&foreign));
}

#[test]
fn complete_definitions_force_a_fact_and_an_unsupported_atom() {
    let theory = constructed_theory(2, |_| vec![0]);
    let cancellation = Cancellation::default();
    let extracted = producers(&theory, RegionLimits::default(), &cancellation).unwrap();
    let mut quota = Quota {
        ceiling: u64::MAX,
        held: 0,
    };
    let result = RootPreparation::prepare_reserved(
        &theory,
        extracted.producers.as_ref(),
        limits(),
        &cancellation,
        &mut quota,
    );
    let mut prepared = result.result.unwrap().unwrap();
    assert_eq!(
        prepared.parity.units().collect::<Vec<_>>(),
        [(0, true), (1, false)]
    );
    assert_eq!(
        (result.parity.rows, result.parity.one, result.parity.units),
        (2, 2, 2)
    );
    assert_eq!(result.parity.phases.total(), result.parity.preparation_work);
    assert_eq!(
        result.cardinality.phases.total(),
        result.cardinality.preparation_work
    );
    assert!(result.parity.phases.root_seed > 0);
    let seeded = prepared.take_seeded_root().unwrap();
    assert_eq!(seeded.held().collect::<Vec<_>>(), [0]);
    assert!(!seeded.is_open(1));
    assert!(prepared.take_seeded_root().is_none());
}

#[test]
fn every_declined_work_prefix_refunds_spare_permits() {
    let theory = cycle(3);
    let cancellation = Cancellation::default();
    let mut full_quota = Quota {
        ceiling: u64::MAX,
        held: 0,
    };
    let full =
        RootPreparation::prepare_reserved(&theory, None, limits(), &cancellation, &mut full_quota);
    assert!(full.result.unwrap().unwrap().root_refuted());
    for work in 0..full.work {
        let mut quota = Quota {
            ceiling: u64::MAX,
            held: 0,
        };
        let attempt = RootPreparation::prepare_reserved(
            &theory,
            None,
            PreparationLimits { work, ..limits() },
            &cancellation,
            &mut quota,
        );
        assert!(attempt.result.unwrap().is_none(), "allowance {work}");
        assert_eq!(attempt.parity.status, PreparationStatus::WorkDeclined);
        assert!(attempt.work <= work);
        assert_eq!(attempt.work, quota.held);
        assert_eq!(
            attempt.parity.phases.total(),
            attempt.parity.preparation_work
        );
        assert_eq!(attempt.parity.retained_bytes, 0);
    }
}

#[test]
fn storage_decline_never_publishes_a_partial_preparation() {
    let theory = cycle(3);
    let mut quota = Quota {
        ceiling: u64::MAX,
        held: 0,
    };
    let attempt = RootPreparation::prepare_reserved(
        &theory,
        None,
        PreparationLimits {
            bytes: 0,
            ..limits()
        },
        &Cancellation::default(),
        &mut quota,
    );
    assert!(attempt.result.unwrap().is_none());
    assert_eq!(attempt.parity.status, PreparationStatus::StorageDeclined);
    assert!(attempt.peak_bytes > 0);
    assert_eq!(attempt.work, quota.held);
    assert_eq!(
        attempt.parity.phases.total(),
        attempt.parity.preparation_work
    );
}

#[test]
fn mandatory_refusal_is_not_optional_decline_or_refutation() {
    let theory = cycle(3);
    let mut quota = Quota {
        ceiling: 5,
        held: 0,
    };
    let attempt = RootPreparation::prepare_reserved(
        &theory,
        None,
        limits(),
        &Cancellation::default(),
        &mut quota,
    );
    assert!(matches!(
        attempt.result,
        Err(PreparationError::Stopped(Stop::WorkLimit))
    ));
    assert_eq!(attempt.parity.status, PreparationStatus::Incomplete);
    assert!(!attempt.parity.root_refuted);
    assert_eq!(attempt.work, quota.held);
}
