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
