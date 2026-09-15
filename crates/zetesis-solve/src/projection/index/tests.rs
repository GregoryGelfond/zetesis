use zetesis_cpu::{Control, Stop};

use super::{History, Slot, first_slot, hash};
use crate::projection::{
    ProjectionError, ProjectionLimits, ProjectionResource, ProjectionStatistics, Work,
};

const BASE: u128 = size_of::<History>() as u128;

fn statistics() -> ProjectionStatistics {
    ProjectionStatistics {
        retained_bytes: BASE,
        peak_bytes: BASE,
        ..ProjectionStatistics::default()
    }
}

fn insert(
    history: &mut History,
    key: &[u64],
    limits: ProjectionLimits,
    stats: &mut ProjectionStatistics,
) -> Result<bool, ProjectionError> {
    history.insert(
        key,
        BASE,
        &mut Work {
            limits,
            stats,
            control: &Control::default(),
        },
    )
}

fn seeded() -> (History, ProjectionStatistics) {
    let mut history = History::new(2);
    let mut stats = statistics();
    for key in [[7, 0], [7, u64::MAX]] {
        assert!(insert(&mut history, &key, ProjectionLimits::default(), &mut stats).unwrap());
    }
    (history, stats)
}

fn contains(history: &History, key: &[u64]) -> bool {
    let mut stats = statistics();
    let control = Control::default();
    let mut work = Work {
        limits: ProjectionLimits::default(),
        stats: &mut stats,
        control: &control,
    };
    let hash = hash(key, &mut work).unwrap();
    history.contains(key, hash, &mut work).unwrap()
}

#[test]
fn colliding_slots_preserve_complete_key_equality() {
    let mut history = History::new(2);
    let mut stats = statistics();
    let control = Control::default();
    let mut colliding = Vec::new();
    let mut buckets: [Vec<[u64; 2]>; 16] = std::array::from_fn(|_| Vec::new());
    let mut first = 0;
    // Select actual collisions under the current build's hasher, without
    // assuming a particular algorithm or persisting hashes as identities.
    for last in 0..128 {
        let mut work = Work {
            limits: ProjectionLimits::default(),
            stats: &mut stats,
            control: &control,
        };
        let key = [u64::MAX, last];
        let slot = first_slot(hash(&key, &mut work).unwrap(), 16).unwrap();
        buckets[slot].push(key);
        if buckets[slot].len() == 8 {
            first = slot;
            colliding = std::mem::take(&mut buckets[slot]);
            break;
        }
    }
    assert_eq!(colliding.len(), 8);
    for key in colliding.iter().rev() {
        assert!(insert(&mut history, key, ProjectionLimits::default(), &mut stats).unwrap());
    }
    assert_eq!(history.slots.len(), 16);
    assert!((0..8).all(|offset| history.slots[(first + offset) & 15].is_some()));
    let full = ProjectionLimits {
        max_keys: 8,
        ..ProjectionLimits::default()
    };
    for key in &colliding {
        assert!(contains(&history, key));
        assert!(!insert(&mut history, key, full, &mut stats).unwrap());
    }
    assert_eq!(history.entries, 8);
    assert_eq!(history.keys.len(), 16);
}

#[test]
fn every_failed_growth_prefix_preserves_keys_and_retries() {
    let (mut complete, mut stats) = seeded();
    let before = stats.work;
    assert!(
        insert(
            &mut complete,
            &[7, 9],
            ProjectionLimits::default(),
            &mut stats
        )
        .unwrap()
    );
    let inclusive = stats.work;
    let mut saw_key_growth = false;
    let mut saw_rehash = false;
    for max_work in before..inclusive {
        let (mut history, mut stats) = seeded();
        let old_keys = history.keys.clone();
        let old_slots = history.slots.len();
        let old_capacity = history.keys.capacity();
        let result = insert(
            &mut history,
            &[7, 9],
            ProjectionLimits {
                max_work,
                ..ProjectionLimits::default()
            },
            &mut stats,
        );
        assert!(matches!(
            result,
            Err(ProjectionError::Limit {
                resource: ProjectionResource::Work,
                ..
            })
        ));
        assert_eq!(stats.work, max_work);
        assert_eq!(history.entries, 2);
        assert_eq!(history.keys, old_keys);
        assert_eq!(stats.retained_bytes, history.retained(BASE).unwrap());
        assert!(contains(&history, &[7, 0]));
        assert!(contains(&history, &[7, u64::MAX]));
        assert!(!contains(&history, &[7, 9]));
        saw_key_growth |= history.keys.capacity() > old_capacity;
        saw_rehash |= history.slots.len() > old_slots;
        assert!(
            insert(
                &mut history,
                &[7, 9],
                ProjectionLimits::default(),
                &mut stats
            )
            .unwrap()
        );
        assert_eq!(history.entries, 3);
        assert!(contains(&history, &[7, 9]));
    }
    assert!(saw_key_growth && saw_rehash);
    let (mut history, mut stats) = seeded();
    assert!(
        insert(
            &mut history,
            &[7, 9],
            ProjectionLimits {
                max_work: inclusive,
                ..ProjectionLimits::default()
            },
            &mut stats
        )
        .unwrap()
    );
    assert_eq!(stats.work, inclusive);
}

