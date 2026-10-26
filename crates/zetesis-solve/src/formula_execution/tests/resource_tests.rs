//! Formula setup must reuse the exact context, not merely the same adapter.

use super::Execution;
use crate::{Backend, ExecutionResources, GpuApi, SolveConfig};
use zetesis_cpu::Cancellation;
use zetesis_wgpu::{GpuContext, GpuOptions};

fn supplied_context(backend: Backend, expected_api: &str) {
    let context = GpuContext::new_selected(
        GpuOptions::default(),
        crate::execution_resources::selection(backend),
    )
    .unwrap();
    assert_eq!(context.info().backend(), expected_api);
    println!("adapter={} api={expected_api}", context.info().name());
    let execution = Execution::with_resources(
        &SolveConfig {
            backend,
            ..Default::default()
        },
        &ExecutionResources::with_gpu(&context),
        None,
        &Cancellation::default(),
        &mut crate::execution_observation::Ignore,
    )
    .unwrap();
    let Execution::Hybrid { oracle, .. } = execution else {
        panic!("forced device setup must retain the formula oracle");
    };
    assert!(context.same_instance(oracle.context()));
    let theory = zetesis_ferraris::Theory::new(
        1,
        zetesis_ferraris::FormulaParts::new(vec![zetesis_ferraris::Node::atom(0)], vec![]).unwrap(),
        vec![0],
        zetesis_ferraris::AdmissionLimits::default(),
    )
    .unwrap();
    let plan = std::sync::Arc::new(
        zetesis_ferraris::TightPlan::compile(
            &theory,
            zetesis_ferraris::TightPlanLimits::default(),
            &zetesis_cpu::Cancellation::default(),
        )
        .unwrap(),
    );
    let execution = Execution::with_resources(
        &SolveConfig {
            backend,
            ..Default::default()
        },
        &ExecutionResources::with_gpu(&context),
        Some(plan),
        &Cancellation::default(),
        &mut crate::execution_observation::Ignore,
    )
    .unwrap();
    let Execution::Tight { oracle, .. } = execution else {
        panic!("a supplied tight plan must select the support primitive");
    };
    assert!(context.same_instance(oracle.context()));
}

#[test]
#[ignore = "requires Metal: checks exact formula context identity"]
fn metal_formula_retains_the_supplied_context() {
    supplied_context(Backend::Gpu(Some(GpuApi::Metal)), "Metal");
}

#[test]
#[ignore = "requires Vulkan: checks exact formula context identity"]
fn vulkan_formula_retains_the_supplied_context() {
    supplied_context(Backend::Gpu(Some(GpuApi::Vulkan)), "Vulkan");
}

fn supplied_profile(backend: Backend, expected_api: &str) {
    use zetesis_ferraris::{AdmissionLimits, Interpretation, Node, Theory};
    use zetesis_wgpu::{GateProjection, GpuFormulaProfile};
    let context = GpuContext::new_selected(
        GpuOptions::default(),
        crate::execution_resources::selection(backend),
    )
    .unwrap();
    assert_eq!(context.info().backend(), expected_api);
    assert!(context.info().is_hardware_gpu());
    let theory = Theory::new(
        1,
        zetesis_ferraris::FormulaParts::new(vec![Node::atom(0)], vec![]).unwrap(),
        vec![0],
        AdmissionLimits::default(),
    )
    .unwrap();
    let candidates = [Interpretation::new(&theory, [0]).unwrap()];
    let options = SolveConfig {
        backend,
        ..Default::default()
    };
    for projection in GateProjection::ALL {
        let profile =
            GpuFormulaProfile::from_context_with_projection(&context, projection).unwrap();
        let resources = ExecutionResources::with_formula_profile(&profile);
        assert!(resources.formula_profile().unwrap().same_instance(&profile));
        assert!(resources.gpu_context().unwrap().same_instance(&context));
        let mut consumers = Vec::new();
        for _ in 0..2 {
            let mut execution = Execution::with_resources(
                &options,
                &resources,
                None,
                &Cancellation::default(),
                &mut crate::execution_observation::Ignore,
            )
            .unwrap();
            let Execution::Hybrid {
                oracle,
                queue,
                statistics,
                ..
            } = &mut execution
            else {
                panic!("forced formula execution retains a compiled oracle");
            };
            assert!(profile.same_instance(oracle.compiled_profile()));
            assert!(context.same_instance(oracle.context()));
            assert_eq!(oracle.projection(), projection);
            assert!(oracle.last_batch_stats().is_none());
            assert_eq!(queue.len(), 0);
            assert_eq!(statistics.gpu_batches, 0);
            assert_eq!(
                statistics.gpu_limits,
                Some(super::FormulaDeviceLimits::from(&options))
            );
            assert_eq!(statistics.gpu_submitted_batches, 0);
            assert_eq!(statistics.gpu_submitted_candidates, 0);
            assert_eq!(statistics.gpu_candidates, 0);
            assert_eq!(statistics.gpu_work, 0);
            assert_eq!(statistics.gpu_rounds, 0);
            assert_eq!(
                statistics.gpu_residuals,
                Some(super::FormulaResidualStatistics::default())
            );
            assert_eq!(statistics.cpu_residuals, 0);
            assert_eq!(statistics.gpu_decided, 0);
            let result = super::propagate(
                oracle,
                statistics,
                &theory,
                &candidates,
                &options,
                &zetesis_cpu::Cancellation::default(),
                &crate::phase_timing::Recorder::new(false),
            );
            assert!(result.is_ok());
            assert_eq!(statistics.gpu_batches, 1);
            assert_eq!(statistics.gpu_submitted_batches, 1);
            assert_eq!(statistics.gpu_submitted_candidates, 1);
            assert_eq!(statistics.gpu_candidates, 1);
            assert!(statistics.gpu_work > 0);
            assert!(oracle.last_batch_stats().unwrap().theory_uploaded);
            assert!(oracle.last_batch_stats().unwrap().transport_allocated);
            consumers.push(execution);
        }
        for policy in [crate::GpuBatching::Fixed, crate::GpuBatching::Adaptive] {
            qualify_prefix_orchestration(&profile, backend, policy);
        }
    }
}

