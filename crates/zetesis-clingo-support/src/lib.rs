//! Clingo, the external comparison oracle, for the zetesis workspace's tests.
//!
//! A test that compares zetesis with clingo finds clingo ([`executable`]),
//! runs it within bounds on a source ([`run`], [`run_accepting`]) or on
//! program files ([`run_in`]), and decodes its JSON report ([`json`],
//! [`model_records`], [`answers()`]); [`records`] does all three for a complete
//! enumeration. Each test keeps the clingo arguments its comparison needs.
//!
//! The crate depends on `zetesis-validation`, whose bounded process capture
//! runs clingo, and on nothing that solves, so a test reaches clingo without
//! compiling a solver. It is not published or installed.

use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::Value;
use zetesis_test_support::records::Records;
use zetesis_validation::{answers, process};

/// The clingo release the comparisons are recorded against.
pub const VERSION: &str = "clingo version 5.8.2";

/// The arguments of a complete enumeration: every answer set, optimal or
/// not, as JSON, without warnings.
pub const ENUMERATION: [&str; 4] = ["0", "--outf=2", "--opt-mode=enum", "--warn=none"];

/// The clingo the comparisons run: `CLINGO` when it is set, which must name
/// an absolute path, as the oracle gate requires; otherwise the first eligible
/// `clingo` on `PATH`.
///
/// # Panics
/// Panics if `CLINGO` names a relative path, or, naming both sources, if
/// neither yields clingo.
#[must_use]
pub fn executable() -> PathBuf {
    discover(
        std::env::var_os("CLINGO"),
        std::env::var_os("PATH").as_deref(),
    )
}

/// [`executable`]'s rule, over the values of `CLINGO` and `PATH`.
fn discover(clingo: Option<OsString>, search: Option<&OsStr>) -> PathBuf {
    if let Some(path) = clingo {
        let path = PathBuf::from(path);
        assert!(
            path.is_absolute(),
            "CLINGO must name an absolute path: {}",
            path.display()
        );
        return path;
    }
    process::resolve_executable("clingo".as_ref(), search)
        .unwrap_or_else(|error| panic!("CLINGO is unset and PATH holds no clingo: {error}"))
}

/// The byte ceiling of a clingo report the comparisons read: 64 KiB.
pub const REPORT_BYTES: usize = 64 * 1024;

/// The bounds of one clingo run.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// The run's deadline.
    pub timeout: Duration,
    /// An inclusive ceiling on the bytes of standard output and error together.
    pub max_output_bytes: usize,
}

impl Default for Limits {
    /// Five seconds and [`REPORT_BYTES`].
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(5),
            max_output_bytes: REPORT_BYTES,
        }
    }
}

/// A completed clingo run: its exit code and both output streams.
#[derive(Debug)]
pub struct Run {
    code: i32,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

impl Run {
    /// The exit code, one of those the run accepted.
    #[must_use]
    pub fn code(&self) -> i32 {
        self.code
    }

    /// The complete standard output.
    #[must_use]
    pub fn stdout(&self) -> &[u8] {
        &self.stdout
    }

