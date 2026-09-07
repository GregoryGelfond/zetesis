//! Publication failures preserve original causes and never replace evidence.

use super::*;
use serde::Serialize;
use std::cell::Cell;

fn destination(directory: &Path) -> Destination {
    prepare(
        &directory.join("report.json"),
        &directory.join("corpus"),
        &[],
    )
    .unwrap()
}

struct FailsOnWrite(Cell<bool>);
impl Serialize for FailsOnWrite {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if self.0.replace(true) {
            Err(serde::ser::Error::custom(
                "authoritative serialization failure",
            ))
        } else {
            serializer.serialize_str("measured successfully")
        }
    }
}

#[test]
fn serialization_failure_removes_partial_temporary_output() {
    let directory = tempfile::tempdir().unwrap();
    let target = destination(directory.path());
    let error = write(&FailsOnWrite(Cell::new(false)), &target, 1024).unwrap_err();
    assert!(
        matches!(&error, Error::Json(source) if source.to_string().contains("authoritative serialization failure"))
    );
    assert!(std::error::Error::source(&error).is_some());
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
}

struct AppearsDuringEncoding<'a> {
    target: &'a Path,
    encoded: Cell<bool>,
}
impl Serialize for AppearsDuringEncoding<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if self.encoded.replace(true) {
            std::fs::write(self.target, b"concurrent evidence")
                .map_err(serde::ser::Error::custom)?;
        }
        serializer.serialize_str("new report")
    }
}

#[test]
fn evidence_appearing_during_encoding_is_preserved() {
    let directory = tempfile::tempdir().unwrap();
    let target = destination(directory.path());
    let report = AppearsDuringEncoding {
        target: &target.path,
        encoded: Cell::new(false),
    };
    let error = write(&report, &target, 1024).unwrap_err();
    assert!(matches!(
        error,
        Error::Path {
            detail: "report destination already exists",
            ..
        }
    ));
    assert_eq!(std::fs::read(&target.path).unwrap(), b"concurrent evidence");
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
}

#[test]
fn lexical_parent_components_cannot_enter_the_corpus() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("corpus");
    let path = directory.path().join("unrelated/../corpus/report.json");
    let error = prepare(&path, &root, &[]).unwrap_err();
    assert!(matches!(
        error,
        Error::Path {
            detail: "report must be outside the curated input directory",
            ..
        }
    ));
}

#[test]
fn root_paths_cannot_be_report_files() {
    let directory = tempfile::tempdir().unwrap();
    let error = prepare(Path::new("/"), directory.path(), &[]).unwrap_err();
    assert!(matches!(
        error,
        Error::Path {
            detail: "report has no parent",
            ..
        }
    ));
}

#[test]
fn missing_report_parents_preserve_filesystem_causes() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("missing/report.json");
    let error = prepare(&path, &directory.path().join("corpus"), &[]).unwrap_err();
    let cause = std::error::Error::source(&error)
        .unwrap()
        .downcast_ref::<std::io::Error>()
        .unwrap();
    assert_eq!(cause.kind(), std::io::ErrorKind::NotFound);
    assert!(error.to_string().contains("missing"));
}

#[test]
fn disappearing_parents_cannot_receive_a_report() {
    let directory = tempfile::tempdir().unwrap();
    let parent = directory.path().join("reports");
    std::fs::create_dir(&parent).unwrap();
    let target = destination(&parent);
    std::fs::remove_dir(&parent).unwrap();
    assert!(matches!(
        write(&serde_json::json!({"passed":true}), &target, 1024),
        Err(Error::Io { .. })
    ));
    assert!(!target.path.exists());
}

#[test]
fn report_limits_count_the_terminal_newline() {
    let directory = tempfile::tempdir().unwrap();
    let target = destination(directory.path());
    // An empty JSON object has two bytes; the published newline is the third.
    assert!(matches!(
        write(&serde_json::json!({}), &target, 2),
        Err(Error::Bytes { limit: 2, .. })
    ));
    write(&serde_json::json!({}), &target, 3).unwrap();
    assert_eq!(std::fs::read(target.path).unwrap(), b"{}\n");
}

#[test]
fn cleanup_failure_retains_the_primary_cause() {
    let directory = tempfile::tempdir().unwrap();
    let temporary = tempfile::NamedTempFile::new_in(directory.path()).unwrap();
    let path = temporary.path().to_owned();
    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir(&path).unwrap();
    let primary = Error::Bytes {
        path: directory.path().join("report"),
        limit: 7,
    };
    let error = cleanup(temporary, primary).unwrap_err();
    assert!(
        matches!(&error, Error::Cleanup { primary, .. } if matches!(primary.as_ref(), Error::Bytes {limit:7,..}))
    );
    assert!(error.to_string().contains("temporary cleanup"));
    let cause = std::error::Error::source(&error).unwrap();
    assert!(matches!(
        cause.downcast_ref::<Error>(),
        Some(Error::Bytes { limit: 7, .. })
    ));
}

struct ExpandsDuringEncoding(Cell<bool>);
impl Serialize for ExpandsDuringEncoding {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(if self.0.replace(true) {
            "far beyond the measured byte count"
        } else {
            "a"
        })
    }
}

#[test]
fn expanded_second_encodings_cannot_exceed_the_report_limit() {
    let directory = tempfile::tempdir().unwrap();
    let target = destination(directory.path());
    let error = write(&ExpandsDuringEncoding(Cell::new(false)), &target, 4).unwrap_err();
    assert!(matches!(error, Error::Bytes { limit: 4, .. }));
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
}

struct FailingWriter {
    remaining: usize,
    flush_fails: bool,
}
impl Write for FailingWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.remaining == 0 {
            return Err(io::Error::other("authoritative writer failure"));
        }
        let written = self.remaining.min(bytes.len());
        self.remaining -= written;
        Ok(written)
    }
    fn flush(&mut self) -> io::Result<()> {
        if self.flush_fails {
            Err(io::Error::other("authoritative flush failure"))
        } else {
            Ok(())
        }
    }
}

#[test]
fn encoding_io_errors_are_not_reported_as_budget_exhaustion() {
    let error = encode(
        &serde_json::json!({"message":"complete"}),
        FailingWriter {
            remaining: 0,
            flush_fails: false,
        },
        Path::new("report.json"),
        1024,
    )
    .unwrap_err();
    assert!(matches!(&error, Error::Json(source) if source.is_io()));
    assert!(error.to_string().contains("authoritative writer failure"));
}

#[test]
fn final_newline_io_failure_retains_its_cause() {
    let error = encode(
        &serde_json::json!({}),
        FailingWriter {
            remaining: 2,
            flush_fails: false,
        },
        Path::new("report.json"),
        3,
    )
    .unwrap_err();
    assert!(
        matches!(&error, Error::Io {source,..} if source.to_string() == "authoritative writer failure")
    );
    assert!(std::error::Error::source(&error).is_some());
}

#[test]
fn flush_failure_cannot_complete_report_encoding() {
    let error = encode(
        &serde_json::json!({}),
        FailingWriter {
            remaining: 3,
            flush_fails: true,
        },
        Path::new("report.json"),
        3,
    )
    .unwrap_err();
    assert!(
        matches!(error, Error::Io {source,..} if source.to_string() == "authoritative flush failure")
    );
}
