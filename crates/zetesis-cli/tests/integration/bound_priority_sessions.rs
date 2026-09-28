//! Independent answer families for logical bounds and resolved priorities.

use crate::support::bound_priority_sources as sources;
use crate::support::source_oracle;
use crate::support::source_records;

use std::collections::BTreeSet;
use std::num::NonZeroUsize;

use clap::Parser;
use source_records::Records;
use zetesis_cli::{
    Backend, Completion, Options, Oracle, PreparedInput, Session, SolveConfig, run_with_diagnostics,
};
use zetesis_cpu::Cancellation;
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

fn correlated() -> Expected {
    // Choosing a incurs two at priority one; choosing b incurs one at the
    // higher priority two. Both fields belong to the same row binding.
    let first = (
        ["row(1,2)", "row(2,1)", "a", "selected(1)"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        Some(vec![0, 2]),
    );
    let second = (
        ["row(1,2)", "row(2,1)", "b", "selected(2)"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        Some(vec![1, 0]),
    );
    Expected {
        all: Records::from([first.clone(), second]),
        optimal: Records::from([first]),
        priorities: &[2, 1],
    }
}

fn generated() -> Expected {
    // Required key 1 already fixes the count, regardless of optional b.
    // At priority two, choosing x costs zero and choosing y costs one.
    let mut all = Records::new();
    let mut optimal = Records::new();
    for (choice, cost) in [("x", vec![0, 2]), ("y", vec![1, 0])] {
        for optional in [false, true] {
            let mut atoms = BTreeSet::from(["a".into(), "n(1)".into(), choice.into()]);
            if optional {
                atoms.insert("b".into());
            }
            let record = (atoms, Some(cost.clone()));
            if choice == "x" {
                assert!(optimal.insert(record.clone()));
            }
            assert!(all.insert(record));
        }
    }
    Expected {
        all,
        optimal,
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
        correlated(),
        generated(),
        unscored(&[&["b", "n(foo)"], &["a", "b", "n(foo)"]]),
        unscored(&[&["n(#inf)"]]),
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
    let mut multiple_workers = false;
    for (source, expected) in sources::SOURCES.into_iter().zip(expectations()) {
        let input = source_records::admit(source, &FormulaLimits::default()).unwrap();
        assert_eq!(input.objectives().priorities(), expected.priorities);
        for (workers, batch) in [(1, 1), (4, 3)] {
            // The batched completion receipts this test reads belong to the
            // clause method.
            let config = SolveConfig {
                backend: Backend::Cpu,
                oracle: Oracle::Countermodel,
                search: zetesis_cli::SearchMethod::Clauses,
                models: 0,
                workers: NonZeroUsize::new(workers).unwrap(),
                completion_workers: NonZeroUsize::new(workers).unwrap(),
                batch_size: NonZeroUsize::new(batch).unwrap(),
                ..SolveConfig::default()
            };
            let mut session = Session::enumerate(
                PreparedInput::formula(&input),
                config,
                Cancellation::default(),
            )
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
            assert!(!outcome.optimum_proved());
            assert_eq!(outcome.unsatisfiable(), expected.all.is_empty());
            assert_eq!(outcome.formula_execution().is_some(), workers > 1);
            if let Some(execution) = outcome.formula_execution() {
                let completion = execution.completion;
                assert_eq!(completion.entered, completion.completed);
                assert_eq!(completion.failed, 0);
                assert_eq!(execution.pending_candidates, 0);
                assert_eq!(execution.queued_models, 0);
                assert!(!completion.overflowed);
                if completion.entered > 0 {
                    multiple_workers |= completion.effective_workers > 1;
                    assert_eq!(completion.requested_workers, workers);
                    assert!(completion.effective_workers <= workers.min(batch));
                }
            }
        }
    }
    assert!(multiple_workers);
}

#[test]
fn json_preserves_hidden_optimum_identity() {
    let options =
        Options::try_parse_from(["zetesis", "--backend", "cpu", "--models", "0", "--json"])
            .unwrap();
    let mut output = Vec::new();
    let report = run_with_diagnostics(
        sources::PROJECTED.into(),
        &options,
        &mut output,
        &mut Vec::new(),
        &Cancellation::default(),
    )
    .unwrap();
    assert_eq!(report.completion, Completion::Exhausted);
    let answers = zetesis_validation::answers::native_json::parse(
        &output,
        zetesis_validation::answers::native_json::Limits::default(),
    )
    .unwrap();
    let records: Records = answers
        .records()
        .iter()
        .map(|answer| {
            let shown: Vec<_> = answer
                .shown_atom_indices()
                .iter()
                .map(|&index| source_records::canonical(&answer.full_model()[index]))
                .collect();
            assert_eq!(shown, ["b"]);
            assert!(answer.shown_terms().is_empty());
            assert_eq!(answer.costs(), Some([(2, 0), (1, 0)].as_slice()));
            (
                answer
                    .full_model()
                    .iter()
                    .map(source_records::canonical)
                    .collect(),
                Some(vec![0, 0]),
            )
        })
        .collect();
    assert_eq!(answers.records().len(), 2);
    assert_eq!(records, objective_records(&[(&["b"], 0)]));
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
                &Cancellation::default(),
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
