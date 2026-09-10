//! Independent answer families for logical bounds and resolved priorities.

#[path = "support/bound_priority_sources.rs"]
mod sources;
#[path = "../../zetesis-themelios/tests/support/source_records.rs"]
mod source_records;
#[path = "../../zetesis-themelios/tests/support/source_oracle.rs"]
mod source_oracle;

use std::collections::BTreeSet;
use std::num::NonZeroUsize;

use clap::Parser;
use source_records::Records;
use zetesis_cli::{
    Backend, Completion, Options, Oracle, PreparedInput, Session, SolveConfig, run_with_diagnostics,
};
use zetesis_cpu::Control;
use zetesis_themelios::FormulaLimits;

struct Expected {
    all: Records,
    optimal: Records,
    priorities: &'static [i32],
}

fn unscored(models: &[&[&str]]) -> Expected {
    let all: Records = models
        .iter()
        .map(|atoms| (atoms.iter().map(|atom| (*atom).into()).collect(), None))
        .collect();
    Expected {
        optimal: all.clone(),
        all,
        priorities: &[],
    }
}

fn objective_records(rows: &[(&[&str], i64)]) -> Records {
    let mut records = Records::new();
    for &(selected, cost) in rows {
        let mut atoms: BTreeSet<String> = ["priority(1)", "priority(2)"]
            .into_iter()
            .chain(selected.iter().copied())
            .map(str::to_owned)
            .collect();
        // The independent hidden choice doubles each complete answer family.
        assert!(records.insert((atoms.clone(), Some(vec![cost, cost]))));
        atoms.insert("hidden".into());
        assert!(records.insert((atoms, Some(vec![cost, cost]))));
    }
    records
}

fn scored(all: &[(&[&str], i64)], optimal: &[(&[&str], i64)]) -> Expected {
    Expected {
        all: objective_records(all),
        optimal: objective_records(optimal),
        priorities: &[2, 1],
    }
}

fn expectations() -> [Expected; sources::SOURCES.len()] {
    [
        unscored(&[&[], &["a"], &["b"], &["a", "b"]]),
        unscored(&[]),
        unscored(&[&[], &["a"], &["b"], &["a", "b"]]),
        // The double-negated operand supplies no producer for a.
        unscored(&[&["b"]]),
        scored(&[(&["a"], 1), (&["b"], 0)], &[(&["b"], 0)]),
        scored(&[(&["a"], -1), (&["b"], 0)], &[(&["a"], -1)]),
        scored(&[(&["a"], 1), (&["b"], 0)], &[(&["b"], 0)]),
        scored(
            &[(&["a"], 1), (&["b"], 0), (&["a", "b"], 1)],
            &[(&["b"], 0)],
        ),
        scored(&[(&["a"], 0), (&["b"], 0)], &[(&["a"], 0), (&["b"], 0)]),
    ]
}

#[test]
fn exhaustive_reduct_checks_preserve_answer_families() {
    for (source, expected) in sources::SOURCES.into_iter().zip(expectations()) {
        let input = source_records::admit(source, &FormulaLimits::default()).unwrap();
        assert_eq!(source_records::exhaustive(&input), expected.all, "{source}");
    }
}

#[test]
fn sessions_preserve_complete_scored_answers() {
    for (source, expected) in sources::SOURCES.into_iter().zip(expectations()) {
        let input = source_records::admit(source, &FormulaLimits::default()).unwrap();
        assert_eq!(input.objectives().priorities(), expected.priorities);
        for (workers, batch) in [(1, 1), (4, 3)] {
            let config = SolveConfig {
                backend: Backend::Cpu,
                oracle: Oracle::Countermodel,
                models: 0,
                workers: NonZeroUsize::new(workers).unwrap(),
                completion_workers: NonZeroUsize::new(workers).unwrap(),
                batch_size: NonZeroUsize::new(batch).unwrap(),
                ..SolveConfig::default()
            };
            let mut session =
                Session::enumerate(PreparedInput::formula(&input), config, Control::default())
                    .unwrap();
            let mut records = Records::new();
            for answer in session.by_ref() {
                let answer = answer.unwrap();
                let costs = answer.score().map(|score| {
                    let priorities: Vec<_> = score.costs().iter().map(|&(p, _)| p).collect();
                    assert_eq!(priorities, expected.priorities);
                    score.costs().iter().map(|&(_, value)| value).collect()
                });
                assert!(
                    records.insert((
                        answer
                            .interpretation()
                            .atoms()
                            .iter()
                            .map(source_records::canonical)
                            .collect(),
                        costs,
                    ))
                );
            }
            assert_eq!(records, expected.all, "{source}; workers={workers}");
            let outcome = session.outcome().unwrap();
            assert_eq!(outcome.completion(), Some(Completion::Exhausted));
            assert_eq!(outcome.unsatisfiable(), expected.all.is_empty());
            assert_eq!(outcome.formula_execution().is_some(), workers > 1);
        }
    }
}

#[test]
fn json_preserves_complete_optimum_answers() {
    for (source, expected) in sources::SOURCES.into_iter().zip(expectations()) {
        for grounder in ["auto", "eager"] {
            let options = Options::try_parse_from([
                "zetesis",
                "--backend",
                "cpu",
                "--models",
                "0",
                "--json",
                "--grounder",
                grounder,
            ])
            .unwrap();
            let mut output = Vec::new();
            let report = run_with_diagnostics(
                source.into(),
                &options,
                &mut output,
                &mut Vec::new(),
                &Control::default(),
            )
            .unwrap();
            assert_eq!(report.completion, Completion::Exhausted);
            let answers = zetesis_validation::answers::native_json::parse(
                &output,
                zetesis_validation::answers::native_json::Limits::default(),
            )
            .unwrap();
            let mut records = Records::new();
            for answer in answers.records() {
                let costs = answer.costs().map(|costs| {
                    assert_eq!(
                        costs.iter().map(|&(p, _)| p).collect::<Vec<_>>(),
                        expected.priorities
                    );
                    costs.iter().map(|&(_, value)| value).collect()
                });
                assert!(
                    records.insert((
                        answer
                            .full_model()
                            .iter()
                            .map(source_records::canonical)
                            .collect(),
                        costs
                    ))
                );
            }
            assert_eq!(records, expected.optimal, "{grounder}: {source}");
        }
    }
}

#[test]
#[ignore = "requires an independently installed clingo executable"]
fn original_sources_match_independent_answers() {
    for (source, expected) in sources::SOURCES.into_iter().zip(expectations()) {
        let actual = source_oracle::records(source);
        println!("source={source:?} complete_answers={}", actual.len());
        assert_eq!(actual, expected.all, "{source}");
    }
}
