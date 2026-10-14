//! Unchanged external source closures, sealed independently of the corpus.

mod graph;

use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{Budget, Error, Source, Workload, WorkloadLimits, constants, examples, field, reserve};
use crate::{digest, selected::FileSeal};

/// One source file and its include occurrences in source order.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BundleFile {
    /// Portable root-relative member path, with normal components only.
    pub path: String,
    /// Lowercase SHA-256 of the unchanged UTF-8 source bytes.
    pub sha256: String,
    /// Every parsed include occurrence, including repeated includes.
    pub includes: Vec<BundleInclude>,
}

/// A parsed include spelling and the declared member it resolves to.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BundleInclude {
    /// Original include string. Parent components may stay within the root.
    pub spelling: String,
    /// Portable root-relative target path, with normal components only.
    pub target: String,
}

/// Borrowed specification of an unchanged, explicitly sealed source closure.
#[derive(Clone, Copy, Debug)]
pub struct BundleProgram<'a> {
    /// Source root, canonicalized once before confined reads.
    pub root: &'a Path,
    /// Original entry member; it is never flattened or rewritten.
    pub entry: &'a str,
    /// Exact reachable graph, at most 256 files and 1024 include occurrences.
    pub files: &'a [BundleFile],
    /// Reviewed complete ordinary or optimal-family display contract.
    pub contract: &'a examples::Contract,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct Bundle {
    root: PathBuf,
    files: Vec<BundleFile>,
    seals: Vec<FileSeal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    manifest: Option<FileSeal>,
}

impl Workload {
    /// Admit an unchanged external entry and its complete declared include graph.
    ///
    /// Reuses the corpus workload's syntax/include validation, without solver
    /// admission or a new source parser. Reads and hashes bounded UTF-8 files;
    /// retains paths, graph, contract and seals, never source payload. Identity
    /// includes entry, sorted member paths/hashes, ordered edges and the contract,
    /// independently of host paths and the correctness-corpus identity.
    ///
    /// Work is O(source bytes + files² + edges × files); retained metadata is
    /// bounded by `metadata_bytes`, with at most 256 files, 1024 edges and include
    /// depth 32. Input plus identical materialized bytes count toward
    /// `total_source_bytes`, as for other workloads. Parser storage is bounded by
    /// source input, not by this metadata ceiling. No immutable filesystem lease
    /// is claimed: source seals are checked before copying and after the campaign.
    ///
    /// # Errors
    /// Refuses invalid contracts, syntax/include discrepancies, missing or extra
    /// members, cycles, path escape/aliasing, cwd include shadows, changed files
    /// and exceeded bounds. Relative parent includes must resolve within the
    /// declared root to the named member.
    pub fn bundle(program: BundleProgram<'_>, limits: WorkloadLimits) -> Result<Self, Error> {
        program.contract.validate().map_err(Error::Corpus)?;
        check_fields(program, limits)?;
        graph::validate(program.entry, program.files)?;
        let root = examples::Files::canonical(program.root).map_err(Error::Corpus)?;
        let mut budget = Budget {
            limits,
            metadata: 0,
            sources: 0,
        };
        budget.metadata(root.as_os_str().len())?;
        let contract = super::authored::contract_bytes(program.contract, limits.metadata_bytes)?;
        budget.metadata(contract.len())?;
        let mut files = reserve(program.files.len())?;
        let mut seals = reserve(program.files.len())?;
        let mut sources = reserve(program.files.len())?;
        for file in program.files {
            let (copy, seal, source) = admit_file(&root, file, &mut budget)?;
            if seals
                .iter()
                .any(|prior: &FileSeal| prior.canonical() == seal.canonical())
            {
                return Err(Error::Configuration(
                    "bundle members alias the same canonical file",
                ));
            }
            files.push(copy);
            seals.push(seal);
            sources.push(source);
        }
        graph::resolve(&root, &files)?;
        let identity = identity(program.entry, &files, &contract);
        Ok(Self {
            entry: budget.text(program.entry)?,
            identity: budget.text(&identity)?,
            manifest_sha256: None,
            sources,
            default_contract: program.contract.clone(),
            amended: false,
            generated: None,
            authored: None,
            bundle: Some(Bundle {
                root,
                files,
                seals,
                manifest: None,
            }),
            metadata_bytes: budget.metadata,
            source_bytes: budget.sources,
        })
    }

