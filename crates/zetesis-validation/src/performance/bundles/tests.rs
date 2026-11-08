use super::*;
use serde_json::{Value, json};

fn fixture() -> (tempfile::TempDir, Value) {
    let directory = tempfile::tempdir().unwrap();
    std::fs::create_dir(directory.path().join("sources")).unwrap();
    std::fs::write(directory.path().join("sources/entry.lp"), "p.").unwrap();
    let manifest = json!({"schema_version": 1, "root": "sources", "cases": [{
        "entry": "entry.lp",
        "contract": {"satisfiability":"sat", "family":"all", "model_count":1,
            "cost":null, "witnesses":[], "required_symbols":[], "notes":[]},
        "files":[{"path":"entry.lp", "sha256":crate::digest::sha256(b"p."), "includes":[]}]
    }]});
    (directory, manifest)
}

fn write(directory: &Path, manifest: &Value) -> PathBuf {
    let path = directory.join("workloads.json");
    std::fs::write(&path, serde_json::to_vec(manifest).unwrap()).unwrap();
    path
}

#[test]
fn relative_root_and_manifest_seal_survive_loading() {
    let (directory, manifest) = fixture();
    let path = write(directory.path(), &manifest);
    let workloads = load(&path, matrix::WorkloadLimits::default()).unwrap();
    assert_eq!(workloads.len(), 1);
    assert!(workloads[0].is_bundle());
    let record = serde_json::to_value(&workloads[0]).unwrap();
    assert_eq!(
        record["bundle"]["manifest"]["sha256"],
        crate::digest::sha256(&std::fs::read(&path).unwrap())
    );
    assert_eq!(
        record["bundle"]["root"],
        directory
            .path()
            .join("sources")
            .canonicalize()
            .unwrap()
            .to_str()
            .unwrap()
    );
}

#[test]
fn unknown_fields_versions_and_duplicate_identities_are_refused() {
    let (directory, manifest) = fixture();
    for changed in [
        {
            let mut changed = manifest.clone();
            changed["schema_version"] = json!(2);
            changed
        },
        {
            let mut changed = manifest.clone();
            changed["unknown"] = json!(true);
            changed
        },
        {
            let mut changed = manifest.clone();
            changed["cases"]
                .as_array_mut()
                .unwrap()
                .push(manifest["cases"][0].clone());
            changed
        },
    ] {
        assert!(
            load(
                &write(directory.path(), &changed),
                matrix::WorkloadLimits::default()
            )
            .is_err()
        );
    }
}

#[test]
fn manifest_read_ceiling_is_inclusive() {
    let (directory, manifest) = fixture();
    let path = write(directory.path(), &manifest);
    let mut bytes = std::fs::read(&path).unwrap();
    bytes.resize(4096, b' ');
    std::fs::write(&path, bytes).unwrap();
    assert!(
        load(
            &path,
            matrix::WorkloadLimits {
                metadata_bytes: 4096,
                ..matrix::WorkloadLimits::default()
            }
        )
        .is_ok()
    );
    let result = load(
        &path,
        matrix::WorkloadLimits {
            metadata_bytes: 4095,
            ..matrix::WorkloadLimits::default()
        },
    );
    assert!(matches!(result, Err(Error::Boundary(_))));
}

#[test]
fn reviewed_loading_checks_the_manifest_bytes_it_admits() {
    let (directory, manifest) = fixture();
    let path = write(directory.path(), &manifest);
    let mut bytes = std::fs::read(&path).unwrap();
    let expected = crate::digest::sha256(&bytes);
    assert_eq!(
        load_reviewed(&path, matrix::WorkloadLimits::default(), &expected)
            .unwrap()
            .len(),
        1
    );
    bytes.push(b' ');
    std::fs::write(&path, bytes).unwrap();
    assert!(matches!(
        load_reviewed(&path, matrix::WorkloadLimits::default(), &expected),
        Err(Error::Corpus(examples::Error::Digest { .. }))
    ));
}
