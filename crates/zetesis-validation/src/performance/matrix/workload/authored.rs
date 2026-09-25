//! Authored singleton source graphs reuse bounded reads and checked constant edits.

use std::path::Path;

use serde::Serialize;
use sha2::{Digest, Sha256};

use super::{
    Budget, ConstantAmendment, Error, Source, Workload, WorkloadLimits, constants, examples, field,
};
use crate::selected::FileSeal;

/// A reviewed source and display contract, independent of the correctness corpus.
///
/// The first authored boundary accepts a singleton source graph: parsed includes
/// are refused explicitly. The file is read below `root`, checked against the
/// expected digest, and sealed again before and after the campaign.
#[derive(Clone, Copy, Debug)]
pub struct AuthoredProgram<'a> {
    /// Source root, canonicalized before confined file access.
    pub root: &'a Path,
    /// Portable relative entry path beneath the source root.
    pub entry: &'a str,
    /// Reviewed SHA-256 of the unchanged source bytes.
    pub sha256: &'a str,
    /// Expected complete displays at the source's original parameter values.
    pub contract: &'a examples::Contract,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct Authored {
    /// The complete original source graph has this one file.
    source: FileSeal,
}

impl Workload {
    /// Admit a sealed authored program, optionally changing literal constants.
    ///
    /// Uses the same parser, byte edits, bounds and materialization as corpus
    /// workloads. Amended inputs retain their original contract only as
    /// provenance; reference qualification establishes their new answer family.
    /// The workload identity includes its own source and contract, never the
    /// unrelated correctness-catalog manifest.
    /// No-op edits retain their provenance but keep the unchanged contract.
    ///
    /// # Errors
    /// Refuses unconfined paths, changed sources, includes, invalid contracts,
    /// unsupported amendments, and source or metadata limits.
    pub fn authored(
        program: AuthoredProgram<'_>,
        amendments: &[ConstantAmendment<'_>],
        limits: WorkloadLimits,
    ) -> Result<Self, Error> {
        program.contract.validate().map_err(Error::Corpus)?;
        if amendments.len() > limits.amendments.min(super::MAX_AMENDMENTS)
            || amendments.iter().enumerate().any(|(position, amendment)| {
                amendment.source_path != program.entry
                    || amendments[..position]
                        .iter()
                        .any(|prior| prior.name == amendment.name)
            })
        {
            return Err(Error::Configuration(
                "authored amendments are outside the source or repeated",
            ));
        }
        let root = examples::files::canonical(program.root).map_err(Error::Corpus)?;
        let path = examples::files::confined(&root, program.entry).map_err(Error::Corpus)?;
        let seal =
            super::super::super::run::checked_seal(&path, limits.source_bytes, program.sha256)?;
        let text = read(&seal, limits.source_bytes)?;
        let mut budget = Budget {
            limits,
            metadata: 0,
            sources: 0,
        };
        budget.source(text.len())?;
        budget.metadata(seal.requested().as_os_str().len())?;
        budget.metadata(seal.canonical().as_os_str().len())?;
        budget.metadata(seal.sha256().len())?;
        let contract = contract_bytes(program.contract, limits.metadata_bytes)?;
        budget.metadata(contract.len())?;
        for amendment in amendments {
            budget.metadata(amendment.source_path.len())?;
            budget.metadata(amendment.name.len())?;
        }
        let edits = constants::source_edits(
            &text,
            program.entry,
            std::iter::empty(),
            amendments,
            &mut budget,
        )?;
        let remaining = limits.total_source_bytes - budget.sources;
        let derived = examples::derive_source(&text, &edits, limits.source_bytes.min(remaining))
            .map_err(Error::Corpus)?;
        budget.source(derived.len())?;
        let derived_sha256 = format!("{:x}", Sha256::digest(derived.as_bytes()));
        let mut hash = Sha256::new();
        hash.update(b"zetesis-authored-workload-v1\0");
        for part in [
            program.entry.as_bytes(),
            program.sha256.as_bytes(),
            derived_sha256.as_bytes(),
            &contract,
        ] {
            field(&mut hash, part);
        }
        let entry = budget.text(program.entry)?;
        let source = Source {
            path: budget.text(program.entry)?,
            base_sha256: budget.text(program.sha256)?,
            derived_sha256: budget.text(&derived_sha256)?,
            derived_bytes: derived.len(),
            edits,
        };
        let amended = source.base_sha256 != source.derived_sha256;
        Ok(Self {
            entry,
            identity: budget.text(&format!("{:x}", hash.finalize()))?,
            manifest_sha256: None,
            sources: vec![source],
            default_contract: program.contract.clone(),
            amended,
            generated: None,
            authored: Some(Authored { source: seal }),
            metadata_bytes: budget.metadata,
            source_bytes: budget.sources,
        })
    }

    pub(in crate::performance::matrix) fn authored_seal(&self) -> Option<&FileSeal> {
        self.authored.as_ref().map(|authored| &authored.source)
    }

    pub(super) fn validate_authored(
        &self,
        limits: crate::performance::Limits,
    ) -> Result<(), Error> {
        let seal = self
            .authored_seal()
            .ok_or(Error::Configuration("missing authored source"))?;
        let current = super::super::super::run::checked_seal(
            seal.requested(),
            limits.corpus.source_bytes,
            seal.sha256(),
        )?;
        if !current.same_identity(seal)
            || self.metadata_bytes > limits.corpus.manifest_bytes
            || self.sources[0].derived_bytes > limits.corpus.source_bytes
        {
            return Err(Error::Configuration(
                "authored source identity or bounds changed",
            ));
        }
        Ok(())
    }

    pub(super) fn materialize_authored(
        &self,
        directory: &Path,
        limit: usize,
    ) -> Result<Vec<FileSeal>, Error> {
        let seal = self
            .authored_seal()
            .ok_or(Error::Configuration("missing authored source"))?;
        let original = read(seal, limit)?;
        let source = &self.sources[0];
        let derived =
            examples::derive_source(&original, &source.edits, limit).map_err(Error::Corpus)?;
        examples::files::digest(&source.path, derived.as_bytes(), &source.derived_sha256)
            .map_err(Error::Corpus)?;
        let path = directory.join(&source.path);
        let parent = path
            .parent()
            .ok_or(Error::Configuration("authored source has no parent"))?;
        std::fs::create_dir_all(parent).map_err(|error| crate::performance::io(parent, error))?;
        std::fs::write(&path, derived).map_err(|error| crate::performance::io(&path, error))?;
        Ok(vec![super::super::super::run::checked_seal(
            &path,
            limit,
            &source.derived_sha256,
        )?])
    }
}

fn read(seal: &FileSeal, limit: usize) -> Result<String, Error> {
    let bytes = examples::files::read(seal.requested(), limit, examples::Resource::SourceBytes)
        .map_err(Error::Corpus)?;
    examples::files::digest("authored source", &bytes, seal.sha256()).map_err(Error::Corpus)?;
    String::from_utf8(bytes).map_err(|error| Error::Corpus(examples::Error::Utf8(error)))
}

// A fixed-capacity slice writer refuses excess bytes before extending storage.
// The same shared Contract serialization supplies both identity and retention bounds.
fn contract_bytes(contract: &examples::Contract, limit: usize) -> Result<Vec<u8>, Error> {
    let mut bytes = super::reserve(limit)?;
    bytes.resize(limit, 0);
    let mut remaining = bytes.as_mut_slice();
    serde_json::to_writer(&mut remaining, contract)
        .map_err(|_| Error::Configuration("authored contract exceeds its metadata ceiling"))?;
    let used = limit - remaining.len();
    bytes.truncate(used);
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn campaign_read_ceiling_is_independent_of_admission() {
        let root = tempfile::tempdir().unwrap();
        let source = "p.";
        std::fs::write(root.path().join("case.lp"), source).unwrap();
        let hash = format!("{:x}", Sha256::digest(source));
        let contract = examples::Contract::complete_family(std::num::NonZeroU64::MIN);
        let workload = Workload::authored(
            AuthoredProgram {
                root: root.path(),
                entry: "case.lp",
                sha256: &hash,
                contract: &contract,
            },
            &[],
            WorkloadLimits {
                source_bytes: 1024,
                ..WorkloadLimits::default()
            },
        )
        .unwrap();
        let mut limits = crate::performance::Limits::default();
        assert!(workload.validate_authored(limits).is_ok());
        limits.corpus.source_bytes = source.len();
        assert!(workload.validate_authored(limits).is_ok());
        limits.corpus.source_bytes = source.len() - 1;
        assert!(workload.validate_authored(limits).is_err());
    }

    #[test]
    fn materialization_rechecks_the_original_and_seals_only_the_derived_bytes() {
        let root = tempfile::tempdir().unwrap();
        let source = "#const n=8. p(n).";
        let path = root.path().join("case.lp");
        std::fs::write(&path, source).unwrap();
        let hash = format!("{:x}", Sha256::digest(source));
        let contract = examples::Contract::complete_family(std::num::NonZeroU64::MIN);
        let workload = Workload::authored(
            AuthoredProgram {
                root: root.path(),
                entry: "case.lp",
                sha256: &hash,
                contract: &contract,
            },
            &[ConstantAmendment {
                source_path: "case.lp",
                name: "n",
                expected: 8,
                replacement: 9,
            }],
            WorkloadLimits::default(),
        )
        .unwrap();
        let derived = tempfile::tempdir().unwrap();
        let seals = workload.materialize_authored(derived.path(), 1024).unwrap();
        let bytes = std::fs::read(derived.path().join("case.lp")).unwrap();
        assert_eq!(bytes, b"#const n=9. p(n).");
        assert_eq!(seals.len(), 1);
        assert_eq!(seals[0].sha256(), format!("{:x}", Sha256::digest(&bytes)));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
        std::fs::write(&path, "#const n=7. p(n).").unwrap();
        assert!(
            workload
                .validate_authored(crate::performance::Limits::default())
                .is_err()
        );
        assert!(workload.materialize_authored(derived.path(), 1024).is_err());
    }
}