#[test]
fn refused_byte_proposals_do_not_invent_capacity_peaks() {
    let (mut history, mut stats) = seeded();
    let retained = stats.retained_bytes;
    let peak = stats.peak_bytes;
    let keys = history.keys.clone();
    let slots = history.slots.clone();
    let limits = ProjectionLimits {
        max_bytes: usize::try_from(retained).unwrap(),
        ..ProjectionLimits::default()
    };
    for _ in 0..2 {
        assert!(matches!(
            insert(&mut history, &[7, 9], limits, &mut stats),
            Err(ProjectionError::Limit {
                resource: ProjectionResource::Bytes,
                ..
            })
        ));
        assert_eq!(history.entries, 2);
        assert_eq!(history.keys, keys);
        assert_eq!(history.slots, slots);
        assert_eq!((stats.retained_bytes, stats.peak_bytes), (retained, peak));
    }
    assert!(
        insert(
            &mut history,
            &[7, 9],
            ProjectionLimits::default(),
            &mut stats
        )
        .unwrap()
    );
}

#[test]
fn growth_receipts_cover_actual_replacement_capacities() {
    let mut history = History::new(3);
    let mut stats = statistics();
    let mut peak = BASE;
    for value in (0..33).rev() {
        let old_keys = history.keys.capacity();
        let old_slots = history.slots.capacity();
        assert!(
            insert(
                &mut history,
                &[value, 1, u64::MAX],
                ProjectionLimits::default(),
                &mut stats
            )
            .unwrap()
        );
        let new_keys = history.keys.capacity();
        let new_slots = history.slots.capacity();
        if old_keys != new_keys {
            peak = peak.max(
                BASE + ((old_keys + new_keys) * size_of::<u64>() + old_slots * size_of::<Slot>())
                    as u128,
            );
        }
        if old_slots != new_slots {
            peak = peak.max(
                BASE + (new_keys * size_of::<u64>() + (old_slots + new_slots) * size_of::<Slot>())
                    as u128,
            );
        }
        assert_eq!(stats.peak_bytes, peak);
        assert_eq!(
            stats.retained_bytes,
            BASE + (new_keys * size_of::<u64>() + new_slots * size_of::<Slot>()) as u128
        );
    }
    assert_eq!(history.entries, 33);
    assert_eq!(history.keys.len(), 99);
}

#[test]
fn zero_width_keys_need_no_word_or_index_allocation() {
    let mut history = History::new(0);
    let mut stats = statistics();
    let limits = ProjectionLimits {
        max_keys: 1,
        max_bytes: usize::try_from(BASE).unwrap(),
        ..ProjectionLimits::default()
    };
    assert!(insert(&mut history, &[], limits, &mut stats).unwrap());
    assert!(!insert(&mut history, &[], limits, &mut stats).unwrap());
    assert_eq!(history.entries, 1);
    assert_eq!(history.keys.capacity(), 0);
    assert_eq!(history.slots.capacity(), 0);
    assert_eq!((stats.retained_bytes, stats.peak_bytes), (BASE, BASE));
}

#[test]
fn cancellation_precedes_duplicate_lookup_and_work_refusal() {
    let (mut history, mut stats) = seeded();
    let old_work = stats.work;
    let control = Control::default();
    control.cancel();
    let mut work = Work {
        limits: ProjectionLimits {
            max_work: 0,
            ..ProjectionLimits::default()
        },
        stats: &mut stats,
        control: &control,
    };
    assert_eq!(
        history.insert(&[7, 0], BASE, &mut work),
        Err(ProjectionError::Control(Stop::Cancelled))
    );
    assert_eq!(history.entries, 2);
    assert_eq!(stats.work, old_work);
}
