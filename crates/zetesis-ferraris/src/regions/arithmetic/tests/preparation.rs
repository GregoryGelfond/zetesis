use zetesis_algebra::Meter;

use zetesis_algebra::{Cancellation, Stop};

use super::super::{PreparationError, PreparationLimits, PreparationStatus, RootPreparation};
use crate::test_support::{constructed_theory, push};
use crate::{NodeView, RegionLimits, Theory, producers};

struct Quota {
    ceiling: u64,
    held: u64,
}

impl zetesis_algebra::Quota for Quota {
    type Error = Stop;
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
        &mut Meter::reserved(&mut quota),
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
        &mut Meter::reserved(&mut quota),
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
        &mut Meter::reserved(&mut quota),
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
    let full = RootPreparation::prepare_reserved(
        &theory,
        None,
        limits(),
        &cancellation,
        &mut Meter::reserved(&mut full_quota),
    );
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
            &mut Meter::reserved(&mut quota),
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
        &mut Meter::reserved(&mut quota),
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
        &mut Meter::reserved(&mut quota),
    );
    assert!(matches!(
        attempt.result,
        Err(PreparationError::Stopped(
            zetesis_algebra::Failure::Refused(Stop::WorkLimit)
        ))
    ));
    assert_eq!(attempt.parity.status, PreparationStatus::Incomplete);
    assert!(!attempt.parity.root_refuted);
    assert_eq!(attempt.work, quota.held);
}

#[test]
fn seeded_root_receipts_follow_actual_constructor_and_units() {
    use super::super::{CardinalitySystem, ParitySystem};
    use std::mem::size_of;
    use zetesis_algebra::region::Region;

    for atoms in [1usize, 2, 64, 65] {
        let theory = constructed_theory(atoms, |_| vec![0]);
        let cancellation = Cancellation::default();
        let extracted = producers(&theory, RegionLimits::default(), &cancellation).unwrap();
        let mut quota = Quota {
            ceiling: u64::MAX,
            held: 0,
        };
        let attempt = RootPreparation::prepare_reserved(
            &theory,
            extracted.producers.as_ref(),
            limits(),
            &cancellation,
            &mut Meter::reserved(&mut quota),
        );
        let mut prepared = attempt.result.unwrap().unwrap();
        assert!(!prepared.root_refuted());
        assert_eq!(prepared.parity.units().count(), atoms);
        // Empty-unit test (1), actual Region constructor (11+4W), each
        // distinct pivot's branch and decision (12), Some publication (1).
        let expected = 13
            + 4 * u64::try_from(atoms.div_ceil(64)).unwrap()
            + 12 * u64::try_from(atoms).unwrap();
        assert_eq!(attempt.parity.phases.root_seed, expected, "atoms {atoms}");
        assert_eq!(attempt.cardinality.phases.root_seed, 0);
        assert_eq!(attempt.work, quota.held);
        assert_eq!(
            attempt.work,
            attempt.parity.phases.total() + attempt.cardinality.phases.total()
        );
        assert_eq!(
            attempt.parity.retained_bytes,
            prepared.parity.retained_bytes()
        );
        let cardinality = prepared.cardinality.as_ref().unwrap();
        assert_eq!(
            attempt.cardinality.retained_bytes,
            cardinality.retained_bytes()
        );
        // The actual composite owns its inline Option slots even after take.
        // Add only the allocations beyond each embedded system/header.
        let systems = size_of::<RootPreparation>() as u128 + prepared.parity.retained_bytes()
            - size_of::<ParitySystem>() as u128
            + cardinality.retained_bytes()
            - size_of::<CardinalitySystem>() as u128;
        let root = prepared.seeded_root.as_ref().unwrap();
        let masks = 2 * atoms.div_ceil(64) as u128 * size_of::<u64>() as u128;
        assert_eq!(root.retained_bytes() - size_of::<Region>() as u128, masks);
        assert!(attempt.peak_bytes >= systems + masks);
        assert_eq!(
            attempt.peak_bytes,
            attempt
                .parity
                .preparation_bytes
                .max(attempt.cardinality.preparation_bytes)
        );
        let seeded = prepared.take_seeded_root().unwrap();
        assert_eq!(seeded.held().collect::<Vec<_>>(), [0]);
        assert!((1..atoms).all(|atom| seeded.decision(atom) == Some(false)));
        assert!(prepared.take_seeded_root().is_none());
    }
}

#[test]
fn absent_and_refuted_seeds_do_not_charge_a_constructor() {
    for (atoms, refuted, seed_work) in [(4, false, 2), (3, true, 0)] {
        let theory = cycle(atoms);
        let mut quota = Quota {
            ceiling: u64::MAX,
            held: 0,
        };
        let attempt = RootPreparation::prepare_reserved(
            &theory,
            None,
            limits(),
            &Cancellation::default(),
            &mut Meter::reserved(&mut quota),
        );
        let mut prepared = attempt.result.unwrap().unwrap();
        assert_eq!(prepared.root_refuted(), refuted);
        assert!(prepared.take_seeded_root().is_none());
        assert_eq!(attempt.parity.phases.root_seed, seed_work);
        assert_eq!(attempt.cardinality.phases.root_seed, 0);
        assert_eq!(attempt.work, quota.held);
        assert_eq!(
            attempt.work,
            attempt.parity.phases.total() + attempt.cardinality.phases.total()
        );
    }
}

