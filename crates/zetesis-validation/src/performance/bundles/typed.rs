//! Typed selection admission only; construction and execution keep their owners.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::{Error, Selection, examples, matrix};
use crate::{performance::families::Family, selected::FileSeal};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema_version: u32,
    catalog: Option<PathBuf>,
    sources: Vec<Authored>,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Authored {
    id: String,
    root: PathBuf,
    entry: String,
    sha256: String,
    contract: examples::Contract,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Case {
    Authored {
        source: String,
        amendments: Vec<Amendment>,
    },
    Corpus {
        entry: String,
        amendments: Vec<Amendment>,
    },
    Generated {
        family: String,
        size: u32,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Amendment {
    source_path: String,
    name: String,
    expected: i32,
    replacement: i32,
}

fn amendments(
    values: &[Amendment],
    limit: usize,
) -> Result<Vec<matrix::ConstantAmendment<'_>>, Error> {
    if values.len() > limit {
        return Err(Error::Configuration(
            "workload amendment count exceeds its ceiling",
        ));
    }
    Ok(values
        .iter()
        .map(|value| matrix::ConstantAmendment {
            source_path: &value.source_path,
            name: &value.name,
            expected: value.expected,
            replacement: value.replacement,
        })
        .collect())
}

fn case_limits(limits: matrix::WorkloadLimits) -> matrix::WorkloadLimits {
    let defaults = matrix::WorkloadLimits::default();
    matrix::WorkloadLimits {
        source_bytes: limits.source_bytes.min(defaults.source_bytes),
        total_source_bytes: limits.total_source_bytes.min(defaults.total_source_bytes),
        metadata_bytes: limits.metadata_bytes.min(defaults.metadata_bytes),
        amendments: limits.amendments.min(defaults.amendments),
    }
}

fn check_manifest(manifest: &Manifest) -> Result<(), Error> {
    if manifest.schema_version != 2
        || manifest.cases.is_empty()
        || manifest.cases.len() > crate::performance::MAX_CASES
        || manifest.sources.len() > crate::performance::MAX_CASES
    {
        return Err(Error::Configuration(
            "typed workload manifest requires schema 2 and 1..=95 cases",
        ));
    }
    for (index, source) in manifest.sources.iter().enumerate() {
        if source.id.is_empty()
            || manifest.sources[..index]
                .iter()
                .any(|prior| prior.id == source.id)
        {
            return Err(Error::Configuration(
                "typed source names must be nonempty and unique",
            ));
        }
        if !manifest.cases.iter().any(|case| {
            matches!(case,
            Case::Authored { source: id, .. } if *id == source.id)
        }) {
            return Err(Error::Configuration(
                "typed source is not used by the selection",
            ));
        }
    }
    Ok(())
}

fn workload(
    case: &Case,
    sources: &[Authored],
    parent: &Path,
    corpus: Option<&examples::Corpus>,
    limits: matrix::WorkloadLimits,
) -> Result<matrix::Workload, Error> {
    match case {
        Case::Authored {
            source,
            amendments: edits,
        } => {
            let source = sources
                .iter()
                .find(|item| item.id == *source)
                .ok_or(Error::Configuration("typed authored source is unknown"))?;
            let root = parent.join(&source.root);
            matrix::Workload::authored(
                matrix::AuthoredProgram {
                    root: &root,
                    entry: &source.entry,
                    sha256: &source.sha256,
                    contract: &source.contract,
                },
                &amendments(edits, limits.amendments)?,
                limits,
            )
        }
        Case::Corpus {
            entry,
            amendments: edits,
        } => {
            let corpus =
                corpus.ok_or(Error::Configuration("typed corpus case requires a catalog"))?;
            matrix::Workload::amended(
                corpus,
                entry,
                &amendments(edits, limits.amendments)?,
                limits,
            )
        }
        Case::Generated { family, size } => {
            let family = Family::ALL
                .into_iter()
                .find(|item| item.label() == family.as_str())
                .ok_or(Error::Configuration("typed generated family is unknown"))?;
            matrix::Workload::generated(family, *size, limits)
        }
    }
}

pub(super) fn load(
    bytes: &[u8],
    seal: &FileSeal,
    limits: matrix::WorkloadLimits,
) -> Result<Vec<matrix::Workload>, Error> {
    let manifest: Manifest = serde_json::from_slice(bytes)
        .map_err(|error| Error::Corpus(examples::Error::Json(error)))?;
    check_manifest(&manifest)?;
    let parent = seal.canonical().parent().ok_or(Error::Configuration(
        "typed workload manifest has no parent",
    ))?;
    let corpus = manifest
        .catalog
        .as_ref()
        .map(|path| {
            examples::load(
                &parent.join(path),
                examples::Limits {
                    manifest_bytes: limits.metadata_bytes,
                    source_bytes: limits.source_bytes,
                    total_source_bytes: limits.total_source_bytes,
                    ..examples::Limits::default()
                },
            )
        })
        .transpose()
        .map_err(Error::Corpus)?;
    let individual = case_limits(limits);
    let mut selection = Selection::new(limits);
    for case in &manifest.cases {
        let mut workload = workload(case, &manifest.sources, parent, corpus.as_ref(), individual)?;
        workload.attach_selection_manifest(seal, individual.metadata_bytes)?;
        selection.push(workload)?;
    }
    Ok(selection.workloads)
}
