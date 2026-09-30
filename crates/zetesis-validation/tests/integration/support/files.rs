//! Files a test copies into its fixture directories.

use std::fs;
use std::path::Path;

/// Copy `source` to `destination`, creating `destination`'s directory.
pub fn copy(source: &Path, destination: &Path) {
    fs::create_dir_all(destination.parent().unwrap()).unwrap();
    fs::copy(source, destination).unwrap();
}
