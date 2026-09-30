//! Separate fresh-helper observations of solver-child peak resident memory.
//!
//! The parent uses [`super::invoke_supervised`] to capture a fresh helper. The helper calls
//! [`measure`] exactly once, inheriting its parent's process group, stdout and
//! stderr. Thus the parent's deadline/output stop signals the helper and solver
//! together. The helper exclusively waits for the solver; the parent exclusively
//! waits for the helper. Neither uses another reaper or cumulative runner usage.
//! As with ordinary capture, this does not certify escaped descendant cleanup.

mod record;
pub use record::{RecordError, measure_to_file};

use std::ffi::OsString;
use std::fmt;
use std::io::{self, Write};
use std::process::ExitCode;

/// The first argument that makes a campaign's helper executable measure one
/// child: `HELPER RECORD EXECUTABLE [ARGUMENT…]`, dispatched by [`run_helper`].
pub const HELPER_COMMAND: &str = "__measure-child";

/// The measurement helper's whole behaviour, for the executable a campaign
/// launches as its helper.
///
/// `arguments` follow [`HELPER_COMMAND`]: the new record's path, then the
/// absolute executable to measure and its unmodified arguments. Measures that
/// one child from the current directory with [`measure_to_file`], in this
/// fresh process; the child's own output passes through. A missing argument or
/// a failed measurement is reported to `diagnostics` and exits with status 2.
pub fn run_helper(
    arguments: impl IntoIterator<Item = OsString>,
    diagnostics: &mut impl Write,
) -> ExitCode {
    let mut arguments = arguments.into_iter();
    let (Some(record), Some(executable)) = (arguments.next(), arguments.next()) else {
        let _ = writeln!(
            diagnostics,
            "usage: {HELPER_COMMAND} RECORD EXECUTABLE [ARGUMENT...]"
        );
        return ExitCode::from(2);
    };
    let child_arguments: Vec<OsString> = arguments.collect();
    let result = std::env::current_dir()
        .map_err(RecordError::Io)
        .and_then(|directory| {
            measure_to_file(
                super::Invocation {
                    executable: std::path::Path::new(&executable),
                    arguments: &child_arguments,
                    directory: &directory,
                },
                std::path::Path::new(&record),
            )
        });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _ = writeln!(diagnostics, "measurement helper: {error}");
            ExitCode::from(2)
        }
    }
}

use serde::{Deserialize, Serialize};

/// Native `ru_maxrss` unit, before the explicit conversion to bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Unit {
    /// macOS reports bytes.
    Bytes,
    /// Linux reports kibibytes.
    Kibibytes,
}

impl Unit {
    /// Native reporting unit on the current supported platform.
    #[must_use]
    pub const fn current() -> Option<Self> {
        if cfg!(target_os = "macos") {
            Some(Self::Bytes)
        } else if cfg!(target_os = "linux") {
            Some(Self::Kibibytes)
        } else {
            None
        }
    }
}

/// Resource evidence from one fresh helper's waited-for solver child.
///
/// `RUSAGE_CHILDREN` excludes the helper itself. The operating system may include
/// usage propagated from descendants reaped by the solver; this is neither a
/// simultaneous process-tree RSS sum nor device-memory measurement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Measurement {
    /// Format version for the private helper record.
    pub schema: u32,
    /// Solver process ID, distinct from the captured helper process.
    pub child: u32,
    /// Solver normal-exit code; absent on signal termination.
    pub exit_code: Option<i32>,
    /// Solver termination signal; absent on normal exit.
    pub signal: Option<i32>,
    /// Unconverted nonnegative `ru_maxrss` value.
    pub raw_max_rss: u64,
    /// Operating-system unit of the raw observation.
    pub raw_unit: Unit,
    /// Checked conversion of the raw observation to bytes.
    pub peak_rss_bytes: u64,
}

