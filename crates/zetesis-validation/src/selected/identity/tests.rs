//! Adversarial contracts for the private corpus boundary and for the rule
//! that settles the executables native runs name.

use super::*;

#[test]
fn immediate_eof_cannot_seal_an_advertised_nonempty_file() {
    // Reproduce the observed metadata/read contradiction without depending on
    // dataless files or racing a writer. This is the production hashing path.
    let error = content_seal(
        &mut std::io::empty(),
        Path::new("input"),
        18_454_848,
        18_454_848,
        |_| Ok(18_454_848),
    )
    .unwrap_err();
    let Error::Io { path, source } = error else {
        panic!("expected a read consistency error")
    };
    assert_eq!(path, Path::new("input"));
    assert_eq!(source.kind(), std::io::ErrorKind::UnexpectedEof);
    assert_eq!(
        source.to_string(),
        "sealed input length disagrees: before 18454848 bytes, read 0 bytes, after 18454848 bytes"
    );
}

#[test]
fn a_truncated_prefix_cannot_receive_a_content_seal() {
    let error = content_seal(&mut &b"ab"[..], Path::new("input"), 3, 3, |_| Ok(3)).unwrap_err();
    assert!(matches!(
        error,
        Error::Io { source, .. } if source.kind() == std::io::ErrorKind::UnexpectedEof
    ));
}

struct ShortReads<'a>(&'a [u8]);

impl Read for ShortReads<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let count = buffer.len().min(2).min(self.0.len());
        buffer[..count].copy_from_slice(&self.0[..count]);
        self.0 = &self.0[count..];
        Ok(count)
    }
}

#[test]
fn short_reads_preserve_the_complete_exact_limit_digest() {
    let (bytes, digest) =
        content_seal(&mut ShortReads(b"abc"), Path::new("input"), 3, 3, |_| Ok(3)).unwrap();
    assert_eq!(bytes, 3);
    assert_eq!(
        digest,
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn an_empty_regular_file_has_a_valid_zero_limit_seal() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("input");
    std::fs::write(&path, b"").unwrap();
    let observed = seal(&path, 0).unwrap();
    assert_eq!(observed.bytes(), 0);
    assert_eq!(
        observed.sha256(),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

#[test]
fn growth_within_the_byte_ceiling_refuses_a_seal() {
    let error = content_seal(&mut &b"abcd"[..], Path::new("input"), 4, 3, |_| Ok(4)).unwrap_err();
    assert!(matches!(
        error,
        Error::Io { source, .. } if source.kind() == std::io::ErrorKind::InvalidData
    ));
}

#[test]
fn post_read_length_changes_refuse_a_seal() {
    for after in [2, 4] {
        let error =
            content_seal(&mut &b"abc"[..], Path::new("input"), 4, 3, |_| Ok(after)).unwrap_err();
        assert!(matches!(
            error,
            Error::Io { source, .. } if source.kind() == std::io::ErrorKind::InvalidData
        ));
    }
}

#[test]
fn growth_over_the_ceiling_stops_at_the_bounded_probe() {
    let mut reader = std::io::Cursor::new(b"abcdef");
    let error = content_seal(&mut reader, Path::new("input"), 3, 3, |_| {
        panic!("the byte ceiling must stop hashing before the metadata recheck")
    })
    .unwrap_err();
    assert!(matches!(error, Error::Bytes { limit: 3, .. }));
    assert_eq!(reader.position(), 4);
}

#[test]
fn failed_post_read_metadata_cannot_receive_a_seal() {
    let error = content_seal(&mut &b"abc"[..], Path::new("input"), 3, 3, |_| {
        Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "metadata unavailable",
        ))
    })
    .unwrap_err();
    assert!(matches!(
        error,
        Error::Io { source, .. }
            if source.kind() == std::io::ErrorKind::PermissionDenied
                && source.to_string() == "metadata unavailable"
    ));
}

#[test]
fn deleted_inputs_retain_absence_as_absence() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("input");
    std::fs::write(&path, b"original").unwrap();
    let before = seal(&path, 8).unwrap();
    assert_eq!(before.bytes(), 8);
    std::fs::remove_file(&path).unwrap();
    let after = recheck(&before);
    assert!(!after.unchanged());
    assert!(after.after().is_none());
    assert!(after.error().is_some_and(|error| error.contains("input")));
}

