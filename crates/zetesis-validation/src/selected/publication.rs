//! New-file report publication under an explicit exclusive-parent assumption.
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};

use super::{Error, FileSeal, Report, identity};

#[derive(Debug)]
pub(super) struct Destination {
    path: PathBuf,
    root: PathBuf,
    protected: Vec<PathBuf>,
}

pub(super) fn prepare(path: &Path, root: &Path, inputs: &[FileSeal]) -> Result<Destination, Error> {
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

struct Counter {
    bytes: usize,
    limit: usize,
    exceeded: bool,
}
impl Write for Counter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self
            .bytes
            .checked_add(bytes.len())
            .is_none_or(|value| value > self.limit)
        {
            self.exceeded = true;
            return Err(io::Error::other("report byte ceiling"));
        }
        self.bytes += bytes.len();
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub(super) fn write(report: &Report, destination: &Destination, limit: usize) -> Result<(), Error> {
    validate(destination)?;
    let report = super::view::Published {
        passed: report.passed(),
        report,
    };
    let mut size = Counter {
        bytes: 0,
        limit,
        exceeded: false,
    };
    let encoded = serde_json::to_writer(&mut size, &report)
        .and_then(|()| size.write_all(b"\n").map_err(serde_json::Error::io));
    if size.exceeded {
        return Err(Error::Bytes {
            path: destination.path.clone(),
            limit,
        });
    }
    encoded.map_err(Error::Json)?;
    let parent = destination
        .path
        .parent()
        .expect("prepared report has a canonical parent");
    let mut temporary =
        tempfile::NamedTempFile::new_in(parent).map_err(|source| identity::io(parent, source))?;
    let written = serde_json::to_writer(&mut temporary, &report)
        .map_err(Error::Json)
        .and_then(|()| {
            temporary
                .write_all(b"\n")
                .map_err(|source| identity::io(temporary.path(), source))
        })
        .and_then(|()| {
            temporary
                .flush()
                .map_err(|source| identity::io(temporary.path(), source))
        });
    if let Err(primary) = written {
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
