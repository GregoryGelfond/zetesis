//! Bounded JSON loading for unchanged external source workloads.
//!
//! This adapter owns specification I/O only. [`matrix::Workload::bundle`] owns
//! graph admission; the existing matrix owns qualification, captures and timing.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::{Error, matrix};
use crate::{examples, selected::identity};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema_version: u32,
    root: PathBuf,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    entry: String,
    contract: examples::Contract,
    files: Vec<matrix::BundleFile>,
}

/// Load 1..=95 explicitly sealed external workloads from a bounded JSON manifest.
///
/// Schema version 1 has `root` and `cases`; each case has `entry`, the existing
/// typed `contract`, and `files`. Each file has `path`, lowercase `sha256` and
/// ordered `includes`, each with `spelling` and root-relative `target`. Unknown
/// fields are refused. Relative `root` resolves against the manifest's canonical
/// parent; absolute roots are also admitted. Members never escape that root.
///
/// Reads at most `metadata_bytes` of manifest before deserialization. The same
/// inclusive metadata and combined source ceilings bound the aggregate admitted
/// workloads; individual source reads use `source_bytes`. Retains each original
/// member seal and the manifest seal for the matrix's before/after checks, with
/// no retained source payload. Work is the sum of bundle admission and bounded
/// manifest decoding. This function neither launches a solver nor grants rights
/// to redistribute caller-owned sources.
///
/// # Errors
/// Refuses invalid JSON/schema, empty or oversized selections, duplicate workload
/// identities, changed files, invalid graphs/contracts and exceeded byte bounds.
pub fn load(path: &Path, limits: matrix::WorkloadLimits) -> Result<Vec<matrix::Workload>, Error> {
    load_inner(path, limits, None)
}

/// Admit the same bytes that match a maintained manifest's reviewed digest.
pub(super) fn load_reviewed(
    path: &Path,
    limits: matrix::WorkloadLimits,
    expected: &str,
) -> Result<Vec<matrix::Workload>, Error> {
    load_inner(path, limits, Some(expected))
}

fn load_inner(
    path: &Path,
    limits: matrix::WorkloadLimits,
    expected: Option<&str>,
) -> Result<Vec<matrix::Workload>, Error> {
    let seal = identity::seal(path, limits.metadata_bytes)?;
    let bytes = examples::Files::read(
        seal.requested(),
        limits.metadata_bytes,
        examples::Resource::ManifestBytes,
    )
    .map_err(Error::Corpus)?;
    examples::Files::digest("external workload manifest", &bytes, seal.sha256())
        .map_err(Error::Corpus)?;
    if let Some(expected) = expected {
        examples::Files::digest("reviewed workload manifest", &bytes, expected)
            .map_err(Error::Corpus)?;
    }
    let manifest: Manifest = serde_json::from_slice(&bytes)
        .map_err(|error| Error::Corpus(examples::Error::Json(error)))?;
    if manifest.schema_version != 1
        || manifest.cases.is_empty()
        || manifest.cases.len() > super::MAX_CASES
    {
        return Err(Error::Configuration(
            "external workload manifest requires schema 1 and 1..=95 cases",
        ));
    }
    let parent = seal.canonical().parent().ok_or(Error::Configuration(
        "external workload manifest has no parent",
    ))?;
    let root = parent.join(&manifest.root);
    let mut workloads = Vec::<matrix::Workload>::new();
    let mut metadata_bytes = 0usize;
    let mut source_bytes = 0usize;
    for case in &manifest.cases {
        let mut workload = matrix::Workload::bundle(
            matrix::BundleProgram {
                root: &root,
                entry: &case.entry,
                files: &case.files,
                contract: &case.contract,
            },
            limits,
        )?;
        workload.attach_bundle_manifest(&seal, limits.metadata_bytes)?;
        if workloads
            .iter()
            .any(|prior| prior.identity() == workload.identity())
        {
            return Err(Error::Configuration(
                "external workload identities are repeated",
            ));
        }
        metadata_bytes = metadata_bytes
            .checked_add(workload.metadata_bytes)
            .ok_or(Error::Configuration("external workload metadata overflow"))?;
        source_bytes =
            source_bytes
                .checked_add(workload.source_bytes)
                .ok_or(Error::Configuration(
                    "external workload source bytes overflow",
                ))?;
        if metadata_bytes > limits.metadata_bytes || source_bytes > limits.total_source_bytes {
            return Err(Error::Configuration(
                "external workload selection exceeds aggregate ceilings",
            ));
        }
        workloads.push(workload);
    }
    Ok(workloads)
}

#[cfg(test)]
mod tests;
