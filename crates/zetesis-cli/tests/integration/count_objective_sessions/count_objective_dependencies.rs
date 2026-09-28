//! Count-head activity determines complete model scores before optimum selection.

use std::{collections::BTreeSet, num::NonZeroUsize};

use zetesis_core::Value;
use zetesis_cpu::Cancellation;
use zetesis_solve::{
    AnswerSelection, AnswerSet, Backend, Completion, Oracle, PreparedInput, SearchMethod,
    SemanticOutcome, Session, SolveConfig, Subject, WorldView, WorldViewLimits,
};
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionLimits, FormulaLimits, admit_formula,
};

use super::{Record, atom};

struct Case {
    source: &'static str,
    models: &'static [(&'static [&'static str], i32, i64)],
}

const CASES: &[Case] = &[
    // The head shares tuple 1, permitting every nonempty subset of {a,b}.
    // The body counts distinct tuples 1 and 2, so the pair costs two.
    Case {
        source: "1#count{1:a;1:b}1.n(N):-N=#count{1:a;2:b}.p(X):-n(X).#minimize{X@7:p(X)}.",
        models: &[(&["a"], 1, 1), (&["b"], 1, 1), (&["a", "b"], 2, 2)],
    },
    // One atom activates both head tuples, but only one body tuple.
    Case {
        source: "2#count{1:a;2:a}2.n(N):-N=#count{1:a}.p(X):-n(X).#minimize{X@7:p(X)}.",
        models: &[(&["a"], 1, 1)],
    },
    // The optional atom activates two tuples. Maximization negates its weight.
    Case {
        source: "0#count{1:a;2:a}2.n(N):-N=#count{1:a;2:a}.p(X):-n(X).#maximize{X@7:p(X)}.",
        models: &[(&[], 0, 0), (&["a"], 2, -2)],
    },
    // Without b no head atom is supported; with b every nonempty subset of
    // {a,c} is permitted. The body keeps their distinct tuple identities.
    Case {
        source: "{b}.1#count{1:a;1:c}1:-b.n(N):-N=#count{1:a;2:c}.p(X):-n(X).#minimize{X@7:p(X)}.",
        models: &[
            (&[], 0, 0),
            (&["a", "b"], 1, 1),
            (&["b", "c"], 1, 1),
            (&["a", "b", "c"], 2, 2),
        ],
    },
    // The head demands exactly one tuple, but a activates either zero or two.
    Case {
        source: "1#count{1:a;2:a}1.n(N):-N=#count{1:a}.p(X):-n(X).#minimize{X@7:p(X)}.",
        models: &[],
    },
];

impl Case {
    fn admit(&self) -> AdmittedFormula {
        let input = admit_formula(
            self.source.into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap_or_else(|error| panic!("{}: {error}", self.source));
        assert_eq!(input.source().text(), self.source);
        input
    }

    fn records(&self, selection: AnswerSelection) -> BTreeSet<Record> {
        let best = self.models.iter().map(|&(_, _, cost)| cost).min();
        self.models
            .iter()
            .filter(|&&(_, _, cost)| selection == AnswerSelection::All || Some(cost) == best)
            .map(|&(selected, value, cost)| {
                let atoms = selected
                    .iter()
                    .map(|name| atom(name, vec![]))
                    .chain(["n", "p"].map(|name| atom(name, vec![Value::Number(value)])))
                    .collect();
                (atoms, Some(vec![(7, cost)]))
            })
            .collect()
    }
}

fn config(workers: usize, batch: usize) -> SolveConfig {
    SolveConfig {
        backend: Backend::Cpu,
        // The batched completion and its accounting belong to the clause search.
        search: SearchMethod::Clauses,
        oracle: Oracle::Countermodel,
        models: 0,
        workers: NonZeroUsize::new(workers).unwrap(),
        completion_workers: NonZeroUsize::new(workers).unwrap(),
        batch_size: NonZeroUsize::new(batch).unwrap(),
        ..SolveConfig::default()
    }
}

fn record(answer: &AnswerSet, subject: &Subject) -> Record {
    assert!(answer.subject().same_instance(subject));
    let score = answer.score().expect("the numeric objective is present");
    assert!(score.is_present());
    (
        answer
            .interpretation()
            .atoms()
            .iter()
            .map(|atom| atom.to_atom(zetesis_core::ValueLimits::default()).unwrap())
            .collect(),
        Some(score.costs().to_vec()),
    )
}

fn exhausted(outcome: &SemanticOutcome, subject: &Subject, workers: usize, batch: usize) {
    assert!(outcome.subject().unwrap().same_instance(subject));
    assert_eq!(outcome.completion(), Some(Completion::Exhausted));
    assert!(outcome.interruption().is_none());
    assert_eq!(outcome.formula_execution().is_some(), workers > 1);
    if let Some(execution) = outcome.formula_execution() {
        assert_eq!(execution.pending_candidates, 0);
        assert_eq!(execution.queued_models, 0);
        let accounting = execution.completion;
        assert_eq!(accounting.entered, accounting.completed);
        assert_eq!(accounting.failed, 0);
        assert!(!accounting.overflowed);
        if outcome.verified_models() > 0 {
            assert!(accounting.entered > 0);
            assert_eq!(accounting.requested_workers, workers);
            assert!(accounting.effective_workers > 0);
            assert!(accounting.effective_workers <= workers.min(batch));
        }
    }
}

#[test]
fn count_dependencies_preserve_complete_scored_families() {
    for case in CASES {
        let input = case.admit();
        let subject = Subject::Theory(input.theory().clone());
        let expected = case.records(AnswerSelection::All);
        for workers in [1, 4] {
            for batch in [1, 8] {
                let family = WorldView::collect(
                    PreparedInput::formula(&input),
                    config(workers, batch),
                    WorldViewLimits::default(),
                    Cancellation::default(),
                )
                .unwrap();
                assert!(family.subject().same_instance(&subject));
                let actual: BTreeSet<_> = family
                    .answer_sets()
                    .iter()
                    .map(|answer| record(answer, &subject))
                    .collect();
                assert_eq!(family.len(), expected.len(), "{}", case.source);
                assert_eq!(actual, expected, "{}", case.source);
                let outcome = family.outcome();
                exhausted(outcome, &subject, workers, batch);
                assert_eq!(outcome.selection(), Some(AnswerSelection::All));
                assert_eq!(outcome.unsatisfiable(), expected.is_empty());
                assert_eq!(
                    outcome.verified_models(),
                    u64::try_from(expected.len()).unwrap()
                );
                assert_eq!(outcome.scored_models(), outcome.verified_models());
                assert!(outcome.incumbent().is_none());
                assert!(!outcome.optimum_proved());
                assert_eq!(outcome.retained_models(), 0);
                assert_eq!(
                    outcome
                        .countermodel_statistics()
                        .unwrap()
                        .candidate_restrictions,
                    0
                );
            }
        }
    }
}

#[test]
fn count_dependencies_preserve_every_optimal_answer() {
    for case in CASES {
        let input = case.admit();
        let subject = Subject::Theory(input.theory().clone());
        let expected = case.records(AnswerSelection::Optimal);
        for workers in [1, 4] {
            for batch in [1, 8] {
                let mut session = Session::new(
                    PreparedInput::formula(&input),
                    config(workers, batch),
                    Cancellation::default(),
                )
                .unwrap();
                let actual: Vec<_> = session
                    .by_ref()
                    .map(|answer| record(&answer.unwrap(), &subject))
                    .collect();
                assert_eq!(actual.len(), expected.len(), "{}", case.source);
                assert_eq!(
                    actual.into_iter().collect::<BTreeSet<_>>(),
                    expected,
                    "{}",
                    case.source
                );
                let outcome = session.outcome().unwrap();
                exhausted(&outcome, &subject, workers, batch);
                assert_eq!(outcome.selection(), Some(AnswerSelection::Optimal));
                assert_eq!(outcome.unsatisfiable(), expected.is_empty());
                assert_eq!(outcome.optimum_proved(), !expected.is_empty());
                assert_eq!(outcome.retained_models(), expected.len());
                if let Some(incumbent) = outcome.incumbent() {
                    assert_eq!(
                        incumbent.tied_models,
                        u64::try_from(expected.len()).unwrap()
                    );
                    for (_, score) in &expected {
                        assert_eq!(Some(incumbent.score.costs()), score.as_deref());
                    }
                } else {
                    assert!(expected.is_empty());
                }
            }
        }
    }
}

mod reference;
