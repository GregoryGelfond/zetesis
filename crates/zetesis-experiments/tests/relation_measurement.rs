//! Portable shared-fixture measurement contracts; no GPU is requested.

use std::io;

use clap::Parser;
use zetesis_cpu::{Control, Stop};
use zetesis_experiments::{
    Backend, CommandOptions, Experiment,
    relation_fixtures::{Family, Payload},
    relation_measurement::{self, Configuration, Error, Event},
};

fn configuration() -> Configuration {
    Configuration {
        family: Family::Independent,
        payload: Payload::Tuple,
        rows: 9,
        queries: 8,
        backend: Backend::Cpu,
        workers: 2,
        warmups: 0,
        repetitions: 1,
        max_bytes: 128 * 1024 * 1024,
    }
}

fn records(configuration: Configuration) -> Vec<serde_json::Value> {
    let mut records = Vec::new();
    relation_measurement::measure(configuration, &Control::default(), |event| {
        records.push(serde_json::to_value(event).unwrap());
        Ok(())
    })
    .unwrap();
    records
}

#[test]
fn report_schema_identifies_direct_mask_accounting() {
    let records = records(configuration());
    assert_eq!(records[0]["event"], "start");
    assert_eq!(records[0]["schema"], 2);
}

#[test]
fn cpu_work_includes_impossible_query_mask_initialization() {
    let records = records(Configuration {
        rows: 33,
        queries: 2,
        ..configuration()
    });
    // The first query accepts all rows; the second uses an absent value.
    // Both initialize two mask words, while only the first visits 33 rows.
    let observations: Vec<_> = records
        .iter()
        .filter(|record| record["event"] == "observation")
        .collect();
    assert_eq!(observations.len(), 4);
    for observation in observations {
        assert_eq!(observation["cpu_selection_work"], 70);
        assert_eq!(observation["masks"], serde_json::json!([u32::MAX, 1, 0, 0]));
    }
}

#[test]
fn cpu_routes_publish_identical_packed_masks() {
    let records = records(configuration());
    let observations: Vec<_> = records
        .iter()
        .filter(|record| record["event"] == "observation")
        .collect();
    assert_eq!(observations.len(), 4);
    for pair in observations.chunks_exact(2) {
        assert_eq!(pair[0]["route"], "scalar");
        assert_eq!(pair[1]["route"], "rayon");
        assert_eq!(pair[0]["masks"], pair[1]["masks"]);
        assert_eq!(pair[0]["subject_sha256"], pair[1]["subject_sha256"]);
        assert_eq!(
            pair[0]["reconstructed_cells"],
            pair[1]["reconstructed_cells"]
        );
        assert!(pair[0]["reconstructed_cells"].as_u64().unwrap() > 0);
        assert!(pair[0]["device"].is_null());
        assert!(pair[1]["device"].is_null());
    }
    assert_eq!(records.last().unwrap()["event"], "complete");
    assert_eq!(records.last().unwrap()["observations"], 4);
}

#[test]
fn source_identity_preserves_typed_payloads() {
    let numeric = records(Configuration {
        payload: Payload::Numeric,
        ..configuration()
    });
    let tuple = records(configuration());
    let left = &numeric[1];
    let right = &tuple[1];
    assert_eq!(left["event"], "subject");
    assert_eq!(left["subject"]["rows"][0][0]["kind"], "number");
    assert_eq!(right["subject"]["rows"][0][0]["kind"], "structured");
    assert_ne!(left["sha256"], right["sha256"]);
    assert_eq!(right["sha256"], records(configuration())[1]["sha256"]);
    assert_eq!(right["subject"]["queries"].as_array().unwrap().len(), 8);
    assert_eq!(right["subject"]["rows"].as_array().unwrap().len(), 9);
}

#[test]
fn failed_observation_prevents_completion_offer() {
    let mut events = Vec::new();
    let result = relation_measurement::measure(configuration(), &Control::default(), |event| {
        let event = serde_json::to_value(event).unwrap();
        let name = event["event"].as_str().unwrap().to_owned();
        events.push(name.clone());
        if name == "observation" {
            Err(io::Error::other("closed output"))
        } else {
            Ok(())
        }
    });
    assert!(matches!(result, Err(Error::Output(_))));
    assert_eq!(events, ["start", "subject", "observation"]);
}

