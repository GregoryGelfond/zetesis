//! Matched aggregate measurements preserve exact values and failed prefixes.
use clap::Parser;
use std::{io, num::NonZeroUsize, time::Duration};
use zetesis_backend::GpuApi;
use zetesis_cpu::{Cancellation, Stop};
use zetesis_experiments::{
    Backend, CommandOptions, Experiment,
    aggregate_measurement::{
        self as measurement, Configuration, Error, Event, Function, Phase, Route, Value,
    },
};

fn nonzero(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap()
}
fn configuration() -> Configuration {
    let command = CommandOptions::try_parse_from([
        "zetesis-bench",
        "aggregate",
        "--backend",
        "cpu",
        "--tuples",
        "33",
        "--batches",
        "41",
        "--functions",
        "sum",
        "--warmups",
        "0",
        "--repetitions",
        "1",
        "--workers",
        "2",
    ])
    .unwrap();
    let Experiment::Aggregate(options) = command.command.unwrap() else {
        panic!("wrong command")
    };
    options.configuration().unwrap()
}

#[test]
fn every_acquired_occurrence_matches_its_native_reference() {
    for function in [
        Function::Count,
        Function::Sum,
        Function::SumPlus,
        Function::Min,
        Function::Max,
    ] {
        let mut configuration = configuration();
        configuration.cases[0].function = function;
        let mut reference = Vec::new();
        let mut samples = 0;
        measurement::measure(&configuration, |event| {
            match event {
                Event::Prepared {
                    reference: actual,
                    preparation,
                    interpretations,
                    ..
                } => {
                    reference = actual.to_vec();
                    assert_eq!(interpretations.len(), 41);
                    assert_eq!(interpretations[0], interpretations[20]);
                    assert_eq!(interpretations[4].1, None);
                    assert!(preparation.acquisition_work > 0);
                    assert_eq!(preparation.numeric_work, 34);
                    assert!(preparation.eligibility_bytes > 0);
                    assert!(reference.iter().any(|value| value.frozen.is_none()));
                    assert_eq!(reference[0], reference[20]);
                }
                Event::Sample { sample } => {
                    samples += 1;
                    assert_eq!(sample.outcomes, reference);
                    assert_eq!(sample.observation.activity.cpu_attempts, 41);
                    assert_eq!(sample.observation.activity.cpu_completed, 41);
                    assert!(sample.observation.activity.cpu_work > 0);
                    assert!(sample.observation.activity.device.is_none());
                }
                Event::Complete { samples: actual } => assert_eq!(*actual, samples),
                _ => {}
            }
            Ok(())
        })
        .unwrap();
        assert_eq!(samples, 4);
    }
}

#[test]
fn empty_extrema_are_reported_as_endpoints() {
    for (function, endpoint) in [
        (Function::Min, Value::Supremum),
        (Function::Max, Value::Infimum),
    ] {
        let mut configuration = configuration();
        configuration.cases[0].tuples = 0;
        configuration.cases[0].function = function;
        measurement::measure(&configuration, |event| {
            if let Event::Prepared {
                reference,
                preparation,
                ..
            } = event
            {
                assert_eq!(preparation.numeric_bytes, 16);
                for value in *reference {
                    assert_eq!(value.original.value, endpoint);
                    assert_eq!(value.original.holds, function == Function::Min);
                }
            }
            Ok(())
        })
        .unwrap();
    }
}

#[test]
fn timed_route_rotation_is_independent_of_warmups() {
    let mut configuration = configuration();
    configuration.warmups = 1;
    configuration.repetitions = nonzero(2);
    let mut routes = Vec::new();
    measurement::measure(&configuration, |event| {
        if let Event::Sample { sample } = event
            && sample.observation.phase == Phase::Timed
        {
            let observation = sample.observation;
            routes.push((
                observation.iteration,
                observation.position,
                observation.route,
            ));
        }
        Ok(())
    })
    .unwrap();
    assert_eq!(
        routes,
        [
            (0, 0, Route::Scalar),
            (0, 1, Route::Rayon),
            (1, 0, Route::Rayon),
            (1, 1, Route::Scalar)
        ]
    );
}

