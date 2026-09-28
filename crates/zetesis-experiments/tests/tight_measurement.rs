//! Complete-theory certificate measurement has explicit evidence boundaries.
use std::{io, num::NonZeroUsize};
use zetesis_backend::GpuApi;

use clap::Parser;
use zetesis_experiments::{
    Backend, CommandOptions, Experiment,
    tight_measurement::{
        self as measurement, Case, Certificate, Configuration, Decision, Error, Event, Family,
        Phase, Reference, Route,
    },
};

fn nonzero(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap()
}
fn configuration(family: Family) -> Configuration {
    Configuration {
        cases: vec![Case {
            family,
            atoms: nonzero(4),
            candidates: nonzero(32),
            reference: Reference::ExhaustiveReduct,
        }],
        backend: Backend::Cpu,
        support: zetesis_experiments::tight_measurement::Support::Atomic,
        warmups: 0,
        repetitions: nonzero(1),
        workers: nonzero(2),
        plan_limits: zetesis_ferraris::TightPlanLimits::default(),
        certificate_limits: zetesis_ferraris::TightCheckLimits::default(),
        reference_limits: zetesis_ferraris::Limits::default(),
        residual_limits: zetesis_sat::Limits::default(),
        gpu_limits: zetesis_wgpu::TightGpuLimits::default(),
    }
}

#[test]
fn every_unfiltered_occurrence_matches_its_reference() {
    for family in [Family::Normal, Family::Choices] {
        let mut reference = Vec::new();
        let mut samples = Vec::new();
        measurement::measure(&configuration(family), |event| {
            match event {
                Event::Prepared {
                    reference: actual, ..
                } => reference = actual.to_vec(),
                Event::Sample { sample } => samples.push((*sample).clone()),
                _ => {}
            }
            Ok(())
        })
        .unwrap();
        assert_eq!(samples.len(), 4);
        assert_eq!(reference.len(), 32);
        for sample in samples {
            assert_eq!(sample.outcomes.len(), reference.len());
            for (actual, expected) in sample.outcomes.iter().zip(&reference) {
                assert_eq!(actual.certificate, expected.certificate);
                assert_eq!(
                    std::mem::discriminant(&actual.decision),
                    std::mem::discriminant(&expected.decision)
                );
            }
        }
    }
}

#[test]
fn normal_population_retains_three_decision_controls() {
    measurement::measure(&configuration(Family::Normal), |event| {
        if let Event::Prepared {
            candidates,
            reference,
            ..
        } = event
        {
            assert_eq!(candidates[..3], [vec![], vec![0, 1, 2, 3], vec![0, 1, 2]]);
            assert!(matches!(
                reference[0].certificate,
                Certificate::NotModel { .. }
            ));
            assert_eq!(
                reference[1].certificate,
                Certificate::Residual {
                    unsupported_atom: 3
                }
            );
            assert_eq!(reference[2].certificate, Certificate::Stable);
        }
        Ok(())
    })
    .unwrap();
}

#[test]
fn choice_population_preserves_empty_stability() {
    measurement::measure(&configuration(Family::Choices), |event| {
        if let Event::Prepared { reference, .. } = event {
            assert_eq!(reference[0].decision, Decision::Stable);
            assert!(matches!(reference[1].decision, Decision::NonMinimal { .. }));
        }
        Ok(())
    })
    .unwrap();
}

#[test]
fn cpu_samples_attribute_every_exact_completion() {
    measurement::measure(&configuration(Family::Normal), |event| {
        if let Event::Sample { sample } = event {
            let observation = sample.observation;
            let activity = observation.activity;
            let residuals = sample
                .outcomes
                .iter()
                .filter(|outcome| matches!(outcome.certificate, Certificate::Residual { .. }))
                .count();
            assert_eq!(activity.cpu_certificate_attempts, 32);
            assert_eq!(activity.classified, 32);
            assert_eq!(activity.cpu_decisions + residuals, 32);
            assert_eq!(activity.residual_attempts, residuals);
            assert_eq!(activity.residual_completed, residuals);
            assert_eq!(activity.gpu_decisions, 0);
            assert!(activity.device.is_none());
            assert!(activity.cpu_certificate_work > 0);
            assert!(
                observation.elapsed_ns
                    >= observation.classification_ns + observation.completion_ns.unwrap()
            );
        }
        Ok(())
    })
    .unwrap();
}

