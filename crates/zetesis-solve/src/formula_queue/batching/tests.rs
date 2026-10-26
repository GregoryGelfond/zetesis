//! Adaptive scheduling changes slate shape, never membership or pending
//! ownership. Bound feedback is observed in the production window: a test
//! tightens the bound after a commit and before the next selection.

use std::{collections::BTreeSet, num::NonZeroUsize};

use zetesis_cpu::Cancellation;
use zetesis_ferraris::{AdmissionLimits, FormulaParts, Node, Theory};
use zetesis_objective::{PlanIdentity, Score};
use zetesis_stable::{BatchVerdict, Incomplete, Limits, StableModels};

use super::Batching;
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

fn count(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap()
}

/// A stream over every subset of five choice atoms: 32 answers.
fn stream() -> (zetesis_themelios::AdmittedFormula, StableModels) {
    let admitted = zetesis_reference_support::formula("{a;b;c;d;e}.");
    let models = StableModels::new(
        admitted.theory(),
        Limits::default(),
        Cancellation::default(),
    )
    .unwrap();
    (admitted, models)
}

/// A candidate bound requiring the first of `atoms` atoms.
fn bound(atoms: usize) -> Theory {
    Theory::new(
        atoms,
        FormulaParts::new(vec![Node::atom(0)], vec![]).unwrap(),
        vec![0],
        AdmissionLimits::default(),
    )
    .unwrap()
}

