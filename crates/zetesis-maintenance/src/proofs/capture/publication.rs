//! Exclusive record publication with local renames and recoverable originals.
use super::{Result, bytes, copy_flat};
use std::{
    error::Error,
    fmt, fs,
    path::{Path, PathBuf},
};

pub(super) struct Lock {
    path: PathBuf,
    held: bool,
}
impl Lock {
    pub(super) fn acquire(root: &Path) -> Result<Self> {
        let path = root.join("verification/.capture-lock");
        fs::create_dir(&path)?;
        Ok(Self { path, held: true })
    }
    pub(super) fn release(mut self) -> Result<()> {
        fs::remove_dir(&self.path)?;
        self.held = false;
        Ok(())
    }
}
impl Drop for Lock {
    fn drop(&mut self) {
        if self.held {
            let _ = fs::remove_dir(&self.path);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    Prepared,
    CurrentMoved,
    CurrentInstalled,
    AuditMoved,
    AuditInstalled,
    RecordMoved,
    RecordInstalled,
    Verified,
}

#[derive(Debug)]
struct RecoveryFailure {
    cause: Box<dyn Error>,
    recovery: Box<dyn Error>,
    archive: PathBuf,
}
impl fmt::Display for RecoveryFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "publication failed: {}; rollback also failed: {}; original archive remains at {}",
            self.cause,
            self.recovery,
            self.archive.display()
        )
    }
}
impl Error for RecoveryFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.cause.as_ref())
    }
}

/// A committed generation whose local old-generation storage can now be removed.
pub(super) struct Published {
    holder: PathBuf,
}
impl Published {
    pub(super) fn cleanup(self) -> Result<()> {
        Ok(fs::remove_dir_all(self.holder)?)
    }
}

struct Transaction<'a> {
    root: &'a Path,
    holder: PathBuf,
    moved: [bool; 3],
}
impl Transaction<'_> {
    fn rollback(&self) -> Result<()> {
        for (index, name) in [
            "verification.json",
            "axiom-audit.txt",
            "verification/current",
        ]
        .into_iter()
        .enumerate()
        {
            let slot = 2 - index;
            if self.moved[slot] {
                let target = self.root.join(name);
                let original = self.holder.join(format!("previous-{slot}"));
                if slot == 0 && target.exists() {
                    fs::remove_dir_all(&target)?;
                }
                fs::rename(original, target)?;
            }
        }
        Ok(())
    }
    fn install(
        &mut self,
        slot: usize,
        name: &str,
        checkpoint: &mut impl FnMut(Step) -> Result<()>,
    ) -> Result<()> {
        let target = self.root.join(name);
        fs::rename(&target, self.holder.join(format!("previous-{slot}")))?;
        self.moved[slot] = true;
        checkpoint([Step::CurrentMoved, Step::AuditMoved, Step::RecordMoved][slot])?;
        fs::rename(self.holder.join(format!("next-{slot}")), &target)?;
        checkpoint(
            [
                Step::CurrentInstalled,
                Step::AuditInstalled,
                Step::RecordInstalled,
            ][slot],
        )
    }
}

pub(super) fn publish(
    stage: &Path,
    root: &Path,
    archive: &Path,
    validate: impl FnOnce() -> Result<()>,
) -> Result<Published> {
    publish_with(stage, root, archive, validate, |_| Ok(()))
}