#[test]
fn timed_routes_rotate_independently_of_warmups() {
    let mut configuration = configuration(Family::Normal);
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
fn cancelled_classification_retains_its_attempts() {
    let cancellation = zetesis_cpu::Cancellation::default();
    let mut failure = None;
    let error = measurement::measure_with_cancellation(
        &configuration(Family::Normal),
        &cancellation,
        |event| {
            if matches!(event, Event::Prepared { .. }) {
                cancellation.cancel();
            }
            if let Event::Failed { observation, .. } = event {
                failure = Some(*observation);
            }
            assert!(!matches!(event, Event::Complete { .. }));
            Ok(())
        },
    )
    .unwrap_err();
    assert!(matches!(
        error,
        Error::Certificate(zetesis_ferraris::TightError::Stopped(
            zetesis_cpu::Stop::Cancelled
        ))
    ));
    let failure = failure.unwrap();
    assert_eq!(failure.activity.cpu_certificate_attempts, 32);
    assert_eq!(failure.activity.cpu_certificate_work, 0);
    assert_eq!(failure.activity.classified, 0);
    assert!(failure.completion_ns.is_none());
}

#[test]
fn residual_failure_cannot_publish_a_complete_sample() {
    let mut configuration = configuration(Family::Normal);
    configuration.residual_limits.search.max_work = 0;
    let mut events = Vec::new();
    let error = measurement::measure(&configuration, |event| {
        events.push(serde_json::to_value(event).unwrap());
        Ok(())
    })
    .unwrap_err();
    assert!(matches!(error, Error::Residual(_)));
    let failed = events.last().unwrap();
    assert_eq!(failed["event"], "failed");
    assert_eq!(failed["observation"]["activity"]["residual_attempts"], 1);
    assert_eq!(failed["observation"]["activity"]["residual_completed"], 0);
    assert!(
        !events
            .iter()
            .any(|event| event["event"] == "sample" || event["event"] == "complete")
    );
}

#[test]
fn every_refused_event_stops_its_publication_prefix() {
    // Configuration, setup, prepared, four samples and completion.
    for refused in 0..8 {
        let mut observed = 0;
        let error = measurement::measure(&configuration(Family::Normal), |_| {
            let current = observed;
            observed += 1;
            if current == refused {
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
fn reference_exhaustion_omits_prepared_evidence() {
    let mut configuration = configuration(Family::Normal);
    configuration.reference_limits.max_subsets = 0;
    let mut events = Vec::new();
    let error = measurement::measure(&configuration, |event| {
        events.push(serde_json::to_value(event).unwrap());
        Ok(())
    })
    .unwrap_err();
    assert!(matches!(error, Error::Cpu(_)));
    assert_eq!(events.len(), 2);
}

#[test]
fn wide_cases_use_complete_general_reduct_references() {
    for family in [Family::Normal, Family::Choices] {
        let mut configuration = configuration(family);
        configuration.cases[0].atoms = nonzero(256);
        configuration.cases[0].candidates = nonzero(3);
        configuration.cases[0].reference = Reference::GeneralReduct;
        let mut completed = false;
        measurement::measure(&configuration, |event| {
            if let Event::Complete { samples } = event {
                assert_eq!(*samples, 4);
                completed = true;
            }
            Ok(())
        })
        .unwrap();
        assert!(completed);
    }
}

#[test]
fn invalid_dimensions_emit_no_observations() {
    let base = configuration(Family::Normal);
    let mut invalid = Vec::new();
    let mut empty = base.clone();
    empty.cases.clear();
    invalid.push(empty);
    let mut many = base.clone();
    many.cases.resize(25, base.cases[0]);
    invalid.push(many);
    let mut large = base.clone();
    large.cases[0].atoms = nonzero(257);
    invalid.push(large);
    let mut tiny = base.clone();
    tiny.cases[0].atoms = nonzero(1);
    invalid.push(tiny);
    let mut exponential = base.clone();
    exponential.cases[0].atoms = nonzero(64);
    invalid.push(exponential);
    let mut occurrences = base.clone();
    occurrences.cases[0].candidates = nonzero(257);
    invalid.push(occurrences);
    let mut warmups = base.clone();
    warmups.warmups = 7;
    invalid.push(warmups);
    let mut repetitions = base.clone();
    repetitions.repetitions = nonzero(61);
    invalid.push(repetitions);
    let mut workers = base;
    workers.workers = nonzero(65);
    invalid.push(workers);
    for configuration in invalid {
        let error = measurement::measure(&configuration, |_| {
            panic!("invalid configuration emitted evidence")
        })
        .unwrap_err();
        assert!(matches!(error, Error::Configuration(_)));
    }
}

#[test]
fn command_defaults_select_the_declared_cases_on_the_cpu() {
    let command = CommandOptions::try_parse_from(["zetesis-bench", "tight"]).unwrap();
    let Experiment::Tight(options) = command.command.unwrap() else {
        panic!("wrong experiment")
    };
    let configuration = options.configuration().unwrap();
    assert_eq!(configuration.backend, Backend::Cpu);
    assert_eq!(configuration.cases.len(), 18);
    assert_eq!(configuration.repetitions.get(), 12);
    assert_eq!(configuration.warmups, 2);
    let record = serde_json::to_value(&configuration).unwrap();
    assert_eq!(record["gpu_limits"]["max_work_per_candidate"], 100_000_000);
    assert_eq!(
        record["residual_limits"]["search"]["max_decisions"],
        1_000_000
    );
    assert_eq!(record["cases"][0]["reference"], "exhaustive_reduct");
    assert_eq!(record["cases"][3]["reference"], "general_reduct");
}

#[test]
fn command_refuses_a_zero_candidate_population() {
    assert!(CommandOptions::try_parse_from(["zetesis-bench", "tight", "--batches", "0"]).is_err());
}

#[test]
fn command_records_the_selected_support_construction() {
    for (name, policy) in [
        ("atomic", measurement::Support::Atomic),
        ("grouped", measurement::Support::Grouped),
    ] {
        let command =
            CommandOptions::try_parse_from(["zetesis-bench", "tight", "--support", name]).unwrap();
        let Experiment::Tight(options) = command.command.unwrap() else {
            panic!("wrong experiment")
        };
        let configuration = options.configuration().unwrap();
        assert_eq!(configuration.support, policy);
        assert_eq!(
            serde_json::to_value(configuration).unwrap()["support"],
            name
        );
    }
}

#[test]
fn wide_dimensions_require_an_explicit_support_family() {
    for family in ["normal", "choices", "support-uniform", "support-skewed"] {
        let options = |atoms, batches| {
            let command = CommandOptions::try_parse_from([
                "zetesis-bench",
                "tight",
                "--families",
                family,
                "--atoms",
                atoms,
                "--batches",
                batches,
            ])
            .unwrap();
            let Experiment::Tight(options) = command.command.unwrap() else {
                panic!("wrong experiment")
            };
            options.configuration()
        };
        assert_eq!(
            options("4096", "1024").is_ok(),
            family.starts_with("support-")
        );
        assert!(options("4097", "1024").is_err());
        assert!(options("4096", "1025").is_err());
    }
}

#[test]
fn declared_physical_fixtures_qualify_on_cpu() {
    let command = CommandOptions::try_parse_from(["zetesis-bench", "tight"]).unwrap();
    let Experiment::Tight(options) = command.command.unwrap() else {
        panic!("wrong experiment")
    };
    let mut configuration = options.configuration().unwrap();
    configuration.backend = Backend::Cpu;
    configuration.warmups = 0;
    configuration.repetitions = nonzero(1);
    let mut prepared = 0;
    let mut occurrences = 0;
    let mut samples = 0;
    measurement::measure(&configuration, |event| {
        match event {
            Event::Prepared { candidates, .. } => {
                prepared += 1;
                occurrences += candidates.len();
            }
            Event::Sample { .. } => samples += 1,
            Event::Complete { samples: total } => assert_eq!(*total, 72),
            _ => {}
        }
        Ok(())
    })
    .unwrap();
    assert_eq!(prepared, 18);
    assert_eq!(occurrences, 966);
    assert_eq!(samples, 72);
}

#[test]
fn cancellation_before_setup_creates_no_resources() {
    let cancellation = zetesis_cpu::Cancellation::default();
    let mut observed = 0;
    let mut configuration = configuration(Family::Normal);
    configuration.backend = Backend::Gpu(Some(GpuApi::Metal));
    let error = measurement::measure_with_cancellation(&configuration, &cancellation, |event| {
        observed += 1;
        assert!(matches!(event, Event::Configuration { .. }));
        cancellation.cancel();
        Ok(())
    })
    .unwrap_err();
    assert!(matches!(error, Error::Cpu(zetesis_cpu::Stop::Cancelled)));
    assert_eq!(observed, 1);
}

#[test]
fn pool_callback_cancellation_prevents_device_setup() {
    let cancellation = zetesis_cpu::Cancellation::default();
    let mut observed = 0;
    let mut configuration = configuration(Family::Normal);
    configuration.backend = Backend::Gpu(Some(GpuApi::Metal));
    let error = measurement::measure_with_cancellation(&configuration, &cancellation, |event| {
        observed += 1;
        match event {
            Event::Configuration { .. } => {}
            Event::Setup {
                resource: Route::Rayon,
                ..
            } => cancellation.cancel(),
            _ => panic!("cancelled pool callback proceeded to device setup"),
        }
        Ok(())
    })
    .unwrap_err();
    assert!(matches!(error, Error::Cpu(zetesis_cpu::Stop::Cancelled)));
    assert_eq!(observed, 2);
}

#[test]
fn last_sample_cancellation_omits_campaign_completion() {
    let cancellation = zetesis_cpu::Cancellation::default();
    let mut samples = 0;
    let error = measurement::measure_with_cancellation(
        &configuration(Family::Normal),
        &cancellation,
        |event| {
            if matches!(event, Event::Sample { .. }) {
                samples += 1;
                if samples == 4 {
                    cancellation.cancel();
                }
            }
            assert!(!matches!(event, Event::Complete { .. }));
            Ok(())
        },
    )
    .unwrap_err();
    assert!(matches!(error, Error::Cpu(zetesis_cpu::Stop::Cancelled)));
    assert_eq!(samples, 4);
}

#[test]
fn cancellation_after_committed_completion_is_not_retroactive() {
    let cancellation = zetesis_cpu::Cancellation::default();
    let mut complete = false;
    measurement::measure_with_cancellation(
        &configuration(Family::Normal),
        &cancellation,
        |event| {
            if matches!(event, Event::Complete { .. }) {
                complete = true;
                cancellation.cancel();
            }
            Ok(())
        },
    )
    .unwrap();
    assert!(complete);
}
