//! Reproduction and refusal controls over the fixed comparison record views.

use clap::Parser;
use serde_json::Value;

use super::{
    Options,
    catalog_dataset::ATOM_CATALOG,
    data,
    dataset::HISTORICAL,
    prepared_dataset::{PREPARED_ALGORITHMS, PREPARED_GROUNDING},
    render,
};

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
        include_str!("../../../../docs/book/reference/grounding-measurements.md")
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
        include_str!("../../../../docs/book/reference/grounding-measurements.md")
            .contains(ATOM_CATALOG.tables.trim_end())
    );
}

#[test]
fn prepared_grounding_samples_reproduce_the_published_tables() {
    let data = data::load(&PREPARED_GROUNDING).unwrap();
    assert_eq!(
        render::tables(&data, &PREPARED_GROUNDING).unwrap(),
        PREPARED_GROUNDING.tables
    );
    assert!(
        include_str!("../../../../docs/book/reference/grounding-measurements.md")
            .contains(PREPARED_GROUNDING.tables.trim_end())
    );
}

#[test]
fn prepared_algorithm_samples_reproduce_the_published_tables() {
    let data = data::load(&PREPARED_ALGORITHMS).unwrap();
    assert_eq!(
        render::tables(&data, &PREPARED_ALGORITHMS).unwrap(),
        PREPARED_ALGORITHMS.tables
    );
    assert!(
        include_str!("../../../../docs/book/reference/grounding-measurements.md")
            .contains(PREPARED_ALGORITHMS.tables.trim_end())
    );
}

#[test]
fn prepared_selectors_reproduce_their_own_observations() {
    for (name, expected) in [
        ("release-ca10a5e7-679ca856", &PREPARED_GROUNDING),
        ("release-f56a5a24-679ca856", &PREPARED_ALGORITHMS),
    ] {
        for checked in [false, true] {
            let mut arguments = vec!["release_observations", "--dataset", name];
            if checked {
                arguments.push("--check");
            }
            let options = Options::try_parse_from(arguments).unwrap();
            assert_eq!(options.check, checked);
            let selected = options.dataset.dataset();
            assert_eq!(selected.sources, expected.sources);
            assert_eq!(selected.labels, expected.labels);
            let data = data::load(selected).unwrap();
            assert_eq!(render::tables(&data, selected).unwrap(), expected.tables);
        }
    }
}

#[test]
fn prepared_views_preserve_six_unique_observation_populations() {
    let first: Value = serde_json::from_str(PREPARED_GROUNDING.observations).unwrap();
    let second: Value = serde_json::from_str(PREPARED_ALGORITHMS.observations).unwrap();
    let mut unique = std::collections::BTreeMap::new();
    for document in [&first, &second] {
        for block in document["blocks"].as_array().unwrap() {
            let hash = block["original_report_sha256"].as_str().unwrap();
            if let Some(previous) = unique.insert(hash, block) {
                assert_eq!(
                    previous, block,
                    "shared reports must retain identical observations"
                );
            }
        }
    }
    assert_eq!(unique.len(), 6);
    assert_eq!(
        unique
            .values()
            .map(|block| block["observations"].as_array().unwrap().len())
            .sum::<usize>(),
        702
    );
}

#[test]
fn prepared_provenance_preserves_the_full_acquisition_order() {
    let first: Value = serde_json::from_str(PREPARED_GROUNDING.provenance).unwrap();
    let second: Value = serde_json::from_str(PREPARED_ALGORITHMS.provenance).unwrap();
    let labels = serde_json::json!([
        "ca10a5e7-1",
        "f56a5a24-1",
        "679ca856-1",
        "679ca856-2",
        "f56a5a24-2",
        "ca10a5e7-2"
    ]);
    assert_eq!(first["acquisition_order"], labels);
    assert_eq!(second["acquisition_order"], labels);
    assert_eq!(first["acquisition_reports"], second["acquisition_reports"]);
    let actual: Vec<_> = first["acquisition_reports"]
        .as_array()
        .unwrap()
        .iter()
        .map(|report| report["label"].clone())
        .collect();
    assert_eq!(Value::Array(actual), labels);
}

#[test]
fn undeclared_block_labels_refuse_publication() {
    let mut dataset = HISTORICAL;
    dataset.labels[0] = "different-acquisition";
    assert!(data::load(&dataset).is_err());
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
