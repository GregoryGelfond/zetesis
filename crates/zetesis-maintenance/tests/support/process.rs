//! Bounded Linux/macOS capture for the commands authored by these tests.
//!
//! The adapter transfers arguments, working directory and individual environment
//! changes through `/usr/bin/env`. The restricted builder cannot clear the whole
//! environment or configure streams: neither operation is part of this contract.
//! The validation library owns process groups, capture polling and cleanup.

use std::{
    env,
    ffi::{OsStr, OsString},
    os::unix::ffi::OsStrExt,
    path::Path,
    time::Duration,
};
use zetesis_validation::process::{self, Capture, Exit, Invocation, Limits, Stop};

const CASE_DEADLINE: Duration = Duration::from_secs(20);
const CAPTURE_BYTES: usize = 4 * 1024 * 1024;
const CLEANUP_DEADLINE: Duration = Duration::from_secs(1);

/// Bytes and exit evidence from an uninterrupted, completely captured child.
pub struct Output {
    /// Reaped direct-child exit status.
    pub status: Status,
    /// Complete standard output within the combined capture ceiling.
    pub stdout: Vec<u8>,
    /// Complete standard error within the combined capture ceiling.
    pub stderr: Vec<u8>,
}

/// A direct-child exit; capture completion does not imply exit success.
pub struct Status(Exit);

impl Status {
    /// Whether the child exited normally with code zero.
    #[must_use]
    pub fn success(&self) -> bool {
        self.code() == Some(0) && self.0.signal.is_none()
    }

    /// Normal exit code, absent for termination by a signal.
    #[must_use]
    pub fn code(&self) -> Option<i32> {
        self.0.code
    }
}

/// Restricted process builder whose settings are all transferred by this adapter.
#[derive(Debug)]
pub struct Command(std::process::Command);

impl Command {
    /// Select an executable; an unqualified name is resolved by `env` using PATH.
    ///
    /// # Panics
    /// Refuses empty names and `=` in executable names, which `env` would parse
    /// as another environment assignment instead of an executable.
    #[must_use]
    pub fn new(program: impl AsRef<OsStr>) -> Self {
        let program = program.as_ref();
        assert!(!program.is_empty() && !program.as_bytes().contains(&b'='));
        Self(std::process::Command::new(program))
    }

    /// Append one literal argument without shell interpolation.
    pub fn arg(&mut self, argument: impl AsRef<OsStr>) -> &mut Self {
        self.0.arg(argument);
        self
    }

    /// Append literal arguments without shell interpolation.
    pub fn args<I, S>(&mut self, arguments: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.0.args(arguments);
        self
    }

    /// Set the absolute working directory used by the capture library.
    pub fn current_dir(&mut self, directory: impl AsRef<Path>) -> &mut Self {
        self.0.current_dir(directory);
        self
    }

    /// Set one environment variable for this child only.
    ///
    /// # Panics
    /// Refuses empty keys or keys containing `=` or NUL.
    pub fn env(&mut self, key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> &mut Self {
        check_key(key.as_ref());
        self.0.env(key, value);
        self
    }

    /// Set environment variables for this child only.
    ///
    /// # Panics
    /// Refuses empty keys or keys containing `=` or NUL.
    pub fn envs<I, K, V>(&mut self, variables: I) -> &mut Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        for (key, value) in variables {
            self.env(key, value);
        }
        self
    }

    /// Remove an inherited environment variable for this child only.
    ///
    /// # Panics
    /// Refuses empty keys or keys containing `=` or NUL.
    pub fn env_remove(&mut self, key: impl AsRef<OsStr>) -> &mut Self {
        check_key(key.as_ref());
        self.0.env_remove(key);
        self
    }

    /// Capture within 20 seconds and four MiB, with explicit bounded cleanup.
    ///
    /// # Panics
    /// Fails the test on start refusal, interrupted capture or unresolved cleanup.
    pub fn bounded_output(&mut self) -> Output {
        let capture = capture(
            self,
            Limits {
                timeout: CASE_DEADLINE,
                max_output_bytes: CAPTURE_BYTES,
                cleanup_timeout: CLEANUP_DEADLINE,
            },
        );
        assert_eq!(capture.stop(), Stop::Completed, "{self:?}: {capture:?}");
        Output {
            status: Status(
                capture
                    .exit()
                    .expect("completed capture has a reaped child"),
            ),
            stdout: capture.stdout().to_vec(),
            stderr: capture.stderr().to_vec(),
        }
    }
}

/// Retain the library outcome, including stops, under the supplied capture limits.
/// Cleanup is retried once if necessary; this establishes no descendant guarantee.
///
/// # Panics
/// Fails the test on start refusal or failed/unresolved cleanup retry. Initial
/// cleanup failures remain in the returned capture. Any abandoned direct-child
/// ownership is explicitly identified in the failure message.
#[must_use]
pub fn capture(command: &Command, limits: Limits) -> Capture {
    let mut arguments = Vec::new();
    for (key, value) in command.0.get_envs() {
        if value.is_none() {
            arguments.extend([OsString::from("-u"), key.to_owned()]);
        }
    }
    arguments.push("--".into());
    for (key, value) in command.0.get_envs() {
        if let Some(value) = value {
            let mut assignment = key.to_owned();
            assignment.push("=");
            assignment.push(value);
            arguments.push(assignment);
        }
    }
    arguments.push(command.0.get_program().to_owned());
    arguments.extend(command.0.get_args().map(std::ffi::OsStr::to_owned));
    let directory = command
        .0
        .get_current_dir()
        .map_or_else(|| env::current_dir().unwrap(), Path::to_path_buf);
    let outcome = process::invoke(
        Invocation {
            executable: Path::new("/usr/bin/env"),
            arguments: &arguments,
            directory: &directory,
        },
        limits,
    )
    .unwrap_or_else(|error| panic!("{command:?}: {error}"));
    let (capture, pending) = outcome.into_parts();
    if let Some(child) = pending {
        let cleanup = child.retry(CLEANUP_DEADLINE);
        if let Some(child) = cleanup.pending {
            let id = child.abandon();
            panic!(
                "unreaped test child {id}; capture={capture:?}; cleanup={:?}",
                cleanup.failure
            );
        }
        assert!(cleanup.failure.is_none(), "{cleanup:?}");
    }
    capture
}

fn check_key(key: &OsStr) {
    assert!(!key.is_empty() && !key.as_bytes().iter().any(|byte| matches!(byte, b'=' | 0)));
}
