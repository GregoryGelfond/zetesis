//! Independent complete answers for finite bindings and source measure carriers.

#[path = "support/finite_carrier_sources.rs"]
mod sources;
#[path = "../../zetesis-themelios/tests/support/source_records.rs"]
mod source_records;
#[path = "../../zetesis-themelios/tests/support/source_oracle.rs"]
mod source_oracle;

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
    priorities: &'static [i32],
}

impl Expected {
    fn unscored(models: &[&[&str]]) -> Self {
        Self {
            all: models
                .iter()
                .map(|atoms| (atoms.iter().map(|atom| (*atom).into()).collect(), None))
                .collect(),
            priorities: &[],
        }
    }

    fn scored(priorities: &'static [i32], rows: &[(&[&str], &[i64])]) -> Self {
        Self {
            all: rows
                .iter()
                .map(|(atoms, costs)| {
                    assert_eq!(costs.len(), priorities.len());
                    (
                        atoms.iter().map(|atom| (*atom).into()).collect(),
                        Some(costs.to_vec()),
                    )
                })
                .collect(),
            priorities,
        }
    }

    fn optima(&self) -> Records {
        // Independent expected vectors are already ordered by descending
        // priority. Lexicographic minimum retains every full optimum answer.
        let minimum = self.all.iter().map(|(_, costs)| costs).min();
        self.all
            .iter()
            .filter(|(_, costs)| Some(costs) == minimum)
            .cloned()
            .collect()
    }
}

fn expectations() -> [Expected; sources::SOURCES.len()] {
    [
        Expected::unscored(&[&["p(1,1)", "p(1,2)", "p(2,1)", "p(2,2)"]]),
        Expected::unscored(&[&["p(1,2)"], &["p(1,3)"], &["p(2,3)"]]),
        Expected::unscored(&[&[], &["p(1,1)"], &["p(2,2)"], &["p(1,1)", "p(2,2)", "q"]]),
        Expected::unscored(&[&["n(3)"]]),
        Expected::unscored(&[&["p(1,1)", "p(1,2)", "p(2,2)"]]),
        // Both count keys share a selector. The intermediate priority still
        // belongs to the source carrier, although n(1) is never an answer atom.
        Expected::scored(
            &[2, 1, 0],
            &[(&["n(0)"], &[0, 0, 1]), (&["a", "n(2)"], &[1, 0, 0])],
        ),
        Expected::scored(
            &[5, 3, 2, 0],
            &[(&["n(0)"], &[0, 0, 0, 0]), (&["a", "n(5)"], &[5, 0, 0, 0])],
        ),
        Expected::scored(
            &[5, 2],
            &[(&["r", "n(2)"], &[0, -1]), (&["r", "a", "n(5)"], &[-1, 0])],
        ),
        // The optional numeric maximum contributes a source priority even
        // though the shared selector makes each realized result nonnumeric.
        Expected::scored(&[2], &[(&["n(#inf)"], &[0]), (&["a", "n(foo)"], &[0])]),
        Expected::scored(
            &[3, 2],
            &[(&["n(#sup)"], &[0, 0]), (&["a", "n(2)"], &[0, 1])],
        ),
        Expected::scored(
            &[2, 1, 0],
            &[
                (&["n(0)", "rank(0)"], &[0, 0, 1]),
                (&["a", "n(2)", "rank(2)"], &[1, 0, 0]),
            ],
        ),
        Expected::scored(
            &[3, 2, 1, 0],
            &[
                (&["p(1,2)", "n(0)"], &[0, 0, 0, 1]),
                (&["p(1,2)", "a", "n(3)"], &[1, 0, 0, 0]),
            ],
        ),
    ]
}

#[test]
fn reduct_checks_preserve_complete_answer_families() {
    for (source, expected) in sources::SOURCES.into_iter().zip(expectations()) {
        let input = source_records::admit(source, &FormulaLimits::default()).unwrap();
        assert_eq!(
            input.objectives().priorities(),
            expected.priorities,
            "{source}"
        );
        assert_eq!(source_records::exhaustive(&input), expected.all, "{source}");
    }
}

#[test]
fn sessions_preserve_scored_answer_families() {
    let mut parallel_completion = false;
    for (source, expected) in sources::SOURCES.into_iter().zip(expectations()) {
        let input = source_records::admit(source, &FormulaLimits::default()).unwrap();
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
                    assert_eq!(
                        score.costs().iter().map(|&(p, _)| p).collect::<Vec<_>>(),
                        expected.priorities
                    );
                    score.costs().iter().map(|&(_, cost)| cost).collect()
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
                assert_eq!(
                    (execution.pending_candidates, execution.queued_models),
                    (0, 0)
                );
                assert!(!completion.overflowed);
                if completion.entered > 0 {
                    parallel_completion |= completion.effective_workers > 1;
                    assert_eq!(completion.requested_workers, workers);
                    assert!(completion.effective_workers <= workers.min(batch));
                }
            }
        }
    }
    assert!(parallel_completion);
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
                    costs.iter().map(|&(_, cost)| cost).collect()
                });
                assert!(
                    records.insert((
                        answer
                            .full_model()
                            .iter()
                            .map(source_records::canonical)
                            .collect(),
                        costs,
                    ))
                );
            }
            assert_eq!(records, expected.optima(), "{grounder}: {source}");
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
