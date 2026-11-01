use super::*;
use std::io::Write;

#[test]
fn a_second_publication_cannot_replace_the_first_artifact() {
    let slot = tempfile::tempdir().unwrap();
    for (contents, first) in [("first", true), ("second", false)] {
        let (guard, mut file) = Publication::new(slot.path()).unwrap();
        file.write_all(contents.as_bytes()).unwrap();
        drop(file);
        assert_eq!(guard.publish().is_ok(), first);
    }
    assert_eq!(fs::read(slot.path().join(ARTIFACT_NAME)).unwrap(), b"first");
    assert!(!slot.path().join("samples.partial").exists());
}

#[test]
fn a_failed_offer_keeps_another_invocations_staging_file() {
    let slot = tempfile::tempdir().unwrap();
    let (owner, file) = Publication::new(slot.path()).unwrap();
    assert!(Publication::new(slot.path()).is_err());
    assert!(owner.staging.exists());
    drop(file);
    drop(owner);
    assert!(!slot.path().join("samples.partial").exists());
}
