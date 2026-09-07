//! Pinned target and read-only source-integrity validation.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::Options;

const REVISION: &str = "38f0660ded448ed268c5a68759ceb0e2840dd497";
const MANIFEST_SHA256: &str = "a99dafc272fb0047c01f984e27bf22943f2aa5f9c8acf04e4ed1de6ac1a3fe88";
const MAX_MANIFEST: u64 = 4_194_304;
const MAX_SOURCE: u64 = 1_048_576;

#[derive(Debug, Deserialize)]
pub(crate) struct Manifest {
    pub(crate) revision: String,
    pub(crate) reference_toolchain: serde_json::Value,
    pub(crate) open_encodings: Vec<FileEntry>,
    pub(crate) cases: Vec<Case>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct FileEntry {
    pub(crate) path: String,
    pub(crate) sha256: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Case {
    pub(crate) path: String,
    pub(crate) sha256: String,
    pub(crate) includes: Vec<FileEntry>,
    pub(crate) contracts: Vec<Contract>,
    pub(crate) expected_satisfiability: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Contract {
    pub(crate) tag: String,
    pub(crate) arguments: String,
}

pub(crate) struct Loaded {
    pub(crate) root: PathBuf,
    pub(crate) manifest: Manifest,
    pub(crate) manifest_sha256: String,
}

pub(crate) fn load(options: &Options) -> Result<Loaded, String> {
    let root = options
        .corpus
        .clone()
        .unwrap_or_else(|| options.repo.join("validation/corpus/kr-domains"));
    let root = root
        .canonicalize()
        .map_err(|error| format!("corpus {}: {error}", root.display()))?;
    let path = options.manifest.clone().unwrap_or_else(|| {
        options
            .repo
            .join("docs/verification/kr-domains-target-manifest.json")
    });
    let bytes = bounded_read(&path, MAX_MANIFEST)?;
    if hash(&bytes) != MANIFEST_SHA256 {
        return Err("manifest identity differs from the trusted pinned target".into());
    }
    let manifest: Manifest =
        serde_json::from_slice(&bytes).map_err(|error| format!("manifest: {error}"))?;
    if manifest.revision != REVISION
        || manifest.cases.len() != 94
        || manifest.open_encodings.len() != 14
    {
        return Err("manifest must describe the pinned 94-case, 14-schema target".into());
    }
    let schemas: BTreeMap<_, _> = manifest
        .open_encodings
        .iter()
        .map(|item| (item.path.as_str(), item.sha256.as_str()))
        .collect();
    let mut paths = BTreeSet::new();
    for (path, hash) in manifest
        .open_encodings
        .iter()
        .map(|item| (&item.path, &item.sha256))
        .chain(manifest.cases.iter().map(|item| (&item.path, &item.sha256)))
    {
        if !paths.insert(path) || path.ends_with("-clingcon.lp") {
            return Err(format!("duplicate or excluded target path: {path}"));
        }
        verify(&root, path, hash)?;
    }
    for case in &manifest.cases {
        if !matches!(case.expected_satisfiability.as_str(), "sat" | "unsat") {
            return Err(format!("invalid satisfiability contract: {}", case.path));
        }
        for dependency in &case.includes {
            if schemas.get(dependency.path.as_str()) != Some(&dependency.sha256.as_str()) {
                return Err(format!(
                    "include does not match a pinned schema: {}",
                    dependency.path
                ));
            }
        }
    }
    Ok(Loaded {
        root,
        manifest,
        manifest_sha256: hash(&bytes),
    })
}

fn verify(root: &Path, relative: &str, expected: &str) -> Result<(), String> {
    let path = Path::new(relative);
    if path
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(format!(
            "target path must be a confined relative path: {relative}"
        ));
    }
    let actual = root
        .join(path)
        .canonicalize()
        .map_err(|error| format!("{relative}: {error}"))?;
    if !actual.starts_with(root) {
        return Err(format!("target symlink leaves corpus: {relative}"));
    }
    let bytes = bounded_read(&actual, MAX_SOURCE)?;
    if hash(&bytes) != expected {
        return Err(format!("source hash mismatch: {relative}"));
    }
    Ok(())
}

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn bounded_read(path: &Path, maximum: u64) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|error| format!("{}: {error}", path.display()))?
        .take(maximum + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if u64::try_from(bytes.len()).map_err(|error| error.to_string())? > maximum {
        return Err(format!(
            "input exceeds validation read ceiling: {}",
            path.display()
        ));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::{hash, load, verify};
    use crate::Options;
    use clap::Parser;
    use std::path::PathBuf;

    #[test]
    fn complete_vendored_target_has_94_cases_and_matching_dependencies() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let options = Options::try_parse_from([
            "zetesis-validate".into(),
            "--repo".into(),
            root.into_os_string(),
        ])
        .unwrap();
        let loaded = load(&options).unwrap();
        assert_eq!(loaded.manifest.cases.len(), 94);
        assert_eq!(loaded.manifest.open_encodings.len(), 14);
        assert_eq!(
            loaded
                .manifest
                .cases
                .iter()
                .filter(|case| case.path.starts_with("scenarios/"))
                .count(),
            87
        );
    }

    #[test]
    fn source_mutation_and_parent_paths_fail_integrity_admission() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("source.lp"), "changed.").unwrap();
        let canonical = root.path().canonicalize().unwrap();
        let expected = hash(b"original.");
        assert!(
            verify(&canonical, "source.lp", &expected)
                .unwrap_err()
                .contains("hash mismatch")
        );
        assert!(
            verify(&canonical, "../source.lp", &expected)
                .unwrap_err()
                .contains("confined relative path")
        );
    }

    #[test]
    fn edited_manifest_cannot_redefine_the_pinned_target() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut options = Options::try_parse_from([
            "zetesis-validate".into(),
            "--repo".into(),
            root.clone().into_os_string(),
        ])
        .unwrap();
        let original =
            std::fs::read(root.join("docs/verification/kr-domains-target-manifest.json")).unwrap();
        let mut document: serde_json::Value = serde_json::from_slice(&original).unwrap();
        // Keep the revision and population counts, but remove a required edge.
        document["cases"][0]["includes"] = serde_json::json!([]);
        let edited = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(edited.path(), serde_json::to_vec(&document).unwrap()).unwrap();
        options.manifest = Some(edited.path().to_owned());
        let result = load(&options);
        assert!(matches!(result, Err(error) if error.contains("manifest identity")));
    }
}
