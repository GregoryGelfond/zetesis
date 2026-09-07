//! Automatic routing preserves choice groups, observed ties and objective slots.

use clap::Parser;
use zetesis_cli::{Completion, Options, Report, RunError, run_with_diagnostics};
use zetesis_cpu::Control;

fn solve(source: &str, arguments: &[&str]) -> (Result<Report, RunError>, String, String) {
    let options = Options::try_parse_from(
        ["zetesis", "--models", "0"]
            .into_iter()
            .chain(arguments.iter().copied()),
    )
    .unwrap();
    let mut output = Vec::new();
    let mut diagnostics = Vec::new();
    let result = run_with_diagnostics(
        source.into(),
        &options,
        &mut output,
        &mut diagnostics,
        &Control::default(),
    );
    (
        result,
        String::from_utf8(output).unwrap(),
        String::from_utf8(diagnostics).unwrap(),
    )
}

fn displays(text: &str) -> Vec<&str> {
    let mut result = Vec::new();
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        if line.starts_with("Answer:") {
            result.push(lines.next().expect("complete displayed model"));
        }
    }
    result.sort_unstable();
    result
}

#[test]
fn automatic_interval_choices_keep_group_bounds_products_costs_and_hidden_ties() {
    type Case = (&'static str, &'static [&'static str], Option<(i32, i64)>);
    let cases: &[Case] = &[
        ("1 {p(1..2)} 1.", &["p(1)", "p(2)"], None),
        ("1 {p(2..1)} 1.", &[], None),
        (
            "1 {p(1..2,1..2)} 1.",
            &["p(1,1)", "p(1,2)", "p(2,1)", "p(2,2)"],
            None,
        ),
        (
            "1 {p(1..2)} 1. #maximize{X:p(X)}.",
            &["p(2)"],
            Some((0, -2)),
        ),
        ("0 {p(1..2)} 0. #minimize{1@7:p(1)}.", &[""], Some((7, 0))),
        (
            "1 {p(1..2)} 1. #show. #show chosen.",
            &["chosen", "chosen"],
            None,
        ),
    ];
    for &(source, expected, cost) in cases {
        let (result, output, diagnostics) = solve(source, &[]);
        let report = result.unwrap_or_else(|error| panic!("{source}: {error}"));
        assert_eq!(report.completion, Completion::Exhausted, "{source}");
        assert_eq!(report.models, expected.len(), "{source}");
        assert_eq!(displays(&output), expected, "{source}");
        assert!(diagnostics.contains("oracle: Ferraris reduct countermodel"));
        assert!(output.contains("Coverage: exhausted"));
        assert!(!output.contains("INCOMPLETE"));
        if let Some(cost) = cost {
            let optimum = report.optimization.expect("present objective priority");
            assert_eq!(optimum.score.costs(), &[cost]);
            assert_eq!(optimum.tied_models, u64::try_from(expected.len()).unwrap());
            assert_eq!(
                output
                    .matches(&format!("Optimization: {}\n", cost.1))
                    .count(),
                expected.len()
            );
            assert!(output.contains("OPTIMUM FOUND"));
        } else {
            assert!(report.optimization.is_none());
            assert!(!output.contains("Optimization:"));
            let status = if expected.is_empty() {
                "UNSATISFIABLE"
            } else {
                "SATISFIABLE"
            };
            assert!(output.lines().any(|line| line == status));
        }
    }
}

#[test]
fn explicit_unsupported_routes_and_exhausted_admission_never_emit_answers() {
    let source = "1 {p(1..4)} 1.";
    for arguments in [
        vec!["--oracle", "closure"],
        vec!["--grounder", "lazy"],
        vec!["--backend", "metal"],
        vec!["--backend", "nvidia"],
        vec!["--max-atoms", "1"],
        vec!["--max-substitutions", "1"],
    ] {
        let (result, output, _) = solve(source, &arguments);
        assert!(result.is_err(), "{arguments:?}");
        assert!(output.is_empty(), "{arguments:?}: {output}");
    }
}