#[test]
fn prepared_equations_keep_padded_owned_words() {
    for atoms in [1usize, 63, 64, 65, 127, 128, 129] {
        let theory = constructed_theory(atoms, |nodes| {
            let falsum = push(nodes, NodeView::False);
            vec![push(nodes, NodeView::Implies(atoms - 1, falsum))]
        });
        let mut quota = Quota {
            ceiling: u64::MAX,
            held: 0,
        };
        let attempt = RootPreparation::prepare_reserved(
            &theory,
            None,
            limits(),
            &Cancellation::default(),
            &mut Meter::reserved(&mut quota),
        );
        assert_eq!(attempt.parity.rows, 1, "atoms {atoms}");
        let prepared = attempt.result.unwrap().unwrap();
        assert!(prepared.parity.belongs_to(&theory));
        let equation = &prepared.parity.recognized.equations[0];
        assert_eq!(equation.words.len(), atoms.div_ceil(128) * 2);
        for (word, &bits) in equation.words.iter().enumerate() {
            let expected = if word == (atoms - 1) / 64 {
                1 << ((atoms - 1) % 64)
            } else {
                0
            };
            assert_eq!(bits, expected, "atoms {atoms}, word {word}");
        }
        let allocation = (equation.words.as_ptr(), equation.words.capacity());
        let bytes = prepared.parity.retained_bytes();
        let moved = prepared.parity;
        let moved_equation = &moved.recognized.equations[0];
        assert_eq!(
            (
                moved_equation.words.as_ptr(),
                moved_equation.words.capacity()
            ),
            allocation
        );
        assert_eq!(moved.retained_bytes(), bytes);
    }
}

#[test]
fn equation_keeps_its_original_record_layout() {
    use super::super::rows::{Equation, Source};
    use std::mem::{align_of, offset_of, size_of};

    struct Original {
        words: Vec<u64>,
        constant: bool,
        source: Source,
    }
    assert_eq!(size_of::<Equation>(), size_of::<Original>());
    assert_eq!(align_of::<Equation>(), align_of::<Original>());
    assert_eq!(offset_of!(Equation, words), offset_of!(Original, words));
    assert_eq!(
        offset_of!(Equation, constant),
        offset_of!(Original, constant)
    );
    assert_eq!(offset_of!(Equation, source), offset_of!(Original, source));
}

#[test]
fn typed_preparation_refusal_preserves_the_same_failed_receipt() {
    use super::budget::{TypedQuota, TypedRefusal};
    use zetesis_algebra::Failure;

    let theory = cycle(3);
    let cancellation = Cancellation::default();
    let mut local_quota = Quota {
        ceiling: 64,
        held: 0,
    };
    let baseline = RootPreparation::prepare_reserved(
        &theory,
        None,
        limits(),
        &cancellation,
        &mut Meter::reserved(&mut local_quota),
    );
    assert!(matches!(
        baseline.result,
        Err(PreparationError::Stopped(Failure::Refused(Stop::WorkLimit)))
    ));
    let owner = Box::<str>::from("root-preparation");
    let identity = owner.as_ptr();
    let mut typed = TypedQuota::new([
        Ok(64),
        Err(TypedRefusal::Storage {
            owner,
            required: u128::MAX,
            limit: 43,
        }),
    ]);
    let attempted = RootPreparation::prepare_reserved(
        &theory,
        None,
        limits(),
        &cancellation,
        &mut Meter::reserved(&mut typed),
    );
    let Err(PreparationError::Stopped(Failure::Refused(TypedRefusal::Storage {
        owner,
        required,
        limit,
    }))) = attempted.result
    else {
        panic!("preparation must retain the caller's owned refusal")
    };
    assert_eq!(owner.as_ptr(), identity);
    assert_eq!(&*owner, "root-preparation");
    assert_eq!((required, limit), (u128::MAX, 43));
    assert_eq!(attempted.work, baseline.work);
    assert!(attempted.work > 0);
    assert_eq!(attempted.peak_bytes, baseline.peak_bytes);
    assert_eq!(attempted.parity, baseline.parity);
    assert_eq!(attempted.cardinality, baseline.cardinality);
    assert_eq!(typed.granted - typed.refunded, attempted.work);
    assert_eq!(local_quota.held, attempted.work);
    assert_eq!(attempted.parity.status, PreparationStatus::Incomplete);
    assert_eq!(attempted.parity.retained_bytes, 0);
    assert!(!attempted.parity.root_refuted);
}
