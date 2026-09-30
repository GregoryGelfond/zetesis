//! Recheck admitted inputs at the actual pre-execution sealing boundary.

use super::*;
use std::{fs, path::PathBuf};
use zetesis_test_support::repository;

struct Fixture {
    _directory: tempfile::TempDir,
    corpus: curated::Corpus,
    reference: PathBuf,
    native: PathBuf,
    report: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let original = repository::upstream().join("curated");
        let admitted = curated::open(&original, curated::Limits::default()).unwrap();
        let root = directory.path().join("corpus");
        fs::create_dir(&root).unwrap();
        for name in ["manifest.json", "LICENSE.md"] {
            fs::copy(original.join(name), root.join(name)).unwrap();
        }
        for case in admitted.cases() {
            let path = root.join(case.path());
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, case.source()).unwrap();
        }
        let corpus = curated::open(&root, curated::Limits::default()).unwrap();
        let reference = directory.path().join("reference");
        let native = directory.path().join("native");
        let report = directory.path().join("report.json");
        // Sealing inspects distinct regular-file identities. These plain files
        // are deliberately not executables; this test invokes no producer.
        fs::write(&reference, b"reference identity fixture").unwrap();
        fs::write(&native, b"native identity fixture").unwrap();
        Self {
            _directory: directory,
            corpus,
            reference,
            native,
            report,
        }
    }

    fn request(&self) -> Request<'_> {
        Request {
            corpus: self.corpus.root(),
            reference: &self.reference,
            native: &self.native,
            report: &self.report,
            execution: NativeExecution::default(),
            limits: Limits::default(),
        }
    }
}

#[test]
fn admitted_input_changes_are_refused_before_execution_sealing() {
    let fixture = Fixture::new();
    let original = seals(&fixture.corpus, &fixture.request()).unwrap();
    assert_eq!(original.len(), fixture.corpus.cases().len() + 4);
    for (name, reason) in [
        ("manifest.json", "manifest changed after verification"),
        ("LICENSE.md", "license changed after verification"),
        (
            fixture.corpus.cases()[0].path(),
            "source changed after verification",
        ),
    ] {
        let path = fixture.corpus.root().join(name);
        let before = fs::read(&path).unwrap();
        // A trailing newline preserves source/JSON syntax but changes the
        // exact admitted bytes; semantic similarity cannot replace identity.
        let mut changed = before.clone();
        changed.push(b'\n');
        fs::write(&path, &changed).unwrap();
        let error = seals(&fixture.corpus, &fixture.request()).unwrap_err();
        assert!(matches!(
            &error,
            Error::Path { path: refused, detail } if refused == &path && *detail == reason
        ));
        assert_eq!(fs::read(&path).unwrap(), changed);
        assert!(error.to_string().contains(reason));
        // Restore only this owned test input. The same retained Corpus must
        // again seal every original identity without a new admission pass.
        fs::write(path, before).unwrap();
        let restored = seals(&fixture.corpus, &fixture.request()).unwrap();
        assert_eq!(restored.len(), original.len());
        assert!(restored.iter().zip(&original).all(|(after, before)| {
            after.requested() == before.requested() && after.sha256() == before.sha256()
        }));
    }
    assert!(!fixture.report.exists());
}

#[test]
fn disappeared_admitted_sources_keep_their_io_cause() {
    let fixture = Fixture::new();
    seals(&fixture.corpus, &fixture.request()).unwrap();
    let path = fixture.corpus.root().join(fixture.corpus.cases()[0].path());
    fs::remove_file(&path).unwrap();
    let error = seals(&fixture.corpus, &fixture.request()).unwrap_err();
    assert!(matches!(&error, Error::Io { path: refused, .. } if refused == &path));
    let cause = std::error::Error::source(&error)
        .unwrap()
        .downcast_ref::<std::io::Error>()
        .unwrap();
    assert_eq!(cause.kind(), std::io::ErrorKind::NotFound);
    assert!(error.to_string().contains(path.to_str().unwrap()));
    assert!(!path.exists());
    assert!(!fixture.report.exists());
}
