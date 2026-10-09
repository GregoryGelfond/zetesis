//! Native SAT command routing, model coverage and truthful interruption.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use clap::Parser;
use zetesis_cli::{Completion, Interruption, Options, Report, run_with_diagnostics};
use zetesis_cpu::Cancellation;

fn solve(
    source: &str,
    arguments: &[&str],
    cancellation: &Cancellation,
) -> (Report, String, String) {
    let options = Options::try_parse_from(
        [
            "zetesis",
            "--oracle",
            "countermodel",
            "--models",
            "0",
            "--stats",
        ]
        .into_iter()
        .chain(arguments.iter().copied()),
    )
    .unwrap();
    let mut output = Vec::new();
    let mut diagnostics = Vec::new();
    let report = run_with_diagnostics(
        source.into(),
        &options,
        &mut output,
        &mut diagnostics,
        cancellation,
    )
    .unwrap();
    (
        report,
        String::from_utf8(output).unwrap(),
        String::from_utf8(diagnostics).unwrap(),
    )
}

fn answers(text: &str) -> Vec<BTreeSet<&str>> {
    let mut answers = Vec::new();
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        if line.starts_with("Answer:") {
            answers.push(lines.next().unwrap().split_whitespace().collect());
        }
    }
    answers
}

#[test]
fn sat_models_match_exhaustive_normal_search() {
    for source in [
        "",
        "a :- a.",
        "a :- b. b :- a.",
        "a :- not b. b :- not a.",
        "{a}. {b}. :- a,b.",
        "a :- not not a.",
        "a :- not a.",
        ":-.",
        "node(1). node(2). {pick(X)} :- node(X). :- pick(1), pick(2).",
    ] {
        let (sat, text, diagnostics) = solve(source, &[], &Cancellation::default());
        assert_eq!(sat.completion, Completion::Exhausted, "{source}");
        let mut bytes = Vec::new();
        let options =
            Options::try_parse_from(["zetesis", "--backend", "cpu", "--models", "0"]).unwrap();
        let reference = zetesis_cli::run(
            source.into(),
            &options,
            &mut bytes,
            &Cancellation::default(),
        )
        .unwrap();
        let expected = String::from_utf8(bytes).unwrap();
        assert_eq!(reference.models, sat.models);
        assert_eq!(
            answers(&text).into_iter().collect::<BTreeSet<_>>(),
            answers(&expected).into_iter().collect()
        );
        assert!(diagnostics.contains("oracle: Ferraris reduct countermodel"));
        assert!(diagnostics.contains("grounder: eager"));
        assert!(sat.countermodel_statistics.is_some());
    }
}

#[test]
fn search_limits_are_incomplete_and_hidden_models_remain_distinct() {
    let limits: [fn(&mut zetesis_cli::PublicationConfig); 4] = [
        |c| c.solve.max_search_work = 0,
        |c| c.solve.max_search_decisions = 0,
        |c| c.solve.max_candidates = 0,
        |c| c.solve.max_work = 0,
    ];
    for limit in limits {
        let (report, output, _) =
            crate::support::prepared::formula_run("{a}.", &["--oracle", "countermodel"], limit);
        let report = report.unwrap();
        assert_eq!(report.completion, Completion::Interrupted);
        assert!(matches!(
            report.interruption,
            Some(Interruption::Countermodel(_))
        ));
        assert!(output.contains("INCOMPLETE"));
        assert!(!output.contains("UNSATISFIABLE"));
    }
    let (report, output, _) = solve(
        "{hidden}. visible. #show visible/0.",
        &[],
        &Cancellation::default(),
    );
    assert_eq!(report.completion, Completion::Exhausted);
    assert_eq!(report.models, 2);
    assert_eq!(answers(&output), vec![BTreeSet::from(["visible"]); 2]);
    let (limited, output, _) =
        crate::support::prepared::formula_run("{a}. {b}.", &["--oracle", "countermodel"], |c| {
            c.solve.max_candidates = 1;
        });
    let limited = limited.unwrap();
    assert_eq!(limited.models, 1);
    assert_eq!(limited.completion, Completion::Interrupted);
    assert!(output.contains("Answer: 1"));
}

#[test]
fn cancellation_precedes_eager_materialization() {
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let (report, _, _) = solve("invalid source never admitted", &[], &cancellation);
    assert_eq!(
        report.interruption,
        Some(Interruption::Preparation(zetesis_cpu::Stop::Cancelled))
    );
    assert_eq!(report.completion, Completion::Interrupted);
    assert_eq!(report.models, 0);
    assert_eq!(report.checked, 0);
    assert!(report.countermodel_statistics.is_none());
    assert!(report.formula_execution.is_none());
    assert!(report.lazy_execution.is_none());
    assert!(report.shared_execution.is_none());
}

fn queens(size: usize) -> String {
    // Explicitly synthetic S0 source, not the correctness examples' encoding.
    let mut source = String::new();
    for row in 0..size {
        for column in 0..size {
            writeln!(source, "{{q{row}_{column}}}.").unwrap();
        }
        for transpose in [false, true] {
            source.push_str(":-");
            for column in 0..size {
                if column > 0 {
                    source.push(',');
                }
                let (r, c) = if transpose {
                    (column, row)
                } else {
                    (row, column)
                };
                write!(source, " not q{r}_{c}").unwrap();
            }
            source.push_str(".\n");
        }
    }
    for first in 0..size * size {
        for second in first + 1..size * size {
            let (r, c) = (first / size, first % size);
            let (s, d) = (second / size, second % size);
            if r == s || c == d || r.abs_diff(s) == c.abs_diff(d) {
                writeln!(source, ":- q{r}_{c}, q{s}_{d}.").unwrap();
            }
        }
    }
    source
}

#[test]
fn synthetic_eight_queens_source_has_all_92_models() {
    let (report, output, _) = solve(&queens(8), &[], &Cancellation::default());
    assert_eq!(report.completion, Completion::Exhausted, "{report:?}");
    assert_eq!(report.models, 92);
    let models = answers(&output);
    assert_eq!(models.iter().collect::<BTreeSet<_>>().len(), 92);
    for model in models {
        assert_eq!(model.len(), 8);
        let coordinates: Vec<(usize, usize)> = model
            .iter()
            .map(|atom| {
                let (row, column) = atom.strip_prefix('q').unwrap().split_once('_').unwrap();
                (row.parse().unwrap(), column.parse().unwrap())
            })
            .collect();
        for (index, &(row, column)) in coordinates.iter().enumerate() {
            for &(other_row, other_column) in &coordinates[index + 1..] {
                assert_ne!(row, other_row);
                assert_ne!(column, other_column);
                assert_ne!(row.abs_diff(other_row), column.abs_diff(other_column));
            }
        }
    }
}
