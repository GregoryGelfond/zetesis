//! Automatic maximizing admission keeps normalized costs and complete optimal ties.
use std::io::Write;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use clap::Parser;
use zetesis_cli::{Completion, Options, Oracle, Report, RunError, run_with_diagnostics};
use zetesis_clingo_support as oracle;
use zetesis_cpu::Cancellation;
use zetesis_themelios::{AdmissionFailure, ExpansionFailure, FormulaFailure, ProfileFeature};

type Records = Vec<(Vec<String>, Option<Vec<i64>>)>;
struct Case {
    source: &'static str,
    displays: &'static [&'static str],
    costs: Option<&'static [i64]>,
}
const CASES: &[Case] = &[
    Case {
        source: "{a}. #maximize{2@1,k:a}.",
        displays: &["a"],
        costs: Some(&[-2]),
    },
    Case {
        source: "{a}. #maximize{-2@1,k:a}.",
        displays: &[""],
        costs: Some(&[0]),
    },
    Case {
        source: "{a;b}. #maximize{2@1,k:a}. #minimize{-2@1,k:b}.",
        displays: &["a", "b", "a b"],
        costs: Some(&[-2]),
    },
    Case {
        source: "{a;b}. #maximize{2@1,k:a}. :~b.[-2@1,k]",
        displays: &["a", "b", "a b"],
        costs: Some(&[-2]),
    },
    Case {
        source: "a.b. #maximize{2@1,k:a}. #minimize{2@1,k:b}.",
        displays: &["a b"],
        costs: Some(&[0]),
    },
    Case {
        source: "1{a;b}1. #maximize{3@1,k:a}. #minimize{4@2,k:a;2@2,k:b}.",
        displays: &["b"],
        costs: Some(&[2, 0]),
    },
    Case {
        source: "{a;h}. #show a/0. #maximize{2@1,k:a;0@7,k:h}.",
        displays: &["a", "a"],
        costs: Some(&[0, -2]),
    },
    Case {
        source: "{a}. #maximize{0@7,k:a}.",
        displays: &["", "a"],
        costs: Some(&[0]),
    },
    Case {
        source: "a:-a. #maximize{2@7,k:a}.",
        displays: &[""],
        costs: None,
    },
    Case {
        source: "#maximize{}.",
        displays: &[""],
        costs: None,
    },
    Case {
        source: ":-. #maximize{1@0,k}.",
        displays: &[],
        costs: None,
    },
    Case {
        source: "a.b. #maximize{2147483647@1,k:a;2147483647@1,j:b}.",
        displays: &["a b"],
        costs: Some(&[-4_294_967_294]),
    },
    Case {
        source: "{p(\"a b\");p(\"a\\\"b\")}. #maximize{1@0,k:p(X)}.",
        displays: &["p(\"a b\")", "p(\"a\\\"b\")", "p(\"a b\") p(\"a\\\"b\")"],
        costs: Some(&[-1]),
    },
];

