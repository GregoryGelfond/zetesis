//! Pinned target and read-only source-integrity validation.

mod examples;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::corpus_comparison::Request;

const REVISION: &str = "38f0660ded448ed268c5a68759ceb0e2840dd497";
const MANIFEST_SHA256: &str = "372f44c59f3b6c530d50e6683e087dbceda028d54195b9f1de1ef610803c71fb";
const MAX_MANIFEST: usize = 4_194_304;
const MAX_SOURCE: usize = 1_048_576;

/// The bytes the corpus's reads bound.
#[derive(Clone, Copy, Debug)]
enum ReadResource {
    /// The serialized manifest.
    ManifestBytes,
    /// One target source.
    SourceBytes,
}

/// A refusal of a read of the corpus's files, as the shared confined reads
/// report it.
#[derive(Debug)]
enum ReadFailure {
    /// A filesystem operation failed at `path`.
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    /// The name is not a regular file confined to the corpus.
    Unconfined(String),
    /// A read exceeded its inclusive ceiling.
    Limit {
        resource: ReadResource,
        observed: u128,
        limit: usize,
    },
    /// The named source differs from its seal.
    Digest(String),
}

/// This corpus's confined, bounded file reads.
type Files = crate::corpus_files::Files<ReadFailure>;

impl crate::corpus_files::Failure for ReadFailure {
    type Resource = ReadResource;
    fn io(path: &Path, source: std::io::Error) -> Self {
        Self::Io {
            path: path.to_owned(),
            source,
        }
    }
    fn unconfined(text: String) -> Self {
        Self::Unconfined(text)
    }
    fn limit(resource: ReadResource, observed: u128, limit: usize) -> Self {
        Self::Limit {
            resource,
            observed,
            limit,
        }
    }
    fn digest(path: String, _expected: String, _actual: String) -> Self {
        Self::Digest(path)
    }
}