    /// Whether this workload owns an independent, unchanged external source graph.
    #[must_use]
    pub const fn is_bundle(&self) -> bool {
        self.bundle.is_some()
    }

    pub(in crate::performance) fn attach_bundle_manifest(
        &mut self,
        seal: &FileSeal,
        limit: usize,
    ) -> Result<(), Error> {
        let mut metadata = self.metadata_bytes;
        for count in [
            seal.requested().as_os_str().len(),
            seal.canonical().as_os_str().len(),
            seal.sha256().len(),
        ] {
            metadata = super::add(metadata, count)?;
        }
        if metadata > limit {
            return Err(Error::Configuration(
                "bundle manifest seal exceeds metadata ceiling",
            ));
        }
        self.bundle
            .as_mut()
            .ok_or(Error::Configuration("missing external bundle"))?
            .manifest = Some(seal.clone());
        self.metadata_bytes = metadata;
        Ok(())
    }

    pub(in crate::performance::matrix) fn independent_seals(
        &self,
    ) -> impl Iterator<Item = &FileSeal> {
        self.authored_seal().into_iter().chain(
            self.bundle
                .iter()
                .flat_map(|bundle| bundle.seals.iter().chain(bundle.manifest.iter())),
        )
    }

    pub(in crate::performance::matrix) fn validate_bundle_campaign(
        &self,
        reference: bool,
        memory: bool,
    ) -> Result<(), Error> {
        let Some(bundle) = &self.bundle else {
            return Ok(());
        };
        if !reference {
            return Err(Error::Configuration(
                "external bundles require fresh reference qualification",
            ));
        }
        if memory
            && bundle
                .files
                .iter()
                .any(|file| file.path.split('/').next() == Some("child-rss.json"))
        {
            return Err(Error::Configuration(
                "bundle member conflicts with the memory helper record",
            ));
        }
        Ok(())
    }

    pub(super) fn validate_bundle(&self, limits: crate::performance::Limits) -> Result<(), Error> {
        let bundle = self
            .bundle
            .as_ref()
            .ok_or(Error::Configuration("missing external bundle"))?;
        if self.metadata_bytes > limits.corpus.manifest_bytes {
            return Err(Error::Configuration(
                "bundle metadata exceeds campaign ceiling",
            ));
        }
        for seal in &bundle.seals {
            check_identity(seal, limits.corpus.source_bytes)?;
        }
        if let Some(seal) = &bundle.manifest {
            check_identity(seal, limits.corpus.manifest_bytes)?;
        }
        graph::resolve(&bundle.root, &bundle.files)
    }

    pub(super) fn materialize_bundle(
        &self,
        directory: &Path,
        limit: usize,
    ) -> Result<Vec<FileSeal>, Error> {
        let bundle = self
            .bundle
            .as_ref()
            .ok_or(Error::Configuration("missing external bundle"))?;
        let mut seals = reserve(bundle.files.len())?;
        for (file, seal) in bundle.files.iter().zip(&bundle.seals) {
            check_identity(seal, limit)?;
            let text = super::authored::read(seal, limit)?;
            let path = directory.join(&file.path);
            let parent = path
                .parent()
                .ok_or(Error::Configuration("bundle member has no parent"))?;
            std::fs::create_dir_all(parent)
                .map_err(|error| crate::performance::io(parent, error))?;
            // Exclusive creation also refuses case/normalization aliases on the
            // actual destination filesystem rather than overwriting a member.
            let mut output = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .map_err(|error| crate::performance::io(&path, error))?;
            output
                .write_all(text.as_bytes())
                .map_err(|error| crate::performance::io(&path, error))?;
            seals.push(super::super::super::run::checked_seal(
                &path,
                limit,
                &file.sha256,
            )?);
        }
        Ok(seals)
    }

