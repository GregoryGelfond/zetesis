//! Adversarial contracts for the private corpus boundary.

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
