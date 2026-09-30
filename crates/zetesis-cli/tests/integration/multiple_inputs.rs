//! Independent original roots retain full model multiplicity and global objectives.

use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use clap::Parser;
use serde_json::Value as Json;
use zetesis_cli::Options;
use zetesis_clingo_support as oracle;
use zetesis_clingo_support::costs;

type Records = Vec<(BTreeSet<String>, Option<Vec<i64>>)>;
struct Fixture(tempfile::TempDir);
impl Fixture {
    fn new() -> Self {
        Self(tempfile::tempdir().expect("fixture"))
    }
    fn write(&self, name: &str, source: &str) {
        let path = self.0.path().join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, source).unwrap();
    }
    fn case(case: &Json) -> Self {
        let fixture = Self::new();
        for (name, source) in case["files"].as_object().unwrap() {
            fixture.write(name, source.as_str().unwrap());
        }
        fixture
    }
    fn native(&self, arguments: &[&str]) -> Output {
        bounded(
            Command::new(env!("CARGO_BIN_EXE_zetesis"))
                .args(["--backend", "cpu", "--workers", "1", "--models", "0"])
                .args(arguments)
                .current_dir(self.0.path()),
        )
    }
    /// Remove the directory, failing the test if it cannot be removed.
    fn close(self) {
        self.0.close().expect("only this fixture's files");
    }
}

fn bounded(command: &mut Command) -> Output {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let started = Instant::now();
    loop {
        if child.try_wait().unwrap().is_some() {
            let output = child.wait_with_output().unwrap();
            assert!(output.stdout.len() + output.stderr.len() <= 65_536);
            return output;
        }
        if started.elapsed() > Duration::from_secs(5) {
            let _ = child.kill();
            let _ = child.wait();
            panic!("bounded original-file test process timed out");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn atoms(line: &str) -> BTreeSet<String> {
    let mut quoted = false;
    let mut escaped = false;
    let mut start = 0;
    let mut result = BTreeSet::new();
    for (index, character) in line.char_indices() {
        if quoted {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                quoted = false;
            }
        } else if character == '"' {
            quoted = true;
        } else if character.is_whitespace() {
            if start < index {
                assert!(result.insert(line[start..index].to_owned()));
            }
            start = index + character.len_utf8();
        }
    }
    assert!(!quoted && !escaped);
    if start < line.len() {
        assert!(result.insert(line[start..].to_owned()));
    }
    result
}
fn expected(case: &Json) -> Records {
    let mut records: Records = case["records"]
        .as_array()
        .unwrap()
        .iter()
        .map(|record| {
            (
                record[0]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|atom| atom.as_str().unwrap().to_owned())
                    .collect(),
                costs(&record[1]),
            )
        })
        .collect();
    records.sort();
    records
}
fn native_records(output: Output) -> Records {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("Coverage: exhausted"), "{text}");
    let lines: Vec<_> = text.lines().collect();
    let mut records = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if line.starts_with("Answer:") {
            let model = atoms(lines[index + 1]);
            let costs = lines
                .get(index + 2)
                .and_then(|line| line.strip_prefix("Optimization:"))
                .map(|line| {
                    line.split_whitespace()
                        .map(|value| value.parse().unwrap())
                        .collect()
                });
            records.push((model, costs));
        }
    }
    let count: usize = lines
        .iter()
        .find_map(|line| line.strip_prefix("Models: "))
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    assert_eq!(count, records.len());
    records.sort();
    records
}

#[test]
fn file_arguments_preserve_the_first_input_library_field_and_order() {
    let options =
        Options::try_parse_from(["zetesis", "one.lp", "two.lp", "three.lp", "--models", "0"])
            .unwrap();
    assert_eq!(options.input, PathBuf::from("one.lp"));
    assert_eq!(
        options.additional_inputs,
        ["two.lp", "three.lp"].map(PathBuf::from)
    );
    let defaults = Options::try_parse_from(["zetesis"]).unwrap();
    assert_eq!(defaults.input, PathBuf::from("-"));
    assert!(defaults.additional_inputs.is_empty());
}

