//! Finite selection and serial libtest protocol validation.
use crate::{Error, require};
use regex::Regex;
use serde::Serialize;
use std::{collections::BTreeSet, sync::LazyLock};
use zetesis_backend::GpuApi;

/// One reviewed physical target and its exact selected tests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Group {
    /// Stable report directory label.
    pub group: String,
    /// Cargo library or integration-test target.
    pub target_kind: String,
    /// Serialized target identity, not an executable filename.
    pub target: String,
    /// Exact libtest names; successful zero-match runs cannot substitute.
    pub tests: Vec<String>,
    /// Independently specified count for this group.
    pub expected_tests: usize,
}
/// A reviewed physical selection: the GPU API it qualifies and its groups.
#[derive(Clone, Debug)]
pub struct Selection {
    /// The API whose reviewed table this is.
    pub api: GpuApi,
    /// The fourteen groups with their exact tests.
    pub groups: Vec<Group>,
}

/// The reviewed selections: the Metal one and the Vulkan one, the same
/// fourteen groups and counts, each naming the tests of its own API.
const SELECTIONS: [(GpuApi, &str); 2] = [
    (GpuApi::Metal, include_str!("physical-selection.txt")),
    (
        GpuApi::Vulkan,
        include_str!("physical-selection-vulkan.txt"),
    ),
];

/// Parse the maintained shell table, checking its finite inventory independently.
/// The table must be one reviewed selection whole; one API's tests cannot
/// stand in for the other's, and the selection says which API's it is.
/// # Errors
/// Refuses missing/extra groups, altered target/count identities or duplicate tests.
pub fn selection(table: &str) -> Result<Selection, Error> {
    const EXPECTED: [(&str, &str, usize); 14] = [
        ("wgpu-lib", "lib", 15),
        ("tight", "integration", 4),
        ("formula", "integration", 2),
        ("aggregate", "integration", 3),
        ("lazy", "integration", 4),
        ("cli-lazy", "integration", 5),
        ("cli-formula", "integration", 3),
        ("world-views", "integration", 4),
        ("relation", "integration", 2),
        ("context", "integration", 1),
        ("solve-context", "lib", 3),
        ("session-resources", "integration", 13),
        ("language-consumers", "integration", 2),
        ("static", "integration", 2),
    ];
    let api = SELECTIONS
        .iter()
        .find(|(_, selection)| table.trim() == selection.trim())
        .map(|(api, _)| *api)
        .ok_or_else(|| {
            Error::Invalid(
                "physical qualification requires one reviewed selection of 63 exact test identities".into(),
            )
        })?;
    let rows: Vec<_> = table.lines().collect();
    require(
        rows.len() == EXPECTED.len(),
        "physical qualification requires all fourteen groups and 63 named tests",
    )?;
    let mut groups = Vec::new();
    let mut all_names = BTreeSet::new();
    for (row, (name, target, count)) in rows.iter().zip(EXPECTED) {
        let fields: Vec<_> = row.split('|').collect();
        require(
            fields.len() == 4,
            "invalid physical qualification table row",
        )?;
        require(
            fields[0] == name && fields[1] == target && fields[2].parse::<usize>() == Ok(count),
            "changed physical qualification group identity",
        )?;
        let tests: Vec<String> = fields[3].split_whitespace().map(str::to_owned).collect();
        require(
            tests.len() == count && tests.iter().all(|test| all_names.insert(test.clone())),
            "invalid physical qualification selection",
        )?;
        groups.push(Group {
            group: name.into(),
            target_kind: if target == "lib" { "lib" } else { "test" }.into(),
            target: if target == "lib" {
                "workspace libraries"
            } else {
                target
            }
            .into(),
            tests,
            expected_tests: count,
        });
    }
    Ok(Selection { api, groups })
}
static RECORD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^test (\S+) \.\.\.[ \t]*(.*)$").unwrap());
static SUMMARY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^test result: ok\. ([0-9]+) passed; 0 failed; 0 ignored; 0 measured; [0-9]+ filtered out; finished in [^\r\n]+$").unwrap()
});

/// Validate exact individual outcomes and complete libtest summaries.
/// Input must be serial `--nocapture` output: each record's final outcome is `ok`.
/// Library selection may additionally contain zero-test workspace summaries.
/// # Errors
/// Refuses altered names/multiplicities, missing/failed outcomes, extra positive
/// summaries, ignored tests and zero-match invocations. Exit status is checked
/// independently by the invoking orchestration before this parser is called.
pub fn physical_result(output: &str, group: &Group) -> Result<(), Error> {
    super::input_bytes(output.len())?;
    let mut selected = Vec::new();
    let mut current_outcome: Option<&str> = None;
    let mut counts = Vec::new();
    for line in output.lines() {
        if let Some(record) = RECORD.captures(line) {
            if let Some(outcome) = current_outcome {
                require(
                    outcome == "ok",
                    "physical test record lacks a passing outcome",
                )?;
            }
            selected.push(record[1].to_owned());
            current_outcome = Some(
                record
                    .get(2)
                    .ok_or_else(|| Error::Invalid("missing physical outcome capture".into()))?
                    .as_str()
                    .trim(),
            );
        } else if line.starts_with("test result:") {
            if let Some(outcome) = current_outcome.take() {
                require(
                    outcome == "ok",
                    "physical test record lacks a passing outcome",
                )?;
            }
            let summary = SUMMARY
                .captures(line)
                .ok_or_else(|| Error::Invalid("incomplete physical libtest summary".into()))?;
            counts.push(
                summary[1]
                    .parse::<usize>()
                    .map_err(|_| Error::Invalid("invalid physical summary count".into()))?,
            );
        } else if line.starts_with("test ") {
            if let Some(outcome) = current_outcome.take() {
                require(
                    outcome == "ok",
                    "physical test record lacks a passing outcome",
                )?;
            }
        } else if current_outcome.is_some() && !line.trim().is_empty() {
            current_outcome = Some(line.trim());
        }
    }
    if let Some(outcome) = current_outcome {
        require(
            outcome == "ok",
            "physical test record lacks a passing outcome",
        )?;
    }
    selected.sort();
    let mut expected = group.tests.clone();
    expected.sort();
    require(
        selected == expected,
        "physical qualification requires exactly the named passing tests",
    )?;
    let positive: Vec<_> = counts.iter().copied().filter(|count| *count > 0).collect();
    // A `lib` or `integration` selection runs every workspace package's binary
    // of that name, and the others report zero matches; the one positive
    // summary, of the expected count, is the named tests' own binary.
    let every_package = group.target_kind == "lib" || group.target == "integration";
    require(
        positive == [group.expected_tests] && (every_package || counts.len() == 1),
        "physical qualification requires complete libtest summaries",
    )
}
