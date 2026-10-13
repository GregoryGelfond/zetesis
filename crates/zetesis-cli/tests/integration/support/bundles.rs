//! Source bundles written into a temporary directory and entered at
//! `entry.lp`, and the CLI run on them.

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

use zetesis_themelios::{BundleLimits, SourceBundle};

/// A temporary directory of bundle sources.
pub(crate) struct Fixture(tempfile::TempDir);

impl Fixture {
    pub(crate) fn new() -> Self {
        Self(tempfile::tempdir().expect("temporary fixture"))
    }

    /// Write `source` as `name` in the directory.
    pub(crate) fn write(&self, name: &str, source: &str) {
        fs::write(self.path(name), source).unwrap();
    }

    /// The path of `name` within the directory.
    pub(crate) fn path(&self, name: &str) -> PathBuf {
        self.0.path().join(name)
    }

    /// The bundle entered at `entry.lp`, loaded under the default limits.
    pub(crate) fn bundle(&self) -> SourceBundle {
        SourceBundle::load(self.path("entry.lp"), BundleLimits::default()).unwrap()
    }

    /// The CLI run on `entry.lp` on the CPU, enumerating every model, with
    /// `arguments` before the input and no standard input.
    pub(crate) fn process(&self, arguments: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_zetesis"))
            .args(["--backend", "cpu", "--models", "0"])
            .args(arguments)
            .arg(self.path("entry.lp"))
            .stdin(Stdio::null())
            .output()
            .unwrap()
    }
}