/// Install the test bound as `plan`'s candidate bound at cost `value`.
fn tighten(models: &mut StableModels, plan: PlanIdentity, value: i64) {
    let score = Score::from_costs(vec![(0, value)]).unwrap();
    models
        .improve_candidate_bound(plan, &score, &bound(5))
        .unwrap();
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

/// Drain a complete-oracle queue with checker prefixes of `prefix`, returning
/// each slate's proposal count after checking that the family is complete and
/// that the queue counted and sized the committed slates.
fn slates(options: &SolveConfig, prefix: usize) -> Vec<usize> {
    let (_admitted, mut models) = stream();
    let cancellation = Cancellation::default();
    let mut queue = BatchQueue::for_complete_oracle(options).unwrap();
    let mut counts = Vec::new();
    let mut family = BTreeSet::new();
    while let Some(next) = queue.next(&mut models, options, &cancellation, |batch| {
        counts.push(batch.candidates().len());
        checked(batch, prefix)
    }) {
        let model = next.unwrap_or_else(|_| panic!("complete joint check must succeed"));
        assert!(family.insert(model.atoms().collect::<Vec<_>>()));
    }
    let expected: BTreeSet<Vec<_>> = (0..32)
        .map(|mask| (0..5).filter(|atom| mask & (1 << atom) != 0).collect())
        .collect();
    assert_eq!(family, expected);
    assert!(models.exhausted());
    assert_eq!(models.batch_statistics().committed, 32);
    assert_eq!(models.batch_statistics().original_validation_work, 0);
    assert_eq!(queue.slates(), u64::try_from(counts.len()).unwrap());
    assert_eq!(
        queue.largest_slate().map(NonZeroUsize::get),
        counts.iter().copied().max()
    );
    counts
}

#[test]
fn a_tightening_after_a_commit_resets_the_next_slate() {
    let (_admitted, mut models) = stream();
    let options = options();
    let mut batching = Batching::new(&options);
    assert_eq!(batching.select(&models, &options), count(2));
    batching.committed(count(2), 2, None, models.bound_generation());
    assert_eq!(batching.select(&models, &options), count(4));
    batching.committed(count(4), 4, None, models.bound_generation());
    tighten(&mut models, PlanIdentity::fresh(), 0);
    assert_eq!(batching.select(&models, &options), options.batch_size);
    assert_eq!(batching.resets(), 1);
}

#[test]
fn a_quiet_filled_commit_doubles_the_next_slate() {
    let (_admitted, models) = stream();
    let options = options();
    let mut batching = Batching::new(&options);
    assert_eq!(batching.select(&models, &options), count(2));
    batching.committed(count(2), 2, None, models.bound_generation());
    assert_eq!(batching.select(&models, &options), count(4));
    assert_eq!(batching.resets(), 0);
}

#[test]
fn a_short_commit_keeps_the_slate() {
    let (_admitted, models) = stream();
    let options = options();
    let mut batching = Batching::new(&options);
    batching.select(&models, &options);
    batching.committed(count(2), 2, None, models.bound_generation());
    assert_eq!(batching.select(&models, &options), count(4));
    batching.committed(count(4), 3, None, models.bound_generation());
    assert_eq!(batching.select(&models, &options), count(4));
}

#[test]
fn a_selection_consumes_its_commit() {
    let (_admitted, models) = stream();
    let options = options();
    let mut batching = Batching::new(&options);
    batching.select(&models, &options);
    batching.committed(count(2), 2, None, models.bound_generation());
    assert_eq!(batching.select(&models, &options), count(4));
    assert_eq!(batching.select(&models, &options), count(4));
}

#[test]
fn fixed_slates_stay_at_the_batch_size_and_never_reset() {
    let (_admitted, mut models) = stream();
    let options = SolveConfig {
        gpu_batching: GpuBatching::Fixed,
        ..options()
    };
    let mut batching = Batching::new(&options);
    let plan = PlanIdentity::fresh();
    for step in 0..3 {
        assert_eq!(batching.select(&models, &options), options.batch_size);
        batching.committed(options.batch_size, 2, None, models.bound_generation());
        tighten(&mut models, plan, -step);
    }
    assert_eq!(batching.largest(), Some(options.batch_size));
    assert_eq!(batching.resets(), 0);
}

#[test]
fn a_pending_retry_keeps_its_count_and_its_commit_resets_nothing() {
    let (_admitted, mut models) = stream();
    let options = options();
    let cancellation = Cancellation::default();
    let mut queue = BatchQueue::for_complete_oracle(&options).unwrap();
    let mut counts = Vec::new();
    let mut models_handed_out = 0;
    // The first slate commits and is drained; the second fails once.
    while models_handed_out < 2 {
        queue
            .next(&mut models, &options, &cancellation, |batch| {
                counts.push(batch.candidates().len());
                checked(batch, usize::MAX)
            })
            .unwrap()
            .unwrap_or_else(|_| panic!("the first slate must commit"));
        models_handed_out += 1;
    }
    let failed = queue.next(&mut models, &options, &cancellation, |batch| {
        counts.push(batch.candidates().len());
        Err(Failure::Search(Incomplete::Allocation))
    });
    assert!(matches!(
        failed,
        Some(Err(Failure::Search(Incomplete::Allocation)))
    ));
    assert_eq!(models.batch_statistics().pending, 4);
    // Tightened while the slate is pending: no slate's answers installed it.
    tighten(&mut models, PlanIdentity::fresh(), 0);
    for _ in 0..5 {
        queue
            .next(&mut models, &options, &cancellation, |batch| {
                counts.push(batch.candidates().len());
                checked(batch, usize::MAX)
            })
            .unwrap()
            .unwrap_or_else(|_| panic!("the retried and the next slates must commit"));
    }
    assert_eq!(counts, [2, 4, 4, 8]);
    assert_eq!(queue.slate_resets(), 0);
}

#[test]
fn a_device_prefix_below_the_batch_size_keeps_slates_at_the_batch_size() {
    let options = SolveConfig {
        batch_size: count(4),
        ..options()
    };
    assert_eq!(slates(&options, 2), [4; 8]);
}

#[test]
fn committed_batches_grow_until_a_physical_prefix_limits_them() {
    assert_eq!(slates(&options(), 6), [2, 4, 8, 6, 6, 6]);
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
    // A selection with pending proposals keeps the target and the state.
    assert_eq!(queue.batching.select(&models, &options), options.batch_size);
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
    assert_eq!(queue.batching.select(&models, &options), options.batch_size);
}

#[test]
fn pending_memory_bounds_the_selected_count() {
    let admitted = zetesis_reference_support::formula("{a}.");
    let models = StableModels::new(
        admitted.theory(),
        Limits::default(),
        Cancellation::default(),
    )
    .unwrap();
    let mut options = options();
    let mut batching = Batching::new(&options);
    batching.select(&models, &options);
    batching.committed(options.batch_size, 2, None, models.bound_generation());
    options.max_batch_bytes = (0..4096)
        .find(|bytes| models.joint_batch_capacity(*bytes) == 3)
        .unwrap();
    assert_eq!(batching.select(&models, &options).get(), 3);
    options.max_batch_bytes = 0;
    // One is attempted only so ordinary admission supplies the precise refusal.
    assert_eq!(batching.select(&models, &options), NonZeroUsize::MIN);
}

#[test]
fn the_ordinary_policy_is_adaptive() {
    assert_eq!(GpuBatching::DEFAULT, GpuBatching::Adaptive);
    assert_eq!(GpuBatching::default(), GpuBatching::DEFAULT);
    assert_eq!(SolveConfig::DEFAULT.gpu_batching, GpuBatching::DEFAULT);
    assert_eq!(SolveConfig::default().gpu_batching, GpuBatching::DEFAULT);
}

#[test]
fn only_nonempty_commits_count_and_size_slates() {
    let options = options();
    let mut batching = Batching::new(&options);
    // The empty proposal that establishes exhaustion commits no slate.
    batching.committed(count(2), 0, None, 0);
    assert_eq!((batching.slates(), batching.largest()), (0, None));
    batching.committed(count(2), 2, None, 0);
    batching.committed(count(8), 5, None, 0);
    batching.committed(count(4), 4, None, 0);
    assert_eq!((batching.slates(), batching.largest()), (3, Some(count(5))));
}
