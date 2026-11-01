//! Exclusive, optional evidence publication inside one private benchmark slot.
use std::fs::{self, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use zetesis_validation::residual_evidence::{ARTIFACT_NAME, REQUEST_NAME, Request};

pub(super) struct Publication {
    staging: PathBuf,
    destination: PathBuf,
}

impl Publication {
    pub(super) fn open(path: &Path, session: &mut crate::Session<'_>) -> io::Result<Self> {
        if path.file_name().is_none_or(|name| name != REQUEST_NAME) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "unexpected residual evidence request name",
            ));
        }
        let request = Request::read(path)?;
        let dir = path.parent().expect("request has a filename");
        let (publication, file) = Self::new(dir)?;
        session.request_residual_evidence(file, request.nonce, request.max_bytes)?;
        Ok(publication)
    }

    fn new(dir: &Path) -> io::Result<(Self, std::fs::File)> {
        let staging = dir.join("samples.partial");
        // The slot belongs to the requesting benchmark. Never remove an
        // existing staging path belonging to another invocation.
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staging)?;
        Ok((
            Self {
                staging,
                destination: dir.join(ARTIFACT_NAME),
            },
            file,
        ))
    }

    pub(super) fn publish(self) -> io::Result<()> {
        // An ordinary rename could overwrite a previous artifact. Linking a
        // complete same-directory staging file publishes its contents atomically
        // and fails if the final name already exists. Drop retires our own link.
        fs::hard_link(&self.staging, &self.destination)
    }
}

impl Drop for Publication {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.staging);
    }
}

#[cfg(test)]
mod tests;
