//! The installer's phases, driven with stand-in build tools.
//!
//! A temporary checkout holds the real `scripts/install.sh` and a manifest.
//! Stand-ins for `cargo`, `rustc`, `uname` and `install` come first on `PATH`:
//! the stand-in `cargo` writes a distinct stand-in executable per package and
//! profile, naming the source and the identity it was built with, and fails a
//! numbered build of the run on request; the stand-in `install` runs the
//! host's own and fails writing a named file on request. The SHA-256 tool and the file
//! utilities are the host's. The stand-in `uname` reports Linux, because the
//! macOS load-command guard reads `/usr/bin/otool`, which no stand-in can
//! replace: every default install on macOS drives that guard instead.

use crate::support::process::{Command, Output};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

const CARGO: &str = r#"#!/bin/sh
set -eu
count=$(cat "$INSTALL_TEST_ROOT/builds" 2>/dev/null || printf 0)
count=$((count + 1))
printf '%s\n' "$count" > "$INSTALL_TEST_ROOT/builds"
if [ "${INSTALL_TEST_FAIL_BUILD:-}" = "$count" ]; then
    printf '%s\n' "stand-in cargo: build $count fails" >&2
    exit 101
fi
profile=full
target=
directory=
packages=
while [ "$#" -gt 0 ]; do
    case "$1" in
        --target) target=$2; shift ;;
        --target-dir) directory=$2; shift ;;
        --no-default-features) if [ "$profile" = full ]; then profile=cpu-only; fi ;;
        --features) if [ "$2" = front ]; then profile=front; fi; shift ;;
        -p) packages="$packages $2"; shift ;;
    esac
    shift
done
mkdir -p "$directory/$target/release"
for package in $packages; do
    case "$package" in
        zetesis-cli) name=zetesis ;;
        *) name=$package ;;
    esac
    printf '#!/bin/sh\n# %s %s source=%s identity=%s\n' "$profile" "$name" \
        "${INSTALL_TEST_SOURCE:-}" "${ZETESIS_SIBLING_IDENTITY:-}" \
        > "$directory/$target/release/$name"
    chmod 755 "$directory/$target/release/$name"
done
"#;

const RUSTC: &str = "#!/bin/sh\nprintf '%s\\n' 'rustc 1.97.1' 'host: test-host'\n";

const UNAME: &str = "#!/bin/sh\nprintf '%s\\n' Linux\n";

const INSTALL: &str = r#"#!/bin/sh
for last in "$@"; do :; done
if [ "$last" = "${INSTALL_TEST_FAIL_PLACING:-}" ]; then
    printf '%s\n' "stand-in install: writing $last fails" >&2
    exit 1
fi
exec /usr/bin/install "$@"
"#;

/// A temporary checkout with stand-in build tools, and the binary directory
/// it installs into.
struct Installer {
    directory: tempfile::TempDir,
}

impl Installer {
    fn new() -> Self {
        let directory = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
        let installer = Self { directory };
        installer.write(
            "scripts/install.sh",
            include_str!("../../../../scripts/install.sh"),
        );
        installer.write("Cargo.toml", "[workspace.package]\nversion = \"0.5.0\"\n");
        for (tool, script) in [
            ("cargo", CARGO),
            ("rustc", RUSTC),
            ("uname", UNAME),
            ("install", INSTALL),
        ] {
            installer.write(&format!("tools/{tool}"), script);
            installer.executable(&format!("tools/{tool}"));
        }
        installer
    }

    fn root(&self) -> &Path {
        self.directory.path()
    }

    fn installed(&self) -> PathBuf {
        self.root().join("installed")
    }

