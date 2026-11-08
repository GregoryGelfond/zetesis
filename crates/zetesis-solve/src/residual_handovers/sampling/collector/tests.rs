use super::{Batch, Group, RunSamples, World, capacities};

#[test]
fn a_known_reason_width_can_bound_earlier_mixed_batches() {
    let batch = |counts: [u64; 4], width| Batch {
        residuals: counts.map(Some),
        width: Some(width),
        times: [None; 4],
    };
    let mut batches = [batch([2, 1, 1, 0], 8), batch([1, 1, 0, 0], 8)];
    let mut fixed = Group::default();
    let mut round = Group::default();
    for &entry in &batches {
        fixed.batch(entry, 1);
        round.batch(entry, 2);
    }
    assert_eq!(
        fixed.finish(1, &mut [], &mut batches).width.maximum,
        Some(8)
    );
    assert_eq!(round.finish(2, &mut [], &mut batches).width.maximum, None);
}

#[test]
fn capacity_targets_match_the_funded_record_sizes() {
    let full = 65_536 * size_of::<World>() + 4_096 * size_of::<Batch>();
    assert_eq!(capacities(full as u128), (65_536, 4_096));
    assert_eq!(capacities((size_of::<World>() - 1) as u128), (0, 0));
    assert_eq!(capacities(size_of::<World>() as u128), (1, 0));
    let (worlds, batches) = capacities((full - 1) as u128);
    assert!(worlds * size_of::<World>() + batches * size_of::<Batch>() < full);
}

#[test]
fn ordinary_memory_funds_both_sample_targets() {
    let config = crate::Resources::DEFAULT.solve_config();
    let samples = RunSamples::new(config.max_batch_bytes).unwrap().snapshot();
    assert_eq!(samples.worlds.capacity, 65_536);
    assert_eq!(samples.batches.capacity, 4_096);
    assert_eq!((samples.worlds.retained, samples.batches.retained), (0, 0));
}

#[test]
fn unobserved_worlds_do_not_invalidate_measured_batch_widths() {
    let mut sampler = RunSamples::new(64 * 1024 * 1024).unwrap();
    let state = &mut sampler.state[0];
    let batches = [
        Batch {
            residuals: [None; 4],
            width: Some(7),
            times: [None; 4],
        },
        Batch {
            residuals: [Some(0); 4],
            width: Some(0),
            times: [None; 4],
        },
    ];
    state.worlds.lose_population();
    for total in &mut state.groups[0].worlds {
        total.missing_members(None);
    }
    for batch in batches {
        state.groups[0].batch(batch, 0);
        state
            .batches
            .insert(batch, state.retained_bytes(), &mut state.peak);
    }
    sampler.settle();
    let report = sampler.snapshot();
    assert_eq!(report.worlds.seen, None);
    assert_eq!(report.groups.unwrap()[0].width.maximum, Some(7));
}
