//! Reproduction and refusal controls over the fixed historical record view.

use serde_json::Value;

use super::{data, dataset::HISTORICAL, render};

fn changed(change: impl FnOnce(&mut Value)) -> data::Observations {
    let mut document: Value = serde_json::from_str(HISTORICAL.observations).unwrap();
    change(&mut document);
    serde_json::from_value(document).unwrap()
}

#[test]
fn historical_samples_reproduce_the_published_tables() {
    let data = data::load(&HISTORICAL).unwrap();
    assert_eq!(
        render::tables(&data, &HISTORICAL).unwrap(),
        HISTORICAL.tables
    );
    // The retained table fixture is the actual public section, not an unrelated
    // expectation that can drift independently of the manual's numbers.
    assert!(
        include_str!("../../../../docs/book/reference/performance.md")
            .contains(HISTORICAL.tables.trim_end())
    );
}

#[test]
fn missing_or_extra_observations_refuse_table_publication() {
    for extra in [false, true] {
        let data = changed(|document| {
            let observations = document["blocks"][0]["observations"]
                .as_array_mut()
                .unwrap();
            if extra {
                observations.push(observations[0].clone());
            } else {
                observations.pop();
            }
        });
        assert!(
            render::tables(&data, &HISTORICAL)
                .unwrap_err()
                .to_string()
                .contains("missing or extra observation")
        );
    }
}

#[test]
fn a_duplicate_position_cannot_replace_another_observation() {
    let data = changed(|document| {
        document["blocks"][0]["observations"][1] = document["blocks"][0]["observations"][0].clone();
    });
    assert!(
        render::tables(&data, &HISTORICAL)
            .unwrap_err()
            .to_string()
            .contains("schedule position")
    );
}

#[test]
fn case_changes_refuse_table_publication() {
    for change in ["missing", "extra", "foreign"] {
        let data = changed(|document| {
            let cases = document["cases"].as_array_mut().unwrap();
            match change {
                "missing" => {
                    cases.pop();
                }
                "extra" => cases.push(cases[0].clone()),
                _ => cases[0]["path"] = "standalone/foreign.lp".into(),
            }
        });
        assert!(render::tables(&data, &HISTORICAL).is_err());
    }
}

#[test]
fn incomplete_capture_is_not_a_timed_sample() {
    let data = changed(|document| {
        let sample = document["blocks"][0]["observations"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|sample| sample["phase"] == "timed")
            .unwrap();
        sample["capture"]["stop"] = "deadline".into();
    });
    assert!(
        render::tables(&data, &HISTORICAL)
            .unwrap_err()
            .to_string()
            .contains("capture is incomplete")
    );
}

#[test]
fn changed_reported_cost_refuses_table_publication() {
    let data = changed(|document| {
        document["blocks"][0]["observations"][0]["cost"] = serde_json::json!([99]);
    });
    assert!(
        render::tables(&data, &HISTORICAL)
            .unwrap_err()
            .to_string()
            .contains("display/cost qualification")
    );
}

#[test]
fn helper_success_does_not_replace_the_memory_child_exit() {
    let data = changed(|document| {
        let sample = document["blocks"][0]["observations"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|sample| sample["phase"] == "memory" && sample["producer"] == "reference")
            .unwrap();
        assert_eq!(sample["capture"]["exit_code"], 0);
        sample["memory"]["exit_code"] = 0.into();
    });
    assert!(
        render::tables(&data, &HISTORICAL)
            .unwrap_err()
            .to_string()
            .contains("child-RSS receipt")
    );
}

#[test]
fn changed_executable_identity_refuses_table_publication() {
    let data = changed(|document| {
        document["blocks"][1]["binary_sha256"] = "0".repeat(64).into();
    });
    assert!(
        render::tables(&data, &HISTORICAL)
            .unwrap_err()
            .to_string()
            .contains("executable identity")
    );
}

#[test]
fn source_identity_is_checked_against_the_selected_dataset() {
    let data = data::load(&HISTORICAL).unwrap();
    let mut foreign = HISTORICAL;
    foreign.sources.swap(0, 1);
    assert!(
        render::tables(&data, &foreign)
            .unwrap_err()
            .to_string()
            .contains("executable identity")
    );
}

#[test]
fn join_policy_is_checked_against_the_selected_dataset() {
    let data = data::load(&HISTORICAL).unwrap();
    let mut foreign = HISTORICAL;
    foreign.joins = [Some("indexed"), Some("indexed")];
    assert!(
        render::tables(&data, &foreign)
            .unwrap_err()
            .to_string()
            .contains("join strategy")
    );
}
