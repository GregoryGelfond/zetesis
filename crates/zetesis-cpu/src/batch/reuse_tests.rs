//! Fixed borrowed ranges preserve input occurrence order and reusable ownership.

use rayon::prelude::*;
use std::num::NonZeroUsize;

use super::{BatchError, BatchOracle, prepared::Cache};
use crate::{Control, Limits, PreparationLimits, Stop};

#[test]
fn growing_batches_reuse_their_assigned_workspaces() {
    let (graph, seeds) = super::tests::fixture();
    let mut pool = BatchOracle::new(
        NonZeroUsize::new(3).unwrap(),
        NonZeroUsize::new(12).unwrap(),
    )
    .unwrap();
    assert_eq!(pool.query_statistics().unwrap().preparation, None);
    for count in [1, 3, 7, 12, 2] {
        let results = pool
            .check_batch_views(
                graph.program(),
                (0..count)
                    .into_par_iter()
                    .map(|index| seeds[index % seeds.len()].view()),
                Limits::default(),
                &Control::default(),
            )
            .unwrap();
        for (index, result) in results.iter().enumerate() {
            let expected = crate::check(
                graph.program(),
                &seeds[index % seeds.len()],
                Limits::default(),
                &Control::default(),
            )
            .unwrap();
            let actual = result.as_ref().unwrap();
            assert_eq!(actual.closure(), expected.closure());
            assert_eq!(actual.accepted(), expected.accepted());
            assert_eq!(actual.constraint_violated(), expected.constraint_violated());
            assert_eq!(actual.seed_mismatch(), expected.seed_mismatch());
        }
        let statistics = pool.query_statistics().unwrap();
        assert_eq!(statistics.preparation_builds, 1);
        assert_eq!(statistics.active_workspaces, count.min(3));
        assert_eq!(
            statistics.retained_workspaces,
            if count == 1 { 1 } else { 3 }
        );
        assert_eq!(
            statistics.reused_workspaces,
            match count {
                1 => 0,
                3 => 1,
                2 => 2,
                _ => 3,
            }
        );
        // No preparation work can run in the following submissions. Reuse is
        // established by the real bound, not just the recorded build count.
        pool = pool.with_preparation_limits(PreparationLimits {
            max_work: 0,
            ..PreparationLimits::default()
        });
    }
    let fresh = BatchOracle::new(
        NonZeroUsize::new(3).unwrap(),
        NonZeroUsize::new(12).unwrap(),
    )
    .unwrap()
    .with_preparation_limits(PreparationLimits {
        max_work: 0,
        ..PreparationLimits::default()
    });
    assert!(matches!(
        fresh.check_batch(
            graph.program(),
            &seeds,
            Limits::default(),
            &Control::default()
        ),
        Err(BatchError::Preparation(Stop::WorkLimit))
    ));
}

#[test]
fn collective_reservation_includes_idle_cache() {
    let (graph, _) = super::tests::fixture();
    let mut cache = Cache::default();
    cache
        .prepare(
            graph.program(),
            3,
            PreparationLimits::default(),
            usize::MAX,
            &Control::default(),
        )
        .unwrap();
    let limits = Limits::default();
    let stats = cache.statistics().unwrap();
    let prepared = stats.preparation.unwrap().retained_bytes as u128;
    let retained_slot = crate::ClosureWorkspace::default().retained_bytes().unwrap();
    // Two disjoint active slots replace their retained charge by L - P; the
    // third slot remains idle. The shared cache/preparation stays counted once.
    let expected = stats.retained_bytes - 2 * retained_slot
        + 2 * (limits.max_closure_bytes as u128 - prepared);
    cache
        .admit(2, limits, usize::try_from(expected).unwrap())
        .unwrap();
    assert_eq!(cache.statistics().unwrap().reserved_bytes, expected);
    assert!(
        matches!(cache.admit(2, limits, usize::try_from(expected - 1).unwrap()),
        Err(BatchError::ClosureStorage { required, limit }) if required == expected && limit == expected - 1)
    );
}
