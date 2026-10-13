//! Source bundles written into a temporary directory and entered at
//! `entry.lp`.

use std::fs;
use std::path::{Path, PathBuf};

use zetesis_themelios::{BundleLimits, SourceBundle};

/// A temporary directory of bundle sources, entered at `entry.lp`; it is
/// removed when the value is dropped.
pub struct BundleFixture {
    directory: tempfile::TempDir,
}

impl BundleFixture {
    /// An empty bundle directory.
    ///
    /// # Panics
    /// Panics if the temporary directory cannot be created.
    #[must_use]
    pub fn new() -> Self {
        Self {
            directory: tempfile::tempdir().expect("bundle fixture directory"),
        }
    }

    /// Write `source` as `name`, a path relative to the directory, creating
    /// its parent directories.
    ///
    /// # Panics
    /// Panics if a directory or the file cannot be written.
    pub fn write(&self, name: &str, source: &str) {
        let path = self.path(name);
        fs::create_dir_all(path.parent().expect("a bundle source names a file"))
            .expect("bundle source directories");
        fs::write(path, source).expect("bundle source");
    }

    /// The path of `name` within the directory.
    #[must_use]
    pub fn path(&self, name: &str) -> PathBuf {
        self.directory.path().join(name)
    }

    /// The directory the sources are written in.
    #[must_use]
    pub fn directory(&self) -> &Path {
        self.directory.path()
    }

    /// The bundle entered at `entry.lp`, loaded under the default limits.
    ///
    /// # Panics
    /// Panics if the bundle does not load.
    #[must_use]
    pub fn bundle(&self) -> SourceBundle {
        SourceBundle::load(self.path("entry.lp"), BundleLimits::default())
            .expect("loaded source graph")
    }

    /// Remove the directory now, for a test that checks the removal.
    ///
    /// # Panics
    /// Panics if the directory cannot be removed.
    pub fn close(self) {
        self.directory.close().expect("bundle fixture removal");
    }
}

impl Default for BundleFixture {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_written_bundle_loads_from_its_entry() {
        let fixture = BundleFixture::new();
        fixture.write("entry.lp", "#include \"nested/child.lp\".\n");
        fixture.write("nested/child.lp", "p.\n");
        assert!(fixture.path("nested/child.lp").is_file());
        assert_eq!(fixture.bundle().sources().len(), 2);
    }

    #[test]
    fn a_closed_fixture_removes_its_directory() {
        let fixture = BundleFixture::new();
        fixture.write("entry.lp", "p.\n");
        let directory = fixture.directory().to_owned();
        fixture.close();
        assert!(!directory.exists());
    }
}
