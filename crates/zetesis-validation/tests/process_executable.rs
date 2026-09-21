//! Executable selection receives its search path without mutating the environment.

use std::path::{Path, PathBuf};
use zetesis_validation::process::resolve_executable;

fn candidate(directory: &Path, name: &str) -> PathBuf {
    let path = directory.join(name);
    std::fs::write(&path, b"fixture; selection never executes it").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    path
}

#[test]
fn search_preserves_the_first_eligible_candidate() {
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    let expected = candidate(first.path(), "solver");
    candidate(second.path(), "solver");
    let search = std::env::join_paths([first.path(), second.path()]).unwrap();
    assert_eq!(
        resolve_executable(Path::new("solver"), Some(&search)).unwrap(),
        expected
    );
}

#[test]
#[cfg(unix)]
fn search_skips_a_non_executable_regular_file() {
    use std::os::unix::fs::PermissionsExt as _;
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    let refused = candidate(first.path(), "solver");
    std::fs::set_permissions(refused, std::fs::Permissions::from_mode(0o600)).unwrap();
    let expected = candidate(second.path(), "solver");
    let search = std::env::join_paths([first.path(), second.path()]).unwrap();
    assert_eq!(
        resolve_executable(Path::new("solver"), Some(&search)).unwrap(),
        expected
    );
}

#[test]
fn search_skips_a_directory_with_the_requested_name() {
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    std::fs::create_dir(first.path().join("solver")).unwrap();
    let expected = candidate(second.path(), "solver");
    let search = std::env::join_paths([first.path(), second.path()]).unwrap();
    assert_eq!(
        resolve_executable(Path::new("solver"), Some(&search)).unwrap(),
        expected
    );
}

#[test]
fn explicit_path_does_not_fall_back_to_search() {
    let directory = tempfile::tempdir().unwrap();
    candidate(directory.path(), "solver");
    let search = std::env::join_paths([directory.path()]).unwrap();
    let requested = directory.path().join("missing/solver");
    assert_eq!(
        resolve_executable(&requested, Some(&search)).unwrap(),
        requested
    );
}

#[test]
fn relative_explicit_path_uses_the_callers_directory() {
    let requested = Path::new("./missing/solver");
    assert_eq!(
        resolve_executable(requested, None).unwrap(),
        std::path::absolute(requested).unwrap()
    );
}

#[test]
fn bare_name_requires_a_supplied_search_path() {
    let error = resolve_executable(Path::new("solver"), None).unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
}

#[test]
#[cfg(windows)]
fn windows_search_retains_the_executable_extension_fallback() {
    let directory = tempfile::tempdir().unwrap();
    let expected = candidate(directory.path(), "solver.exe");
    let search = std::env::join_paths([directory.path()]).unwrap();
    assert_eq!(
        resolve_executable(Path::new("solver"), Some(&search)).unwrap(),
        expected
    );
}
