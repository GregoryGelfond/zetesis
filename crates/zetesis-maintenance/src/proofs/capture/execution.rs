//! Bounded command execution and observed tool identities.
use super::{Request, Result, digest, json_file, require, retention};
use crate::proofs;
use regex::Regex;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use zetesis_validation::process::{self, Invocation, Limits};

#[derive(Debug)]
struct StartReceiptFailure {
    start: process::StartError,
    receipt: Box<dyn std::error::Error>,
}
impl std::fmt::Display for StartReceiptFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{}; refusal receipt also failed: {}",
            self.start, self.receipt
        )
    }
}
impl std::error::Error for StartReceiptFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.start)
    }
}

pub(super) struct Runner {
    pub(super) repository: PathBuf,
    pub(super) stage: PathBuf,
    pub(super) evidence: PathBuf,
    pub(super) commands: Vec<Value>,
    pub(super) environment: BTreeMap<String, OsString>,
    pub(super) command_limits: Limits,
    pub(super) logs: BTreeMap<String, String>,
}

impl Runner {
    pub(super) fn run(
        &mut self,
        executable: &Path,
        command: &[String],
        cwd: &str,
        name: &str,
    ) -> Result<Vec<u8>> {
        let mut arguments = vec![OsString::from("-i")];
        for (key, value) in &self.environment {
            let mut assignment = OsString::from(key);
            assignment.push("=");
            assignment.push(value);
            arguments.push(assignment);
        }
        arguments.push(executable.as_os_str().to_owned());
        arguments.extend(command.iter().skip(1).map(OsString::from));
        let directory = self.repository.join(cwd);
        let evidence = self.evidence.join(name);
        fs::create_dir(&evidence)?;
        let actual_arguments: Vec<&str> = arguments
            .iter()
            .map(|value| value.to_str().ok_or("non-UTF-8 command argument"))
            .collect::<std::result::Result<_, _>>()?;
        json_file(
            &evidence.join("invocation.json"),
            &json!({
                "executable": "/usr/bin/env", "arguments": actual_arguments,
                "directory": directory, "environment": "cleared by env -i; assignments are in argv",
                "timeout_seconds": self.command_limits.timeout.as_secs_f64(),
                "max_output_bytes": self.command_limits.max_output_bytes,
                "cleanup_seconds": self.command_limits.cleanup_timeout.as_secs_f64()
            }),
        )?;
        let started = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs_f64();
        let outcome = process::invoke(
            Invocation {
                executable: Path::new("/usr/bin/env"),
                arguments: &arguments,
                directory: &directory,
            },
            self.command_limits,
        );
        let outcome = match outcome {
            Ok(outcome) => outcome,
            Err(error) => {
                // No child was started. Preserve the refusal before returning it.
                if let Err(receipt) = json_file(
                    &evidence.join("result.json"),
                    &json!({"start_error": error.to_string()}),
                ) {
                    return Err(StartReceiptFailure {
                        start: error,
                        receipt,
                    }
                    .into());
                }
                return Err(error.into());
            }
        };
        let retained = retention::retain(outcome, &self.stage, &evidence, name)?;
        let mut observation = json!({
            "command": command,
            "cwd": cwd,
            "started_unix_seconds": started,
            "elapsed_seconds": retained.capture.elapsed().as_secs_f64(),
            "exit_code": 0,
            "result": "PASS",
            "log": retained.log,
            "stdout_bytes": retained.capture.stdout().len(),
            "stderr_bytes": retained.capture.stderr().len(),
            "stream_layout": retained.layout,
        });
        if name == "record-regressions" {
            observation["raw_log_sha256"] =
                format!("{:x}", Sha256::digest(&retained.output)).into();
        }
        self.commands.push(observation);
        self.logs.insert(retained.log, retained.sha256);
        Ok(retained.output)
    }
}

fn arguments(items: &[&str]) -> Vec<String> {
    items.iter().map(|value| (*value).to_owned()).collect()
}

pub(super) struct Tools {
    lake: PathBuf,
    lean: PathBuf,
    rustc: PathBuf,
    cargo: PathBuf,
}

impl Tools {
    pub(super) fn new(request: &Request<'_>) -> Result<Self> {
        Ok(Self {
            lake: request.lean_bin.join("lake").canonicalize()?,
            lean: request.lean_bin.join("lean").canonicalize()?,
            rustc: request.rust_bin.join("rustc").canonicalize()?,
            cargo: request.rust_bin.join("cargo").canonicalize()?,
        })
    }
    pub(super) fn hashes(&self, maintenance: &Path) -> Result<BTreeMap<String, String>> {
        [
            ("lake", self.lake.as_path()),
            ("lean", self.lean.as_path()),
            ("rustc", self.rustc.as_path()),
            ("cargo", self.cargo.as_path()),
            ("zetesis-maintenance", maintenance),
        ]
        .into_iter()
        .map(|(name, path)| Ok((name.into(), digest(path)?)))
        .collect()
    }
}