// A five-candidate slate crosses several two-candidate submissions, forced by an
// explicit test-facing ceiling. The ordinary coordinator must establish
// original satisfaction on the device and publish only the complete ordered
// slate.
fn qualify_prefix_orchestration(
    profile: &zetesis_wgpu::GpuFormulaProfile,
    backend: Backend,
    policy: crate::GpuBatching,
) {
    use std::collections::BTreeSet;
    use std::num::NonZeroUsize;
    use zetesis_stable::{Limits, StableModels};

    let admitted = zetesis_reference_support::formula("{a;b;c}.");
    let theory = admitted.theory();
    let cancellation = zetesis_cpu::Cancellation::default();
    let mut models = StableModels::new(theory, Limits::default(), cancellation.clone()).unwrap();
    let logical = SolveConfig {
        backend,
        batch_size: NonZeroUsize::new(5).unwrap(),
        gpu_batching: policy,
        ..Default::default()
    };
    let mut queue = crate::formula_queue::BatchQueue::for_joint_oracle(&logical).unwrap();
    let mut oracle = zetesis_wgpu::GpuFormulaOracle::from_profile(profile).unwrap();
    let mut statistics = super::FormulaExecutionStatistics::default();
    let phases = crate::phase_timing::Recorder::new(false);
    let mut family = BTreeSet::new();
    let mut counts = Vec::new();
    while let Some(next) = queue.next(&mut models, &logical, &cancellation, |batch| {
        counts.push(batch.candidates().len());
        let ceiling = zetesis_wgpu::FormulaLimits {
            max_candidates: 2,
            ..super::slate_limits(&logical, batch.candidates().len())
        };
        let verdicts = super::propagate_within(
            &mut oracle,
            &mut statistics,
            batch.theory(),
            batch.candidates(),
            ceiling,
            &cancellation,
            &phases,
        )?;
        verdicts.finish(batch).map_err(super::Failure::from)
    }) {
        let model = next.unwrap_or_else(|_| panic!("joint prefix execution must complete"));
        assert!(family.insert(model.atoms().collect::<Vec<_>>()));
    }
    let expected: BTreeSet<Vec<_>> = (0..8)
        .map(|mask| (0..3).filter(|atom| mask & (1 << atom) != 0).collect())
        .collect();
    assert_eq!(family, expected);
    // Under either policy: a device prefix below the batch size splits a round
    // into submissions and never shrinks the round.
    assert_eq!(counts, [5, 3]);
    assert_eq!(statistics.gpu_submitted_batches, 5);
    assert_eq!(statistics.gpu_batches, 5);
    assert_eq!(statistics.gpu_submitted_candidates, 8);
    assert_eq!(statistics.gpu_candidates, 8);
    assert_eq!(models.batch_statistics().committed, 8);
    assert_eq!(models.batch_statistics().original_validation_work, 0);
    assert!(models.exhausted());
    assert_eq!(queue.len(), 0);
}

#[test]
#[ignore = "requires Metal: ordinary formula execution reuses one compilation"]
fn metal_formula_sessions_reuse_the_supplied_profile() {
    supplied_profile(Backend::Gpu(Some(GpuApi::Metal)), "Metal");
}

#[test]
#[ignore = "requires Vulkan: ordinary formula execution reuses one compilation"]
fn vulkan_formula_sessions_reuse_the_supplied_profile() {
    supplied_profile(Backend::Gpu(Some(GpuApi::Vulkan)), "Vulkan");
}

#[test]
#[ignore = "requires Metal: steady enumeration grows candidate slates"]
fn metal_steady_enumeration_grows_candidate_slates() {
    steady_enumeration(Backend::Gpu(Some(GpuApi::Metal)));
}

#[test]
#[ignore = "requires Vulkan: steady enumeration grows candidate slates"]
fn vulkan_steady_enumeration_grows_candidate_slates() {
    steady_enumeration(Backend::Gpu(Some(GpuApi::Vulkan)));
}

