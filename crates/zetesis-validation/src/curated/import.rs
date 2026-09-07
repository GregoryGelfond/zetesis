//! Explicit legacy import with complete preflight before creating a destination.

use super::{
    Corpus, Error, LICENSE_SHA256, Limits, MANIFEST_SHA256, Resource, UPSTREAM_REVISION, ceiling,
    contracts, cpp, document, files,
};
use document::{CaseRecord, ContractRecord, Document, OriginRecord, ProvenanceRecord};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyCase {
    id: String,
    source_file: String,
    section: String,
    assertion_ordinal: usize,
    line_start: usize,
    line_end: usize,
    byte_start: usize,
    byte_end: usize,
    assertion: String,
    source: String,
    helper_arguments: Vec<String>,
    expected_helper_output: String,
    source_sha256: String,
    file_sha256: String,
    filters: Vec<String>,
    expected_helper_models: Vec<Vec<String>>,
    models: Vec<Vec<String>>,
    #[serde(rename = "native")]
    _native: String,
}
struct Imported {
    document: Document,
    sources: Vec<Vec<u8>>,
    license: Vec<u8>,
}

/// Import exactly the selected legacy assertions into a new curated directory.
/// Reads `cases.jsonl` and the pinned `originals/` under `legacy_root`. Every
/// original file, assertion span, decoded source and helper/full-model contract
/// is verified before filesystem output begins. Native admission tags do not
/// enter the curated target. Existing destinations are never replaced.
///
/// The caller must own the destination parent exclusively during publication;
/// concurrent adversarial directory replacement is outside this filesystem API.
/// A write or verification failure attempts to remove only the newly created
/// import directory. [`Error::ImportCleanup`] retains both failures if cleanup
/// also fails. Publication is not crash-atomic or a durability guarantee. This
/// is an integrity migration, not a fresh semantic solver qualification.
///
/// # Errors
/// Returns a typed integrity/resource/decoder refusal without output, or a
/// located I/O refusal. Failed imports never alter the legacy catalog/originals.
pub fn import_legacy(
    legacy_root: &Path,
    destination: &Path,
    limits: Limits,
) -> Result<Corpus, Error> {
    let root = files::canonical(legacy_root)?;
    let imported = prepare(&root, limits)?;
    let bytes = encode(&imported.document, limits)?;
    files::digest("curated manifest", &bytes, MANIFEST_SHA256)?;
    let destination = destination_path(&root, destination)?;
    publish(&destination, &imported, &bytes)?;
    document::load(destination.clone(), imported.document, limits)
        .map_err(|failure| cleanup(&destination, failure))
}
fn prepare(root: &Path, limits: Limits) -> Result<Imported, Error> {
    let catalog = files::read(
        &files::confined(root, "cases.jsonl")?,
        limits.manifest_bytes,
        Resource::ManifestBytes,
    )?;
    let catalog = std::str::from_utf8(&catalog)
        .map_err(|_| Error::Contract("legacy catalog must be UTF-8".into()))?;
    let originals = files::confined(root, "originals")?;
    let license = files::read(
        &files::confined(&originals, "LICENSE.md")?,
        limits.original_bytes,
        Resource::OriginalBytes,
    )?;
    files::digest("original MIT license", &license, LICENSE_SHA256)?;
    let mut authority = BTreeMap::new();
    let mut origins = Vec::new();
    for &(path, hash) in contracts::ORIGINALS {
        let bytes = files::read(
            &files::confined(&originals, path)?,
            limits.original_bytes,
            Resource::OriginalBytes,
        )?;
        files::digest(path, &bytes, hash)?;
        let text = String::from_utf8(bytes)
            .map_err(|_| Error::Contract(format!("{path}: original must be UTF-8")))?;
        let stop = text
            .find("// }}}")
            .ok_or_else(|| Error::Contract(format!("{path}: missing copyright notice")))?
            + 6;
        origins.push(OriginRecord {
            path: path.into(),
            url: format!("https://github.com/potassco/clingo/blob/{UPSTREAM_REVISION}/{path}"),
            sha256: hash.into(),
            copyright_notice: text[..stop].to_owned(),
        });
        authority.insert(path, text);
    }
    let mut cases = Vec::new();
    let mut sources = Vec::new();
    let mut total = 0_u128;
    for line in catalog.lines() {
        ceiling(Resource::Cases, cases.len() as u128 + 1, limits.cases)?;
        let legacy: LegacyCase = serde_json::from_str(line)?;
        ceiling(
            Resource::SourceBytes,
            legacy.source.len() as u128,
            limits.source_bytes,
        )?;
        total += legacy.source.len() as u128;
        ceiling(Resource::TotalSourceBytes, total, limits.total_source_bytes)?;
        let text = authority
            .get(legacy.source_file.as_str())
            .ok_or_else(|| Error::Contract(format!("{}: unknown original file", legacy.id)))?;
        verify_original(&legacy, text)?;
        let (case, source) = convert(legacy);
        cases.push(case);
        sources.push(source);
    }
    let document = Document {
        schema: 1,
        upstream_revision: UPSTREAM_REVISION.into(),
        license_path: "LICENSE.md".into(),
        license_sha256: LICENSE_SHA256.into(),
        origins,
        cases,
    };
    document::validate(&document, limits)?;
    Ok(Imported {
        document,
        sources,
        license,
    })
}
fn verify_original(case: &LegacyCase, text: &str) -> Result<(), Error> {
    let Some(assertion) = text.get(case.byte_start..case.byte_end) else {
        return Err(Error::Contract(format!(
            "{}: invalid original byte span",
            case.id
        )));
    };
    let line_start = text[..case.byte_start]
        .bytes()
        .filter(|&byte| byte == b'\n')
        .count()
        + 1;
    let line_end = text[..case.byte_end]
        .bytes()
        .filter(|&byte| byte == b'\n')
        .count()
        + 1;
    if assertion != case.assertion || line_start != case.line_start || line_end != case.line_end {
        return Err(Error::Contract(format!(
            "{}: original coordinates or assertion differ",
            case.id
        )));
    }
    let expected_hash = contracts::ORIGINALS
        .iter()
        .find(|(path, _)| *path == case.source_file)
        .map(|(_, hash)| *hash)
        .ok_or_else(|| Error::Contract("unknown original".into()))?;
    if case.file_sha256 != expected_hash {
        return Err(Error::Contract(format!(
            "{}: original file seal differs",
            case.id
        )));
    }
    let (section, ordinal) = cpp::identity(text, case.byte_start)?;
    if section != case.section || ordinal != case.assertion_ordinal {
        return Err(Error::Contract(format!(
            "{}: section or ordinal differs",
            case.id
        )));
    }
    let (source, arguments, expected) = cpp::assertion(assertion)?;
    if source != case.source
        || arguments != case.helper_arguments
        || expected != case.expected_helper_output
    {
        return Err(Error::Contract(format!(
            "{}: decoded assertion differs",
            case.id
        )));
    }
    files::digest(&case.id, source.as_bytes(), &case.source_sha256)?;
    if cpp::prefixes(&arguments)? != case.filters
        || cpp::helper_models(&expected)? != case.expected_helper_models
    {
        return Err(Error::Contract(format!(
            "{}: derived helper contract differs",
            case.id
        )));
    }
    Ok(())
}
fn convert(case: LegacyCase) -> (CaseRecord, Vec<u8>) {
    let provenance = ProvenanceRecord {
        source_file: case.source_file,
        section: case.section,
        assertion_ordinal: case.assertion_ordinal,
        byte_start: case.byte_start,
        byte_end: case.byte_end,
        line_start: case.line_start,
        line_end: case.line_end,
        assertion: case.assertion,
        helper_arguments: case.helper_arguments,
        expected_helper_output: case.expected_helper_output,
    };
    let contract = ContractRecord {
        full_models: case.models,
        prefixes: case.filters,
        helper_models: case.expected_helper_models,
    };
    (
        CaseRecord {
            path: format!("programs/{}.lp", case.id),
            id: case.id,
            source_sha256: case.source_sha256,
            license: "MIT".into(),
            provenance,
            contract,
        },
        case.source.into_bytes(),
    )
}
fn encode(document: &Document, limits: Limits) -> Result<Vec<u8>, Error> {
    let mut bytes = serde_json::to_vec_pretty(document)?;
    bytes.push(b'\n');
    ceiling(
        Resource::ManifestBytes,
        bytes.len() as u128,
        limits.manifest_bytes,
    )?;
    Ok(bytes)
}
fn destination_path(root: &Path, destination: &Path) -> Result<PathBuf, Error> {
    if std::fs::symlink_metadata(destination).is_ok() {
        return Err(Error::DestinationExists(destination.to_owned()));
    }
    let parent = destination
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let parent = files::canonical(parent)?;
    let name = destination
        .file_name()
        .ok_or_else(|| Error::Path(destination.display().to_string()))?;
    let actual = parent.join(name);
    if actual.starts_with(files::confined(root, "originals")?) {
        return Err(Error::Path(actual.display().to_string()));
    }
    Ok(actual)
}
fn write_new(path: &Path, bytes: &[u8]) -> Result<(), Error> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|source| files::io(path, source))?;
    file.write_all(bytes)
        .map_err(|source| files::io(path, source))
}
fn publish(destination: &Path, imported: &Imported, manifest: &[u8]) -> Result<(), Error> {
    std::fs::create_dir(destination).map_err(|source| {
        if source.kind() == std::io::ErrorKind::AlreadyExists {
            Error::DestinationExists(destination.to_owned())
        } else {
            files::io(destination, source)
        }
    })?;
    let result = (|| {
        for (case, bytes) in imported.document.cases.iter().zip(&imported.sources) {
            let path = destination.join(files::relative(&case.path)?);
            let parent = path
                .parent()
                .ok_or_else(|| Error::Path(case.path.clone()))?;
            std::fs::create_dir_all(parent).map_err(|source| files::io(parent, source))?;
            write_new(&path, bytes)?;
        }
        write_new(&destination.join("LICENSE.md"), &imported.license)?;
        // The manifest is written last; incomplete output never has its seal.
        write_new(&destination.join("manifest.json"), manifest)
    })();
    result.map_err(|failure| cleanup(destination, failure))
}
fn cleanup(destination: &Path, failure: Error) -> Error {
    match std::fs::remove_dir_all(destination) {
        Ok(()) => failure,
        Err(cleanup) => Error::ImportCleanup {
            destination: destination.to_owned(),
            failure: Box::new(failure),
            cleanup,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{Document, Error, Imported, cleanup, publish};

    #[test]
    fn a_failed_publication_removes_only_its_new_directory() {
        let parent = tempfile::tempdir().unwrap();
        let retained = parent.path().join("retained");
        std::fs::write(&retained, b"existing parent data").unwrap();
        let destination = parent.path().join("new-corpus");
        let mut document: Document = serde_json::from_str(include_str!(
            "../../../../validation/upstream/clingo-5.8.2/curated/manifest.json"
        ))
        .unwrap();
        document.cases.truncate(1);
        // A collision forces a late create_new refusal after a source write.
        document.cases[0].path = "LICENSE.md".into();
        let imported = Imported {
            document,
            sources: vec![b"p.".to_vec()],
            license: b"license".to_vec(),
        };
        assert!(matches!(
            publish(&destination, &imported, b"{}"),
            Err(Error::Io { .. })
        ));
        assert!(!destination.exists());
        assert_eq!(std::fs::read(retained).unwrap(), b"existing parent data");
    }

    #[test]
    fn a_cleanup_failure_retains_the_publication_cause() {
        let parent = tempfile::tempdir().unwrap();
        let destination = parent.path().join("not-a-directory");
        std::fs::write(&destination, b"retained").unwrap();
        let error = cleanup(&destination, Error::Contract("primary".into()));
        let Error::ImportCleanup {
            failure,
            destination: retained,
            ..
        } = error
        else {
            panic!("cleanup failure must remain explicit");
        };
        assert!(matches!(*failure, Error::Contract(ref reason) if reason == "primary"));
        assert_eq!(retained, destination);
        assert_eq!(std::fs::read(destination).unwrap(), b"retained");
    }
}
