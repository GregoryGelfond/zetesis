//! Adapt the typed example corpus to the existing campaign/report boundary.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use zetesis_validation::examples::{self, Satisfiability};

use super::{Case, FileEntry, Loaded, Manifest, SourceView};

pub(super) fn load(repo: &Path) -> Result<Loaded, String> {
    let corpus = examples::load(
        &repo.join("examples/kr-domains"),
        examples::Limits::default(),
    )
    .map_err(|error| error.to_string())?;
    let sources: BTreeMap<_, _> = corpus
        .files()
        .iter()
        .map(|file| (file.path(), file))
        .collect();
    let cases = corpus
        .cases()
        .iter()
        .map(|case| case_record(case, &sources))
        .collect::<Result<_, _>>()?;
    Ok(Loaded {
        root: corpus.root().into(),
        manifest: Manifest {
            revision: corpus.revision().into(),
            reference_toolchain: serde_json::to_value(corpus.reference_toolchain())
                .map_err(|error| format!("reference toolchain view: {error}"))?,
            open_encodings: schema_records(&corpus),
            cases,
        },
        manifest_sha256: corpus.manifest_sha256().into(),
        view: SourceView::AnnotationCleaned,
    })
}

fn schema_records(corpus: &examples::Corpus) -> Vec<FileEntry> {
    let entries: BTreeSet<_> = corpus.cases().iter().map(examples::Case::path).collect();
    corpus
        .files()
        .iter()
        .filter(|source| !entries.contains(source.path()))
        .map(source_record)
        .collect()
}

fn source_record(source: &examples::Source) -> FileEntry {
    FileEntry {
        path: source.path().into(),
        sha256: source.source_sha256().into(),
    }
}

fn case_record(
    case: &examples::Case,
    sources: &BTreeMap<&str, &examples::Source>,
) -> Result<Case, String> {
    let includes = case
        .transitive_source_paths()
        .iter()
        .filter(|path| path.as_str() != case.path())
        .map(|path| {
            sources
                .get(path.as_str())
                .map(|source| source_record(source))
                .ok_or_else(|| format!("verified include has no source identity: {path}"))
        })
        .collect::<Result<_, _>>()?;
    Ok(Case {
        path: case.path().into(),
        sha256: case.source_sha256().into(),
        includes,
        contracts: Vec::new(),
        expected_satisfiability: match case.contract().satisfiability() {
            Satisfiability::Sat => "sat",
            Satisfiability::Unsat => "unsat",
        }
        .into(),
        original_sha256: Some(case.original_sha256().into()),
        example_contract: Some(case.contract().clone()),
    })
}