fn publish_with(
    stage: &Path,
    root: &Path,
    archive: &Path,
    validate: impl FnOnce() -> Result<()>,
    mut checkpoint: impl FnMut(Step) -> Result<()>,
) -> Result<Published> {
    let holder = root.join("verification/.publication");
    fs::create_dir(&holder)?;
    let mut transaction = Transaction {
        root,
        holder,
        moved: [false; 3],
    };
    let attempt = (|| {
        copy_flat(
            &stage.join("verification/current"),
            &transaction.holder.join("next-0"),
        )?;
        fs::write(
            transaction.holder.join("next-1"),
            bytes(&stage.join("axiom-audit.txt"))?,
        )?;
        fs::write(
            transaction.holder.join("next-2"),
            bytes(&stage.join("verification.json"))?,
        )?;
        checkpoint(Step::Prepared)?;
        transaction.install(0, "verification/current", &mut checkpoint)?;
        transaction.install(1, "axiom-audit.txt", &mut checkpoint)?;
        transaction.install(2, "verification.json", &mut checkpoint)?;
        validate()?;
        checkpoint(Step::Verified)
    })();
    if let Err(cause) = attempt {
        if let Err(recovery) = transaction.rollback() {
            return Err(RecoveryFailure {
                cause,
                recovery,
                archive: archive.into(),
            }
            .into());
        }
        // Keep the original failure if discarding this owned scratch also fails.
        // The external archive remains authoritative for manual cleanup/recovery.
        let _ = fs::remove_dir_all(&transaction.holder);
        return Err(cause);
    }
    Ok(Published {
        holder: transaction.holder,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proofs::capture::require;
    use std::collections::BTreeMap;

    fn generation(root: &Path, content: &str) {
        fs::create_dir_all(root.join("verification/current")).unwrap();
        fs::write(root.join("verification/current/build.log"), content).unwrap();
        fs::write(root.join("axiom-audit.txt"), content).unwrap();
        fs::write(root.join("verification.json"), content).unwrap();
    }
    fn snapshot(root: &Path) -> BTreeMap<&'static str, Vec<u8>> {
        [
            "verification/current/build.log",
            "axiom-audit.txt",
            "verification.json",
        ]
        .into_iter()
        .map(|name| (name, fs::read(root.join(name)).unwrap()))
        .collect()
    }

    #[test]
    fn every_publication_step_restores_the_original_generation() {
        for failed in [
            Step::Prepared,
            Step::CurrentMoved,
            Step::CurrentInstalled,
            Step::AuditMoved,
            Step::AuditInstalled,
            Step::RecordMoved,
            Step::RecordInstalled,
            Step::Verified,
        ] {
            let directory = tempfile::tempdir().unwrap();
            let root = directory.path().join("root");
            let stage = directory.path().join("stage");
            let archive = directory.path().join("archive");
            generation(&root, "original");
            generation(&stage, "replacement");
            generation(&archive, "original");
            let before = snapshot(&root);
            let result = publish_with(
                &stage,
                &root,
                &archive,
                || Ok(()),
                |step| require(step != failed, "injected publication failure"),
            );
            assert!(result.is_err(), "{failed:?}");
            assert_eq!(snapshot(&root), before, "{failed:?}");
            assert_eq!(snapshot(&archive), before, "{failed:?}");
            assert!(!root.join("verification/.publication").exists());
        }
    }

    #[test]
    fn failed_final_validation_restores_the_original_generation() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("root");
        let stage = directory.path().join("stage");
        generation(&root, "original");
        generation(&stage, "replacement");
        let before = snapshot(&root);
        assert!(
            publish(&stage, &root, directory.path(), || Err(
                "validation failure".into()
            ))
            .is_err()
        );
        assert_eq!(snapshot(&root), before);
    }

    #[test]
    fn successful_publication_retains_the_original_archive() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("root");
        let stage = directory.path().join("stage");
        let archive = directory.path().join("archive");
        generation(&root, "original");
        generation(&archive, "original");
        generation(&stage, "replacement");
        let before = snapshot(&root);
        let published = publish(&stage, &root, &archive, || {
            require(
                snapshot(&root) == snapshot(&stage),
                "wrong published content",
            )
        })
        .unwrap();
        published.cleanup().unwrap();
        assert_eq!(snapshot(&root), snapshot(&stage));
        assert_eq!(snapshot(&archive), before);
    }

    #[test]
    fn failed_rollback_retains_both_causes_and_originals() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("root");
        let stage = directory.path().join("stage");
        let archive = directory.path().join("archive");
        generation(&root, "original");
        generation(&archive, "original");
        generation(&stage, "replacement");
        let before = snapshot(&archive);
        let failure = publish_with(
            &stage,
            &root,
            &archive,
            || Ok(()),
            |step| {
                if step == Step::AuditMoved {
                    // A directory prevents restoring the former regular audit file.
                    fs::create_dir(root.join("axiom-audit.txt"))?;
                    return Err("injected publication failure".into());
                }
                Ok(())
            },
        )
        .err()
        .unwrap();
        let recovery = failure.downcast_ref::<RecoveryFailure>().unwrap();
        assert_eq!(recovery.cause.to_string(), "injected publication failure");
        assert!(recovery.recovery.downcast_ref::<std::io::Error>().is_some());
        assert_eq!(recovery.archive, archive);
        assert_eq!(snapshot(&archive), before);
        assert_eq!(
            fs::read(root.join("verification/.publication/previous-1")).unwrap(),
            b"original"
        );
    }

    #[test]
    fn a_second_capture_cannot_take_an_existing_lock() {
        let directory = tempfile::tempdir().unwrap();
        generation(directory.path(), "original");
        let lock = Lock::acquire(directory.path()).unwrap();
        assert!(Lock::acquire(directory.path()).is_err());
        lock.release().unwrap();
        assert!(Lock::acquire(directory.path()).is_ok());
    }
}
