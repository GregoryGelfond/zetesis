//! Matched lazy experiment routes preserve evidence and failure boundaries.
use std::{io, num::NonZeroUsize, time::Duration};

use clap::Parser;
use zetesis_experiments::{
    Backend, CommandOptions, Experiment,
    lazy_measurement::{
        self as measurement, Case, Configuration, Error, Event, Family, Phase, Route,
    },
};

fn nonzero(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap()
}

fn configuration(family: Family) -> Configuration {
    Configuration {
        cases: vec![Case {
            family,
            width: nonzero(3),
            worlds: nonzero(5),
        }],
        backend: Backend::Cpu,
        warmups: 0,
        repetitions: nonzero(1),
        workers: nonzero(2),
        cpu_limits: zetesis_cpu::Limits::default(),
        source_limits: zetesis_cpu::lazy::Limits::default(),
        gpu_limits: zetesis_wgpu::GpuLimits::default(),
    }
}

#[test]
fn cpu_routes_check_every_ordered_seed_occurrence() {
    for (family, accepted) in [(Family::Sparse, 3), (Family::Dense, 0)] {
        let mut samples = Vec::new();
        let mut complete = None;
        measurement::measure(&configuration(family), |event| {
            if let Event::Sample(sample) = event {
                samples.push(sample.clone());
            }
            if let Event::Complete { samples } = event {
                complete = Some(*samples);
            }
            Ok(())
        })
        .unwrap();
        assert_eq!(complete, Some(8));
        assert_eq!(samples.len(), 8);
        for sample in &samples {
            assert_eq!((sample.checked, sample.accepted), (5, accepted));
            assert!(sample.device.is_none());
        }
        assert_eq!(
            samples
                .iter()
                .filter(|sample| sample.phase == Phase::Timed)
                .count(),
            4
        );
    }
}

#[test]
fn samples_expose_only_receipts_of_their_route() {
    measurement::measure(&configuration(Family::Sparse), |event| {
        if let Event::Sample(sample) = event {
            let independent = matches!(sample.route, Route::Scalar | Route::Rayon);
            assert_eq!(sample.independent.is_some(), independent);
            assert_eq!(sample.queries.is_some(), sample.route == Route::Rayon);
            assert_eq!(sample.source.is_some(), !independent);
            let json = serde_json::to_value(sample).unwrap();
            assert_eq!(json["independent"].is_null(), !independent);
            if let Some(statistics) = sample.queries {
                let preparation = statistics.preparation.unwrap();
                assert_eq!(
                    json["queries"],
                    serde_json::json!({
                        "preparation": {
                            "work": preparation.work,
                            "retained_bytes": preparation.retained_bytes,
                        },
                        "preparation_builds": statistics.preparation_builds,
                        "retained_workspaces": statistics.retained_workspaces,
                        "active_workspaces": statistics.active_workspaces,
                        "reused_workspaces": statistics.reused_workspaces,
                        "retained_bytes": statistics.retained_bytes,
                        "reserved_bytes": statistics.reserved_bytes,
                    })
                );
            } else {
                assert!(json["queries"].is_null());
            }
        }
        Ok(())
    })
    .unwrap();
}

#[test]
fn sparse_worlds_omit_cross_world_instances() {
    let mut samples = Vec::new();
    measurement::measure(&configuration(Family::Sparse), |event| {
        if let Event::Sample(sample) = event {
            samples.push(sample.clone());
        }
        Ok(())
    })
    .unwrap();
    let union = samples
        .iter()
        .find(|sample| sample.route == Route::PortableUnion)
        .unwrap()
        .source
        .unwrap();
    let worlds = samples
        .iter()
        .find(|sample| sample.route == Route::PortableWorlds)
        .unwrap()
        .source
        .unwrap();
    assert!(worlds.instances < union.instances);
    assert!(worlds.pruned_prefixes > 0);
    assert_eq!(union.pruned_prefixes, 0);
    // Union still selects source rows from packed per-world truth. Its final
    // canonical ID vector retains all 4×3 unary identities and 3³ possible
    // triple identities, even though cross-world triples are never derived.
    let union_identities = 4 * 3 + 3_usize.pow(3);
    assert_eq!(union.catalog_atoms, union_identities);
    assert!(union.mask_words > 0);
    assert!(union.peak_mask_bytes >= union_identities * size_of::<usize>());
    assert!(worlds.peak_mask_bytes > 0);
}

#[test]
fn dense_worlds_preserve_the_union_product() {
    let mut instances = Vec::new();
    measurement::measure(&configuration(Family::Dense), |event| {
        if let Event::Sample(sample) = event
            && let Some(source) = sample.source
        {
            assert_eq!(source.pruned_prefixes, 0);
            instances.push(source.instances);
        }
        Ok(())
    })
    .unwrap();
    assert_eq!(instances.len(), 4);
    assert!(instances.iter().all(|count| *count == instances[0]));
}