impl Measurement {
    /// Validate the record's version, exit evidence and unit conversion.
    #[must_use]
    pub fn valid(self) -> bool {
        self.schema == 1
            && self.child > 1
            && (self.exit_code.is_some() != self.signal.is_some())
            && self.exit_code.is_none_or(|code| (0..=255).contains(&code))
            && self.signal.is_none_or(|signal| signal > 0)
            && bytes(self.raw_max_rss, self.raw_unit) == Some(self.peak_rss_bytes)
    }
}

fn bytes(raw: u64, unit: Unit) -> Option<u64> {
    match unit {
        Unit::Bytes => Some(raw),
        Unit::Kibibytes => raw.checked_mul(1024),
    }
}

/// Failure to obtain one complete child usage observation.
#[derive(Debug)]
pub enum Error {
    /// Native units have not been qualified on this platform.
    UnsupportedPlatform,
    /// Executable or directory was not absolute.
    RelativePath,
    /// The supposedly fresh helper has already accumulated child usage.
    NotFresh,
    /// The OS could not start or wait for the child, or obtain its usage.
    Operation(io::Error),
    /// The OS returned unrepresentable usage or exit evidence.
    InvalidUsage,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedPlatform => f.write_str("child RSS units require macOS or Linux"),
            Self::RelativePath => {
                f.write_str("child RSS executable and directory must be absolute")
            }
            Self::NotFresh => f.write_str("child RSS helper has previous waited-for child usage"),
            Self::Operation(error) => write!(f, "child RSS operation: {error}"),
            Self::InvalidUsage => f.write_str("invalid child RSS or exit evidence"),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Operation(error) => Some(error),
            _ => None,
        }
    }
}

/// Run and wait for one solver in a fresh, separately bounded helper process.
///
/// Call only once in a fresh helper that starts no other children. This operation
/// deliberately inherits its process group and stdout/stderr. The supervising
/// process must apply [`super::invoke_supervised`]'s deadline and output bounds; calling this
/// directly in the campaign would mix cumulative usage and remove that bound.
/// The blocking wait has exactly one owner. No process tree or device RSS is
/// inferred. The returned elapsed helper execution must never enter timed runs.
///
/// # Errors
/// Returns unsupported platform, path, freshness, spawn/wait/usage or conversion
/// failures. A wait error requires the supervising process to terminate the
/// still-owned group; the helper cannot establish successful solver completion.
pub fn measure(invocation: super::Invocation<'_>) -> Result<Measurement, Error> {
    if !invocation.executable.is_absolute() || !invocation.directory.is_absolute() {
        return Err(Error::RelativePath);
    }
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        use nix::sys::resource::{UsageWho, getrusage};
        use std::os::unix::process::ExitStatusExt;
        use std::process::{Command, Stdio};

        let usage = || {
            getrusage(UsageWho::RUSAGE_CHILDREN)
                .map_err(|error| Error::Operation(io::Error::from(error)))
        };
        if usage()?.max_rss() != 0 {
            return Err(Error::NotFresh);
        }
        let mut child = Command::new(invocation.executable)
            .args(invocation.arguments)
            .current_dir(invocation.directory)
            .stdin(Stdio::null())
            .spawn()
            .map_err(Error::Operation)?;
        let id = child.id();
        let exit = child.wait().map_err(Error::Operation)?;
        let raw = u64::try_from(usage()?.max_rss()).map_err(|_| Error::InvalidUsage)?;
        let unit = if cfg!(target_os = "linux") {
            Unit::Kibibytes
        } else {
            Unit::Bytes
        };
        let measurement = Measurement {
            schema: 1,
            child: id,
            exit_code: exit.code(),
            signal: exit.signal(),
            raw_max_rss: raw,
            raw_unit: unit,
            peak_rss_bytes: bytes(raw, unit).ok_or(Error::InvalidUsage)?,
        };
        if !measurement.valid() {
            return Err(Error::InvalidUsage);
        }
        Ok(measurement)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        Err(Error::UnsupportedPlatform)
    }
}
