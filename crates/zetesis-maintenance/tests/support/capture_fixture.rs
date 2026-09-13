//! Explicit synthetic tool responses; these never execute Lean or Cargo.
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

fn root() -> Result<PathBuf, String> {
    let current = std::env::current_dir().map_err(|error| error.to_string())?;
    if current.join(".capture-fixture").is_file() {
        Ok(current)
    } else if let Some(parent) = current.parent() {
        if parent.join(".capture-fixture").is_file() {
            return Ok(parent.into());
        }
        Err("missing explicit capture fixture marker".into())
    } else {
        Err("missing capture fixture root".into())
    }
}
fn lake(root: &Path, arguments: &[String], mode: &str) -> Result<(), String> {
    if arguments == ["env", "lean", "--version"] {
        let version = if mode == "lean-version" {
            "4.32.0"
        } else {
            "4.33.1"
        };
        println!(
            "Lean (version {version}, fixture-host, commit 0123456789012345678901234567890123456789, Release)"
        );
    } else if arguments == ["build"] && mode == "build" {
        return Err("synthetic kernel build failure".into());
    } else if arguments.last().is_some_and(|arg| arg == "Audit.lean") {
        println!("'Example.fact' does not depend on any axioms");
        if mode == "audit-stderr" {
            eprintln!("synthetic audit warning");
        }
        if mode == "source-change" {
            fs::OpenOptions::new()
                .append(true)
                .open(root.join("proofs/Zetesis/Example.lean"))
                .and_then(|mut file| file.write_all(b"-- changed during capture\n"))
                .map_err(|error| error.to_string())?;
        }
        if mode == "documentation-change" {
            fs::write(root.join("proofs/README.md"), b"changed during capture\n")
                .map_err(|error| error.to_string())?;
        }
        if mode == "tool-change" {
            // Replace only the fixture's lean role, which this stand-in never
            // executes. Do not modify its shared executable inode in place.
            let parent = root.parent().ok_or("missing fixture parent")?;
            let replacement = parent.join("replacement-lean");
            fs::write(&replacement, b"changed Lean executable input")
                .map_err(|error| error.to_string())?;
            fs::rename(replacement, parent.join("tools/lean"))
                .map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}
pub fn run(tool: &str, arguments: &[String]) -> Result<(), String> {
    let root = root()?;
    let mode =
        fs::read_to_string(root.join(".capture-fixture")).map_err(|error| error.to_string())?;
    let mode = mode.trim();
    match tool {
        "lake" => lake(&root, arguments, mode),
        "rustc" if arguments == ["--version"] => {
            println!(
                "rustc {} (synthetic)",
                if mode == "rust-version" {
                    "1.96.0"
                } else {
                    "1.97.1"
                }
            );
            Ok(())
        }
        "cargo" if arguments == ["--version"] => {
            println!(
                "cargo {} (synthetic)",
                if mode == "cargo-version" {
                    "1.96.0"
                } else {
                    "1.97.1"
                }
            );
            Ok(())
        }
        "cargo" if arguments.first().is_some_and(|arg| arg == "test") => {
            if mode == "self-replace" {
                let parent = root.parent().ok_or("missing fixture parent")?;
                let replacement = parent.join("replacement");
                fs::write(&replacement, b"a subsequently rebuilt command")
                    .map_err(|error| error.to_string())?;
                fs::rename(replacement, parent.join("tools/zetesis-maintenance"))
                    .map_err(|error| error.to_string())?;
            }
            println!("synthetic regression result");
            Ok(())
        }
        "zetesis-maintenance" if arguments == ["--version"] => {
            println!("zetesis-maintenance {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        _ => Err("unexpected synthetic capture invocation".into()),
    }
}
