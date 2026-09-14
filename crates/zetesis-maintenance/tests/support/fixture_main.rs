//! Deterministic subprocess fixture for gate tests; never invokes real tools.
mod capture_fixture;
use serde_json::json;
use std::{
    env,
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
    process::ExitCode,
};

fn variable(name: &str, default: &str) -> String {
    env::var(name).unwrap_or_else(|_| default.into())
}
fn value<'a>(arguments: &'a [String], flag: &str) -> Option<&'a str> {
    arguments
        .iter()
        .position(|arg| arg == flag)
        .and_then(|index| arguments.get(index + 1))
        .map(String::as_str)
}
fn has(arguments: &[String], flag: &str) -> bool {
    arguments.iter().any(|arg| arg == flag)
}
fn fail(reason: &str) -> Result<(), String> {
    Err(reason.into())
}
fn trace(role: &str, arguments: &[String]) {
    if let Ok(path) = env::var("CHECK_TEST_TRACE") {
        writeln!(
            OpenOptions::new()
                .append(true)
                .create(true)
                .open(path)
                .unwrap(),
            "{role} {}",
            arguments.join(" ")
        )
        .unwrap();
    }
}
fn report_arguments(arguments: &[String]) -> Result<(), String> {
    if arguments.get(2).map(String::as_str) != Some("report") {
        return Ok(());
    }
    let mut remaining = &arguments[3..];
    while let Some((argument, rest)) = remaining.split_first() {
        if ["--locked", "--json", "--html"].contains(&argument.as_str()) {
            remaining = rest;
        } else if [
            "--package",
            "--output-path",
            "--output-dir",
            "--fail-under-lines",
        ]
        .contains(&argument.as_str())
            && !rest.is_empty()
        {
            remaining = &rest[1..];
        } else {
            return fail(&format!("invalid mock report option: {argument}"));
        }
    }
    Ok(())
}
fn physical(arguments: &[String]) -> Result<(), String> {
    let target = if has(arguments, "--lib") {
        "lib"
    } else {
        value(arguments, "--test").ok_or("missing mock test target")?
    };
    let position = arguments
        .iter()
        .position(|arg| arg == "--exact")
        .ok_or("missing exact filter")?;
    let filters = &arguments[position + 1..];
    let fields: Vec<_> = include_str!("physical-selection.txt")
        .lines()
        .map(|line| line.split('|').collect::<Vec<_>>())
        .find(|fields| {
            fields[1] == target
                && fields[3]
                    .split_whitespace()
                    .any(|name| filters.iter().any(|filter| filter == name))
        })
        .ok_or("unexpected physical target")?;
    let group = fields[0];
    let mut mode = variable("COVERAGE_TEST_PHYSICAL", "passed");
    if variable("COVERAGE_TEST_PHYSICAL_GROUP", "wgpu-lib") != group {
        mode = "passed".into();
    }
    if mode == "failed" {
        return fail("simulated physical test failure");
    }
    let mut selected: Vec<String> = fields[3]
        .split_whitespace()
        .filter(|name| filters.iter().any(|arg| arg == name))
        .map(str::to_owned)
        .collect();
    match mode.as_str() {
        "missing" => {
            selected.pop();
        }
        "empty" => selected.clear(),
        "wrong-name" => {
            *selected.last_mut().ok_or("no selected tests")? = "unrelated_physical_test".into();
        }
        "duplicate-name" => {
            let first = selected.first().ok_or("no selected tests")?.clone();
            *selected.last_mut().unwrap() = first;
        }
        _ => {}
    }
    for (index, name) in selected.iter().enumerate() {
        println!("test {name} ... adapter=qualified-fixture");
        if mode != "missing-outcome" || index + 1 != selected.len() {
            println!(
                "{}",
                if mode == "failed-outcome" && index + 1 == selected.len() {
                    "FAILED"
                } else {
                    "ok"
                }
            );
        }
    }
    if target == "lib" {
        println!(
            "test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out; finished in 0.00s"
        );
    }
    if mode != "unreported" {
        println!(
            "test result: ok. {} passed; 0 failed; {} ignored; 0 measured; 1 filtered out; finished in 0.01s",
            selected.len(),
            usize::from(mode == "ignored")
        );
    }
    if mode == "extra-summary" {
        println!(
            "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s"
        );
    }
    if mode == "failed-summary" {
        println!(
            "test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s"
        );
    }
    Ok(())
}
fn cargo(arguments: &[String]) -> Result<(), String> {
    if env::var_os("CHECK_TEST_TRACE").is_some() {
        let failed = variable("CHECK_TEST_ORACLE_FAILURE", "");
        if arguments.first().is_some_and(|argument| argument == "test")
            && arguments
                .windows(2)
                .any(|pair| pair[0] == "--test" && failed.split(',').any(|name| name == pair[1]))
        {
            return fail("simulated oracle campaign failure");
        }
        if has(arguments, "--message-format=json-render-diagnostics") {
            let directory = env::current_dir()
                .map_err(|error| error.to_string())?
                .join("target/debug/deps");
            fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
            for name in ["zetesis_cli", "zetesis_solve", "zetesis_validation"] {
                let file = directory.join(format!("lib{name}-current.rlib"));
                fs::write(&file, b"current").map_err(|error| error.to_string())?;
                println!(
                    "{}",
                    json!({"reason":"compiler-artifact","target":{"name":name,"kind":["lib"]},"filenames":[file],"fresh":true})
                );
            }
            println!("{}", json!({"reason":"build-finished","success":true}));
        }
        return Ok(());
    }
    if arguments == ["+1.97.1", "llvm-cov", "--version"] {
        println!(
            "{}",
            variable("COVERAGE_TEST_CARGO_VERSION", "cargo-llvm-cov 0.8.7")
        );
        return Ok(());
    }
    if arguments
        .get(..2)
        .is_none_or(|args| args != ["+1.97.1", "llvm-cov"])
    {
        return fail("unexpected mock Cargo invocation");
    }
    report_arguments(arguments)?;
    let directory = env::var("CARGO_LLVM_COV_TARGET_DIR").map_err(|error| error.to_string())?;
    let profile = Path::new(&directory).file_name().unwrap().to_str().unwrap();
    let log = env::var("COVERAGE_TEST_LOG").map_err(|error| error.to_string())?;
    writeln!(
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(log)
            .unwrap(),
        "{}",
        json!({"args":arguments,"profile":profile})
    )
    .unwrap();
    let phase = if has(arguments, "--fail-under-lines") {
        "gate"
    } else if has(arguments, "--html") {
        "html"
    } else if has(arguments, "--json") {
        "json"
    } else if has(arguments, "clean") {
        "clean"
    } else {
        "test"
    };
    let failure = variable("COVERAGE_TEST_FAIL", "");
    if failure == phase || failure == format!("{profile}:{phase}") {
        return fail(&format!("simulated failure {phase}"));
    }
    if has(arguments, "--ignored") {
        physical(arguments)?;
    }
    if let Some(path) = value(arguments, "--output-path") {
        fs::write(path, "{}\n").unwrap();
    }
    if let Some(path) = value(arguments, "--output-dir") {
        let path = Path::new(path).join("html");
        fs::create_dir_all(&path).unwrap();
        fs::write(path.join("index.html"), "mock HTML").unwrap();
    }
    Ok(())
}
fn execute(role: &str, arguments: &[String]) -> Result<(), String> {
    trace(role, arguments);
    if let Some(expected) = env::var_os("CHECK_TEST_EXPECT_DIRECTORY")
        && env::current_dir().map_err(|error| error.to_string())? != Path::new(&expected)
    {
        return fail("tool started outside the expected repository");
    }
    if let Ok(expected) = env::var("CHECK_TEST_BOOK_TOOLCHAIN")
        && ((role == "mdbook" && has(arguments, "test"))
            || (role == "cargo" && has(arguments, "--message-format=json-render-diagnostics")))
        && env::var("RUSTUP_TOOLCHAIN").ok().as_deref() != Some(expected.as_str())
    {
        return fail("temporary book compiler selected a different toolchain");
    }
    if role == "lake"
        && has(arguments, "Audit.lean")
        && env::var_os("CHECK_TEST_AUDIT_STDERR").is_some()
    {
        eprintln!("synthetic unexpected Audit stderr");
    }
    let failure = variable("CHECK_TEST_FAILURE", "");
    if (role == "lake"
        && ((arguments == ["build"] && failure == "build")
            || (has(arguments, "Audit.lean") && failure == "audit")))
        || (role == "maintenance"
            && arguments
                .first()
                .is_some_and(|argument| argument == "proof-record")
            && failure == "record")
    {
        return fail("simulated check failure");
    }
    match role {
        "cargo" => cargo(arguments),
        "rustup" if arguments == ["show", "active-toolchain"] => {
            println!(
                "{} (overridden by repository toolchain)",
                variable("CHECK_TEST_BOOK_TOOLCHAIN", "1.97.1-fake-host")
            );
            Ok(())
        }
        "rustc" if arguments == ["+1.97.1", "--print", "sysroot"] => {
            println!("{}", env::var("COVERAGE_TEST_SYSROOT").unwrap());
            Ok(())
        }
        "rustc" if arguments == ["+1.97.1", "-vV"] => {
            println!("rustc 1.97.1\nhost: fake-host\nLLVM version: 22.1.6");
            Ok(())
        }
        "llvm" => {
            println!(
                "LLVM (http://llvm.org/):\n  LLVM version {}\n  Optimized build.",
                variable("COVERAGE_TEST_LLVM_VERSION", "22.1.6")
            );
            Ok(())
        }
        "clingo" if arguments == ["--version"] => {
            if variable("CHECK_TEST_CLINGO_FAILURE", "") == "version" {
                return fail("simulated clingo version failure");
            }
            println!(
                "clingo version {}",
                variable("CHECK_TEST_CLINGO_VERSION", "5.8.2")
            );
            Ok(())
        }
        "mdbook" => {
            if arguments == ["--version"] {
                println!("mdbook v0.5.4");
            }
            if has(arguments, "test") && env::var_os("CHECK_TEST_BOOK_ARTIFACTS").is_some() {
                let path =
                    Path::new(value(arguments, "--library-path").ok_or("missing book view")?);
                let entries: std::collections::BTreeSet<_> = fs::read_dir(path)
                    .map_err(|error| error.to_string())?
                    .map(|entry| {
                        entry
                            .map(|entry| entry.file_name())
                            .map_err(|error| error.to_string())
                    })
                    .collect::<Result<_, _>>()?;
                let expected = [
                    "libzetesis_cli-current.rlib",
                    "libzetesis_solve-current.rlib",
                    "libzetesis_validation-current.rlib",
                ]
                .map(std::ffi::OsString::from)
                .into_iter()
                .collect();
                if entries != expected {
                    return fail("rustdoc received stale or missing libraries");
                }
            }
            Ok(())
        }
        "maintenance" | "lake" | "coverage.sh" | "validate.sh" => Ok(()),
        _ => fail("unexpected mock tool invocation"),
    }
}
fn main() -> ExitCode {
    let arguments: Vec<_> = env::args().skip(1).collect();
    let executable = env::args().next().unwrap();
    let name = Path::new(&executable)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap();
    let result = if ["lake", "lean", "rustc", "cargo", "zetesis-maintenance"].contains(&name) {
        capture_fixture::run(name, &arguments)
    } else {
        arguments.split_first().map_or_else(
            || fail("missing tool role"),
            |(role, arguments)| execute(role, arguments),
        )
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(reason) => {
            eprintln!("{reason}");
            ExitCode::from(if env::var_os("CHECK_TEST_TRACE").is_some() {
                23
            } else {
                37
            })
        }
    }
}