impl fmt::Display for ReadFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => write!(formatter, "{}: {source}", path.display()),
            Self::Unconfined(text) => write!(
                formatter,
                "target path must be a confined relative path: {text}"
            ),
            Self::Limit {
                resource,
                observed,
                limit,
            } => write!(
                formatter,
                "{resource:?} {observed} exceeds the validation read ceiling {limit}"
            ),
            Self::Digest(path) => write!(formatter, "source hash mismatch: {path}"),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct Manifest {
    pub(crate) revision: String,
    pub(crate) reference_toolchain: serde_json::Value,
    pub(crate) open_encodings: Vec<FileEntry>,
    pub(crate) cases: Vec<Case>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct FileEntry {
    pub(crate) path: String,
    pub(crate) sha256: String,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct Case {
    pub(crate) path: String,
    pub(crate) sha256: String,
    pub(crate) includes: Vec<FileEntry>,
    pub(crate) contracts: Vec<Contract>,
    pub(crate) expected_satisfiability: String,
    pub(crate) original_sha256: Option<String>,
    #[serde(skip)]
    pub(crate) example_contract: Option<crate::examples::Contract>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct Contract {
    pub(crate) tag: String,
    pub(crate) arguments: String,
}

#[derive(Clone, Debug)]
pub(crate) struct Loaded {
    pub(crate) root: PathBuf,
    pub(crate) manifest: Manifest,
    pub(crate) manifest_sha256: String,
    pub(crate) view: SourceView,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SourceView {
    Original,
    AnnotationCleaned,
}

pub(crate) fn load(options: &Request) -> Result<Loaded, String> {
    if options.corpus.is_none() && options.manifest.is_none() {
        return examples::load(&options.repo);
    }
    let root = options
        .corpus
        .clone()
        .unwrap_or_else(|| options.repo.join("validation/corpus/kr-domains"));
    let root = root
        .canonicalize()
        .map_err(|error| format!("corpus {}: {error}", root.display()))?;
    let path = options
        .manifest
        .clone()
        .unwrap_or_else(|| options.repo.join("validation/corpus/manifest.json"));
    let bytes = Files::read(&path, MAX_MANIFEST, ReadResource::ManifestBytes)
        .map_err(|error| error.to_string())?;
    if crate::digest::sha256(&bytes) != MANIFEST_SHA256 {
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
        manifest_sha256: crate::digest::sha256(&bytes),
        view: SourceView::Original,
    })
}

/// Whether the regular file `relative` names, confined to `root`, holds at
/// most the source ceiling of bytes with the SHA-256 `expected`.
fn verify(root: &Path, relative: &str, expected: &str) -> Result<(), String> {
    let sealed = || {
        let path = Files::confined(root, relative)?;
        let bytes = Files::read(&path, MAX_SOURCE, ReadResource::SourceBytes)?;
        Files::digest(relative, &bytes, expected)
    };
    sealed().map_err(|error: ReadFailure| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::{load, verify};
    use crate::corpus_comparison::Request as Options;
    use zetesis_test_support::repository;

    #[test]
    fn complete_vendored_target_has_94_cases_and_matching_dependencies() {
        let root = repository::root();
        let options = Options {
            repo: root.clone(),
            ..Options::default()
        };
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
        let expected = crate::digest::sha256(b"original.");
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
    fn target_paths_follow_the_shared_confinement_rule() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("dir")).unwrap();
        std::fs::write(root.path().join("dir/source.lp"), "original.").unwrap();
        let canonical = root.path().canonicalize().unwrap();
        let expected = crate::digest::sha256(b"original.");
        verify(&canonical, "dir/source.lp", &expected).unwrap();
        // Separators that a platform could read differently, and a name that
        // is not a regular file, are refused before any digest is compared.
        for unconfined in ["dir//source.lp", "dir\\source.lp", "C:source.lp", "dir"] {
            let refusal = verify(&canonical, unconfined, &expected).unwrap_err();
            assert!(
                refusal.contains("confined relative path"),
                "{unconfined}: {refusal}"
            );
        }
    }

    #[test]
    fn edited_manifest_cannot_redefine_the_pinned_target() {
        let root = repository::root();
        let mut options = Options {
            repo: root.clone(),
            ..Options::default()
        };
        let original = std::fs::read(root.join("validation/corpus/manifest.json")).unwrap();
        let mut document: serde_json::Value = serde_json::from_slice(&original).unwrap();
        // Keep the revision and population counts, but remove a required edge.
        document["cases"][0]["includes"] = serde_json::json!([]);
        let edited = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(edited.path(), serde_json::to_vec(&document).unwrap()).unwrap();
        options.manifest = Some(edited.path().to_owned());
        let result = load(&options);
        assert!(matches!(result, Err(error) if error.contains("manifest identity")));
    }

    fn repository_options() -> Options {
        Options {
            repo: repository::root(),
            ..Options::default()
        }
    }

    #[test]
    fn default_loading_preserves_clean_case_metadata() {
        let options = repository_options();
        let loaded = load(&options).unwrap();
        let expected = crate::examples::load(
            &options.repo.join("examples/correctness"),
            crate::examples::Limits::default(),
        )
        .unwrap();
        assert!(matches!(loaded.view, super::SourceView::AnnotationCleaned));
        assert_eq!(loaded.root, expected.root());
        assert_eq!(loaded.manifest_sha256, expected.manifest_sha256());
        assert_eq!(
            loaded.manifest.reference_toolchain,
            serde_json::to_value(expected.reference_toolchain()).unwrap()
        );
        for (actual, source) in loaded.manifest.cases.iter().zip(expected.cases()) {
            assert_eq!(actual.path, source.path());
            assert_eq!(actual.sha256, source.source_sha256());
            assert_eq!(
                actual.original_sha256.as_deref(),
                Some(source.original_sha256())
            );
            assert_eq!(actual.example_contract.as_ref(), Some(source.contract()));
            assert!(actual.contracts.is_empty());
        }
    }

    #[test]
    fn clean_dependency_records_keep_the_complete_closure() {
        let options = repository_options();
        let loaded = load(&options).unwrap();
        let expected = crate::examples::load(
            &options.repo.join("examples/correctness"),
            crate::examples::Limits::default(),
        )
        .unwrap();
        for (actual, source) in loaded.manifest.cases.iter().zip(expected.cases()) {
            let expected_paths: Vec<_> = source
                .transitive_source_paths()
                .iter()
                .filter(|path| path.as_str() != source.path())
                .collect();
            assert_eq!(actual.includes.len(), expected_paths.len());
            for (include, path) in actual.includes.iter().zip(expected_paths) {
                assert_eq!(&include.path, path);
                let file = expected
                    .files()
                    .iter()
                    .find(|file| file.path() == path)
                    .unwrap();
                assert_eq!(include.sha256, file.source_sha256());
            }
        }
    }

    fn assert_original(loaded: &super::Loaded) {
        assert!(matches!(loaded.view, super::SourceView::Original));
        assert_eq!(loaded.manifest_sha256, super::MANIFEST_SHA256);
        for case in &loaded.manifest.cases {
            assert!(case.original_sha256.is_none());
            assert!(case.example_contract.is_none());
            assert!(!case.contracts.is_empty());
        }
    }

    #[test]
    fn explicit_corpus_override_preserves_original_mode() {
        let mut options = repository_options();
        options.corpus = Some(options.repo.join("validation/corpus/kr-domains"));
        assert_original(&load(&options).unwrap());
    }

    #[test]
    fn explicit_manifest_override_preserves_original_mode() {
        let mut options = repository_options();
        options.manifest = Some(options.repo.join("validation/corpus/manifest.json"));
        assert_original(&load(&options).unwrap());
    }

    #[test]
    fn clean_manifest_is_not_a_historical_manifest() {
        let mut options = repository_options();
        options.manifest = Some(options.repo.join("examples/correctness/manifest.json"));
        assert!(matches!(load(&options), Err(error) if error.contains("manifest identity")));
    }

    #[test]
    fn missing_clean_examples_do_not_fall_back_to_originals() {
        let mut source_options = repository_options();
        source_options.corpus = Some(source_options.repo.join("validation/corpus/kr-domains"));
        let historical = load(&source_options).unwrap();
        let directory = tempfile::tempdir().unwrap();
        let original_root = directory.path().join("validation/corpus/kr-domains");
        for path in historical
            .manifest
            .open_encodings
            .iter()
            .map(|source| &source.path)
            .chain(historical.manifest.cases.iter().map(|case| &case.path))
        {
            let destination = original_root.join(path);
            std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
            std::fs::copy(historical.root.join(path), destination).unwrap();
        }
        let manifest = "validation/corpus/manifest.json";
        let destination = directory.path().join(manifest);
        std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
        std::fs::copy(source_options.repo.join(manifest), destination).unwrap();
        let mut options = repository_options();
        options.repo = directory.path().to_owned();
        options.corpus = Some(original_root);
        assert_original(&load(&options).unwrap());
        options.corpus = None;
        assert!(matches!(load(&options), Err(error) if error.contains("examples/correctness")));
    }
}
