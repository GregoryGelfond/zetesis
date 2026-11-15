//! Schema-2 admission boundaries reuse the existing typed constructors.

use super::*;

fn generated() -> Value {
    json!({"schema_version":2,"sources":[],"cases":[
        {"kind":"generated","family":"chain","size":4},
        {"kind":"generated","family":"chain","size":5}
    ]})
}

fn authored() -> (tempfile::TempDir, Value) {
    let (directory, original) = fixture();
    let manifest = json!({"schema_version":2,"sources":[{
        "id":"p", "root":"sources", "entry":"entry.lp",
        "sha256":crate::digest::sha256(b"p."),
        "contract":original["cases"][0]["contract"]
    }],"cases":[{"kind":"authored","source":"p","amendments":[]}]});
    (directory, manifest)
}

#[test]
fn schema_versions_preserve_the_same_authored_contract() {
    let (directory, old) = fixture();
    let path = write(directory.path(), &old);
    let unchanged = load(&path, matrix::WorkloadLimits::default()).unwrap();
    let typed = json!({"schema_version":2,"sources":[{
        "id":"p", "root":"sources", "entry":"entry.lp",
        "sha256":crate::digest::sha256(b"p."),"contract":old["cases"][0]["contract"]
    }],"cases":[{"kind":"authored","source":"p","amendments":[]}]});
    let loaded = load(
        &write(directory.path(), &typed),
        matrix::WorkloadLimits::default(),
    )
    .unwrap();
    assert_eq!(loaded[0].contract(), unchanged[0].contract());
    assert!(unchanged[0].is_bundle());
    assert!(loaded[0].is_authored());
    let legacy = serde_json::to_value(&unchanged[0]).unwrap();
    let current = serde_json::to_value(&loaded[0]).unwrap();
    assert_eq!(legacy["sources"], current["sources"]);
    assert!(legacy.get("selection_manifest").is_none());
    assert!(current.get("selection_manifest").is_some());
}

