//! Formula setup must reuse the exact context, not merely the same adapter.

use super::Execution;
use crate::{Backend, ExecutionResources, SolveConfig};
use zetesis_wgpu::{GpuContext, GpuOptions};

fn supplied_context(backend: Backend, expected_api: &str) {
    let context =
        GpuContext::new_selected(GpuOptions::default(), crate::engine::selection(backend)).unwrap();
    assert_eq!(context.info().backend(), expected_api);
    println!("adapter={} api={expected_api}", context.info().name());
    let execution = Execution::with_resources(
        &SolveConfig {
            backend,
            ..Default::default()
        },
        &ExecutionResources::with_gpu(&context),
        &mut crate::execution_observation::Ignore,
    )
    .unwrap();
    let Execution::Hybrid { oracle, .. } = execution else {
        panic!("forced device setup must retain the formula oracle");
    };
    assert!(context.same_instance(oracle.context()));
}

#[test]
#[ignore = "requires actual Metal; checks exact formula context identity"]
fn metal_formula_retains_the_supplied_context() {
    supplied_context(Backend::Metal, "Metal");
}

#[test]
#[ignore = "requires actual Vulkan; checks exact formula context identity"]
fn vulkan_formula_retains_the_supplied_context() {
    supplied_context(Backend::Vulkan, "Vulkan");
}

fn supplied_profile(backend: Backend, expected_api: &str) {
    use zetesis_ferraris::{AdmissionLimits, Interpretation, Node, Theory};
    use zetesis_wgpu::{GateProjection, GpuFormulaProfile};
    let context =
        GpuContext::new_selected(GpuOptions::default(), crate::engine::selection(backend)).unwrap();
    assert_eq!(context.info().backend(), expected_api);
    assert!(context.info().is_hardware_gpu());
    let theory = Theory::new(1, vec![Node::Atom(0)], vec![0], AdmissionLimits::default()).unwrap();
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
                &mut crate::execution_observation::Ignore,
            )
            .unwrap();
            let Execution::Hybrid {
                oracle,
                queue,
                statistics,
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
            assert_eq!(statistics.gpu_candidates, 0);
            assert_eq!(statistics.gpu_work, 0);
            assert_eq!(statistics.gpu_rounds, 0);
            assert_eq!(statistics.cpu_residuals, 0);
            assert_eq!(statistics.gpu_decided, 0);
            let result = super::propagate(
                oracle,
                statistics,
                &theory,
                &candidates,
                &options,
                &crate::phase_timing::Recorder::new(false),
            );
            assert!(result.is_ok());
            assert_eq!(statistics.gpu_batches, 1);
            assert_eq!(statistics.gpu_candidates, 1);
            assert!(statistics.gpu_work > 0);
            assert!(oracle.last_batch_stats().unwrap().theory_uploaded);
            assert!(oracle.last_batch_stats().unwrap().transport_allocated);
            consumers.push(execution);
        }
    }
}

#[test]
#[ignore = "requires actual Metal; ordinary formula execution reuses one compilation"]
fn metal_formula_sessions_reuse_the_supplied_profile() {
    supplied_profile(Backend::Metal, "Metal");
}

#[test]
#[ignore = "requires actual Vulkan; ordinary formula execution reuses one compilation"]
fn vulkan_formula_sessions_reuse_the_supplied_profile() {
    supplied_profile(Backend::Vulkan, "Vulkan");
}
