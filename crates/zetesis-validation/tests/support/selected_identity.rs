//! Adversarial contracts for the private corpus boundary.

use super::*;

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
