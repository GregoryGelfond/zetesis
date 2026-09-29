//! Clingo, the external comparison oracle, for the zetesis workspace's tests.
//!
//! A test that compares zetesis with clingo finds clingo ([`executable`]),
//! runs it within bounds ([`run`]) and decodes its JSON report ([`json`],
//! [`model_records`], [`answers()`]); [`records`] does all three for a complete
//! enumeration. Each test keeps the clingo arguments its comparison needs.
//!
//! The crate depends on `zetesis-validation`, whose bounded process capture
//! runs clingo, and on nothing that solves, so a test reaches clingo without
//! compiling a solver. It is not published or installed.

use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::path::PathBuf;
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

/// The bounds of one clingo run.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// The run's deadline.
    pub timeout: Duration,
    /// An inclusive ceiling on the bytes of standard output and error together.
    pub max_output_bytes: usize,
}

impl Default for Limits {
    /// Five seconds and 64 KiB.
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(5),
            max_output_bytes: 64 * 1024,
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
    /// The exit code: 10, 20 or 30.
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

/// Run clingo on `source` with `arguments`, within `limits`.
///
/// The source is written to a file in a fresh temporary directory, where
/// clingo runs; the file is its last argument.
///
/// # Panics
/// Panics if clingo cannot start, does not complete within the limits, is
/// signalled, exits with a code other than 10, 20 or 30, or leaves a child
/// that cannot be reaped. The message carries clingo's standard error.
#[must_use]
pub fn run(source: &str, arguments: &[&str], limits: Limits) -> Run {
    let directory = tempfile::tempdir().expect("temporary directory for clingo");
    let input = directory.path().join("case.lp");
    std::fs::write(&input, source).expect("clingo input");
    let mut arguments: Vec<OsString> = arguments.iter().map(OsString::from).collect();
    arguments.push(input.into_os_string());
    let (capture, pending) = process::invoke(
        process::Invocation {
            executable: &executable(),
            arguments: &arguments,
            directory: directory.path(),
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
    }
    let stderr = String::from_utf8_lossy(capture.stderr()).into_owned();
    assert_eq!(
        capture.stop(),
        process::Stop::Completed,
        "clingo did not complete within its limits: {stderr}"
    );
    assert!(
        capture.failure().is_none(),
        "{:?}: {stderr}",
        capture.failure()
    );
    assert!(
        capture.cleanup_failure().is_none(),
        "{:?}: {stderr}",
        capture.cleanup_failure()
    );
    let exit = capture.exit().expect("a completed run has an exit");
    assert!(exit.signal.is_none(), "clingo was signalled: {stderr}");
    let code = exit.code.expect("an unsignalled exit has a code");
    assert!(
        matches!(code, 10 | 20 | 30),
        "clingo exited {code}: {stderr}"
    );
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