#[test]
fn rejected_completion_returns_output_failure() {
    let mut offered = false;
    let result = relation_measurement::measure(configuration(), &Control::default(), |event| {
        if matches!(event, Event::Complete { .. }) {
            offered = true;
            Err(io::Error::other("completion sink failed"))
        } else {
            Ok(())
        }
    });
    assert!(offered);
    assert!(matches!(result, Err(Error::Output(_))));
}

#[test]
fn cancellation_after_preparation_stops_measurement() {
    let control = Control::default();
    let mut observations = 0;
    let result = relation_measurement::measure(configuration(), &control, |event| {
        match event {
            Event::Subject { .. } => control.cancel(),
            Event::Observation(_) | Event::Complete { .. } => observations += 1,
            Event::Start { .. } => {}
        }
        Ok(())
    });
    assert!(matches!(result, Err(Error::Stopped(Stop::Cancelled))));
    assert_eq!(observations, 0);
}

#[test]
fn exhausted_capacity_is_not_a_complete_measurement() {
    let mut events = Vec::new();
    let result = relation_measurement::measure(
        Configuration {
            max_bytes: 1,
            ..configuration()
        },
        &Control::default(),
        |event| {
            events.push(serde_json::to_value(event).unwrap());
            Ok(())
        },
    );
    assert!(result.is_err());
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["event"], "start");
}

#[test]
fn empty_query_populations_remain_explicit() {
    let records = records(Configuration {
        queries: 0,
        ..configuration()
    });
    let observations: Vec<_> = records
        .iter()
        .filter(|record| record["event"] == "observation")
        .collect();
    assert_eq!(observations.len(), 4);
    for observation in observations {
        assert_eq!(observation["queries"], 0);
        assert_eq!(observation["masks"], serde_json::json!([]));
        assert_eq!(observation["reconstructed_cells"], 0);
        assert!(observation["device"].is_null());
    }
}

#[test]
fn relation_command_preserves_explicit_scope() {
    let command = CommandOptions::try_parse_from([
        "zetesis-bench",
        "relation",
        "--family",
        "skewed",
        "--payload",
        "tuple",
        "--rows",
        "33",
        "--queries",
        "8",
        "--backend",
        "vulkan",
        "--workers",
        "4",
        "--warmups",
        "0",
        "--repetitions",
        "1",
    ])
    .unwrap();
    let Experiment::Relation(options) = command.command.unwrap() else {
        panic!("relation command")
    };
    let configuration = options.configuration().unwrap();
    assert_eq!(configuration.family, Family::Skewed);
    assert_eq!(configuration.payload, Payload::Tuple);
    assert_eq!(configuration.backend, Backend::Vulkan);
    assert_eq!(configuration.rows, 33);
    assert_eq!(configuration.queries, 8);
}

fn physical_records(backend: Backend) {
    let records = records(Configuration {
        backend,
        rows: 33,
        ..configuration()
    });
    let expected = match backend {
        Backend::Metal => "metal",
        Backend::Vulkan => "vulkan",
        Backend::Cpu => panic!("physical backend"),
    };
    let observations: Vec<_> = records
        .iter()
        .filter(|record| record["event"] == "observation")
        .collect();
    assert_eq!(observations.len(), 6);
    for group in observations.chunks_exact(3) {
        assert_eq!(group[0]["route"], "scalar");
        assert_eq!(group[1]["route"], "rayon");
        assert_eq!(group[2]["route"], expected);
        assert_eq!(group[0]["masks"], group[2]["masks"]);
        assert_eq!(group[1]["masks"], group[2]["masks"]);
        assert_eq!(
            group[0]["reconstructed_cells"],
            group[2]["reconstructed_cells"]
        );
        assert_eq!(group[2]["device"]["submissions"], 1);
        assert_eq!(group[2]["device"]["submitted_queries"], 8);
        assert_eq!(group[2]["device"]["completed_queries"], 8);
        assert_eq!(group[2]["device"]["workgroups"], 8);
        assert!(group[2]["device"]["downloaded_bytes"].as_u64().unwrap() > 0);
    }
    assert_eq!(records.last().unwrap()["observations"], 6);
}

#[test]
#[ignore = "requires actual Metal; explicit physical qualification"]
fn metal_relation_measurement_keeps_complete_masks() {
    physical_records(Backend::Metal);
}

#[test]
#[ignore = "requires actual Vulkan GPU; explicit physical qualification"]
fn vulkan_relation_measurement_keeps_complete_masks() {
    physical_records(Backend::Vulkan);
}