    /// The complete standard error.
    #[must_use]
    pub fn stderr(&self) -> &[u8] {
        &self.stderr
    }
}

/// The exit codes of a run that decided its program: satisfiable (10),
/// unsatisfiable (20), or satisfiable with its search exhausted (30).
pub const DECIDED: [i32; 3] = [10, 20, 30];

/// Run clingo on `source` with `arguments`, within `limits`, expecting it to
/// decide the program ([`DECIDED`]).
///
/// # Panics
/// As [`run_accepting`].
#[must_use]
pub fn run(source: &str, arguments: &[&str], limits: Limits) -> Run {
    run_accepting(source, arguments, &DECIDED, limits)
}

/// Run clingo on `source` with `arguments`, within `limits`, accepting the
/// exit codes in `exits`: a comparison that expects clingo to refuse a
/// program accepts its error code, 65.
///
/// The source is written to a file in a fresh temporary directory, where
/// clingo runs; the file is its last argument.
///
/// # Panics
/// As [`run_in`]; the message quotes the source.
#[must_use]
pub fn run_accepting(source: &str, arguments: &[&str], exits: &[i32], limits: Limits) -> Run {
    run_source(&executable(), source, arguments, exits, limits)
}

/// Run clingo in `directory` with `arguments`, which name the program files
/// it reads, within `limits`, accepting the exit codes in `exits`. A relative
/// file name resolves from `directory`.
///
/// # Panics
/// Panics if clingo cannot start, does not complete within the limits, is
/// signalled, exits with a code `exits` does not hold, or leaves a child that
/// cannot be reaped cleanly. The message names the arguments and carries
/// clingo's standard error.
#[must_use]
pub fn run_in<I>(directory: &Path, arguments: I, exits: &[i32], limits: Limits) -> Run
where
    I: IntoIterator,
    I::Item: AsRef<OsStr>,
{
    let arguments: Vec<OsString> = arguments
        .into_iter()
        .map(|argument| argument.as_ref().to_owned())
        .collect();
    let subject = format!("{arguments:?}");
    invoke(
        &executable(),
        directory,
        &arguments,
        exits,
        limits,
        &subject,
    )
}

/// [`run_accepting`], with `executable` in clingo's place.
fn run_source(
    executable: &Path,
    source: &str,
    arguments: &[&str],
    exits: &[i32],
    limits: Limits,
) -> Run {
    let directory = tempfile::tempdir().expect("temporary directory for clingo");
    let input = directory.path().join("case.lp");
    std::fs::write(&input, source).expect("clingo input");
    let mut arguments: Vec<OsString> = arguments.iter().map(OsString::from).collect();
    arguments.push(input.into_os_string());
    invoke(
        executable,
        directory.path(),
        &arguments,
        exits,
        limits,
        source,
    )
}

/// One bounded run of `executable` in `directory`; `subject` names its input
/// in a failure's message. The executable is clingo, except in this crate's
/// tests of a run's contract.
fn invoke(
    executable: &Path,
    directory: &Path,
    arguments: &[OsString],
    exits: &[i32],
    limits: Limits,
    subject: &str,
) -> Run {
    let (capture, pending) = process::invoke(
        process::Invocation {
            executable,
            arguments,
            directory,
        },
        process::Limits {
            timeout: limits.timeout,
            max_output_bytes: limits.max_output_bytes,
            cleanup_timeout: Duration::from_secs(1),
        },
    )
    .expect("clingo starts")
    .into_parts();
    if let Some(pending) = pending {
        let cleanup = pending.retry(Duration::from_secs(1));
        if let Some(pending) = cleanup.pending {
            panic!(
                "clingo child {} could not be reaped: {:?}",
                pending.abandon(),
                cleanup.failure
            );
        }
        assert!(
            cleanup.failure.is_none(),
            "clingo's cleanup failed: {:?}",
            cleanup.failure
        );
    }
    let context = format!(
        "clingo on {subject}\nstandard error:\n{}",
        String::from_utf8_lossy(capture.stderr())
    );
    assert_eq!(capture.stop(), process::Stop::Completed, "{context}");
    assert!(
        capture.failure().is_none(),
        "{:?}: {context}",
        capture.failure()
    );
    assert!(
        capture.cleanup_failure().is_none(),
        "{:?}: {context}",
        capture.cleanup_failure()
    );
    let exit = capture.exit().expect("a completed run has an exit");
    assert!(exit.signal.is_none(), "signalled: {context}");
    let code = exit.code.expect("an unsignalled exit has a code");
    assert!(exits.contains(&code), "exited {code}: {context}");
    Run {
        code,
        stdout: capture.stdout().to_vec(),
        stderr: capture.stderr().to_vec(),
    }
}

/// The JSON report of a run made with `--outf=2`.
///
/// # Panics
/// Panics if the standard output is not one JSON document.
#[must_use]
pub fn json(run: &Run) -> Value {
    serde_json::from_slice(run.stdout()).expect("clingo's JSON report")
}

/// Every answer set of `source` with its costs, as clingo enumerates them
/// ([`ENUMERATION`]).
///
/// # Panics
/// As [`run`] and [`model_records`], and if the report is not [`VERSION`]'s.
#[must_use]
pub fn records(source: &str) -> Records {
    let report = json(&run(source, &ENUMERATION, Limits::default()));
    assert_eq!(report["Solver"].as_str(), Some(VERSION));
    model_records(&report)
}

/// Every witness of a complete JSON report, with its costs.
///
/// # Panics
/// Panics unless the enumeration is complete and satisfiable, unsatisfiable
/// or optimal, if a witness repeats, or if the report's model count differs
/// from its witnesses.
#[must_use]
pub fn model_records(report: &Value) -> Records {
    assert_eq!(
        report["Models"]["More"].as_str(),
        Some("no"),
        "incomplete enumeration"
    );
    assert!(
        matches!(
            report["Result"].as_str(),
            Some("SATISFIABLE" | "UNSATISFIABLE" | "OPTIMUM FOUND")
        ),
        "undecided result: {}",
        report["Result"]
    );
    let mut records = Records::new();
    let mut count = 0;
    for call in report["Call"].as_array().expect("clingo calls") {
        if let Some(witnesses) = call["Witnesses"].as_array() {
            for witness in witnesses {
                count += 1;
                assert!(
                    records.insert((atoms(&witness["Value"]), costs(&witness["Costs"]))),
                    "duplicate full model"
                );
            }
        }
    }
    assert_eq!(
        report["Models"]["Number"].as_u64(),
        Some(count),
        "the model count contradicts the witnesses"
    );
    records
}

/// A witness's atoms, as their spellings.
///
/// # Panics
/// Panics if `values` is not an array of strings.
#[must_use]
pub fn atoms(values: &Value) -> BTreeSet<String> {
    values
        .as_array()
        .unwrap()
        .iter()
        .map(|atom| atom.as_str().unwrap().into())
        .collect()
}

/// A witness's costs, absent when the program has no objective.
///
/// # Panics
/// Panics if a cost is not an integer.
#[must_use]
pub fn costs(values: &Value) -> Option<Vec<i64>> {
    values
        .as_array()
        .map(|values| values.iter().map(|value| value.as_i64().unwrap()).collect())
}

/// The answers a run reports, as `zetesis-validation` reads a clingo JSON
/// report.
///
/// # Panics
/// Panics if the report does not decode, or is not [`VERSION`]'s.
#[must_use]
pub fn answers(run: &Run) -> answers::ReportedAnswers {
    let report = answers::clingo_json(run.stdout(), answers::Limits::default()).unwrap();
    assert_eq!(report.solver(), VERSION);
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_set_clingo_is_taken_without_a_search() {
        let path = discover(Some("/opt/clingo/bin/clingo".into()), None);
        assert_eq!(path, PathBuf::from("/opt/clingo/bin/clingo"));
    }

    #[test]
    #[should_panic(expected = "CLINGO must name an absolute path")]
    fn a_relative_clingo_is_refused() {
        let _ = discover(Some("bin/clingo".into()), None);
    }

    #[test]
    #[cfg(unix)]
    fn an_unset_clingo_is_found_on_the_search_path() {
        use std::os::unix::fs::PermissionsExt as _;
        let empty = tempfile::tempdir().unwrap();
        let holding = tempfile::tempdir().unwrap();
        let clingo = holding.path().join("clingo");
        std::fs::write(&clingo, "").unwrap();
        std::fs::set_permissions(&clingo, std::fs::Permissions::from_mode(0o755)).unwrap();
        let search = std::env::join_paths([empty.path(), holding.path()]).unwrap();
        assert_eq!(discover(None, Some(&search)), clingo);
    }

    #[test]
    #[should_panic(expected = "CLINGO is unset and PATH holds no clingo")]
    fn a_search_path_without_clingo_is_refused() {
        let empty = tempfile::tempdir().unwrap();
        let search = std::env::join_paths([empty.path()]).unwrap();
        let _ = discover(None, Some(&search));
    }

    /// The shell stands in for clingo in the tests of a run's contract.
    #[cfg(unix)]
    const SHELL: &str = "/bin/sh";

    #[test]
    #[cfg(unix)]
    fn a_source_run_passes_the_source_file_last() {
        let run = run_source(
            Path::new(SHELL),
            "p.",
            &["-c", r#"cat "$1"; exit 10"#, "clingo"],
            &DECIDED,
            Limits::default(),
        );
        assert_eq!(run.stdout(), b"p.");
    }

    #[test]
    #[cfg(unix)]
    fn a_run_takes_place_in_its_directory() {
        let directory = tempfile::tempdir().unwrap();
        let arguments = ["-c".into(), "pwd -P; exit 20".into()];
        let run = invoke(
            Path::new(SHELL),
            directory.path(),
            &arguments,
            &DECIDED,
            Limits::default(),
            "pwd",
        );
        let reported = std::str::from_utf8(run.stdout()).unwrap().trim_end();
        assert_eq!(
            Path::new(reported),
            directory.path().canonicalize().unwrap()
        );
    }

    #[test]
    #[cfg(unix)]
    fn an_accepted_refusal_returns_its_code_and_diagnostics() {
        let directory = tempfile::tempdir().unwrap();
        let arguments = ["-c".into(), "echo refused >&2; exit 65".into()];
        let run = invoke(
            Path::new(SHELL),
            directory.path(),
            &arguments,
            &[65],
            Limits::default(),
            "refusal",
        );
        assert_eq!((run.code(), run.stderr()), (65, b"refused\n".as_slice()));
    }

    #[test]
    #[cfg(unix)]
    #[should_panic(expected = "exited 65")]
    fn an_exit_outside_the_accepted_codes_fails_the_run() {
        let directory = tempfile::tempdir().unwrap();
        let arguments = ["-c".into(), "exit 65".into()];
        let _ = invoke(
            Path::new(SHELL),
            directory.path(),
            &arguments,
            &DECIDED,
            Limits::default(),
            "refusal",
        );
    }

    #[test]
    #[cfg(unix)]
    #[should_panic(expected = "left: Deadline")]
    fn a_run_past_its_deadline_fails() {
        let directory = tempfile::tempdir().unwrap();
        let arguments = ["-c".into(), "sleep 5".into()];
        let _ = invoke(
            Path::new(SHELL),
            directory.path(),
            &arguments,
            &DECIDED,
            Limits {
                timeout: Duration::from_millis(100),
                max_output_bytes: 1024,
            },
            "sleep",
        );
    }

    #[test]
    fn model_records_holds_every_witness_with_its_costs() {
        let report = json!({
            "Models": {"More": "no", "Number": 2},
            "Result": "OPTIMUM FOUND",
            "Call": [{"Witnesses": [
                {"Value": ["a"], "Costs": [1]},
                {"Value": ["b", "a"], "Costs": [0]},
            ]}],
        });
        let expected: Records = [
            (["a".to_owned()].into(), Some(vec![1])),
            (["a".to_owned(), "b".to_owned()].into(), Some(vec![0])),
        ]
        .into();
        assert_eq!(model_records(&report), expected);
    }

    #[test]
    #[should_panic(expected = "incomplete enumeration")]
    fn model_records_refuses_an_incomplete_enumeration() {
        let report = json!({
            "Models": {"More": "yes", "Number": 0},
            "Result": "SATISFIABLE",
            "Call": [],
        });
        let _ = model_records(&report);
    }

    #[test]
    #[should_panic(expected = "duplicate full model")]
    fn model_records_refuses_a_repeated_witness() {
        let report = json!({
            "Models": {"More": "no", "Number": 2},
            "Result": "SATISFIABLE",
            "Call": [{"Witnesses": [{"Value": ["a"]}, {"Value": ["a"]}]}],
        });
        let _ = model_records(&report);
    }

    #[test]
    #[should_panic(expected = "the model count contradicts the witnesses")]
    fn model_records_refuses_a_count_its_witnesses_contradict() {
        let report = json!({
            "Models": {"More": "no", "Number": 3},
            "Result": "SATISFIABLE",
            "Call": [{"Witnesses": [{"Value": ["a"]}]}],
        });
        let _ = model_records(&report);
    }
}
