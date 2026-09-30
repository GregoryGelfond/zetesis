//! The backend validates both immutable rows independently of CPU construction.

use super::snapshot_shape;
use crate::GpuErrorKind;

#[test]
fn each_snapshot_input_requires_the_complete_shape() {
    for (words, worlds) in [(0, 3), (1, 0), (1, 1), (3, 2)] {
        let count = words * worlds;
        snapshot_shape(words, worlds, count, count).unwrap();
        for (snapshots, seeds) in [(count + 1, count), (count, count + 1)] {
            assert_eq!(
                snapshot_shape(words, worlds, snapshots, seeds)
                    .unwrap_err()
                    .kind(),
                GpuErrorKind::Capacity
            );
        }
        if count != 0 {
            for (snapshots, seeds) in [(count - 1, count), (count, count - 1)] {
                assert_eq!(
                    snapshot_shape(words, worlds, snapshots, seeds)
                        .unwrap_err()
                        .kind(),
                    GpuErrorKind::Capacity
                );
            }
        }
    }
}

#[test]
fn snapshot_products_cannot_wrap_into_valid_lengths() {
    assert_eq!(
        snapshot_shape(usize::MAX, 2, usize::MAX - 1, usize::MAX - 1)
            .unwrap_err()
            .kind(),
        GpuErrorKind::Capacity
    );
}
