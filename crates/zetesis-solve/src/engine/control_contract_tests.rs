//! Stopped batches preserve typed control before oracle execution.

use super::Engine;
use crate::execution_observation::Ignore;
use crate::{Backend, Grounder, SolveConfig};
use std::{num::NonZeroUsize, time::Instant};
use zetesis_core::SeedSelection;
use zetesis_cpu::{Cancellation, Stop};
use zetesis_themelios::{AdmissionOptions, admit};

#[test]
fn automatic_policy_retains_cpu_execution() {
    let admitted = admit("a.".into(), AdmissionOptions::default()).unwrap();
    for grounder in [Grounder::Auto, Grounder::Lazy, Grounder::Eager] {
        let config = SolveConfig {
            grounder,
            workers: NonZeroUsize::MIN,
            batch_size: NonZeroUsize::new(128).unwrap(),
            ..Default::default()
        };
        let mut engine = Engine::new(
            &config,
            admitted.program(),
            &mut Ignore,
            &crate::phase_timing::Recorder::new(false),
        )
        .unwrap();
        let seed = SeedSelection::new(admitted.program(), []).unwrap();
        for size in [1, 32, 64, 128] {
            let results = engine
                .check(
                    &config,
                    admitted.program(),
                    &vec![seed.clone(); size],
                    None,
                    &Cancellation::default(),
                    &crate::phase_timing::Recorder::new(false),
                )
                .unwrap();
            assert_eq!(results.len(), size);
            assert!(results.into_iter().all(|result| result.unwrap().is_some()));
            assert!(!engine.executor.is_gpu());
        }
        assert_eq!(
            engine.executor.ground().is_some(),
            grounder == Grounder::Eager
        );
    }
}

#[test]
fn cpu_execution_reports_no_device_activity() {
    let admitted = admit("a.".into(), AdmissionOptions::default()).unwrap();
    let config = SolveConfig {
        backend: Backend::Cpu,
        ..Default::default()
    };
    let engine = Engine::new(
        &config,
        admitted.program(),
        &mut Ignore,
        &crate::phase_timing::Recorder::new(false),
    )
    .unwrap();
    assert!(engine.lazy_statistics(32).is_none());
}

#[test]
fn stopped_seeds_never_enter_oracle_execution() {
    let admitted = admit("p.".into(), AdmissionOptions::default()).unwrap();
    let program = admitted.program();
    let seed = SeedSelection::new(program, []).unwrap();
    for grounder in [Grounder::Lazy, Grounder::Eager] {
        let config = SolveConfig {
            backend: Backend::Cpu,
            grounder,
            workers: NonZeroUsize::MIN,
            ..Default::default()
        };
        let phases = crate::phase_timing::Recorder::new(false);
        let mut engine = Engine::new(&config, program, &mut Ignore, &phases).unwrap();
        let cancelled = Cancellation::default();
        cancelled.cancel();
        for (cancellation, reason) in [
            (cancelled, Stop::Cancelled),
            (
                Cancellation::with_deadline(Instant::now()).unwrap(),
                Stop::Deadline,
            ),
        ] {
            let batch = engine
                .check(
                    &config,
                    program,
                    &[seed.clone(), seed.clone()],
                    None,
                    &cancellation,
                    &phases,
                )
                .unwrap();
            assert_eq!(batch, vec![Err(reason)]);
        }
        let batch = engine
            .check(
                &config,
                program,
                &[seed.clone(), seed.clone()],
                None,
                &Cancellation::default(),
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
    let first = super::Executor::cpu(
        &config,
        admitted.program(),
        None,
        &Cancellation::default(),
        &mut Ignore,
        &phases,
    )
    .unwrap();
    let ground = first.ground().unwrap();
    let second = super::Executor::cpu(
        &config,
        admitted.program(),
        Some(ground.clone()),
        &Cancellation::default(),
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
fn observer_failure_remains_an_external_failure() {
    use crate::execution_observation::Observer;
    use crate::{ExecutionObservation, ExecutionObserver, SolveConfig, SolveError};

    struct Adversarial;
    impl ExecutionObserver for Adversarial {
        type Error = SolveError;
        fn observe(&mut self, _: ExecutionObservation<'_>) -> Result<(), Self::Error> {
            Err(SolveError::BackendUnavailable)
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
    let result = super::Engine::new(
        &options,
        admitted.program(),
        &mut Observer(&mut Adversarial),
        &phases,
    );
    let Err(super::PreparationFailure::Run(SolveError::ExecutionObservation(cause))) = result
    else {
        panic!("the observer failure must remain external")
    };
    assert!(matches!(
        cause.downcast_ref::<SolveError>(),
        Some(SolveError::BackendUnavailable)
    ));
}

#[test]
fn static_materialization_preserves_control_stops() {
    let admitted = admit("a.".into(), AdmissionOptions::default()).unwrap();
    let config = SolveConfig {
        grounder: Grounder::Eager,
        ..Default::default()
    };
    let cancelled = Cancellation::default();
    cancelled.cancel();
    for (token, expected) in [
        (cancelled, Stop::Cancelled),
        (
            Cancellation::with_deadline(Instant::now()).unwrap(),
            Stop::Deadline,
        ),
    ] {
        let result = super::compile_static(
            &config,
            admitted.program(),
            config.max_atoms,
            &token,
            &crate::phase_timing::Recorder::new(false),
        );
        assert!(
            matches!(result, Err(super::PreparationFailure::Stopped(actual)) if actual == expected)
        );
    }
}