#[test]
fn a_subcommand_name_after_a_file_remains_an_original_file() {
    let parsed = Options::try_parse_from(["zetesis", "one.lp", "devices"]).unwrap();
    assert!(parsed.command.is_none());
    assert_eq!(parsed.additional_inputs, [PathBuf::from("devices")]);
    let fixture = Fixture::new();
    fixture.write("one.lp", "a.");
    fixture.write("devices", "b.");
    assert_eq!(
        native_records(fixture.native(&["one.lp", "devices"])),
        [(BTreeSet::from(["a".to_owned(), "b".to_owned()]), None)]
    );
    fixture.close();
}

#[test]
fn original_file_sets_match_recorded_complete_models_costs_and_display_counts() {
    let cases: Vec<Json> =
        serde_json::from_str(include_str!("../fixtures/multiple-inputs.json")).unwrap();
    assert_eq!(cases.len(), 14);
    assert_eq!(
        cases.iter().map(|case| expected(case).len()).sum::<usize>(),
        19
    );
    for case in cases {
        let fixture = Fixture::case(&case);
        let roots: Vec<_> = case["roots"]
            .as_array()
            .unwrap()
            .iter()
            .map(|root| root.as_str().unwrap())
            .collect();
        assert_eq!(
            native_records(fixture.native(&roots)),
            expected(&case),
            "{}",
            case["name"]
        );
        fixture.close();
    }
}

