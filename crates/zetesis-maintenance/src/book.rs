//! A current Cargo artifact view for the manual's Rust examples.
//!
//! Incremental build directories may contain old libraries. Selection uses only
//! one successful Cargo JSON message stream, never a directory glob or mtime.
//! Publishing hard-links that set into a fresh directory without copying library
//! payloads. The caller must keep the build artifacts immutable while rustdoc
//! uses the view; this is not a concurrent filesystem snapshot or compiler seal.
use crate::{Error, files, json, require};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

/// Inclusive input and retained-path limits; these are not allocator RSS limits.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Serialized Cargo message bytes.
    pub message_bytes: usize,
    /// Distinct selected library paths.
    pub artifacts: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            message_bytes: 16_777_216,
            artifacts: 16_384,
        }
    }
}

/// Libraries named by one successful build, including reused fresh artifacts.
#[derive(Debug)]
pub struct Libraries {
    paths: Vec<PathBuf>,
}
impl Libraries {
    /// Ordered selected paths, before filesystem confinement is checked.
    #[must_use]
    pub fn paths(&self) -> &[PathBuf] {
        &self.paths
    }

    /// Publish a fresh hard-linked view confined to `build_directory`.
    ///
    /// Validate every source and basename before creating `destination`. The
    /// destination's parent must exist on the same filesystem as the build.
    /// Filesystem failure during linking can leave a partial fresh view; never
    /// pass that view to rustdoc. Existing destinations are never overwritten.
    /// Work/storage are linear in path count after ordered basename lookup;
    /// artifact contents are neither read nor copied.
    ///
    /// # Errors
    /// Refuses missing/nonregular/symbolic-link sources, paths outside the build,
    /// conflicting basenames, existing destinations and filesystem failures.
    pub fn publish(&self, build_directory: &Path, destination: &Path) -> Result<(), Error> {
        let root = build_directory
            .canonicalize()
            .map_err(|error| files::io(build_directory, error))?;
        require(root.is_dir(), "book build root must be a directory")?;
        let mut sources = BTreeMap::new();
        for path in &self.paths {
            let metadata = fs::symlink_metadata(path).map_err(|error| files::io(path, error))?;
            require(metadata.is_file(), "book artifact must be a regular file")?;
            let canonical = path
                .canonicalize()
                .map_err(|error| files::io(path, error))?;
            require(
                canonical.starts_with(&root),
                "book artifact escapes the build directory",
            )?;
            let name = canonical
                .file_name()
                .ok_or_else(|| Error::Invalid("book artifact has no filename".into()))?
                .to_owned();
            require(
                sources.insert(name, canonical).is_none(),
                "conflicting book artifact basenames",
            )?;
        }
        fs::create_dir(destination).map_err(|error| files::io(destination, error))?;
        for (name, source) in sources {
            let target = destination.join(name);
            fs::hard_link(source, &target).map_err(|error| files::io(&target, error))?;
        }
        Ok(())
    }
}

/// Select rlibs and procedural-macro libraries from a complete Cargo build.
///
/// Each `required` crate name must have a selected library. Other current
/// libraries remain available for transitive rustdoc dependencies. Build scripts,
/// executables and metadata-only files are not libraries in this view. A final
/// successful `build-finished` event is required; it does not itself prove that
/// a caller executed Cargo or that the listed bytes came from this source.
/// Parsing is bounded by serialized bytes and selected paths; ordered sets add
/// logarithmic lookup cost. JSON nesting retains `serde_json`'s recursion bound.
///
/// # Errors
/// Refuses malformed/incomplete/failed streams, relative library paths, missing
/// required crates and exceeded inclusive input/path limits. No files are written.
pub fn select(messages: &[u8], required: &[&str], limits: Limits) -> Result<Libraries, Error> {
    require(!required.is_empty(), "book view needs required crate names")?;
    if messages.len() > limits.message_bytes {
        return Err(Error::Limit {
            resource: "Cargo message bytes",
            limit: limits.message_bytes,
        });
    }
    let mut paths = BTreeSet::new();
    let mut crates = BTreeSet::new();
    let mut finished = false;
    for line in messages
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
    {
        require(!finished, "Cargo messages follow build completion")?;
        let message = json::parse(line)?;
        match message["reason"].as_str() {
            Some("build-finished") => {
                require(message["success"] == true, "Cargo build did not succeed")?;
                finished = true;
            }
            Some("compiler-artifact") => {
                let name = json::string(&message["target"]["name"], "Cargo target name")?;
                let kinds = message["target"]["kind"]
                    .as_array()
                    .ok_or_else(|| Error::Invalid("missing Cargo target kinds".into()))?;
                let procedural = kinds.iter().any(|kind| kind == "proc-macro");
                let filenames = message["filenames"]
                    .as_array()
                    .ok_or_else(|| Error::Invalid("missing Cargo artifact filenames".into()))?;
                for filename in filenames {
                    let path = Path::new(json::string(filename, "Cargo artifact filename")?);
                    let suffix = path.extension().and_then(|extension| extension.to_str());
                    if suffix == Some("rlib")
                        || procedural && matches!(suffix, Some("so" | "dylib" | "dll"))
                    {
                        require(path.is_absolute(), "Cargo artifact path must be absolute")?;
                        paths.insert(path.to_owned());
                        if paths.len() > limits.artifacts {
                            return Err(Error::Limit {
                                resource: "book library paths",
                                limit: limits.artifacts,
                            });
                        }
                        crates.insert(name.to_owned());
                    }
                }
            }
            Some("compiler-message" | "build-script-executed") => {}
            _ => return Err(Error::Invalid("unsupported Cargo message kind".into())),
        }
    }
    require(finished, "Cargo build completion is missing")?;
    for name in required {
        require(
            crates.contains(*name),
            format!("missing book crate: {name}"),
        )?;
    }
    Ok(Libraries {
        paths: paths.into_iter().collect(),
    })
}
