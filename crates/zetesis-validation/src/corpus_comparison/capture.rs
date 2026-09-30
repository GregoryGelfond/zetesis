//! The process capture of this platform. On Linux and macOS it owns the
//! child's process group; elsewhere the portable adapter captures a direct
//! child. The portable adapter also compiles in Unix test builds, where its
//! tests exercise it: it must preserve the same cancellation meaning.

#[cfg(any(not(any(target_os = "linux", target_os = "macos")), test))]
mod portable;
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod unix;

#[cfg(all(test, unix, not(any(target_os = "linux", target_os = "macos"))))]
pub(super) use portable::invoke;
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub(super) use portable::{Capture, invoke_with_cancellation};
#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
pub(super) use unix::invoke;
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(super) use unix::{Capture, invoke_with_cancellation};