#[test]
fn rechecks_keep_the_original_byte_ceiling() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("input");
    std::fs::write(&path, b"a").unwrap();
    let before = seal(&path, 1).unwrap();
    std::fs::write(&path, b"aa").unwrap();
    let after = recheck(&before);
    assert!(!after.unchanged());
    assert!(after.after().is_none());
    assert!(
        after
            .error()
            .is_some_and(|error| error.contains("byte ceiling 1"))
    );
}

#[test]
fn directories_cannot_receive_content_seals() {
    let directory = tempfile::tempdir().unwrap();
    let error = seal(directory.path(), 1024).unwrap_err();
    assert!(matches!(
        error,
        Error::Path {
            detail: "sealed input must be a regular file",
            ..
        }
    ));
}

#[cfg(unix)]
#[test]
fn replacing_equal_bytes_changes_file_identity() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("input");
    let replacement = directory.path().join("replacement");
    std::fs::write(&input, b"same").unwrap();
    std::fs::write(&replacement, b"same").unwrap();
    let before = seal(&input, 4).unwrap();
    std::fs::rename(replacement, &input).unwrap();
    let change = recheck(&before);
    assert!(!change.unchanged());
    let after = change.after().unwrap();
    assert_eq!(before.sha256(), after.sha256());
    assert_eq!(before.canonical(), after.canonical());
    assert!(change.error().is_none());
}

#[cfg(unix)]
#[test]
fn non_utf8_requested_paths_keep_the_refused_path() {
    use std::os::unix::ffi::OsStringExt;
    let directory = tempfile::tempdir().unwrap();
    let path = directory
        .path()
        .join(std::ffi::OsString::from_vec(vec![255]));
    let error = seal(&path, 1).unwrap_err();
    assert!(
        matches!(error, Error::Path { path: refused, detail:"report paths require lossless UTF-8 representation" } if refused == path)
    );
}

// Linux permits arbitrary filename bytes; APFS refuses this fixture at creation.
#[cfg(target_os = "linux")]
#[test]
fn non_utf8_symlink_targets_are_refused() {
    use std::os::unix::ffi::OsStringExt;
    let directory = tempfile::tempdir().unwrap();
    let path = directory
        .path()
        .join(std::ffi::OsString::from_vec(vec![255]));
    std::fs::write(&path, b"a").unwrap();
    let alias = directory.path().join("alias");
    std::os::unix::fs::symlink(&path, &alias).unwrap();
    let error = seal(&alias, 1).unwrap_err();
    assert!(
        matches!(error, Error::Path { path: refused, detail:"canonical path requires lossless UTF-8 representation" } if refused == path.canonicalize().unwrap())
    );
}

/// Files standing in for the native executable, the one it hands runs to,
/// and a third, with the rule for runs of the first.
fn executables() -> (tempfile::TempDir, Executables, PathBuf, PathBuf, PathBuf) {
    let directory = tempfile::tempdir().unwrap();
    let path = |name: &str| {
        let path = directory.path().join(name);
        std::fs::write(&path, name).unwrap();
        path.canonicalize().unwrap()
    };
    let (native, sibling, third) = (path("native"), path("sibling"), path("third"));
    let rule = Executables::new(&seal(&native, 1024).unwrap(), 1024);
    (directory, rule, native, sibling, third)
}

#[test]
fn a_statistics_object_names_its_executable_by_key() {
    let named = |statistics: serde_json::Value| super::named(&statistics);
    assert_eq!(named(serde_json::json!({})), Named::Omitted);
    assert_eq!(
        named(serde_json::json!({"executable": null})),
        Named::Unresolved
    );
    assert_eq!(
        named(serde_json::json!({"executable": "/opt/zetesis"})),
        Named::Path("/opt/zetesis".into())
    );
}

#[test]
fn an_executable_value_that_is_not_a_path_reads_as_unresolved() {
    // No zetesis build writes these; another executable or a corrupted report
    // could, and such a run must not count as the native executable.
    let named = |executable: serde_json::Value| {
        super::named(&serde_json::json!({ "executable": executable }))
    };
    for value in [
        serde_json::json!(42),
        serde_json::json!(true),
        serde_json::json!(["/opt/zetesis"]),
        serde_json::json!({}),
    ] {
        assert_eq!(named(value.clone()), Named::Unresolved, "{value}");
    }
}

