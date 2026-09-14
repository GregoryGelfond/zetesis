//! Reproduction and refusal controls over the fixed comparison record views.

use clap::Parser;
use serde_json::Value;

use super::{Options, catalog_dataset::ATOM_CATALOG, data, dataset::HISTORICAL, render};

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
fn catalog_samples_reproduce_the_published_tables() {
    let data = data::load(&ATOM_CATALOG).unwrap();
    assert_eq!(
        render::tables(&data, &ATOM_CATALOG).unwrap(),
        ATOM_CATALOG.tables
    );
    assert!(
        include_str!("../../../../docs/book/reference/performance.md")
            .contains(ATOM_CATALOG.tables.trim_end())
    );
}

#[test]
fn the_default_selection_preserves_the_historical_comparison() {
    for arguments in [
        vec!["release_observations"],
        vec!["release_observations", "--check"],
        vec!["release_observations", "--dataset", "table-grounding"],
    ] {
        let options = Options::try_parse_from(arguments).unwrap();
        assert_eq!(options.dataset.dataset().sources, HISTORICAL.sources);
    }
}

#[test]
fn the_catalog_selector_renders_its_own_observations() {
    for checked in [false, true] {
        let mut arguments = vec!["release_observations", "--dataset", "atom-catalog"];
        if checked {
            arguments.push("--check");
        }
        let options = Options::try_parse_from(arguments).unwrap();
        assert_eq!(options.check, checked);
        let dataset = options.dataset.dataset();
        assert_eq!(dataset.sources, ATOM_CATALOG.sources);
        let data = data::load(dataset).unwrap();
        let tables = render::tables(&data, dataset).unwrap();
        assert_eq!(tables, ATOM_CATALOG.tables);
        assert_ne!(tables, HISTORICAL.tables);
    }
}

#[test]
fn an_unrecognized_or_ambiguous_selection_refuses() {
    use clap::error::ErrorKind;

    for (arguments, expected) in [
        (vec!["--dataset", "foreign"], ErrorKind::InvalidValue),
        (
            vec!["--dataset", "table-grounding", "--dataset", "atom-catalog"],
            ErrorKind::ArgumentConflict,
        ),
        (vec!["--check", "--check"], ErrorKind::ArgumentConflict),
        (vec!["report.json"], ErrorKind::UnknownArgument),
    ] {
        let result =
            Options::try_parse_from(std::iter::once("release_observations").chain(arguments));
        let Err(error) = result else {
            panic!("invalid comparison selection accepted");
        };
        assert_eq!(error.kind(), expected);
    }
}

#[test]
fn catalog_receipts_cannot_be_relabelled_as_the_historical_comparison() {
    let data = data::load(&ATOM_CATALOG).unwrap();
    assert!(
        render::tables(&data, &HISTORICAL)
            .unwrap_err()
            .to_string()
            .contains("provenance digest")
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
