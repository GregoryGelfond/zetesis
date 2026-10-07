//! Actual executor identity, beyond equality of reported adapter metadata.

use super::{Engine, Executor, selection};
use crate::{Backend, ExecutionResources, GpuApi, Grounder, SolveConfig};
use zetesis_cpu::{Cancellation, CandidateLimits, Candidates};
use zetesis_wgpu::{GpuContext, GpuOptions};

fn supplied_context(backend: Backend, expected_api: &str) {
    let context = GpuContext::new_selected(GpuOptions::default(), selection(backend)).unwrap();
    assert_eq!(context.info().backend(), expected_api);
    println!("adapter={} api={expected_api}", context.info().name());
    let resources = ExecutionResources::with_gpu(&context);
    let owner =
        zetesis_themelios::admit("a.".into(), zetesis_themelios::AdmissionOptions::default())
            .unwrap();
    for grounder in [Grounder::Eager, Grounder::Lazy] {
        let config = SolveConfig {
            backend,
            grounder,
            ..Default::default()
        };
        let engine = Engine::with_ground(
            &config,
            owner.program(),
            None,
            &resources,
            &Cancellation::default(),
            &mut crate::execution_observation::Ignore,
            &crate::phase_timing::Recorder::new(false),
        )
        .unwrap();
        require_context(&engine, &context, grounder);
        cpu_keeps_its_route(&context, grounder);
    }
}

fn require_context(engine: &Engine, expected: &GpuContext, grounder: Grounder) {
    let actual = match &engine.executor {
        Executor::Gpu { oracle, .. } => {
            assert_eq!(grounder, Grounder::Eager);
            oracle.context()
        }
        Executor::LazyGpu(executor) => {
            assert_eq!(grounder, Grounder::Lazy);
            executor.oracle.context()
        }
        _ => panic!("device setup must retain a device executor"),
    };
    assert!(expected.same_instance(actual));
}

/// A supplied device leaves a CPU-backend engine on the CPU.
fn cpu_keeps_its_route(context: &GpuContext, grounder: Grounder) {
    let owner = zetesis_themelios::admit(
        "a :- not b. b :- not a. c :- not d. d :- not c. e :- not f. f :- not e.".into(),
        zetesis_themelios::AdmissionOptions::default(),
    )
    .unwrap();
    let config = SolveConfig {
        backend: Backend::Cpu,
        grounder,
        ..Default::default()
    };
    let phases = crate::phase_timing::Recorder::new(false);
    let mut observations = crate::execution_observation::Ignore;
    let mut engine = {
        let resources = ExecutionResources::with_gpu(context);
        Engine::with_ground(
            &config,
            owner.program(),
            None,
            &resources,
            &Cancellation::default(),
            &mut observations,
            &phases,
        )
        .unwrap()
    };
    assert!(!engine.executor.is_gpu());
    let cancellation = Cancellation::default();
    let mut candidates = Candidates::new(
        owner.program(),
        CandidateLimits::default(),
        cancellation.clone(),
    );
    let first = candidates.next_selection().unwrap().unwrap();
    engine
        .check(
            &config,
            owner.program(),
            &[first],
            None,
            &cancellation,
            &phases,
        )
        .unwrap();
    assert!(!engine.executor.is_gpu());
    let batch = std::iter::from_fn(|| candidates.next_selection())
        .take(32)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(batch.len(), 32);
    engine
        .check(
            &config,
            owner.program(),
            &batch,
            None,
            &cancellation,
            &phases,
        )
        .unwrap();
    assert!(!engine.executor.is_gpu());
    assert!(engine.lazy_statistics(32).is_none());
}

#[test]
#[ignore = "requires Metal: checks exact executor context identity"]
fn metal_closure_retains_the_supplied_context() {
    supplied_context(Backend::Gpu(Some(GpuApi::Metal)), "Metal");
}

#[test]
#[ignore = "requires Vulkan: checks exact executor context identity"]
fn vulkan_closure_retains_the_supplied_context() {
    supplied_context(Backend::Gpu(Some(GpuApi::Vulkan)), "Vulkan");
}