pub(super) fn command_environment(tools: &Tools) -> Result<BTreeMap<String, OsString>> {
    let mut paths = vec![
        tools
            .lake
            .parent()
            .ok_or("lake has no parent")?
            .to_path_buf(),
        tools
            .cargo
            .parent()
            .ok_or("cargo has no parent")?
            .to_path_buf(),
    ];
    paths.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    let mut environment = BTreeMap::from([
        ("PATH".into(), std::env::join_paths(paths)?),
        ("CARGO_BUILD_JOBS".into(), "1".into()),
        ("CARGO_NET_OFFLINE".into(), "true".into()),
        ("RUSTUP_TOOLCHAIN".into(), "1.97.1".into()),
        ("ELAN_TOOLCHAIN".into(), "leanprover/lean4:v4.33.1".into()),
    ]);
    // These are the only inherited settings forwarded through env -i. Compiler
    // flags are deliberately absent; an alternate build configuration requires
    // a separate reviewed invocation rather than an invisible ambient change.
    for name in [
        "HOME",
        "TMPDIR",
        "TMP",
        "TEMP",
        "CARGO_HOME",
        "CARGO_TARGET_DIR",
        "RUSTUP_HOME",
        "ELAN_HOME",
    ] {
        if let Some(value) = std::env::var_os(name) {
            environment.insert(name.into(), value);
        }
    }
    environment.insert("LC_ALL".into(), "C".into());
    Ok(environment)
}

pub(super) fn execute_kernel(
    runner: &mut Runner,
    source: &proofs::Inventory,
    tools: &Tools,
) -> Result<Kernel> {
    let version = runner.run(
        &tools.lake,
        &arguments(&["lake", "env", "lean", "--version"]),
        "proofs",
        "version",
    )?;
    let identity = LeanIdentity::parse(std::str::from_utf8(&version)?.trim())?;
    runner.run(
        &tools.lake,
        &arguments(&["lake", "clean"]),
        "proofs",
        "clean",
    )?;
    runner.run(
        &tools.lake,
        &arguments(&["lake", "build"]),
        "proofs",
        "build",
    )?;
    let mut strict = arguments(&[
        "sh",
        "-c",
        "for source in \"$@\"; do lake env lean -DautoImplicit=false -DwarningAsError=true \"$source\" || exit \"$?\"; done",
        "strict-modules",
    ]);
    strict.extend(source.modules().iter().cloned());
    runner.run(Path::new("/bin/sh"), &strict, "proofs", "strict-modules")?;
    let audit = runner.run(
        &tools.lake,
        &arguments(&[
            "lake",
            "env",
            "lean",
            "-DautoImplicit=false",
            "-DwarningAsError=true",
            "Audit.lean",
        ]),
        "proofs",
        "audit",
    )?;
    fs::write(runner.stage.join("axiom-audit.txt"), &audit)?;
    Ok(Kernel { audit, identity })
}

pub(super) fn check_assurance_tools(
    runner: &mut Runner,
    tools: &Tools,
    maintenance: &Path,
) -> Result<()> {
    let rustc_version = runner.run(
        &tools.rustc,
        &arguments(&["rustc", "--version"]),
        ".",
        "rustc-version",
    )?;
    require(
        std::str::from_utf8(&rustc_version)?.starts_with("rustc 1.97.1 "),
        "wrong Rust toolchain",
    )?;
    let cargo_version = runner.run(
        &tools.cargo,
        &arguments(&["cargo", "--version"]),
        ".",
        "cargo-version",
    )?;
    require(
        std::str::from_utf8(&cargo_version)?.starts_with("cargo 1.97.1 "),
        "wrong Cargo toolchain",
    )?;
    runner.run(
        &tools.cargo,
        &arguments(&[
            "cargo",
            "test",
            "--quiet",
            "--locked",
            "-p",
            "zetesis-maintenance",
            "--test",
            "proof_records",
            "--all-features",
        ]),
        ".",
        "record-regressions",
    )?;

    let maintenance_version = runner.run(
        maintenance,
        &arguments(&["zetesis-maintenance", "--version"]),
        ".",
        "maintenance-version",
    )?;
    require(
        std::str::from_utf8(&maintenance_version)?.trim()
            == concat!("zetesis-maintenance ", env!("CARGO_PKG_VERSION")),
        "unexpected assurance command version",
    )?;
    Ok(())
}

pub(super) struct LeanIdentity {
    pub(super) version: String,
    pub(super) platform: String,
    pub(super) commit: String,
}
impl LeanIdentity {
    pub(super) fn parse(output: &str) -> Result<Self> {
        let pattern = Regex::new(
            r"^Lean \(version (4\.33\.1), ([A-Za-z0-9_.-]+), commit ([a-f0-9]{40}), Release\)$",
        )?;
        let matched = pattern
            .captures(output)
            .ok_or("wrong pinned Lean identity")?;
        Ok(Self {
            version: matched[1].into(),
            platform: matched[2].into(),
            commit: matched[3].into(),
        })
    }
}
pub(super) struct Kernel {
    pub(super) audit: Vec<u8>,
    pub(super) identity: LeanIdentity,
}
