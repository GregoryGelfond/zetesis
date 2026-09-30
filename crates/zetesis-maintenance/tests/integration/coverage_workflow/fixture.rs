//! Isolated shell gates with deterministic Rust tool stand-ins.
use super::subprocess::{Command, Output};
use serde_json::{Value, json};
use std::{
    env, fs,
    path::{Path, PathBuf},
};

pub const TABLE: &str = include_str!("../../../src/coverage/physical-selection.txt");
pub const VULKAN_TABLE: &str = include_str!("../../../src/coverage/physical-selection-vulkan.txt");
use zetesis_maintenance::coverage::SUPPORT_SOURCES;
pub struct Fixture {
    pub directory: tempfile::TempDir,
}
impl Fixture {
    pub fn new() -> Self {
        let directory = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
        let fixture = Self { directory };
        for (name, bytes) in [
            (
                "coverage.sh",
                include_bytes!("../../../../../scripts/coverage.sh").as_slice(),
            ),
            (
                "check.sh",
                include_bytes!("../../../../../scripts/check.sh").as_slice(),
            ),
            (
                "maintenance.sh",
                include_bytes!("../../../../../scripts/maintenance.sh").as_slice(),
            ),
            (
                "coverage-ratchet.sh",
                include_bytes!("../../../../../scripts/coverage-ratchet.sh").as_slice(),
            ),
            (
                "hardware.sh",
                include_bytes!("../../../../../scripts/hardware.sh").as_slice(),
            ),
        ] {
            fixture.write(&format!("scripts/{name}"), bytes);
            fixture.executable(&format!("scripts/{name}"));
        }
        // The reviewed selections the hardware gate reads from the source tree.
        fixture.write(
            "crates/zetesis-maintenance/src/coverage/physical-selection.txt",
            TABLE.as_bytes(),
        );
        fixture.write(
            "crates/zetesis-maintenance/src/coverage/physical-selection-vulkan.txt",
            VULKAN_TABLE.as_bytes(),
        );
        fixture.write("scripts/coverage-floor.txt", b"91\n");
        fixture.write("target/coverage/status.txt", b"gate-passed\n");
        fixture.write("target/coverage/floors.tsv", b"workspace\t0\ncli-cpu\t0\n");
        fs::create_dir_all(fixture.root().join("proofs")).unwrap();
        for role in ["cargo", "rustc", "rustup", "mdbook", "lake", "clingo"] {
            fixture.tool(&format!("bin/{role}"), role);
        }
        for name in ["llvm-cov", "llvm-profdata"] {
            fixture.tool(&format!("sysroot/lib/rustlib/fake-host/bin/{name}"), "llvm");
        }
        for backend in BACKENDS {
            for fields in groups_of(backend) {
                fixture.write(
                    &format!(
                        "target/coverage/workspace/{backend}-{}-status.txt",
                        fields[0]
                    ),
                    b"passed\n",
                );
                fixture.write(
                    &format!("target/coverage/workspace/{backend}-{}.log", fields[0]),
                    b"old physical log\n",
                );
            }
            fixture.write(
                &format!("target/coverage/workspace/{backend}-status.txt"),
                b"passed\n",
            );
        }
        fixture
    }
    pub fn root(&self) -> &Path {
        self.directory.path()
    }
    pub fn write(&self, path: &str, bytes: &[u8]) {
        let path = self.root().join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }
    pub fn read(&self, path: &str) -> String {
        fs::read_to_string(self.root().join(path)).unwrap()
    }
    pub fn executable(&self, path: &str) {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(self.root().join(path), fs::Permissions::from_mode(0o700)).unwrap();
    }
    pub fn tool(&self, path: &str, role: &str) {
        let script = format!(
            "#!/bin/sh\nexec '{}' '{}' \"$@\"\n",
            env!("CARGO_BIN_EXE_zetesis-maintenance-fixture"),
            role
        );
        self.write(path, script.as_bytes());
        self.executable(path);
    }
    pub fn command(&self, script: &str) -> Command {
        let mut command = Command::new("/bin/sh");
        command.arg(script).current_dir(self.root());
        for (key, _) in env::vars_os() {
            let name = key.to_string_lossy();
            if name.starts_with("GIT_")
                || name.starts_with("COVERAGE_TEST_")
                || name.starts_with("CHECK_TEST_")
                || ["LLVM_COV", "LLVM_PROFDATA", "CARGO_LLVM_COV_TARGET_DIR"]
                    .contains(&name.as_ref())
            {
                command.env_remove(key);
            }
        }
        let mut paths = vec![self.root().join("bin")];
        paths.extend(env::split_paths(&env::var_os("PATH").unwrap()));
        command
            .env("PATH", env::join_paths(paths).unwrap())
            .env("CLINGO", self.root().join("bin/clingo"))
            .env(
                "ZETESIS_MAINTENANCE",
                env!("CARGO_BIN_EXE_zetesis-maintenance"),
            )
            .env("COVERAGE_TEST_SYSROOT", self.root().join("sysroot"))
            .env("COVERAGE_TEST_LOG", self.root().join("calls.jsonl"));
        command
    }
    /// Run the coverage script in `mode`, with the physical stage of
    /// `backend`, `metal` or `vulkan`, when one is named.
    pub fn coverage(&self, mode: &str, backend: Option<&str>, changes: &[(&str, &str)]) -> Output {
        let mut command = self.command("scripts/coverage.sh");
        command.arg(mode);
        for &(key, value) in changes {
            command.env(key, value);
        }
        if let Some(backend) = backend {
            command.arg(format!("--{backend}"));
        }
        command.bounded_output()
    }
    pub fn calls(&self) -> Vec<Value> {
        let path = self.root().join("calls.jsonl");
        if path.exists() {
            self.read("calls.jsonl")
                .lines()
                .map(|line| serde_json::from_str(line).unwrap())
                .collect()
        } else {
            Vec::new()
        }
    }
    pub fn assert_status(&self, expected: &str) {
        assert_eq!(self.read("target/coverage/status.txt").trim(), expected);
        assert!(!self.root().join("target/coverage/.lock").exists());
    }
    pub fn assert_preflight_failure(&self, result: &Output) {
        assert!(
            !result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        self.assert_status("incomplete");
        assert!(self.calls().is_empty());
        assert!(self.read("target/coverage/floors.tsv").is_empty());
    }
    pub fn assert_physical_failure(&self, backend: &str, group: &str) {
        self.assert_status("incomplete");
        assert!(
            self.root()
                .join("target/coverage/workspace/portable/coverage.json")
                .is_file()
        );
        assert!(
            !self
                .root()
                .join("target/coverage/workspace/coverage.json")
                .exists()
        );
        let mut failed = false;
        for fields in groups_of(backend) {
            failed |= fields[0] == group;
            let status = if failed { "incomplete\n" } else { "passed\n" };
            assert_eq!(
                self.read(&format!(
                    "target/coverage/workspace/{backend}-{}-status.txt",
                    fields[0]
                )),
                status
            );
            let log = self.read(&format!(
                "target/coverage/workspace/{backend}-{}.log",
                fields[0]
            ));
            assert!(!log.contains("old physical log"));
            if failed && fields[0] != group {
                assert!(log.is_empty());
            }
        }
        let mut expected = profile_prefix(self.root(), true);
        for fields in groups_of(backend) {
            expected.push(call("workspace", physical(&fields)));
            if fields[0] == group {
                break;
            }
        }
        assert_eq!(self.calls(), expected);
    }
    pub fn assert_complete_schedule(&self, mode: &str, backend: Option<&str>, floor: &str) {
        assert_eq!(
            self.read("target/coverage/floors.tsv"),
            if mode == "gate" {
                "workspace\t0\ncli-cpu\t0\n"
            } else {
                ""
            }
        );
        let mut expected = profile_prefix(self.root(), backend.is_some());
        if let Some(backend) = backend {
            for fields in groups_of(backend) {
                expected.push(call("workspace", physical(&fields)));
            }
        }
        expected.extend(reports(self.root(), "workspace"));
        expected.push(call(
            "cli-cpu",
            strings(&["clean", "--workspace", "--locked"]),
        ));
        expected.push(call(
            "cli-cpu",
            strings(&[
                "--package",
                "zetesis-cli",
                "--package",
                "zetesis-solve",
                "--no-default-features",
                "--locked",
                "--no-report",
            ]),
        ));
        expected.extend(reports(self.root(), "cli-cpu"));
        if mode == "gate" {
            expected.push(call(
                "workspace",
                strings(&[
                    "report",
                    "--locked",
                    "--ignore-filename-regex",
                    SUPPORT_SOURCES,
                    "--fail-under-lines",
                    floor,
                ]),
            ));
            expected.push(call(
                "cli-cpu",
                strings(&[
                    "report",
                    "--package",
                    "zetesis-cli",
                    "--package",
                    "zetesis-solve",
                    "--locked",
                    "--ignore-filename-regex",
                    SUPPORT_SOURCES,
                    "--fail-under-lines",
                    floor,
                ]),
            ));
        }
        assert_eq!(self.calls(), expected);
        self.assert_metadata(mode, backend, floor);
        for profile in ["workspace", "cli-cpu"] {
            for report in ["coverage.json", "html/index.html"] {
                assert!(
                    self.root()
                        .join(format!("target/coverage/{profile}/{report}"))
                        .is_file()
                );
            }
        }
        self.assert_status(if mode == "gate" {
            "gate-passed"
        } else {
            "baseline-complete (nongating)"
        });
    }

    /// The recorded coverage metadata of a completed run.
    fn assert_metadata(&self, mode: &str, backend: Option<&str>, floor: &str) {
        let metadata: Value =
            serde_json::from_str(&self.read("target/coverage/toolchain.json")).unwrap();
        assert_eq!(metadata["mode"], mode);
        assert_eq!(metadata["committed_floor"], floor);
        assert_eq!(metadata["profiles_merged"], false);
        assert_eq!(
            metadata["supplemental"],
            "--package zetesis-cli --package zetesis-solve --no-default-features"
        );
        assert_eq!(metadata["floor_profiles"], json!(["workspace", "cli-cpu"]));
        assert_eq!(
            metadata["project_added_filename_filters"],
            json!([SUPPORT_SOURCES])
        );
        let expected_groups: Vec<_> = if let Some(backend) = backend {
            groups_of(backend).iter().map(|fields|json!({"group":fields[0],"target_kind":if fields[1]=="lib"{"lib"}else{"test"},"target":if fields[1]=="lib"{"workspace libraries"}else{fields[1]},"tests":fields[3].split_whitespace().collect::<Vec<_>>(),"expected_tests":fields[2].parse::<usize>().unwrap()})).collect()
        } else {
            Vec::new()
        };
        assert_eq!(metadata["physical_test_groups"], json!(expected_groups));
        assert_eq!(
            metadata["workspace_stages"],
            match backend {
                Some(backend) => json!(["portable", backend]),
                None => json!(["portable"]),
            }
        );
        if let Some(backend) = backend {
            assert_eq!(
                self.read(&format!("target/coverage/workspace/{backend}-status.txt")),
                "passed\n"
            );
        }
    }
}
/// The backends a physical stage can run.
pub const BACKENDS: [&str; 2] = ["metal", "vulkan"];
pub fn groups() -> Vec<Vec<&'static str>> {
    TABLE.lines().map(|row| row.split('|').collect()).collect()
}
/// The reviewed selection's rows of `backend`.
pub fn groups_of(backend: &str) -> Vec<Vec<&'static str>> {
    match backend {
        "metal" => groups(),
        "vulkan" => vulkan_groups(),
        other => panic!("no reviewed selection for {other}"),
    }
}
/// Where the fixture keeps `backend`'s reviewed selection.
pub fn table_path(backend: &str) -> &'static str {
    match backend {
        "metal" => "crates/zetesis-maintenance/src/coverage/physical-selection.txt",
        "vulkan" => "crates/zetesis-maintenance/src/coverage/physical-selection-vulkan.txt",
        other => panic!("no reviewed selection for {other}"),
    }
}
pub fn vulkan_groups() -> Vec<Vec<&'static str>> {
    VULKAN_TABLE
        .lines()
        .map(|row| row.split('|').collect())
        .collect()
}
fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).into()).collect()
}
fn call(profile: &str, arguments: Vec<String>) -> Value {
    let mut args = strings(&["+1.97.1", "llvm-cov"]);
    args.extend(arguments);
    json!({"args":args,"profile":format!("build-{profile}")})
}
fn physical(fields: &[&str]) -> Vec<String> {
    let mut args = strings(&["--workspace", "--all-features"]);
    if fields[1] == "lib" {
        args.push("--lib".into());
    } else {
        args.extend(strings(&["--test", fields[1]]));
    }
    args.extend(strings(&[
        "--locked",
        "--no-report",
        "--",
        "--ignored",
        "--nocapture",
        "--test-threads=1",
        "--exact",
    ]));
    args.extend(fields[3].split_whitespace().map(str::to_owned));
    args
}
fn directory(root: &Path, profile: &str) -> PathBuf {
    root.join(format!("target/coverage/{profile}"))
}
fn report(profile: &str, directory: &Path, html: bool) -> Value {
    let mut args = vec!["report".into()];
    if profile == "cli-cpu" {
        args.extend(strings(&[
            "--package",
            "zetesis-cli",
            "--package",
            "zetesis-solve",
        ]));
    }
    args.extend(strings(&[
        "--locked",
        "--ignore-filename-regex",
        SUPPORT_SOURCES,
        if html { "--html" } else { "--json" },
        if html {
            "--output-dir"
        } else {
            "--output-path"
        },
    ]));
    args.push(
        if html {
            directory.to_path_buf()
        } else {
            directory.join("coverage.json")
        }
        .to_str()
        .unwrap()
        .into(),
    );
    call(profile, args)
}
fn reports(root: &Path, profile: &str) -> [Value; 2] {
    let directory = directory(root, profile);
    [
        report(profile, &directory, false),
        report(profile, &directory, true),
    ]
}
fn profile_prefix(root: &Path, metal: bool) -> Vec<Value> {
    let mut expected = vec![
        call("workspace", strings(&["clean", "--workspace", "--locked"])),
        call(
            "workspace",
            strings(&["--workspace", "--all-features", "--locked", "--no-report"]),
        ),
    ];
    if metal {
        let directory = directory(root, "workspace/portable");
        expected.extend([
            report("workspace", &directory, false),
            report("workspace", &directory, true),
        ]);
    }
    expected
}
