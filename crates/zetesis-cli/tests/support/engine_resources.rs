//! Actual executor identity, beyond equality of reported adapter metadata.

use super::{Engine, Executor, selection};
use crate::{Backend, ExecutionResources, Grounder, SolveConfig};
use zetesis_cpu::{CandidateLimits, Candidates, Control};
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
            &mut crate::execution_observation::Ignore,
            &crate::phase_timing::Recorder::new(false),
        )
        .unwrap();
        require_context(&engine, &context, grounder);
        delayed_context(&context, grounder);
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

fn delayed_context(context: &GpuContext, grounder: Grounder) {
    let owner = zetesis_themelios::admit(
        "a :- not b. b :- not a. c :- not d. d :- not c. e :- not f. f :- not e.".into(),
        zetesis_themelios::AdmissionOptions::default(),
    )
    .unwrap();
    let config = SolveConfig {
        backend: Backend::Auto,
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
            &mut observations,
            &phases,
        )
        .unwrap()
    };
    assert!(!engine.executor.is_gpu());
    let control = Control::default();
    let mut candidates =
        Candidates::new(owner.program(), CandidateLimits::default(), control.clone());
    let first = candidates.next().unwrap().unwrap();
    engine
        .check(
            &config,
            owner.program(),
            &[first],
            &mut observations,
            &control,
            &phases,
        )
        .unwrap();
    assert!(!engine.executor.is_gpu());
    assert!(!engine.attempted_gpu);
    let batch = candidates
        .take(super::AUTO_GPU_MIN_BATCH)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(batch.len(), super::AUTO_GPU_MIN_BATCH);
    engine
        .check(
            &config,
            owner.program(),
            &batch,
            &mut observations,
            &control,
            &phases,
        )
        .unwrap();
    assert!(engine.attempted_gpu);
    require_context(&engine, context, grounder);
    assert!(engine.resources.gpu_context().is_none());
}

#[test]
#[ignore = "requires actual Metal; checks exact executor context identity"]
fn metal_closure_retains_the_supplied_context() {
    supplied_context(Backend::Metal, "Metal");
}

#[test]
#[ignore = "requires actual Vulkan; checks exact executor context identity"]
fn vulkan_closure_retains_the_supplied_context() {
    supplied_context(Backend::Vulkan, "Vulkan");
}
