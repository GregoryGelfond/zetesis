//! Current-build library selection, independent of Cargo or rustdoc execution.
use serde_json::{Value, json};
use std::{fs, path::Path};
use zetesis_maintenance::book::{self, Limits};

fn artifact(name: &str, kind: &str, paths: &[&Path]) -> Value {
    json!({
        "reason": "compiler-artifact", "target": {"name": name, "kind": [kind]},
        "filenames": paths, "fresh": true
    })
}
fn stream(records: &[Value]) -> Vec<u8> {
    let mut output = Vec::new();
    let completion = json!({"reason":"build-finished","success":true});
    for record in records.iter().chain(std::iter::once(&completion)) {
        serde_json::to_writer(&mut output, record).unwrap();
        output.push(b'\n');
    }
    output
}

#[test]
fn stale_build_libraries_do_not_enter_the_view() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    let old = root.join("libsolver-old.rlib");
    let current = root.join("libsolver-current.rlib");
    fs::write(&old, b"old").unwrap();
    fs::write(&current, b"current").unwrap();
    let messages = stream(&[artifact("solver", "lib", &[&current])]);
    let selected = book::select(&messages, &["solver"], Limits::default()).unwrap();
    let view = root.join("view");
    selected.publish(&root, &view).unwrap();
    assert_eq!(fs::read_dir(&view).unwrap().count(), 1);
    assert_eq!(
        fs::read(view.join("libsolver-current.rlib")).unwrap(),
        b"current"
    );
    assert_eq!(fs::read(&old).unwrap(), b"old");
}

#[test]
fn compiler_helpers_are_not_rustdoc_libraries() {
    let messages = stream(&[
        artifact(
            "solver",
            "lib",
            &[
                Path::new("/build/libsolver.rlib"),
                Path::new("/build/libsolver.rmeta"),
            ],
        ),
        artifact("derive", "proc-macro", &[Path::new("/build/libderive.so")]),
        artifact(
            "build-script",
            "custom-build",
            &[Path::new("/build/build-script")],
        ),
    ]);
    let selected = book::select(&messages, &["solver"], Limits::default()).unwrap();
    assert_eq!(
        selected.paths(),
        [
            Path::new("/build/libderive.so"),
            Path::new("/build/libsolver.rlib")
        ]
    );
}

#[test]
fn incomplete_builds_cannot_establish_a_library_set() {
    let current = artifact("solver", "lib", &[Path::new("/build/libsolver.rlib")]);
    for messages in [
        format!("{current}\n"),
        format!(
            "{current}\n{}\n",
            json!({"reason":"build-finished","success":false})
        ),
        format!(
            "{}\n{current}\n",
            json!({"reason":"build-finished","success":true})
        ),
    ] {
        assert!(book::select(messages.as_bytes(), &["solver"], Limits::default()).is_err());
    }
}

#[test]
fn required_crates_must_have_library_artifacts() {
    let messages = stream(&[artifact("solver", "bin", &[Path::new("/build/solver")])]);
    assert!(book::select(&messages, &["solver"], Limits::default()).is_err());
    let messages = stream(&[artifact(
        "other",
        "lib",
        &[Path::new("/build/libother.rlib")],
    )]);
    assert!(book::select(&messages, &["solver"], Limits::default()).is_err());
}

#[test]
fn selection_limits_are_inclusive() {
    let messages = stream(&[artifact(
        "solver",
        "lib",
        &[Path::new("/build/libsolver.rlib")],
    )]);
    let exact = Limits {
        message_bytes: messages.len(),
        artifacts: 1,
    };
    assert!(book::select(&messages, &["solver"], exact).is_ok());
    assert!(
        book::select(
            &messages,
            &["solver"],
            Limits {
                message_bytes: messages.len() - 1,
                ..exact
            }
        )
        .is_err()
    );
    assert!(
        book::select(
            &messages,
            &["solver"],
            Limits {
                artifacts: 0,
                ..exact
            }
        )
        .is_err()
    );
}

#[test]
fn outside_artifacts_leave_no_partial_view() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    let build = root.join("build");
    fs::create_dir(&build).unwrap();
    let outside = root.join("libsolver.rlib");
    fs::write(&outside, b"outside").unwrap();
    let messages = stream(&[artifact("solver", "lib", &[&outside])]);
    let selected = book::select(&messages, &["solver"], Limits::default()).unwrap();
    let view = root.join("view");
    assert!(selected.publish(&build, &view).is_err());
    assert!(!view.exists());
}

#[test]
fn conflicting_basenames_leave_no_partial_view() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    let first = root.join("one/libsolver.rlib");
    let second = root.join("two/libsolver.rlib");
    for path in [&first, &second] {
        fs::create_dir(path.parent().unwrap()).unwrap();
        fs::write(path, b"library").unwrap();
    }
    let messages = stream(&[artifact("solver", "lib", &[&first, &second])]);
    let selected = book::select(&messages, &["solver"], Limits::default()).unwrap();
    let view = root.join("view");
    assert!(selected.publish(&root, &view).is_err());
    assert!(!view.exists());
}

#[test]
fn existing_views_are_not_overwritten() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    let current = root.join("libsolver.rlib");
    fs::write(&current, b"current").unwrap();
    let messages = stream(&[artifact("solver", "lib", &[&current])]);
    let selected = book::select(&messages, &["solver"], Limits::default()).unwrap();
    let view = root.join("view");
    fs::create_dir(&view).unwrap();
    fs::write(view.join("preserved"), b"prior").unwrap();
    assert!(selected.publish(&root, &view).is_err());
    assert_eq!(fs::read_dir(&view).unwrap().count(), 1);
    assert_eq!(fs::read(view.join("preserved")).unwrap(), b"prior");
}

#[cfg(unix)]
#[test]
fn symbolic_artifacts_leave_no_partial_view() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    let real = root.join("libreal.rlib");
    let alias = root.join("libsolver.rlib");
    fs::write(&real, b"library").unwrap();
    std::os::unix::fs::symlink(&real, &alias).unwrap();
    let messages = stream(&[artifact("solver", "lib", &[&alias])]);
    let selected = book::select(&messages, &["solver"], Limits::default()).unwrap();
    let view = root.join("view");
    assert!(selected.publish(&root, &view).is_err());
    assert!(!view.exists());
}
