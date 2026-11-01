use super::*;

#[test]
fn origin_draws_use_fixture_populations_even_when_sample_sizes_match() {
    // Every ordered pair of draws without replacement from 2+3 worlds has
    // probability 1/20. In unanimous suffixes the unused choice is immaterial.
    let mut counts = [0; 3];
    for first in 0..5 {
        for second in 0..4 {
            let mut draws = [(5, first), (4, second)].into_iter();
            let left = origin_count(2, 5, 2, |upper| {
                let (expected, draw) = draws.next().unwrap();
                assert_eq!(upper, expected);
                draw
            });
            counts[left] += 1;
        }
    }
    assert_eq!(counts, [6, 12, 2]);
}

#[test]
fn complete_small_fixtures_preserve_the_larger_safe_sample() {
    let mut f = Frame::new(99, 42);
    f.join(Some(2), vec![1, 2], 4096).unwrap();
    f.join(Some(100), vec![4; 10], 4096).unwrap();
    assert_eq!(f.records.len(), 10);
    assert_eq!(f.seen, Some(102));
    f.join(Some(70), vec![3; 2], 4096).unwrap();
    assert_eq!(f.records.len(), 2);
    f.join(Some(0), vec![], 4096).unwrap();
    assert_eq!(f.records.len(), 2);
    f.join(Some(1), vec![], 4096).unwrap();
    assert!(f.records.is_empty());
    f.join(Some(5), vec![1; 5], 4096).unwrap();
    assert_eq!((f.seen, f.records.len()), (Some(178), 0));
}

#[test]
fn overflow_loses_only_the_unrepresentable_population() {
    let mut worlds = Frame::new(7, 1);
    let mut batches = Frame::new(7, 2);
    for (n, world, batch) in [(u64::MAX, 1, 9), (1, 2, 0), (5, 3, 1)] {
        worlds.join(Some(n), vec![world], 10).unwrap();
        batches.join(Some(1), vec![batch], 10).unwrap();
    }
    assert_eq!(worlds.seen, None);
    assert!(worlds.records.is_empty());
    assert_eq!(batches.seen, Some(3));
    assert_eq!(batches.records, [9, 0, 1]);
}

#[test]
fn merge_seed_replays_unequal_fixtures_on_independent_streams() {
    // Version-one replay vector. Fixture IDs deliberately identify the
    // retained inputs irrespective of the order chosen during the merge.
    for (tag, expected) in [
        (0x504f_4f4c_5f57_3031, [0, 2, 20]),
        (0x504f_4f4c_5f42_3031, [5, 2, 0]),
    ] {
        let mut frame = Frame::new(777, tag);
        for (seen, rows) in [
            (8, vec![0, 2, 5, 7]),
            (5, vec![9, 10, 12]),
            (2, vec![20, 21]),
        ] {
            frame.join(Some(seen), rows, 100).unwrap();
        }
        assert_eq!(frame.seen, Some(15));
        assert_eq!(frame.records, expected);
    }
}