#[test]
fn mixed_and_repeated_standard_input_fail_before_reading_any_source() {
    let fixture = Fixture::new();
    for roots in [&["-", "missing.lp"][..], &["missing.lp", "-"], &["-", "-"]] {
        let result = fixture.native(roots);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        assert!(
            String::from_utf8(result.stderr)
                .unwrap()
                .contains("must be the only input")
        );
    }
    let mut child = Command::new(env!("CARGO_BIN_EXE_zetesis"))
        .args(["--backend", "cpu", "--models", "0", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"p.").unwrap();
    assert_eq!(
        native_records(child.wait_with_output().unwrap()),
        vec![(BTreeSet::from(["p".into()]), None)]
    );
    fixture.close();
}

#[test]
fn cumulative_source_and_root_limits_refuse_before_model_output() {
    let fixture = Fixture::new();
    fixture.write("a.lp", "p.");
    fixture.write("b.lp", "q.");
    for (flag, value, label) in [
        ("--max-source-roots", "1", "Roots"),
        ("--max-source-files", "1", "Files"),
        ("--max-total-source-bytes", "3", "TotalBytes"),
    ] {
        let result = fixture.native(&["a.lp", "b.lp", flag, value]);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        assert!(String::from_utf8(result.stderr).unwrap().contains(label));
    }
    fixture.close();
}

#[test]
fn later_original_file_errors_name_the_original_path() {
    let fixture = Fixture::new();
    fixture.write("a.lp", "p.");
    for source in ["broken(.", "#program other. q.", "q(X):-not r(X)."] {
        fixture.write("b.lp", source);
        let result = fixture.native(&["a.lp", "b.lp"]);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        assert!(String::from_utf8(result.stderr).unwrap().contains("b.lp"));
    }
    fixture.close();
}

#[test]
fn root_aliases_are_explicit_refusals_while_identical_roots_are_shared() {
    let fixture = Fixture::new();
    fixture.write("a.lp", "#const n=1. p(n).");
    assert_eq!(
        native_records(fixture.native(&["a.lp", "a.lp"])),
        vec![(BTreeSet::from(["p(1)".into()]), None)]
    );
    let absolute = fixture.0.path().join("a.lp").to_string_lossy().into_owned();
    for second in ["./a.lp", absolute.as_str()] {
        let result = fixture.native(&["a.lp", second]);
        assert_eq!(result.status.code(), Some(2));
        assert!(
            String::from_utf8(result.stderr)
                .unwrap()
                .contains("root aliases")
        );
    }
    fixture.close();
}

#[test]
#[ignore = "requires clingo: original file sets match fresh complete clingo optima"]
fn original_file_sets_match_fresh_complete_clingo_optima() {
    let cases: Vec<Json> =
        serde_json::from_str(include_str!("../fixtures/multiple-inputs.json")).unwrap();
    for case in cases {
        let fixture = Fixture::case(&case);
        let roots: Vec<_> = case["roots"]
            .as_array()
            .unwrap()
            .iter()
            .map(|root| root.as_str().unwrap())
            .collect();
        let run = oracle::run_in(
            fixture.0.path(),
            oracle::ENUMERATION.iter().chain(&roots),
            &oracle::DECIDED,
            oracle::Limits::default(),
        );
        let json = oracle::json(&run);
        assert_eq!(json["Models"]["More"], "no");
        let mut records: Records = json["Call"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|call| {
                call["Witnesses"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|witness| {
                        (
                            witness["Value"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .map(|atom| atom.as_str().unwrap().to_owned())
                                .collect(),
                            costs(&witness["Costs"]),
                        )
                    })
            })
            .collect();
        assert_eq!(
            json["Models"]["Number"].as_u64().unwrap(),
            records.len() as u64
        );
        if let Some(best) = records
            .iter()
            .filter_map(|(_, costs)| costs.as_ref())
            .min()
            .cloned()
        {
            records.retain(|(_, costs)| costs.as_ref() == Some(&best));
        }
        records.sort();
        assert_eq!(records, expected(&case), "{}", case["name"]);
        assert_eq!(
            native_records(fixture.native(&roots)),
            records,
            "{}",
            case["name"]
        );
        fixture.close();
    }
}

#[cfg(unix)]
#[test]
fn include_lookup_errors_fall_back_but_selected_source_failures_do_not() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let fixture = Fixture::new();
    fixture.write("sub/entry.lp", "#include \"unreadable.lp\".");
    fixture.write("unreadable.lp", "wrong.");
    fixture.write("sub/unreadable.lp", "fallback.");
    fs::set_permissions(
        fixture.0.path().join("unreadable.lp"),
        fs::Permissions::from_mode(0o0),
    )
    .unwrap();
    // A privileged test process may still read mode-zero files. Its lookup
    // correctly selects that original cwd file instead of the fallback.
    let expected = if fs::File::open(fixture.0.path().join("unreadable.lp")).is_ok() {
        "wrong"
    } else {
        "fallback"
    };
    let result = fixture.native(&["sub/entry.lp"]);
    fs::set_permissions(
        fixture.0.path().join("unreadable.lp"),
        fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    assert_eq!(
        native_records(result),
        [(BTreeSet::from([expected.to_owned()]), None)]
    );

    fixture.write("sub/loop-entry.lp", "#include \"loop.lp\".");
    fixture.write("sub/loop.lp", "loop_fallback.");
    symlink("loop.lp", fixture.0.path().join("loop.lp")).unwrap();
    assert_eq!(
        native_records(fixture.native(&["sub/loop-entry.lp"])),
        [(BTreeSet::from(["loop_fallback".to_owned()]), None)]
    );

    fixture.write("unreadable.lp", "invalid(");
    let invalid = fixture.native(&["sub/entry.lp"]);
    assert_eq!(invalid.status.code(), Some(2));
    assert!(invalid.stdout.is_empty());
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("unreadable.lp"));

    fixture.write("sub/bound.lp", "#include \"x\".");
    fixture.write("x", "p(1). q(2). r(3). s(4).");
    fixture.write("sub/x", "fallback.");
    let over_limit = fixture.native(&["sub/bound.lp", "--max-source-bytes", "16"]);
    assert_eq!(over_limit.status.code(), Some(2));
    assert!(over_limit.stdout.is_empty());
    assert!(String::from_utf8_lossy(&over_limit.stderr).contains("FileBytes"));
    fixture.close();
}

#[cfg(unix)]
#[test]
fn nonregular_roots_and_includes_are_refused_before_blocking_open() {
    let fixture = Fixture::new();
    let fifo = fixture.0.path().join("pipe.lp");
    assert!(
        Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap()
            .success()
    );
    fixture.write("entry.lp", "#include \"pipe.lp\".");
    for name in ["pipe.lp", "entry.lp"] {
        let result = fixture.native(&[name]);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        assert!(String::from_utf8_lossy(&result.stderr).contains("regular file"));
    }
    fixture.close();
}
