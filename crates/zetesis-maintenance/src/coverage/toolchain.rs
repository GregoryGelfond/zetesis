//! Coverage metadata retains observed tool identities and disjoint populations.
use super::{Floor, Group, Mode, selection};
use crate::{Error, files, require};
use regex::Regex;
use serde_json::{Value, json};
use std::{path::Path, sync::LazyLock};

static RUST_LLVM: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^LLVM version: ([0-9]+\.[0-9]+\.[0-9]+)$").unwrap());
static TOOL_LLVM: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^[ \t]*LLVM version ([^\r\n]+)$").unwrap());
/// Observed identity of one named LLVM executable.
#[derive(Clone, Copy, Debug)]
pub struct Tool<'a> {
    /// Canonical executable path, resolved by the application.
    pub path: &'a Path,
    /// SHA-256 of the executable bytes actually read by the application.
    pub sha256: &'a str,
    /// Complete captured `--version` output.
    pub version: &'a str,
}
/// Version observations; no tool is executed by metadata construction.
#[derive(Clone, Copy, Debug)]
pub struct Observation<'a> {
    /// Pinned `rustc +1.97.1 -vV` output.
    pub rustc: &'a str,
    /// Complete observed `cargo +1.97.1 llvm-cov --version` output.
    pub cargo_llvm_cov: &'a str,
    /// Observed LLVM coverage executable.
    pub llvm_cov: Tool<'a>,
    /// Observed LLVM profile executable.
    pub llvm_profdata: Tool<'a>,
}
/// Requested coverage policy and optional reviewed physical selection.
#[derive(Clone, Copy, Debug)]
pub struct Metadata<'a> {
    /// Whether this run will gate or establish a baseline.
    pub mode: Mode,
    /// The exact committed file spelling after whitespace removal.
    pub floor: &'a str,
    /// Shell table when physical tests are selected; absence means portable only.
    pub physical_table: Option<&'a str>,
    /// Versions and hashes observed at the command boundary.
    pub observation: Observation<'a>,
}
fn tools(observation: Observation<'_>) -> Result<Value, Error> {
    let captures = RUST_LLVM
        .captures(observation.rustc)
        .ok_or_else(|| Error::Invalid("pinned rustc did not report its LLVM version".into()))?;
    let llvm = &captures[1];
    let rust_build = format!("{llvm}-rust-1.97.1-stable");
    let mut result = serde_json::Map::new();
    for (name, tool) in [
        ("LLVM_COV", observation.llvm_cov),
        ("LLVM_PROFDATA", observation.llvm_profdata),
    ] {
        let matches: Vec<_> = TOOL_LLVM.captures_iter(tool.version).collect();
        require(
            matches.len() == 1 && matches[0][1].trim() == llvm
                || matches.len() == 1 && matches[0][1].trim() == rust_build,
            format!("{name} does not match rustc LLVM {llvm}"),
        )?;
        require(
            tool.path.is_absolute(),
            "LLVM tool identity requires an absolute path",
        )?;
        require(
            tool.sha256.len() == 64
                && tool
                    .sha256
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
            "invalid LLVM tool digest",
        )?;
        result.insert(
            name.into(),
            json!({"path":tool.path,"sha256":tool.sha256,"version":tool.version.trim()}),
        );
    }
    Ok(Value::Object(result))
}
/// Compose the established metadata schema from explicit validated observations.
/// # Errors
/// Refuses invalid policy, altered physical selection, incompatible LLVM version
/// strings or malformed executable identities. This does not attest tool execution.
pub fn metadata(request: Metadata<'_>) -> Result<Value, Error> {
    for value in [
        request.floor,
        request.observation.rustc,
        request.observation.cargo_llvm_cov,
        request.observation.llvm_cov.version,
        request.observation.llvm_profdata.version,
    ] {
        super::input_bytes(value.len())?;
    }
    let cargo_version = request.observation.cargo_llvm_cov.trim();
    require(
        cargo_version == "cargo-llvm-cov 0.8.7",
        "observed cargo-llvm-cov version does not match 0.8.7",
    )?;
    let version = cargo_version
        .strip_prefix("cargo-llvm-cov ")
        .ok_or_else(|| Error::Invalid("invalid cargo-llvm-cov banner".into()))?;
    Floor::parse(request.floor)?.admit(request.mode)?;
    // The recorded coverage scope is the Metal qualification.
    let groups: Vec<Group> = match request.physical_table {
        None => Vec::new(),
        Some(table) => {
            let selection = selection(table)?;
            require(
                selection.backend == super::PhysicalBackend::Metal,
                "coverage metadata records the Metal qualification only",
            )?;
            selection.groups
        }
    };
    let tests: Vec<_> = groups.iter().flat_map(|group| group.tests.iter()).collect();
    let physical = !tests.is_empty();
    Ok(json!({
        "mode":request.mode.label(),"committed_floor":request.floor,
        "rustc":request.observation.rustc.trim(),"cargo_llvm_cov":version,"cargo_llvm_cov_observation":cargo_version,"llvm_tools":tools(request.observation)?,
        "primary":"workspace --all-features","supplemental":"--package zetesis-cli --package zetesis-solve --no-default-features",
        "floor_profiles":["workspace","cli-cpu"],
        "default_filename_filters":"cargo-llvm-cov 0.8.7 src/report.rs::ignore_filename_regex",
        "project_added_filename_filters":[],"profiles_merged":false,
        "profile_merge_scope":"profiles_merged describes floor profiles; raw execution profiles combine only within their own floor profile",
        "workspace_execution":if physical {"portable+metal"} else {"portable"},
        "workspace_stages":if physical {vec!["portable","metal"]} else {vec!["portable"]},
        "physical_test_groups":groups,"physical_tests":tests,"expected_physical_tests":tests.len(),
        "physical_scope":if physical {Some("59 exact Metal tests: native aggregate reduction and measurement, static constructor and complete closure/reference checks, lazy transport and source closure, tight and formula oracles, ordinary lazy/formula CLI paths including automatic materialization and automatic CPU policy, complete-world-view collection, bounded relation equality filtering and measurement, shared-context composition, contention and shared failure handling, caller-supplied session resources with policy and observer failures, observed complete collection on supplied contexts, explicit compiled formula profiles with independent oracle state and compilation identity, and combined language-consumer families with scored observations and optimum ties. Cooperative device control, interrupted preparation reuse and actual submission receipts are included, alongside completed-support table joins with actual GPU candidates and complete CPU/Metal answer families. Ordinary automatic membership sessions check complete tight families, the general device route for non-tight theories, and tight work refusal before dispatch. Unlisted tests and Vulkan are not selected.")} else {None}
    }))
}
/// Read one executable identity with the maintenance file ceiling.
/// # Errors
/// Refuses unavailable, nonregular or oversized executable files.
pub fn executable_identity(path: &Path) -> Result<(std::path::PathBuf, String), Error> {
    let canonical = path
        .canonicalize()
        .map_err(|error| files::io(path, error))?;
    let parent = canonical
        .parent()
        .ok_or_else(|| Error::Invalid("executable has no parent".into()))?;
    let name = canonical
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| Error::Invalid("non-UTF-8 executable name".into()))?;
    let mut tree = files::Tree::new(
        parent,
        files::Limits {
            file_bytes: 268_435_456,
            ..files::Limits::default()
        },
    )?;
    let digest = files::digest(&tree.read(name)?);
    Ok((canonical, digest))
}