fn options(pruning: bool, models: usize) -> Options {
    let mut options =
        Options::try_parse_from(["zetesis", "--backend", "cpu", "--workers", "1"]).unwrap();
    assert_eq!(options.oracle, Oracle::Auto);
    options.models = models;
    options.stats = true;
    if !pruning {
        options.max_objective_bound_work = 0;
    }
    options
}
fn solve(source: &str, options: &Options) -> (Result<Report, RunError>, String, String) {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let result = run_with_diagnostics(
        source.into(),
        options,
        &mut stdout,
        &mut stderr,
        &Cancellation::default(),
    );
    (
        result,
        String::from_utf8(stdout).unwrap(),
        String::from_utf8(stderr).unwrap(),
    )
}
fn symbols(line: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut quoted = false;
    let mut escaped = false;
    let mut start = 0;
    for (index, ch) in line.char_indices() {
        if quoted {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                quoted = false;
            }
        } else if ch == '"' {
            quoted = true;
        } else if ch.is_whitespace() {
            if start < index {
                result.push(line[start..index].to_owned());
            }
            start = index + ch.len_utf8();
        }
    }
    assert!(!quoted && !escaped);
    if start < line.len() {
        result.push(line[start..].to_owned());
    }
    result.sort();
    result
}
fn records(text: &str) -> Records {
    let lines: Vec<_> = text.lines().collect();
    let mut records = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if line.starts_with("Answer:") {
            let atoms = symbols(lines[index + 1]);
            let costs = lines
                .get(index + 2)
                .and_then(|line| line.strip_prefix("Optimization:"))
                .map(|cost| {
                    cost.split_ascii_whitespace()
                        .map(|value| value.parse().unwrap())
                        .collect()
                });
            records.push((atoms, costs));
        }
    }
    records.sort();
    records
}
fn expected(case: &Case) -> Records {
    let mut rows: Records = case
        .displays
        .iter()
        .map(|line| (symbols(line), case.costs.map(<[i64]>::to_vec)))
        .collect();
    rows.sort();
    rows
}
fn assert_complete(report: &Report, text: &str, case: &Case) {
    assert_eq!(
        report.completion,
        Completion::Exhausted,
        "{}: {text}",
        case.source
    );
    assert_eq!(report.models, case.displays.len());
    assert_eq!(records(text), expected(case), "{}", case.source);
    assert!(crate::support::human::exhausted(text));
    assert!(text.contains(&format!("Models: {}\n", case.displays.len())));
    let terminal = if case.displays.is_empty() {
        "UNSATISFIABLE"
    } else if case.costs.is_some() {
        "OPTIMUM FOUND"
    } else {
        "SATISFIABLE"
    };
    assert_eq!(
        text.lines()
            .filter(|line| matches!(*line, "SATISFIABLE" | "UNSATISFIABLE" | "OPTIMUM FOUND"))
            .collect::<Vec<_>>(),
        [terminal]
    );
    assert_eq!(
        text.matches("Optimization:").count(),
        if case.costs.is_some() {
            case.displays.len()
        } else {
            0
        }
    );
    if let Some(costs) = case.costs {
        let optimum = report.optimization.as_ref().unwrap();
        assert_eq!(
            optimum
                .score
                .costs()
                .iter()
                .map(|(_, cost)| *cost)
                .collect::<Vec<_>>(),
            costs
        );
        assert_eq!(
            optimum.tied_models,
            u64::try_from(case.displays.len()).unwrap()
        );
        assert!(!text.contains("Incumbent ties:"));
    } else {
        assert!(report.optimization.is_none());
    }
}

#[test]
fn automatic_admission_and_bounds_preserve_complete_costs_and_display_multisets() {
    assert_eq!(CASES.len(), 13);
    assert_eq!(
        CASES.iter().map(|case| case.displays.len()).sum::<usize>(),
        20
    );
    for (index, case) in CASES.iter().enumerate() {
        for pruning in [false, true] {
            let (report, text, diagnostics) = solve(case.source, &options(pruning, 0));
            let report = report.unwrap_or_else(|error| panic!("{}: {error}", case.source));
            assert_complete(&report, &text, case);
            assert!(
                diagnostics.contains("oracle: Ferraris reduct membership"),
                "{diagnostics}"
            );
            let restrictions = report
                .countermodel_statistics
                .unwrap()
                .candidate_restrictions;
            if !pruning {
                assert_eq!(restrictions, 0);
            } else if index == 0 {
                assert!(restrictions > 0);
            }
        }
    }
}

#[test]
fn default_display_limit_still_proves_all_hidden_optimal_ties() {
    let case = &CASES[6];
    for pruning in [false, true] {
        let (report, text, _) = solve(case.source, &options(pruning, 1));
        let report = report.unwrap();
        assert_eq!(report.completion, Completion::Exhausted);
        assert_eq!(report.models, 1);
        let optimum = report.optimization.unwrap();
        assert_eq!(optimum.tied_models, 2);
        assert_eq!(optimum.score.costs(), [(7, 0), (1, -2)]);
        assert_eq!(records(&text), [(vec!["a".to_owned()], Some(vec![0, -2]))]);
        assert!(text.contains("OPTIMUM FOUND\nModels:"));
        assert!(text.contains("Models: 1\n"));
    }
}