#[test]
fn statistics_that_are_not_an_object_read_as_no_statistics() {
    let named = |statistics: serde_json::Value| super::named(&statistics);
    assert_eq!(named(serde_json::Value::Null), Named::NoStatistics);
    assert_eq!(named(serde_json::json!("statistics")), Named::NoStatistics);
}

#[test]
fn a_run_whose_report_has_no_statistics_is_refused() {
    // Counting it as the native executable would attribute a handed-off GPU
    // run to the front.
    let (_directory, mut rule, _, _, _) = executables();
    let refusal = rule.settle(&Named::NoStatistics).unwrap_err();
    assert!(refusal.contains("no statistics"), "{refusal}");
}

#[test]
fn runs_naming_the_native_executable_or_none_seal_nothing() {
    let (_directory, mut rule, native, _, _) = executables();
    for named in [Named::Path(native), Named::Omitted] {
        rule.settle(&named).unwrap();
    }
    let (sealed, after) = rule.close();
    assert!(sealed.is_none() && after.is_none());
}

#[test]
fn a_run_that_cannot_name_its_executable_is_refused() {
    // After a hand-off the run that cannot name itself is the sibling, so it
    // cannot count as the native executable.
    let (_directory, mut rule, _, _, _) = executables();
    let refusal = rule.settle(&Named::Unresolved).unwrap_err();
    assert!(refusal.contains("could not name"), "{refusal}");
}

#[test]
fn runs_alternating_with_one_other_executable_seal_it_once() {
    let (_directory, mut rule, native, sibling, _) = executables();
    for named in [
        Named::Path(native.clone()),
        Named::Path(sibling.clone()),
        Named::Path(native),
        Named::Path(sibling.clone()),
    ] {
        rule.settle(&named).unwrap();
    }
    let (sealed, after) = rule.close();
    let sealed = sealed.unwrap();
    assert_eq!(sealed.canonical(), sibling);
    assert_eq!(sealed.bytes(), "sibling".len() as u64);
    assert!(after.unwrap().unchanged());
}

#[test]
fn a_run_naming_a_third_executable_is_refused() {
    let (_directory, mut rule, native, sibling, third) = executables();
    rule.settle(&Named::Path(sibling.clone())).unwrap();
    let refusal = rule.settle(&Named::Path(third.clone())).unwrap_err();
    for path in [&third, &sibling, &native] {
        assert!(refusal.contains(&path.display().to_string()), "{refusal}");
    }
    assert_eq!(rule.close().0.unwrap().canonical(), sibling);
}

#[test]
fn an_executable_that_cannot_be_sealed_is_refused() {
    let (directory, mut rule, _, _, _) = executables();
    let refusal = rule
        .settle(&Named::Path(directory.path().join("missing")))
        .unwrap_err();
    assert!(refusal.contains("could not be sealed"), "{refusal}");
}

#[test]
fn a_relative_executable_path_reads_as_unresolved() {
    // A relative path would resolve against this process's working directory,
    // not the run's, so it names no executable this rule can check.
    let named = |executable: &str| super::named(&serde_json::json!({ "executable": executable }));
    for executable in ["zetesis", "bin/zetesis", "./zetesis", ""] {
        assert_eq!(named(executable), Named::Unresolved, "{executable:?}");
    }
}

/// A link named `name` in `directory` to `target`.
#[cfg(unix)]
fn link(directory: &tempfile::TempDir, name: &str, target: &Path) -> PathBuf {
    let alias = directory.path().join(name);
    std::os::unix::fs::symlink(target, &alias).unwrap();
    alias
}

#[cfg(unix)]
#[test]
fn a_link_to_the_native_executable_names_the_native_one() {
    let (directory, mut rule, native, _, _) = executables();
    let alias = link(&directory, "native-alias", &native);
    rule.settle(&Named::Path(alias)).unwrap();
    let (sealed, after) = rule.close();
    assert!(sealed.is_none() && after.is_none());
}

#[cfg(unix)]
#[test]
fn runs_naming_one_executable_through_a_link_settle_to_it() {
    // Comparing the named path itself would refuse the second run as naming a
    // third executable.
    let (directory, mut rule, _, sibling, _) = executables();
    let alias = link(&directory, "sibling-alias", &sibling);
    for named in [alias.clone(), alias, sibling.clone()] {
        rule.settle(&Named::Path(named)).unwrap();
    }
    assert_eq!(rule.close().0.unwrap().canonical(), sibling);
}