#[test]
fn typed_sources_require_a_unique_used_descriptor() {
    let (directory, manifest) = authored();
    for changed in [
        {
            let mut value = manifest.clone();
            value["sources"][0]["id"] = json!("");
            value
        },
        {
            let mut value = manifest.clone();
            value["sources"]
                .as_array_mut()
                .unwrap()
                .push(manifest["sources"][0].clone());
            value
        },
        {
            let mut value = manifest.clone();
            value["cases"][0]["source"] = json!("unknown");
            value
        },
        {
            let mut value = manifest.clone();
            value["cases"] = generated()["cases"].clone();
            value
        },
        {
            let mut value = manifest.clone();
            value["sources"][0]["extra"] = json!(true);
            value
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
fn typed_cases_refuse_unknown_or_duplicate_constructors() {
    let directory = tempfile::tempdir().unwrap();
    let manifest = generated();
    for changed in [
        {
            let mut value = manifest.clone();
            value["cases"] = json!([]);
            value
        },
        {
            let mut value = manifest.clone();
            value["cases"][0]["family"] = json!("unknown");
            value
        },
        {
            let mut value = manifest.clone();
            value["cases"][0]["kind"] = json!("unknown");
            value
        },
        {
            let mut value = manifest.clone();
            value["cases"][0]["extra"] = json!(true);
            value
        },
        {
            let mut value = manifest.clone();
            value["cases"][1] = value["cases"][0].clone();
            value
        },
        {
            let mut value = manifest.clone();
            value["cases"] = json!([{
                "kind":"corpus","entry":"standalone/n-queens/variant-01.lp","amendments":[]
            }]);
            value
        },
        {
            let mut value = manifest.clone();
            value["cases"] = json!(vec![manifest["cases"][0].clone(); 96]);
            value
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
fn typed_aggregate_source_ceiling_is_inclusive() {
    let directory = tempfile::tempdir().unwrap();
    let path = write(directory.path(), &generated());
    let loaded = load(&path, matrix::WorkloadLimits::default()).unwrap();
    let total: usize = loaded.iter().map(|workload| workload.source_bytes).sum();
    let limits = matrix::WorkloadLimits {
        total_source_bytes: total,
        ..matrix::WorkloadLimits::default()
    };
    assert_eq!(load(&path, limits).unwrap().len(), 2);
    assert!(matches!(
        load(
            &path,
            matrix::WorkloadLimits {
                total_source_bytes: total - 1,
                ..limits
            }
        ),
        Err(Error::Configuration(
            "external workload selection exceeds aggregate ceilings"
        ))
    ));
}

#[test]
fn typed_manifest_read_ceiling_is_inclusive() {
    let directory = tempfile::tempdir().unwrap();
    let path = write(directory.path(), &generated());
    let mut bytes = std::fs::read(&path).unwrap();
    bytes.resize(4096, b' ');
    std::fs::write(&path, bytes).unwrap();
    let limits = matrix::WorkloadLimits {
        metadata_bytes: 4096,
        ..matrix::WorkloadLimits::default()
    };
    assert_eq!(load(&path, limits).unwrap().len(), 2);
    assert!(matches!(
        load(
            &path,
            matrix::WorkloadLimits {
                metadata_bytes: 4095,
                ..limits
            }
        ),
        Err(Error::Boundary(_))
    ));
}

#[test]
fn typed_source_digest_is_checked_before_retention() {
    let (directory, manifest) = authored();
    let path = write(directory.path(), &manifest);
    assert_eq!(
        load(&path, matrix::WorkloadLimits::default())
            .unwrap()
            .len(),
        1
    );
    std::fs::write(directory.path().join("sources/entry.lp"), "q.").unwrap();
    assert!(matches!(
        load(&path, matrix::WorkloadLimits::default()),
        Err(Error::Boundary(crate::selected::Error::Path { .. }))
    ));
}

#[test]
fn typed_amendments_preserve_the_constructor_ceiling() {
    let (directory, mut manifest) = authored();
    let text = "#const n=1. p(1..n).";
    std::fs::write(directory.path().join("sources/entry.lp"), text).unwrap();
    manifest["sources"][0]["sha256"] = json!(crate::digest::sha256(text.as_bytes()));
    manifest["cases"][0]["amendments"] = json!([{
        "source_path":"entry.lp","name":"n","expected":1,"replacement":2
    }]);
    let path = write(directory.path(), &manifest);
    let limits = matrix::WorkloadLimits {
        amendments: 1,
        ..matrix::WorkloadLimits::default()
    };
    let loaded = load(&path, limits).unwrap();
    assert!(loaded[0].is_amended());
    assert!(loaded[0].contract().is_none());
    assert!(matches!(
        load(
            &path,
            matrix::WorkloadLimits {
                amendments: 0,
                ..limits
            }
        ),
        Err(Error::Configuration(
            "workload amendment count exceeds its ceiling"
        ))
    ));
}

#[test]
fn typed_reviewed_manifest_refuses_changed_bytes() {
    let directory = tempfile::tempdir().unwrap();
    let path = write(directory.path(), &generated());
    let mut bytes = std::fs::read(&path).unwrap();
    let expected = crate::digest::sha256(&bytes);
    assert_eq!(
        load_reviewed(&path, matrix::WorkloadLimits::default(), &expected)
            .unwrap()
            .len(),
        2
    );
    bytes.push(b' ');
    std::fs::write(&path, bytes).unwrap();
    assert!(matches!(
        load_reviewed(&path, matrix::WorkloadLimits::default(), &expected),
        Err(Error::Corpus(examples::Error::Digest { .. }))
    ));
}

#[test]
fn typed_case_metadata_keeps_the_individual_ceiling() {
    let (directory, mut manifest) = authored();
    manifest["sources"][0]["contract"]["notes"] = json!(["x".repeat(65_536)]);
    let path = write(directory.path(), &manifest);
    let limits = matrix::WorkloadLimits {
        metadata_bytes: 1_048_576,
        ..matrix::WorkloadLimits::default()
    };
    assert!(std::fs::metadata(&path).unwrap().len() < 1_048_576);
    assert!(load(&path, limits).is_err());
}
