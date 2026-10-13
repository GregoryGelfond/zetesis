//! The default install's front: a CPU-only executable that runs every CPU
//! invocation itself and hands each invocation that opens a GPU device to the
//! GPU-capable sibling installed with it. The sibling lives in
//! `zetesis-libexec/<identity>/zetesis` beside the front, where the identity is
//! the sibling's content, compiled into the front by the installer: a front can
//! find only the sibling built with it. The sibling's file is named `zetesis`,
//! so the process keeps its name after the hand-off.
//!
//! Only the call site in the process entry and the identity's read are
//! compiled by the front build alone; everything here also builds under test.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{Invocation, RunError};

/// The directory beside the front that holds its sibling, one subdirectory per
/// install identity. The installer composes the same path.
pub(crate) const LIBEXEC: &str = "zetesis-libexec";

/// The sibling's file name, the front's own, so that the process keeps its
/// name when the front hands off.
const EXECUTABLE: &str = "zetesis";

/// Whether `identity` names one directory inside [`LIBEXEC`]: it is nonempty,
/// holds no `/` and is neither `.` nor `..`. The front asserts this of its
/// compiled identity, so its sibling's path cannot leave [`LIBEXEC`].
pub(crate) const fn names_one_directory(identity: &str) -> bool {
    let bytes = identity.as_bytes();
    if bytes.is_empty() || matches!(bytes, b"." | b"..") {
        return false;
    }
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'/' {
            return false;
        }
        index += 1;
    }
    true
}

/// Whether this process would open a GPU device for `invocation`: a solve whose
/// backend is a GPU, and `devices`. Test commands run their GPU work in child
/// processes of the current executable, each of which decides for itself.
pub(crate) fn opens_device(invocation: &Invocation) -> bool {
    match invocation {
        Invocation::Solve(options) => options.backend.is_gpu(),
        Invocation::Devices => true,
        Invocation::Test(_) => false,
    }
}

/// The sibling of the front at `front`, from the install of `identity`.
pub(crate) fn sibling(front: &Path, identity: &str) -> PathBuf {
    front
        .parent()
        .unwrap_or(Path::new(""))
        .join(LIBEXEC)
        .join(identity)
        .join(EXECUTABLE)
}

/// The command that runs `sibling` in place of the front: the front's program
/// name, the first of `arguments`, as its `arg0`, and the rest unchanged.
pub(crate) fn prepared(sibling: &Path, arguments: &[OsString]) -> Command {
    use std::os::unix::process::CommandExt as _;
    let mut command = Command::new(sibling);
    if let Some((program, rest)) = arguments.split_first() {
        command.arg0(program).args(rest);
    }
    command
}

/// Hand `invocation` to this executable's sibling of `identity` when it opens
/// a GPU device; see [`hand_off_from`].
pub(crate) fn hand_off(
    invocation: &Invocation,
    arguments: &[OsString],
    identity: &str,
) -> Option<RunError> {
    hand_off_from(
        invocation,
        arguments,
        crate::statistics::this_executable(),
        identity,
    )
}

/// Hand `invocation` to the sibling of `identity` beside `front` when it opens
/// a GPU device, returning only the failure of a hand-off: one that succeeds
/// replaces the process, keeping its standard streams untouched. `front` is
/// the front's resolved path, or why it could not be resolved.
pub(crate) fn hand_off_from(
    invocation: &Invocation,
    arguments: &[OsString],
    front: std::io::Result<PathBuf>,
    identity: &str,
) -> Option<RunError> {
    if !opens_device(invocation) {
        return None;
    }
    let front = match front {
        Ok(front) => front,
        Err(error) => return Some(RunError::GpuExecutableUnavailable { path: None, error }),
    };
    let path = sibling(&front, identity);
    let error = std::os::unix::process::CommandExt::exec(&mut prepared(&path, arguments));
    Some(RunError::GpuExecutableUnavailable {
        path: Some(path),
        error,
    })
}

#[cfg(test)]
mod tests;