#[test]
fn timed_iterations_rotate_every_cpu_route_position() {
    let mut configuration = configuration(Family::Sparse);
    configuration.repetitions = nonzero(4);
    configuration.warmups = 1;
    let mut routes = Vec::new();
    measurement::measure(&configuration, |event| {
        if let Event::Sample(sample) = event
            && sample.phase == Phase::Timed
        {
            routes.push((sample.iteration, sample.position, sample.route));
        }
        Ok(())
    })
    .unwrap();
    for route in [
        Route::Scalar,
        Route::Rayon,
        Route::PortableUnion,
        Route::PortableWorlds,
    ] {
        let mut positions = routes
            .iter()
            .filter(|(_, _, actual)| *actual == route)
            .map(|(_, position, _)| *position)
            .collect::<Vec<_>>();
        positions.sort_unstable();
        assert_eq!(positions, [0, 1, 2, 3]);
    }
}

#[test]
fn source_failure_retains_the_attempted_route() {
    let mut configuration = configuration(Family::Sparse);
    configuration.source_limits.max_rounds = 1;
    let mut events = Vec::new();
    let error = measurement::measure(&configuration, |event| {
        events.push(serde_json::to_value(event).unwrap());
        Ok(())
    })
    .unwrap_err();
    assert!(matches!(error, Error::Source(_)));
    let failure = events.last().unwrap();
    assert_eq!(failure["event"], "failed");
    assert_eq!(failure["route"], "portable-union");
    assert_eq!(failure["source"]["rounds"], 1);
    assert!(!events.iter().any(|event| event["event"] == "complete"));
}

#[test]
fn scalar_exhaustion_cannot_publish_completion() {
    let mut configuration = configuration(Family::Sparse);
    configuration.cpu_limits.max_work = 0;
    let mut complete = false;
    let error = measurement::measure(&configuration, |event| {
        complete |= matches!(event, Event::Complete { .. });
        Ok(())
    })
    .unwrap_err();
    assert!(matches!(error, Error::Cpu(_)));
    assert!(!complete);
}

#[test]
fn refused_events_stop_publication() {
    for refused in 0..=11 {
        let mut observed = 0;
        let error = measurement::measure(&configuration(Family::Sparse), |_| {
            let index = observed;
            observed += 1;
            if index == refused {
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
fn invalid_dimensions_fail_before_observation() {
    let base = configuration(Family::Sparse);
    let mut invalid = Vec::new();
    let mut empty = base.clone();
    empty.cases.clear();
    invalid.push(empty);
    let mut many = base.clone();
    many.cases = vec![base.cases[0]; 65];
    invalid.push(many);
    let mut width = base.clone();
    width.cases[0].width = nonzero(17);
    invalid.push(width);
    let mut worlds = base.clone();
    worlds.cases[0].worlds = nonzero(257);
    invalid.push(worlds);
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
        let mut called = false;
        assert!(matches!(
            measurement::measure(&configuration, |_| {
                called = true;
                Ok(())
            }),
            Err(Error::Configuration(_))
        ));
        assert!(!called);
    }
}

#[test]
fn command_json_lines_retain_the_complete_configuration() {
    let command = CommandOptions::try_parse_from([
        "zetesis-bench",
        "lazy",
        "--backend",
        "cpu",
        "--widths",
        "1",
        "--batches",
        "1",
        "--families",
        "sparse",
        "--warmups",
        "0",
        "--repetitions",
        "1",
    ])
    .unwrap();
    let Some(Experiment::Lazy(options)) = command.command else {
        panic!("wrong profile");
    };
    let mut output = Vec::new();
    measurement::run(&options, &mut output).unwrap();
    let records = String::from_utf8(output)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(records[0]["schema"], 2);
    assert_eq!(
        records[0]["configuration"]["source_per_batch"]["max_host_bytes"],
        134_217_728
    );
    assert_eq!(
        records[0]["configuration"]["gpu_per_chunk"]["timeout_ns"],
        30_000_000_000_u64
    );
    assert_eq!(records[1]["adapter"], serde_json::Value::Null);
    assert_eq!(records.last().unwrap()["event"], "complete");
}

#[test]
fn configuration_preserves_candidate_storage_limit() {
    let mut configuration = configuration(Family::Sparse);
    configuration.cpu_limits.max_closure_bytes = 1_234_567;
    let view = serde_json::to_value(&configuration).unwrap();
    assert_eq!(view["cpu_per_candidate"]["max_closure_bytes"], 1_234_567);
}

#[test]
fn oversized_duration_serialization_does_not_panic() {
    let mut configuration = configuration(Family::Sparse);
    configuration.gpu_limits.timeout = Duration::MAX;
    let mut output = Vec::new();
    serde_json::to_writer(&mut output, &configuration).unwrap();
    assert!(
        String::from_utf8(output)
            .unwrap()
            .contains(&Duration::MAX.as_nanos().to_string())
    );
}

#[test]
fn command_dimensions_are_checked_before_case_allocation() {
    let command = CommandOptions::try_parse_from(["zetesis-bench", "lazy"]).unwrap();
    let Some(Experiment::Lazy(mut options)) = command.command else {
        panic!("wrong profile");
    };
    options.widths = vec![nonzero(1); 65];
    assert!(matches!(
        options.configuration(),
        Err(Error::Configuration(_))
    ));
    options.widths.clear();
    assert!(matches!(
        options.configuration(),
        Err(Error::Configuration(_))
    ));
}
