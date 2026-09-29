//! Portable confinement and bounded-read failure evidence.

use super::*;

#[test]
fn nonportable_components_cannot_name_corpus_inputs() {
    for path in [
        "",
        ".",
        "..",
        "../source.lp",
        "/source.lp",
        "a//b.lp",
        "a/./b.lp",
        "a/../b.lp",
        "a\\b.lp",
        "C:source.lp",
    ] {
        let error = relative(path).unwrap_err();
        assert!(matches!(&error, Error::Path(actual) if actual == path));
        assert!(error.to_string().contains("not confined"));
        assert!(std::error::Error::source(&error).is_none());
    }
}

#[test]
fn directories_cannot_supply_source_bytes() {
    let directory = tempfile::tempdir().unwrap();
    assert!(matches!(
        read(directory.path(), 1024, Resource::SourceBytes),
        Err(Error::Path(_))
    ));
}

#[test]
fn missing_sources_retain_the_filesystem_cause() {
    let directory = tempfile::tempdir().unwrap();
    let error = read(
        &directory.path().join("absent.lp"),
        1024,
        Resource::SourceBytes,
    )
    .unwrap_err();
    assert!(error.to_string().contains("absent.lp"));
    let cause = std::error::Error::source(&error)
        .unwrap()
        .downcast_ref::<std::io::Error>()
        .unwrap();
    assert_eq!(cause.kind(), std::io::ErrorKind::NotFound);
}

#[test]
fn read_limit_errors_keep_the_resource_identity() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("source.lp");
    std::fs::write(&path, b"abc").unwrap();
    let error = read(&path, 2, Resource::SourceBytes).unwrap_err();
    assert!(matches!(
        &error,
        Error::Limit {
            resource: Resource::SourceBytes,
            observed: 3,
            limit: 2
        }
    ));
    assert!(error.to_string().contains("SourceBytes 3 exceeds 2"));
    assert!(std::error::Error::source(&error).is_none());
}
