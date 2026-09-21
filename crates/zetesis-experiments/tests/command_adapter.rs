//! Application dispatch preserves typed refusals and observer failures.
use clap::Parser;
use zetesis_experiments::{
    CommandOptions,
    command::{self, Completion, Error},
    primitives::{self, Event, Request},
};

#[test]
fn table_refusal_is_not_a_successful_completion() {
    let options = CommandOptions::try_parse_from([
        "bench",
        "table",
        "--rows",
        "1",
        "--queries",
        "1",
        "--workers",
        "1",
        "--warmups",
        "0",
        "--repetitions",
        "1",
        "--max-table-bytes",
        "0",
    ])
    .unwrap();
    let mut output = Vec::new();
    assert_eq!(
        command::execute(&options, &mut output).unwrap(),
        Completion::Refused
    );
    let records: Vec<serde_json::Value> = String::from_utf8(output)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(records.last().unwrap()["event"], "complete");
    assert_eq!(records.last().unwrap()["passed"], false);
}

#[test]
fn primitive_observer_failure_prevents_completion() {
    use zetesis_experiments::{
        Backend,
        relation_fixtures::{Family, Payload},
        relation_measurement,
    };
    let request = Request::Relation(relation_measurement::Configuration {
        family: Family::Single,
        payload: Payload::Numeric,
        rows: 1,
        queries: 1,
        backend: Backend::Cpu,
        workers: 1,
        warmups: 0,
        repetitions: 1,
        max_bytes: 1024 * 1024,
    });
    let mut starts = 0;
    let result = primitives::measure(&request, |event| {
        assert!(matches!(
            event,
            Event::Relation(relation_measurement::Event::Start { .. })
        ));
        starts += 1;
        Err(std::io::Error::other("injected event refusal"))
    });
    assert_eq!(starts, 1);
    assert!(
        matches!(result, Err(Error::Relation(relation_measurement::Error::Output(error))) if error.to_string() == "injected event refusal")
    );
}
