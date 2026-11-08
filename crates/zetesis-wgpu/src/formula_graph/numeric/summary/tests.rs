use super::*;
use zetesis_ferraris::{ExtremumCut, ExtremumRow, Threshold};

fn ceiling(width: u32) -> u128 {
    match width {
        1 => u128::from(u32::MAX),
        2 => u128::from(u64::MAX),
        4 => i128::MAX as u128,
        _ => panic!("invalid certified width"),
    }
}

#[test]
fn complete_weight_totals_bound_stale_propagation_intermediates() {
    // Repeated condition 0 is independently observed at its two occurrences.
    // A current undecided slot was undecided in both earlier observations.
    for (weights, width) in [
        ([0, 1, 2], 1),
        ([u64::from(u32::MAX) - 1, 1, 0], 1),
        ([u64::from(u32::MAX), 1, 0], 2),
        ([u64::MAX - 1, 1, 0], 2),
        ([u64::MAX, 1, 0], 4),
        ([u64::MAX, u64::MAX, 1], 4),
    ] {
        let total: u128 = weights.iter().copied().map(u128::from).sum();
        let node = |bound| {
            NodeView::Threshold(Threshold {
                conditions: &[0, 1, 0],
                weights: &weights,
                bound,
            })
        };
        let summary = Summary::new(node(1), &Cancellation::default()).unwrap();
        assert_eq!(summary.width(), width);
        let capacity = ceiling(width);
        assert!(total <= capacity);
        for bound in [
            -1,
            0,
            1,
            i128::try_from(total / 2).unwrap(),
            i128::try_from(total).unwrap(),
            i128::try_from(total + 1).unwrap(),
        ] {
            if constant(node(bound), Some(&summary), &Cancellation::default())
                .unwrap()
                .is_none()
            {
                assert!((1..=capacity).contains(&u128::try_from(bound).unwrap()));
            }
        }
        for observed in 0..27u32 {
            let domains = [observed % 3 + 1, observed / 3 % 3 + 1, observed / 9 + 1];
            let minimum: u128 = weights
                .iter()
                .zip(domains)
                .filter(|(_, domain)| *domain == 2)
                .map(|(&weight, _)| u128::from(weight))
                .sum();
            let maximum: u128 = weights
                .iter()
                .zip(domains)
                .filter(|(_, domain)| domain & 2 != 0)
                .map(|(&weight, _)| u128::from(weight))
                .sum();
            assert!(minimum <= maximum && maximum <= capacity);
            for current in 0..9u32 {
                let slots = [current % 3 + 1, current / 3 + 1];
                if [slots[0], slots[1], slots[0]]
                    .into_iter()
                    .zip(domains)
                    .any(|(now, prior)| now & prior != now)
                {
                    continue;
                }
                for (position, slot) in [0, 1, 0].into_iter().enumerate() {
                    if slots[slot] == 3 {
                        let weight = u128::from(weights[position]);
                        assert!(maximum >= weight);
                        assert!(minimum + weight <= total);
                        assert!(maximum - weight <= capacity);
                    }
                }
            }
        }
    }
}

#[test]
fn ordinal_width_covers_empty_sentinel_and_every_live_bound() {
    for (rank_count, width) in [
        (2, 1),
        (1u64 << 32, 1),
        ((1u64 << 32) + 1, 2),
        (u64::MAX, 2),
    ] {
        let ranks = [0, 1, rank_count - 1];
        let row = ExtremumRow {
            conditions: &[0, 1, 0],
            ranks: &ranks,
            rank_count,
        };
        for bound in [0, 1, rank_count - 1, rank_count] {
            for rule in [
                ExtremumCutRule::Prefix(bound),
                ExtremumCutRule::Suffix(bound),
            ] {
                let node = NodeView::ExtremumCut(ExtremumCut { row, rule: &rule });
                let summary = Summary::new(node, &Cancellation::default()).unwrap();
                assert_eq!(summary.width(), width);
                let capacity = ceiling(width);
                assert!(u128::from(rank_count - 1) <= capacity);
                if constant(node, Some(&summary), &Cancellation::default())
                    .unwrap()
                    .is_none()
                {
                    assert!(u128::from(bound) <= capacity);
                }
                for mask in 0..4 {
                    let selected: Vec<_> = ranks
                        .iter()
                        .enumerate()
                        .filter(|(position, _)| mask & (1 << [0, 1, 0][*position]) != 0)
                        .map(|(_, &rank)| rank)
                        .collect();
                    let minimum = selected.iter().copied().min().unwrap_or(rank_count - 1);
                    let maximum = selected.iter().copied().max().unwrap_or(0);
                    assert!(u128::from(minimum) <= capacity && u128::from(maximum) <= capacity);
                }
            }
        }
    }
}
