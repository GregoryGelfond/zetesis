//! Shell scripts a test writes and runs as child processes.

use std::path::Path;

/// `path` quoted for a POSIX shell: in single quotes, with each single quote
/// it holds closed, escaped and reopened.
///
/// # Panics
/// Panics if `path` is not valid Unicode.
#[must_use]
pub fn quote(path: &Path) -> String {
    let text = path.to_str().expect("a Unicode test path");
    format!("'{}'", text.replace('\'', "'\\''"))
}

/// Write the shell script `body` to `path`, executable by its owner alone.
///
/// # Panics
/// Panics if the script cannot be written or its permissions set.
#[cfg(unix)]
pub fn executable(path: &Path, body: &str) {
    use std::os::unix::fs::PermissionsExt as _;

    std::fs::write(path, format!("#!/bin/sh\n{body}\n")).expect("a writable test script");
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
        .expect("the test script's permissions");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_quoted_path_closes_escapes_and_reopens_its_single_quotes() {
        assert_eq!(quote(Path::new("/tmp/it's")), r"'/tmp/it'\''s'");
    }
}