// Under the ordinary adaptive policy, the slates of a program without
// objectives double after each filled slate (4, 8, 16 and 32 of 64 answers,
// then the last 4), and a device submission takes the whole slate.
fn steady_enumeration(backend: Backend) {
    use std::collections::BTreeSet;
    use std::num::NonZeroUsize;
    let owner = zetesis_reference_support::formula("{a;b;c;d;e;f}.");
    let family = |config: SolveConfig| {
        let mut session = crate::Session::new(
            crate::PreparedInput::formula(&owner),
            config,
            zetesis_cpu::Cancellation::default(),
        )
        .unwrap();
        let answers: BTreeSet<_> = session
            .by_ref()
            .map(|answer| answer.unwrap().interpretation().clone())
            .collect();
        (answers, session.outcome().unwrap())
    };
    let config = SolveConfig {
        models: 0,
        batch_size: NonZeroUsize::new(4).unwrap(),
        ..Default::default()
    };
    assert_eq!(config.gpu_batching, crate::GpuBatching::Adaptive);
    let (cpu, _) = family(SolveConfig {
        backend: Backend::Cpu,
        ..config
    });
    let (gpu, outcome) = family(SolveConfig { backend, ..config });
    assert_eq!(cpu.len(), 64);
    assert_eq!(gpu, cpu);
    assert_eq!(outcome.completion(), Some(crate::Completion::Exhausted));
    let statistics = outcome.formula_execution().unwrap();
    assert!(statistics.gpu_slates >= 4);
    assert!(statistics.gpu_largest_slate.unwrap() > config.batch_size.get());
    assert_eq!(statistics.gpu_slate_resets, 0);
    // The route: some device submission took more than the batch size.
    let batch_size = u64::try_from(config.batch_size.get()).unwrap();
    assert!(statistics.gpu_submitted_candidates > batch_size * statistics.gpu_submitted_batches);
}

#[test]
fn device_limits_preserve_exact_configured_units() {
    for (work, rounds) in [(0, 0), (789, 17), (u32::MAX, u32::MAX)] {
        let options = SolveConfig {
            max_work: u64::MAX,
            gpu_formula_work: work,
            gpu_formula_rounds: rounds,
            ..Default::default()
        };
        let limits = super::slate_limits(&options, 1);
        assert_eq!(limits.max_work_per_candidate, work);
        assert_eq!(limits.max_rounds, rounds);
        assert_eq!(
            super::FormulaDeviceLimits::from(&options),
            super::FormulaDeviceLimits {
                work_per_candidate: work,
                rounds_per_candidate: rounds,
            }
        );
    }
    let limits = super::slate_limits(&SolveConfig::default(), 1);
    let library = zetesis_wgpu::FormulaLimits::default();
    assert_eq!(
        limits.max_work_per_candidate,
        library.max_work_per_candidate
    );
    assert_eq!(limits.max_rounds, library.max_rounds);
}

#[test]
fn a_device_submission_may_take_the_whole_slate_under_either_policy() {
    for gpu_batching in [crate::GpuBatching::Fixed, crate::GpuBatching::Adaptive] {
        let options = SolveConfig {
            gpu_batching,
            batch_size: std::num::NonZeroUsize::new(2).unwrap(),
            ..Default::default()
        };
        assert_eq!(super::slate_limits(&options, 7).max_candidates, 7);
    }
}

#[test]
fn formula_slates_keep_the_device_batch_ceiling() {
    // Only the tight support batch has the larger ceiling; distinct values make
    // a formula slate that read it fail here.
    let options = SolveConfig {
        max_batch_bytes: 1_000,
        max_tight_batch_bytes: 2_000,
        ..Default::default()
    };
    assert_eq!(super::slate_limits(&options, 1).max_batch_bytes, 1_000);
}

#[test]
fn submission_accounting_preserves_undecoded_candidates() {
    let mut statistics = super::FormulaExecutionStatistics::default();
    assert!(super::record_submission(&mut statistics, None).is_ok());
    assert!(super::record_submission(&mut statistics, Some(3)).is_ok());
    assert!(super::record_submission(&mut statistics, Some(5)).is_ok());
    assert_eq!(statistics.gpu_submitted_batches, 2);
    assert_eq!(statistics.gpu_submitted_candidates, 8);
    assert_eq!((statistics.gpu_batches, statistics.gpu_candidates), (0, 0));
    assert_eq!((statistics.gpu_work, statistics.gpu_rounds), (0, 0));
    for (batches, candidates) in [(u64::MAX, 8), (2, u64::MAX)] {
        statistics.gpu_submitted_batches = batches;
        statistics.gpu_submitted_candidates = candidates;
        assert!(matches!(
            super::record_submission(&mut statistics, Some(1)),
            Err(super::Failure::Search(
                zetesis_stable::Incomplete::CounterOverflow
            ))
        ));
        assert_eq!(statistics.gpu_submitted_batches, batches);
        assert_eq!(statistics.gpu_submitted_candidates, candidates);
    }
}
