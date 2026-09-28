//! Optional unchanged-corpus admission check against a pinned external checkout.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use zetesis_themelios::{AdmissionFailure, AdmissionOptions, ProfileFeature, admit};

const REVISION: &str = "38f0660ded448ed268c5a68759ceb0e2840dd497";

fn programs(root: &Path) -> Vec<PathBuf> {
    let mut pending = vec![root.to_path_buf()];
    let mut paths = Vec::new();
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory).unwrap() {
            let entry = entry.unwrap();
            let kind = entry.file_type().unwrap();
            if kind.is_dir() && entry.file_name() != ".git" {
                pending.push(entry.path());
            } else if kind.is_file() && entry.path().extension().is_some_and(|ext| ext == "lp") {
                paths.push(entry.path());
            }
        }
    }
    paths.sort();
    paths
}

#[test]
#[ignore = "requires KR_DOMAINS_DIR pointing at the pinned, clean kr-domains checkout"]
fn pinned_original_sources_are_refused_without_partial_admission() {
    let root = PathBuf::from(std::env::var_os("KR_DOMAINS_DIR").expect("set KR_DOMAINS_DIR"));
    let revision = Command::new("git")
        .arg("-C")
        .arg(&root)
        .args(["rev-parse", "HEAD"])
        .output()
        .unwrap();
    assert!(revision.status.success());
    assert_eq!(String::from_utf8(revision.stdout).unwrap().trim(), REVISION);
    let status = Command::new("git")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .arg("-C")
        .arg(&root)
        .args(["status", "--porcelain", "--untracked-files=all"])
        .output()
        .unwrap();
    assert!(status.status.success());
    assert!(
        status.stdout.is_empty(),
        "corpus checkout must be unchanged"
    );
    let paths = programs(&root);
    assert_eq!(paths.len(), 155);
    let mut pools = 0;
    let mut statements = 0;
    let mut terms = 0;
    for path in paths {
        let source = fs::read_to_string(&path).unwrap();
        let error = admit(source, AdmissionOptions::default())
            .expect_err("no unchanged snapshot file fits S0");
        assert!(!error.diagnostics().is_empty());
        match error {
            AdmissionFailure::Profile {
                feature: ProfileFeature::PooledArguments,
                ..
            } => pools += 1,
            AdmissionFailure::Profile {
                feature: ProfileFeature::Statement,
                ..
            } => statements += 1,
            AdmissionFailure::Profile {
                feature: ProfileFeature::Term,
                ..
            } => terms += 1,
            other => panic!("unexpected boundary for {}: {other}", path.display()),
        }
    }
    assert_eq!((pools, statements, terms), (128, 17, 10));
}
