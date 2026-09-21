//! Executable selection before the process owner receives an absolute path.

use std::ffi::OsStr;
use std::io;
use std::path::{Path, PathBuf};

/// Resolve an explicit path or the first eligible executable in a supplied PATH.
///
/// Absolute paths and paths containing directory components bypass search and
/// are made absolute against the caller's working directory. A bare name uses
/// only `search_path`, in its declared order; this function does not read PATH.
/// Search candidates must be regular files, with an execute bit on Unix. Other
/// platforms have no portable execute-bit check. Windows also tries `.exe` for
/// names without an extension, matching direct executable invocation; no shell
/// or PATHEXT interpretation is performed.
///
/// Selection is metadata evidence, not a guarantee that a later spawn succeeds.
/// Permissions, loaders or files can change; process startup retains those
/// failures. Symlinks are followed for eligibility, but the returned path is not
/// canonicalized. Search visits at most two candidates per PATH entry, uses
/// O(maximum candidate path length) scratch, and does not read executable contents.
///
/// # Errors
/// Returns path-resolution failure or `NotFound` when no candidate is eligible.
/// Missing/inaccessible PATH candidates are skipped; an explicit path is left
/// for the process owner to validate at spawn.
pub fn resolve_executable(executable: &Path, search_path: Option<&OsStr>) -> io::Result<PathBuf> {
    if executable.is_absolute() || executable.components().count() != 1 {
        return std::path::absolute(executable);
    }
    if let Some(search_path) = search_path {
        for directory in std::env::split_paths(search_path) {
            let candidate = directory.join(executable);
            if eligible(&candidate) {
                return std::path::absolute(candidate);
            }
            #[cfg(windows)]
            if candidate.extension().is_none() {
                let executable = candidate.with_extension("exe");
                if eligible(&executable) {
                    return std::path::absolute(executable);
                }
            }
        }
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!(
            "executable not found on supplied PATH: {}",
            executable.display()
        ),
    ))
}

fn eligible(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|metadata| {
        if !metadata.is_file() {
            return false;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            metadata.permissions().mode() & 0o111 != 0
        }
        #[cfg(not(unix))]
        {
            true
        }
    })
}