    fn write(&self, path: &str, text: &str) {
        let path = self.root().join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn executable(&self, path: &str) {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(self.root().join(path), fs::Permissions::from_mode(0o755)).unwrap();
    }

    /// Run the installer on the source named `source`, with `options` and the
    /// stand-ins' failure settings in `environment`.
    fn run(&self, source: &str, options: &[&str], environment: &[(&str, &str)]) -> Output {
        // The stand-in cargo numbers the builds of one run.
        let _ = fs::remove_file(self.root().join("builds"));
        let mut command = Command::new("/bin/sh");
        command
            .arg("scripts/install.sh")
            .args(options)
            .arg(self.installed())
            .current_dir(self.root());
        for (key, _) in std::env::vars_os() {
            let name = key.to_string_lossy();
            if name.starts_with("INSTALL_TEST_")
                || ["ZETESIS_SIBLING_IDENTITY", "CARGO_BUILD_TARGET"].contains(&name.as_ref())
            {
                command.env_remove(key);
            }
        }
        let mut paths = vec![self.root().join("tools")];
        paths.extend(std::env::split_paths(&std::env::var_os("PATH").unwrap()));
        command
            .env("PATH", std::env::join_paths(paths).unwrap())
            .env("CARGO_TARGET_DIR", self.root().join("target"))
            .env("INSTALL_TEST_ROOT", self.root())
            .env("INSTALL_TEST_SOURCE", source);
        for (key, value) in environment {
            command.env(key, value);
        }
        command.bounded_output()
    }

    /// Install `source` with the default options, which must succeed.
    fn install(&self, source: &str) {
        let output = self.run(source, &[], &[]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    /// Every file under the binary directory, by relative path, with its text.
    fn files(&self) -> BTreeMap<String, String> {
        fn visit(directory: &Path, root: &Path, files: &mut BTreeMap<String, String>) {
            for entry in fs::read_dir(directory).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    visit(&path, root, files);
                } else {
                    let relative = path.strip_prefix(root).unwrap().display().to_string();
                    files.insert(relative, fs::read_to_string(&path).unwrap());
                }
            }
        }
        let mut files = BTreeMap::new();
        visit(&self.installed(), &self.installed(), &mut files);
        files
    }

    /// The identity directories under `zetesis-libexec`.
    fn identities(&self) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(self.installed().join("zetesis-libexec"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }
}

/// The stand-in executable the stand-in `cargo` builds.
fn built(profile: &str, name: &str, source: &str, identity: &str) -> String {
    format!("#!/bin/sh\n# {profile} {name} source={source} identity={identity}\n")
}

/// The identity of the sibling built from `source`: the version, a hyphen and
/// the first sixteen hexadecimal digits of the sibling's SHA-256.
fn identity(source: &str) -> String {
    let digest = format!("{:x}", Sha256::digest(built("full", "zetesis", source, "")));
    format!("0.5.0-{}", &digest[..16])
}

#[test]
fn a_default_install_places_the_front_the_bench_and_the_sibling() {
    let installer = Installer::new();
    let output = installer.run("a", &[], &[]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let id = identity("a");
    let expected = BTreeMap::from([
        ("zetesis".to_owned(), built("front", "zetesis", "a", &id)),
        (
            "zetesis-bench".to_owned(),
            built("full", "zetesis-bench", "a", ""),
        ),
        (
            format!("zetesis-libexec/{id}/zetesis"),
            built("full", "zetesis", "a", ""),
        ),
    ]);
    assert_eq!(installer.files(), expected);
    let report = String::from_utf8_lossy(&output.stdout);
    assert!(
        report.contains(&format!("zetesis-libexec/{id}/zetesis")),
        "{report}"
    );
}

#[test]
fn reinstalling_the_same_source_keeps_its_one_identity_directory() {
    let installer = Installer::new();
    installer.install("a");
    let first = installer.files();
    installer.install("a");
    assert_eq!(installer.identities(), [identity("a")]);
    assert_eq!(installer.files(), first);
}

#[test]
fn a_new_identity_removes_only_the_identity_directories_it_supersedes() {
    let installer = Installer::new();
    installer.install("a");
    // A directory and a file not named as an identity is: the installer never
    // made them, so it leaves them alone.
    fs::create_dir(installer.installed().join("zetesis-libexec/notes")).unwrap();
    fs::write(installer.installed().join("zetesis-libexec/README"), "kept").unwrap();
    installer.install("b");
    let mut expected = vec!["README".to_owned(), identity("b"), "notes".to_owned()];
    expected.sort();
    assert_eq!(installer.identities(), expected);
}

#[test]
fn a_failed_build_leaves_the_previous_install_unchanged() {
    for build in ["1", "2"] {
        let installer = Installer::new();
        installer.install("a");
        let previous = installer.files();
        let output = installer.run("b", &[], &[("INSTALL_TEST_FAIL_BUILD", build)]);
        assert!(!output.status.success(), "build {build}");
        assert_eq!(installer.files(), previous, "build {build}");
    }
}

#[test]
fn a_failure_placing_the_front_leaves_the_previous_front_with_its_sibling() {
    let installer = Installer::new();
    installer.install("a");
    // The front's temporary name in the binary directory.
    let front = installer.installed().join(".zetesis.installing");
    let output = installer.run(
        "b",
        &[],
        &[("INSTALL_TEST_FAIL_PLACING", front.to_str().unwrap())],
    );
    assert!(!output.status.success());
    let files = installer.files();
    let previous = identity("a");
    assert_eq!(
        files["zetesis"],
        built("front", "zetesis", "a", &previous),
        "the previous front stays"
    );
    assert!(
        files.contains_key(&format!("zetesis-libexec/{previous}/zetesis")),
        "the previous front's sibling stays"
    );
    // The new sibling and zetesis-bench were placed before the front.
    assert!(files.contains_key(&format!("zetesis-libexec/{}/zetesis", identity("b"))));
    assert_eq!(
        files["zetesis-bench"],
        built("full", "zetesis-bench", "b", "")
    );
}

#[test]
fn a_cpu_only_install_removes_the_sibling_directory() {
    let installer = Installer::new();
    installer.install("a");
    let output = installer.run("a", &["--cpu-only"], &[]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let expected = BTreeMap::from([
        ("zetesis".to_owned(), built("cpu-only", "zetesis", "a", "")),
        (
            "zetesis-bench".to_owned(),
            built("cpu-only", "zetesis-bench", "a", ""),
        ),
    ]);
    assert_eq!(installer.files(), expected);
}
