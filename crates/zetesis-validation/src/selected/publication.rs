//! New-file report publication under an explicit exclusive-parent assumption.
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};

use super::{Error, FileSeal, identity};

#[derive(Debug)]
pub(crate) struct Destination {
    path: PathBuf,
    root: PathBuf,
    protected: Vec<PathBuf>,
}

pub(crate) fn prepare(path: &Path, root: &Path, inputs: &[FileSeal]) -> Result<Destination, Error> {
    let requested = identity::absolute(path)?;
    let mut lexical = PathBuf::new();
    for component in requested.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                lexical.pop();
            }
            component => lexical.push(component.as_os_str()),
        }
    }
    if lexical.starts_with(root) {
        return Err(Error::Path {
            path: requested,
            detail: "report must be outside the curated input directory",
        });
    }
    let parent = requested.parent().ok_or_else(|| Error::Path {
        path: requested.clone(),
        detail: "report has no parent",
    })?;
    let name = requested.file_name().ok_or_else(|| Error::Path {
        path: requested.clone(),
        detail: "report has no filename",
    })?;
    let parent = std::fs::canonicalize(parent).map_err(|source| identity::io(parent, source))?;
    let path = parent.join(name);
    let protected = inputs
        .iter()
        .flat_map(|seal| [seal.requested().to_owned(), seal.canonical().to_owned()])
        .collect();
    let result = Destination {
        path,
        root: root.to_owned(),
        protected,
    };
    validate(&result)?;
    Ok(result)
}

fn validate(destination: &Destination) -> Result<(), Error> {
    let path = &destination.path;
    if path.starts_with(&destination.root)
        || destination.protected.iter().any(|input| input == path)
    {
        return Err(Error::Path {
            path: path.clone(),
            detail: "report aliases a protected input",
        });
    }
    // Refuse every existing file, hard link, symlink, dangling symlink or directory.
    // A later race cannot overwrite one: publication also uses persist_noclobber.
    match std::fs::symlink_metadata(path) {
        Ok(_) => Err(Error::Path {
            path: path.clone(),
            detail: "report destination already exists",
        }),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(identity::io(path, error)),
    }
}

struct Counter<W> {
    inner: W,
    bytes: usize,
    limit: usize,
    exceeded: bool,
}
impl<W: Write> Write for Counter<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self
            .bytes
            .checked_add(bytes.len())
            .is_none_or(|value| value > self.limit)
        {
            self.exceeded = true;
            return Err(io::Error::other("report byte ceiling"));
        }
        let written = self.inner.write(bytes)?;
        self.bytes += written;
        Ok(written)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

// Both passes enforce the same inclusive bound. `Serialize` is not required to
// produce identical bytes on successive calls, so preflight alone is insufficient.
fn encode(
    report: &impl serde::Serialize,
    writer: impl Write,
    path: &Path,
    limit: usize,
) -> Result<(), Error> {
    let mut bounded = Counter {
        inner: writer,
        bytes: 0,
        limit,
        exceeded: false,
    };
    let encoded = serde_json::to_writer(&mut bounded, report)
        .map_err(Error::Json)
        .and_then(|()| {
            bounded
                .write_all(b"\n")
                .map_err(|source| identity::io(path, source))
        })
        .and_then(|()| bounded.flush().map_err(|source| identity::io(path, source)));
    if bounded.exceeded {
        Err(Error::Bytes {
            path: path.to_owned(),
            limit,
        })
    } else {
        encoded
    }
}

pub(crate) fn write(
    report: &impl serde::Serialize,
    destination: &Destination,
    limit: usize,
) -> Result<(), Error> {
    validate(destination)?;
    encode(report, io::sink(), &destination.path, limit)?;
    let parent = destination
        .path
        .parent()
        .expect("prepared report has a canonical parent");
    let mut temporary =
        tempfile::NamedTempFile::new_in(parent).map_err(|source| identity::io(parent, source))?;
    if let Err(primary) = encode(report, &mut temporary, &destination.path, limit) {
        return cleanup(temporary, primary);
    }
    if let Err(primary) = validate(destination) {
        return cleanup(temporary, primary);
    }
    match temporary.persist_noclobber(&destination.path) {
        Ok(_) => Ok(()),
        Err(error) => cleanup(error.file, identity::io(&destination.path, error.error)),
    }
}

fn cleanup(temporary: tempfile::NamedTempFile, primary: Error) -> Result<(), Error> {
    match temporary.close() {
        Ok(()) => Err(primary),
        Err(cleanup) => Err(Error::Cleanup {
            primary: Box::new(primary),
            cleanup,
        }),
    }
}

#[cfg(test)]
#[path = "../../tests/support/selected_publication.rs"]
mod tests;
