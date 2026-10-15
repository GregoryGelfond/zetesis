//! Adaptive scheduling changes batch shape, never membership or pending ownership.

use std::{collections::BTreeSet, num::NonZeroUsize};

use zetesis_cpu::Cancellation;
use zetesis_sat::{BatchVerdict, Incomplete, Limits, StableModels};

use crate::batch_executor::{BatchResult, CandidateBatch, DeviceVerdicts};
use crate::formula_execution::Failure;
use crate::formula_queue::BatchQueue;
use crate::{GpuBatching, SolveConfig};

fn options() -> SolveConfig {
    SolveConfig {
        gpu_batching: GpuBatching::Adaptive,
        batch_size: NonZeroUsize::new(2).unwrap(),
        ..Default::default()
    }
}

fn checked(batch: CandidateBatch<'_>, prefix: usize) -> Result<BatchResult<'_>, Failure> {
    let mut verdicts = DeviceVerdicts::new(batch.candidates().len()).map_err(Failure::Search)?;
    for candidate in batch.candidates() {
        assert!(
            zetesis_ferraris::check(
                batch.theory(),
                candidate,
                zetesis_ferraris::Limits::default(),
                &Cancellation::default(),
            )
            .unwrap()
            .accepted()
        );
    }
    while verdicts.values.len() < batch.candidates().len() {
        let offered = batch.candidates().len() - verdicts.values.len();
        let count = prefix.min(offered);
        verdicts.record_prefix(offered, count);
        verdicts
            .values
            .resize(verdicts.values.len() + count, BatchVerdict::NoProperSubset);
    }
    verdicts.finish(batch).map_err(Failure::from)
}

#[test]
fn committed_batches_grow_until_a_physical_prefix_limits_them() {
    let admitted = zetesis_reference_support::formula("{a;b;c;d;e}.");
    let cancellation = Cancellation::default();
    let mut models =
        StableModels::new(admitted.theory(), Limits::default(), cancellation.clone()).unwrap();
    let options = options();
    let mut queue = BatchQueue::for_complete_oracle(&options).unwrap();
    let mut counts = Vec::new();
    let mut family = BTreeSet::new();
    while let Some(next) = queue.next(&mut models, &options, &cancellation, |batch| {
        counts.push(batch.candidates().len());
        checked(batch, 6)
    }) {
        let model = next.unwrap_or_else(|_| panic!("complete joint check must succeed"));
        assert!(family.insert(model.atoms().collect::<Vec<_>>()));
    }
    let expected: BTreeSet<Vec<_>> = (0..32)
        .map(|mask| (0..5).filter(|atom| mask & (1 << atom) != 0).collect())
        .collect();
    assert_eq!(family, expected);
    assert_eq!(counts, [2, 4, 8, 6, 6, 6]);
    assert!(models.exhausted());
    assert_eq!(models.batch_statistics().committed, 32);
    assert_eq!(models.batch_statistics().original_validation_work, 0);
}

#[test]
fn failed_completion_keeps_the_prior_target_and_pending_candidates() {
    let admitted = zetesis_reference_support::formula("{a;b;c}.");
    let cancellation = Cancellation::default();
    let mut models =
        StableModels::new(admitted.theory(), Limits::default(), cancellation.clone()).unwrap();
    let options = SolveConfig {
        max_completion_scratch_bytes: 0,
        ..options()
    };
    let mut queue = BatchQueue::for_joint_oracle(&options).unwrap();
    let mut pending = Vec::new();
    let failed = queue.next(&mut models, &options, &cancellation, |batch| {
        pending = batch
            .candidates()
            .iter()
            .map(|value| value.atoms().collect::<Vec<_>>())
            .collect();
        let mut verdicts =
            DeviceVerdicts::new(batch.candidates().len()).map_err(Failure::Search)?;
        verdicts.record_prefix(batch.candidates().len(), 1);
        verdicts
            .values
            .resize(batch.candidates().len(), BatchVerdict::Residual);
        verdicts.finish(batch).map_err(Failure::from)
    });
    assert!(matches!(
        failed,
        Some(Err(Failure::Search(Incomplete::CompletionScratch)))
    ));
    assert_eq!(queue.batching.target(&models, &options), options.batch_size);
    assert_eq!(models.batch_statistics().pending, 2);
    assert_eq!(models.batch_statistics().committed, 0);
    assert_eq!(queue.len(), 0);
    let retry = queue.next(&mut models, &options, &cancellation, |batch| {
        assert_eq!(
            batch
                .candidates()
                .iter()
                .map(|value| value.atoms().collect::<Vec<_>>())
                .collect::<Vec<_>>(),
            pending
        );
        Err(Failure::Search(Incomplete::Allocation))
    });
    assert!(matches!(
        retry,
        Some(Err(Failure::Search(Incomplete::Allocation)))
    ));
    assert_eq!(models.batch_statistics().pending, 2);
    assert_eq!(queue.batching.target(&models, &options), options.batch_size);
}

#[test]
fn adaptive_target_respects_pending_memory_before_proposal() {
    let admitted = zetesis_reference_support::formula("{a}.");
    let models = StableModels::new(
        admitted.theory(),
        Limits::default(),
        Cancellation::default(),
    )
    .unwrap();
    let mut options = options();
    let mut batching = super::Batching::new(&options);
    batching.committed(options.batch_size, 2, None);
    options.max_batch_bytes = (0..4096)
        .find(|bytes| models.joint_batch_capacity(*bytes) == 3)
        .unwrap();
    assert_eq!(batching.target(&models, &options).get(), 3);
    options.max_batch_bytes = 0;
    // One is attempted only so ordinary admission supplies the precise refusal.
    assert_eq!(batching.target(&models, &options), NonZeroUsize::MIN);
}