    // Matrix calls this after all workloads are materialized, before any child.
    pub(in crate::performance::matrix) fn validate_bundle_directory(
        &self,
        directory: &Path,
        memory: bool,
    ) -> Result<(), Error> {
        if let Some(bundle) = &self.bundle {
            graph::resolve(directory, &bundle.files)?;
            if memory {
                let record = directory.join("child-rss.json");
                match record.symlink_metadata() {
                    Ok(_) => {
                        return Err(Error::Configuration(
                            "bundle member conflicts with the memory helper record",
                        ));
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => return Err(crate::performance::io(&record, error)),
                }
            }
        }
        Ok(())
    }
}

// Caller-owned fields must fit before path normalization or graph traversal can
// allocate from them. The retained Budget below charges each actual copy.
fn check_fields(program: BundleProgram<'_>, limits: WorkloadLimits) -> Result<(), Error> {
    let mut budget = Budget {
        limits,
        metadata: 0,
        sources: 0,
    };
    budget.metadata(program.root.as_os_str().len())?;
    budget.metadata(program.entry.len())?;
    for file in program.files {
        budget.metadata(file.path.len())?;
        budget.metadata(file.sha256.len())?;
        for include in &file.includes {
            budget.metadata(include.spelling.len())?;
            budget.metadata(include.target.len())?;
        }
    }
    Ok(())
}

fn check_identity(seal: &FileSeal, limit: usize) -> Result<(), Error> {
    let current = super::super::super::run::checked_seal(seal.requested(), limit, seal.sha256())?;
    if !current.same_identity(seal) {
        return Err(Error::Configuration("bundle file identity changed"));
    }
    Ok(())
}

fn admit_file(
    root: &Path,
    file: &BundleFile,
    budget: &mut Budget,
) -> Result<(BundleFile, FileSeal, Source), Error> {
    examples::Files::confined(root, &file.path).map_err(Error::Corpus)?;
    let seal = super::super::super::run::checked_seal(
        &root.join(&file.path),
        budget.limits.source_bytes,
        &file.sha256,
    )?;
    if !seal.canonical().starts_with(root) {
        return Err(Error::Configuration(
            "sealed bundle member escapes its root",
        ));
    }
    let text = super::authored::read(&seal, budget.limits.source_bytes)?;
    budget.source(text.len())?;
    budget.source(text.len())?;
    for count in [
        seal.requested().as_os_str().len(),
        seal.canonical().as_os_str().len(),
        seal.sha256().len(),
    ] {
        budget.metadata(count)?;
    }
    constants::source_edits(
        &text,
        &file.path,
        file.includes
            .iter()
            .map(|include| include.spelling.as_str()),
        &[],
        budget,
    )?;
    let mut includes = reserve(file.includes.len())?;
    for include in &file.includes {
        includes.push(BundleInclude {
            spelling: budget.text(&include.spelling)?,
            target: budget.text(&include.target)?,
        });
    }
    let copy = BundleFile {
        path: budget.text(&file.path)?,
        sha256: budget.text(&file.sha256)?,
        includes,
    };
    let source = Source {
        path: budget.text(&file.path)?,
        base_sha256: budget.text(&file.sha256)?,
        derived_sha256: budget.text(&file.sha256)?,
        derived_bytes: text.len(),
        edits: Vec::new(),
    };
    Ok((copy, seal, source))
}

fn identity(entry: &str, files: &[BundleFile], contract: &[u8]) -> String {
    let mut ordered: Vec<_> = files.iter().collect();
    ordered.sort_unstable_by(|left, right| left.path.cmp(&right.path));
    let mut hash = Sha256::new();
    hash.update(b"zetesis-external-bundle-workload-v1\0");
    field(&mut hash, entry.as_bytes());
    field(&mut hash, contract);
    for file in ordered {
        field(&mut hash, file.path.as_bytes());
        field(&mut hash, file.sha256.as_bytes());
        field(&mut hash, &(file.includes.len() as u128).to_le_bytes());
        for include in &file.includes {
            field(&mut hash, include.spelling.as_bytes());
            field(&mut hash, include.target.as_bytes());
        }
    }
    digest::spelled(&hash.finalize())
}

#[cfg(test)]
mod tests;
