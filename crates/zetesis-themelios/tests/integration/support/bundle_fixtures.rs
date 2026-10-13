//! Source bundles written into a temporary directory and entered at
//! `entry.lp`.

use std::fs;
use std::path::{Path, PathBuf};

use zetesis_themelios::{
    AdmittedBundle, BundleAdmissionFailure, BundleAdmissionOptions, BundleLimits, ExpansionLimits,
    SourceBundle, admit_bundle_extended,
};

/// A temporary directory of bundle sources.
pub(crate) struct Fixture {
    directory: tempfile::TempDir,
}

impl Fixture {
    pub(crate) fn new() -> Self {
        Self {
            directory: tempfile::tempdir().expect("fixture directory"),
        }
    }

    /// Write `source` as `name`, a path relative to the directory, creating
    /// its parent directories.
    pub(crate) fn write(&self, name: &str, source: &str) {
        let path = self.path(name);
        fs::create_dir_all(path.parent().expect("fixture parent")).expect("fixture parents");
        fs::write(path, source).expect("original fixture source");
    }

    /// The directory the sources are written in.
    pub(crate) fn directory(&self) -> &Path {
        self.directory.path()
    }

    /// The path of `name` within the directory.
    pub(crate) fn path(&self, name: &str) -> PathBuf {
        self.directory.path().join(name)
    }

    /// The bundle entered at `entry.lp`, loaded under the default limits.
    pub(crate) fn bundle(&self) -> SourceBundle {
        SourceBundle::load(self.path("entry.lp"), BundleLimits::default())
            .expect("loaded source graph")
    }

    /// The bundle admitted under the default options and expansion limits.
    pub(crate) fn admit(&self) -> Result<AdmittedBundle, BundleAdmissionFailure> {
        admit_bundle_extended(
            self.bundle(),
            BundleAdmissionOptions::default(),
            ExpansionLimits::default(),
        )
    }
}
