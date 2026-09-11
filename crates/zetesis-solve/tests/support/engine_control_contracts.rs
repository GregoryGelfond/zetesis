//! Stopped batches preserve typed control before oracle execution.

use super::Engine;
use crate::execution_observation::{Ignore, Observer};
use crate::{Backend, ExecutionObservation, ExecutionObserver, Grounder, SolveConfig};
use std::{convert::Infallible, num::NonZeroUsize, time::Instant};
use zetesis_core::Seed;
use zetesis_cpu::{Control, Stop};
use zetesis_themelios::{AdmissionOptions, admit};

#[derive(Default)]
struct Observations(usize);
impl ExecutionObserver for Observations {
    type Error = Infallible;
    fn observe(&mut self, _: ExecutionObservation<'_>) -> Result<(), Self::Error> {
        self.0 += 1;
        Ok(())
    }
}

#[test]
fn automatic_lazy_policy_permits_device_discovery() {
    let admitted = admit("a.".into(), AdmissionOptions::default()).unwrap();
    for grounder in [Grounder::Auto, Grounder::Lazy, Grounder::Eager] {
        let config = SolveConfig {
            grounder,
            workers: NonZeroUsize::MIN,
            ..Default::default()
        };
        let engine = Engine::new(
            &config,
            admitted.program(),
            &mut Ignore,
            &crate::phase_timing::Recorder::new(false),
        )
        .unwrap();
        assert_eq!(engine.automatic, cfg!(feature = "gpu"));
        assert!(!engine.attempted_gpu);
        assert_eq!(
            engine.executor.ground().is_some(),
            grounder == Grounder::Eager
        );
    }
}

#[test]
fn retired_device_work_excludes_cpu_queue_entries() {
    let admitted = admit("a.".into(), AdmissionOptions::default()).unwrap();
    let config = SolveConfig {
        backend: Backend::Cpu,
        ..Default::default()
    };
    let mut engine = Engine::new(
        &config,
        admitted.program(),
        &mut Ignore,
        &crate::phase_timing::Recorder::new(false),
    )
    .unwrap();
    // Recorded prior work is a reporting fixture, not physical execution.
    let prior = crate::lazy_execution::tests::fixture();
    engine.retired_lazy_statistics = Some(prior.clone());
    let observed = engine.lazy_statistics(32).unwrap();
    assert_eq!(observed.queued_results, 0);
    assert_eq!(observed.completed_candidates, prior.completed_candidates);
    assert_eq!(observed.stopped_candidates, prior.stopped_candidates);
    assert_eq!(observed.dispatches, prior.dispatches);
}

#[test]
fn stopped_seeds_never_enter_oracle_execution() {
    let admitted = admit("p.".into(), AdmissionOptions::default()).unwrap();
    let program = admitted.program();
    let seed = Seed::new(program, []).unwrap();
    for grounder in [Grounder::Lazy, Grounder::Eager] {
        let config = SolveConfig {
            backend: Backend::Cpu,
            grounder,
            workers: NonZeroUsize::MIN,
            ..Default::default()
        };
        let phases = crate::phase_timing::Recorder::new(false);
        let mut observations = Observations::default();
        let mut engine =
            Engine::new(&config, program, &mut Observer(&mut observations), &phases).unwrap();
        let initial = observations.0;
        let cancelled = Control::default();
        cancelled.cancel();
        for (control, reason) in [
            (cancelled, Stop::Cancelled),
            (Control::with_deadline(Instant::now()), Stop::Deadline),
        ] {
            let batch = engine
                .check(
                    &config,
                    program,
                    &[seed.clone(), seed.clone()],
                    &mut Observer(&mut observations),
                    &control,
                    &phases,
                )
                .unwrap();
            assert_eq!(batch, vec![Err(reason)]);
            assert_eq!(
                observations.0, initial,
                "a control stop cannot schedule or retry"
            );
        }
        let batch = engine
            .check(
                &config,
                program,
                &[seed.clone(), seed.clone()],
                &mut Observer(&mut observations),
                &Control::default(),
                &phases,
            )
            .unwrap();
        assert_eq!(batch.len(), 2);
        for result in batch {
            let model = result
                .unwrap()
                .expect("the unchanged fact remains derivable");
            assert_eq!(model.atoms().len(), 1);
            assert_eq!(model.atoms().iter().next().unwrap().predicate().name(), "p");
        }
    }
}

#[test]
fn eager_static_cache_reuse_avoids_materialization() {
    let admitted = admit("p.".into(), AdmissionOptions::default()).unwrap();
    let config = SolveConfig {
        backend: Backend::Cpu,
        grounder: Grounder::Eager,
        workers: NonZeroUsize::MIN,
        ..Default::default()
    };
    let phases = crate::phase_timing::Recorder::new(true);
    let first =
        super::Executor::cpu(&config, admitted.program(), None, &mut Ignore, &phases).unwrap();
    let ground = first.ground().unwrap();
    let second = super::Executor::cpu(
        &config,
        admitted.program(),
        Some(ground.clone()),
        &mut Ignore,
        &phases,
    )
    .unwrap();
    assert!(std::sync::Arc::ptr_eq(&ground, &second.ground().unwrap()));
    assert_eq!(
        phases
            .snapshot()
            .unwrap()
            .stages
            .get(crate::SolveStage::Grounding)
            .unwrap()
            .calls,
        1
    );
}

#[test]
fn observer_failure_cannot_become_device_unavailability() {
    use crate::execution_observation::{ExecutionSink, Ignore, Observer};
    use crate::{ExecutionObservation, ExecutionObserver, SolveConfig, SolveError};

    struct Adversarial;
    impl ExecutionObserver for Adversarial {
        type Error = SolveError;
        fn observe(&mut self, _: ExecutionObservation<'_>) -> Result<(), Self::Error> {
            Err(SolveError::BackendUnavailable)
        }
    }
    struct RefuseFallback;
    impl ExecutionSink for RefuseFallback {
        fn record(&mut self, _: ExecutionObservation<'_>) -> Result<(), SolveError> {
            panic!("an external observation failure must not emit fallback or retry")
        }
    }
    let admitted = zetesis_themelios::admit_extended(
        "a.".into(),
        zetesis_themelios::AdmissionOptions::default(),
        zetesis_themelios::ExpansionLimits::default(),
    )
    .unwrap();
    let options = SolveConfig {
        backend: crate::Backend::Cpu,
        workers: std::num::NonZeroUsize::MIN,
        ..Default::default()
    };
    let phases = crate::phase_timing::Recorder::new(false);
    let mut engine =
        super::Engine::new(&options, admitted.program(), &mut Ignore, &phases).unwrap();
    let error = Observer(&mut Adversarial)
        .record(ExecutionObservation::SharedCpu)
        .unwrap_err();
    let result = engine.finish_device_attempt(Err(error), &options, &mut RefuseFallback);
    let SolveError::ExecutionObservation(cause) = result.unwrap_err() else {
        panic!("the observer failure must remain external")
    };
    assert!(matches!(
        cause.downcast_ref::<SolveError>(),
        Some(SolveError::BackendUnavailable)
    ));
    assert!(!engine.executor.is_gpu());
    assert!(engine.retired_lazy_statistics.is_none());
}