#[test]
fn maximizing_literal_and_evaluated_minimum_have_distinct_typed_source_refusals() {
    for (source, literal) in [
        ("#maximize{-2147483648@1,k:absent}.", true),
        ("#maximize{(-2147483647-1)@1,k}.", false),
        ("v(-2147483647-1).v(1). #maximize{W@1,k:v(W)}.", false),
    ] {
        let (result, text, _) = solve(source, &options(true, 0));
        let error = result.unwrap_err();
        assert!(crate::support::human::preamble(&text));
        let RunError::FormulaAdmission(FormulaFailure::Expansion(ExpansionFailure::Admission(
            error,
        ))) = error
        else {
            panic!("{source}: {error}");
        };
        if literal {
            assert!(matches!(error, AdmissionFailure::Raise(_)));
        } else {
            assert!(matches!(
                error,
                AdmissionFailure::Profile {
                    feature: ProfileFeature::NumericOverflow,
                    ..
                }
            ));
        }
        assert!(!error.diagnostics().is_empty());
    }
    let source = "v(-2147483647-1).v(1). #maximize{W@1,k:v(W),W!=(-2147483647-1)}.";
    let (report, text, _) = solve(source, &options(true, 0));
    assert_eq!(report.unwrap().completion, Completion::Exhausted);
    assert!(text.contains("Optimization: -1\n"));
}

fn process(command: &mut Command, source: &str) -> Output {
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(source.as_bytes())
        .unwrap();
    let start = Instant::now();
    loop {
        if child.try_wait().unwrap().is_some() {
            let output = child.wait_with_output().unwrap();
            assert!(output.stdout.len() + output.stderr.len() <= 65_536);
            return output;
        }
        if start.elapsed() > Duration::from_secs(5) {
            let _ = child.kill();
            let _ = child.wait();
            panic!("bounded source process timed out");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}
#[test]
fn original_stdin_command_proves_normalized_optimum_without_feature_flags() {
    let case = &CASES[2];
    for budget in ["0", "10000000"] {
        let output = process(
            Command::new(env!("CARGO_BIN_EXE_zetesis")).args([
                "--backend",
                "cpu",
                "--workers",
                "1",
                "--models",
                "0",
                "--max-objective-bound-work",
                budget,
            ]),
            case.source,
        );
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let text = String::from_utf8(output.stdout).unwrap();
        assert_eq!(records(&text), expected(case));
        assert!(text.contains("OPTIMUM FOUND\nModels: 3\n"));
    }
}

fn clingo(case: &Case) -> Records {
    let run = oracle::run(case.source, &oracle::ENUMERATION, oracle::Limits::default());
    let json = oracle::json(&run);
    assert_eq!(json["Models"]["More"].as_str(), Some("no"));
    let mut result = Vec::new();
    for call in json["Call"].as_array().unwrap() {
        for witness in call["Witnesses"].as_array().into_iter().flatten() {
            let mut values: Vec<String> = witness["Value"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_str().unwrap().to_owned())
                .collect();
            values.sort();
            let costs = witness["Costs"].as_array().map(|values| {
                values
                    .iter()
                    .map(|value| value.as_i64().unwrap())
                    .collect::<Vec<_>>()
            });
            result.push((values, costs));
        }
    }
    assert_eq!(
        json["Models"]["Number"].as_u64(),
        Some(u64::try_from(result.len()).unwrap())
    );
    if let Some(best) = result
        .iter()
        .filter_map(|(_, cost)| cost.as_ref())
        .min()
        .cloned()
    {
        result.retain(|(_, cost)| cost.as_ref() == Some(&best));
    }
    result.sort();
    result
}
#[test]
#[ignore = "requires clingo: fresh clingo optima match native bounds on and off; 13 bounded original source comparisons"]
fn fresh_clingo_optima_match_native_bounds_on_and_off() {
    for case in CASES {
        assert_eq!(clingo(case), expected(case), "{}", case.source);
        for pruning in [false, true] {
            let (report, text, _) = solve(case.source, &options(pruning, 0));
            assert_complete(&report.unwrap(), &text, case);
        }
    }
}

#[test]
fn incomplete_maximizing_evaluation_cannot_claim_optimality() {
    let mut bounded = options(false, 0);
    bounded.max_objective_work = 0;
    let (report, output, _) = solve(CASES[0].source, &bounded);
    assert_eq!(report.unwrap().completion, Completion::Interrupted);
    assert!(output.contains("INCOMPLETE:"));
    assert!(output.contains("(search incomplete)"));
    assert!(!output.contains("OPTIMUM FOUND"));
    assert!(!output.contains("UNSATISFIABLE"));
}