#[test]
fn interrupted_reductions_preserve_attempt_accounting() {
    let cancellation = Cancellation::default();
    let mut failed = false;
    let error = measurement::measure_with_cancellation(&configuration(), &cancellation, |event| {
        if matches!(event, Event::Prepared { .. }) {
            cancellation.cancel();
        }
        if let Event::Failed { observation, .. } = event {
            failed = true;
            assert_eq!(observation.activity.cpu_attempts, 41);
            assert_eq!(observation.activity.cpu_completed, 0);
            assert_eq!(observation.activity.cpu_work, 0);
        }
        assert!(!matches!(
            event,
            Event::Sample { .. } | Event::Complete { .. }
        ));
        Ok(())
    })
    .unwrap_err();
    assert!(
        matches!(error, Error::Native(error) if error.kind() == zetesis_ferraris::native_aggregate::ErrorKind::Stopped(Stop::Cancelled))
    );
    assert!(failed);
}

#[test]
fn reduction_exhaustion_cannot_publish_a_successful_sample() {
    let mut configuration = configuration();
    configuration.max_reduction_work = 0;
    let mut failed = false;
    let error = measurement::measure(&configuration, |event| {
        if let Event::Failed { observation, .. } = event {
            failed = true;
            assert_eq!(observation.activity.cpu_attempts, 41);
            assert_eq!(observation.activity.cpu_completed, 0);
        }
        assert!(!matches!(
            event,
            Event::Sample { .. } | Event::Complete { .. }
        ));
        Ok(())
    })
    .unwrap_err();
    assert!(matches!(error, Error::Native(_)));
    assert!(failed);
}

#[test]
fn preparation_failure_omits_prepared_evidence() {
    for reference in [false, true] {
        let mut configuration = configuration();
        if reference {
            configuration.max_reference_work = 0;
        } else {
            configuration.max_acquisition_work = 0;
        }
        let mut events = 0;
        assert!(matches!(
            measurement::measure(&configuration, |event| {
                events += 1;
                assert!(matches!(
                    event,
                    Event::Configuration { .. } | Event::Setup { .. }
                ));
                Ok(())
            }),
            Err(Error::Native(_))
        ));
        assert_eq!(events, 2);
    }
}

