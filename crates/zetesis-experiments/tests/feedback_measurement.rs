//! Public command/schema and finite completion contracts; no timing assertions.

use clap::Parser;
use zetesis_experiments::{CommandOptions, Experiment, feedback_measurement::{self, Event}};

#[test]
fn checked_command_retains_all_eight_exact_sources() {
    let parsed = CommandOptions::try_parse_from(["zetesis-bench", "feedback", "--check"]).unwrap();
    let Some(Experiment::Feedback(options)) = parsed.command else { panic!("feedback command"); };
    let mut output = Vec::new();
    feedback_measurement::run(&options, &mut output).unwrap();
    let records: Vec<serde_json::Value> = output.split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty()).map(|line| serde_json::from_slice(line).unwrap()).collect();
    assert_eq!(records[0]["schema"], 1);
    assert_eq!(records[0]["configuration"]["repetitions"], 0);
    assert_eq!(records[0]["native_limits"]["candidates"], 64);
    assert_eq!(records[0]["native_limits"]["projection_entries"], 64);
    let qualified: Vec<_> = records.iter().filter(|record| record["event"] == "qualified").collect();
    assert_eq!(qualified.len(), 8);
    assert_eq!(qualified.iter().map(|record| record["pairs"].as_u64().unwrap()).sum::<u64>(), 4742);
    for record in qualified {
        assert!(record["nodes"].is_array());
        assert!(record["roots"].is_array());
        assert!(record["stable"].is_array());
    }
    assert_eq!(records.last().unwrap()["event"], "complete");
    assert_eq!(records.last().unwrap()["samples"], 32);
}

#[test]
fn supplied_cancellation_never_publishes_completion() {
    let control = zetesis_cpu::Control::default();
    control.cancel();
    let mut completed = false;
    let result = feedback_measurement::measure_with_control(&Default::default(), &control, |event| {
        completed |= matches!(event, Event::Complete { .. });
        Ok(())
    });
    assert!(matches!(result, Err(feedback_measurement::Error::Control(zetesis_cpu::Stop::Cancelled))));
    assert!(!completed);
}

#[test]
fn finite_schedule_refuses_extra_repetitions() {
    let parsed = CommandOptions::try_parse_from(["zetesis-bench", "feedback", "--repetitions", "13"]).unwrap();
    let Some(Experiment::Feedback(options)) = parsed.command else { panic!("feedback command"); };
    assert!(matches!(options.configuration(), Err(feedback_measurement::Error::Configuration(_))));
}
