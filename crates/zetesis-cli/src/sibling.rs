//! The default install's front: a CPU-only executable that runs every CPU
//! invocation itself and hands each invocation that opens a GPU device to the
//! GPU-capable sibling installed with it. The sibling lives in
//! `zetesis-libexec/<identity>/zetesis` beside the front, where the identity is
//! the sibling's content, compiled into the front by the installer: a front can
//! find only the sibling built with it. The sibling's file is named `zetesis`,
//! so the process keeps its name after the hand-off.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::{Invocation, RunError};

/// The directory beside the front that holds its sibling, one subdirectory per
/// install identity. The installer composes the same path.
pub(crate) const LIBEXEC: &str = "zetesis-libexec";

/// The sibling's file name, the front's own, so that the process keeps its
/// name when the front hands off.
const EXECUTABLE: &str = "zetesis";

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

/// Replace this process with `sibling`, passing `arguments`, the program name
/// first, unchanged. It returns only if the replacement failed, with the
/// failure; standard input is untouched either way.
pub(crate) fn exec(sibling: &Path, arguments: &[OsString]) -> RunError {
    use std::os::unix::process::CommandExt as _;
    let mut command = std::process::Command::new(sibling);
    if let Some((program, rest)) = arguments.split_first() {
        command.arg0(program).args(rest);
    }
    RunError::GpuExecutableUnavailable {
        path: sibling.to_owned(),
        error: command.exec(),
    }
}

#[cfg(test)]
mod tests;