#[test]
fn every_refused_event_stops_the_publication_prefix() {
    for refused in 0..8 {
        let mut observed = 0;
        let error = measurement::measure(&configuration(), |_| {
            observed += 1;
            if observed == refused + 1 {
                Err(io::Error::other("consumer stopped"))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert!(matches!(error, Error::Output(_)));
        assert_eq!(observed, refused + 1);
    }
}

#[test]
fn cancelled_pool_callback_prevents_physical_setup() {
    let mut configuration = configuration();
    configuration.backend = Backend::Gpu(Some(GpuApi::Metal));
    let cancellation = Cancellation::default();
    let mut observed = 0;
    let error = measurement::measure_with_cancellation(&configuration, &cancellation, |event| {
        observed += 1;
        match event {
            Event::Configuration { .. } => {}
            Event::Setup {
                route: Route::Rayon,
                ..
            } => cancellation.cancel(),
            _ => panic!("cancelled callback proceeded to device setup"),
        }
        Ok(())
    })
    .unwrap_err();
    assert!(matches!(error, Error::Cpu(Stop::Cancelled)));
    assert_eq!(observed, 2);
}

#[test]
fn final_sample_cancellation_omits_completion() {
    let cancellation = Cancellation::default();
    let mut samples = 0;
    let error = measurement::measure_with_cancellation(&configuration(), &cancellation, |event| {
        if matches!(event, Event::Sample { .. }) {
            samples += 1;
            if samples == 4 {
                cancellation.cancel();
            }
        }
        assert!(!matches!(event, Event::Complete { .. }));
        Ok(())
    })
    .unwrap_err();
    assert!(matches!(error, Error::Cpu(Stop::Cancelled)));
    assert_eq!(samples, 4);
}

#[test]
fn invalid_schedules_emit_no_observations() {
    let base = configuration();
    let mut invalid = Vec::new();
    let mut value = base.clone();
    value.cases.clear();
    invalid.push(value);
    let mut value = base.clone();
    value.cases.resize(65, base.cases[0]);
    invalid.push(value);
    let mut value = base.clone();
    value.cases[0].tuples = 65_537;
    invalid.push(value);
    let mut value = base.clone();
    value.cases[0].occurrences = nonzero(257);
    invalid.push(value);
    let mut value = base.clone();
    value.warmups = 7;
    invalid.push(value);
    let mut value = base.clone();
    value.repetitions = nonzero(61);
    invalid.push(value);
    let mut value = base.clone();
    value.workers = nonzero(65);
    invalid.push(value);
    let mut value = base;
    value.gpu_timeout = Duration::ZERO;
    invalid.push(value);
    for value in invalid {
        assert!(matches!(
            measurement::measure(&value, |_| panic!("invalid schedule emitted a record")),
            Err(Error::Configuration(_))
        ));
    }
}

#[test]
fn command_defaults_declare_the_complete_case_matrix_on_the_cpu() {
    let command = CommandOptions::try_parse_from(["zetesis-bench", "aggregate"]).unwrap();
    let Experiment::Aggregate(options) = command.command.unwrap() else {
        panic!("wrong command")
    };
    let configuration = options.configuration().unwrap();
    assert_eq!(configuration.backend, Backend::Cpu);
    assert_eq!(configuration.cases.len(), 45);
    assert_eq!(configuration.repetitions.get(), 12);
    let value = serde_json::to_value(&configuration).unwrap();
    assert_eq!(value["gpu_timeout_ns"], 30_000_000_000_u64);
    assert_eq!(value["max_acquisition_work"], 100_000_000);
    assert_eq!(value["backend"], "cpu");
}

#[test]
fn command_refuses_empty_occurrence_populations() {
    assert!(
        CommandOptions::try_parse_from(["zetesis-bench", "aggregate", "--batches", "0"]).is_err()
    );
}

#[test]
fn declared_physical_cases_qualify_on_cpu() {
    let command = CommandOptions::try_parse_from(["zetesis-bench", "aggregate"]).unwrap();
    let Experiment::Aggregate(options) = command.command.unwrap() else {
        panic!("wrong command")
    };
    let mut configuration = options.configuration().unwrap();
    configuration.backend = Backend::Cpu;
    configuration.warmups = 0;
    configuration.repetitions = nonzero(1);
    let mut prepared = 0;
    let mut occurrences = 0;
    measurement::measure(&configuration, |event| {
        match event {
            Event::Prepared { reference, .. } => {
                prepared += 1;
                occurrences += reference.len();
            }
            Event::Complete { samples } => assert_eq!(*samples, 180),
            _ => {}
        }
        Ok(())
    })
    .unwrap();
    assert_eq!(prepared, 45);
    assert_eq!(occurrences, 2415);
}

#[test]
#[ignore = "requires actual Metal; explicit aggregate experiment qualification"]
fn metal_aggregate_measurements_require_actual_submissions() {
    qualify_device(Backend::Gpu(Some(GpuApi::Metal)));
}

#[test]
#[ignore = "requires an actual Vulkan GPU; explicit aggregate experiment qualification"]
fn vulkan_aggregate_measurements_require_actual_submissions() {
    qualify_device(Backend::Gpu(Some(GpuApi::Vulkan)));
}

fn qualify_device(backend: Backend) {
    let mut configuration = configuration();
    configuration.backend = backend;
    configuration.warmups = 1;
    configuration.repetitions = nonzero(4);
    let mut submissions = 0;
    let mut occurrences = 0;
    measurement::measure(&configuration, |event| {
        if let Event::Sample { sample } = event
            && let Some(device) = sample.observation.activity.device
        {
            submissions += device.activity.submissions;
            occurrences += device.activity.completed_occurrences;
            let serialized = serde_json::to_value(device).unwrap();
            assert_eq!(serialized["submissions"], 1);
            assert_eq!(serialized["batch"]["occurrences"], 41);
            assert!(serialized["batch"]["host_work"].as_u64().unwrap() > 0);
            assert_eq!(
                device.activity.completed_occurrences,
                device.activity.submitted_occurrences
            );
        }
        Ok(())
    })
    .unwrap();
    assert_eq!(submissions, 12);
    assert_eq!(occurrences, 492);
}
