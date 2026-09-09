//! Independent model records qualify composition across the bounded source extensions.

#[path = "support/language_value_sources.rs"]
mod language_value_sources;

#[path = "support/projected_conditional_sources.rs"]
mod projected_conditional_sources;

use std::collections::BTreeSet;
use std::num::NonZeroUsize;

use clap::Parser;
use zetesis_cli::{
    Backend, Completion, Interruption, Options, Oracle, PreparedInput, Session, SolveConfig,
    run_with_diagnostics,
};
use zetesis_core::{Atom, Predicate, Value};
use zetesis_cpu::Control;
use zetesis_sat::Incomplete;
use zetesis_themelios::{AdmissionOptions, ExpansionLimits, FormulaLimits, admit_formula};

#[path = "../../zetesis-themelios/tests/support/source_records.rs"]
mod source_records;
#[path = "../../zetesis-themelios/tests/support/source_oracle.rs"]
mod source_oracle;

type Record = (BTreeSet<Atom>, Option<Vec<(i32, i64)>>);

#[test]
#[ignore = "requires an independently installed clingo executable"]
fn projected_conditionals_match_original_source_records() {
    for source in projected_conditional_sources::SOURCES {
        let admitted = source_records::admit(source, &FormulaLimits::default()).unwrap();
        assert_eq!(
            source_records::exhaustive(&admitted),
            source_oracle::records(source),
            "{source}"
        );
    }
}

enum Observer {
    Direct,
    Forwarded,
}

struct Expected {
    choices: &'static [u8],
    marker: bool,
    observer: Option<Observer>,
    costs: Option<&'static [(i32, i64)]>,
}

// Bit one selects p(1,2); bit two selects p(2,4). The original conditional
// derives q exactly for the first pair. Bit four selects a when an observer exists. These are complete semantic atoms,
// including atoms hidden by any future rendering choice.
const EXPECTED: [Expected; language_value_sources::SOURCES.len()] = [
    Expected {
        choices: &[0, 1, 2, 3],
        marker: false,
        observer: None,
        costs: None,
    },
    Expected {
        choices: &[0, 1, 2, 3],
        marker: true,
        observer: None,
        costs: None,
    },
    Expected {
        choices: &[0, 1, 2, 3],
        marker: false,
        observer: None,
        costs: Some(&[(3, 0)]),
    },
    Expected {
        // Equal complete objective keys coalesce: either pair earns weight two.
        choices: &[1, 2, 3],
        marker: true,
        observer: None,
        costs: Some(&[(3, -2)]),
    },
    Expected {
        choices: &[0],
        marker: false,
        observer: None,
        costs: Some(&[(3, 0)]),
    },
    Expected {
        choices: &[],
        marker: false,
        observer: None,
        costs: None,
    },
    Expected {
        choices: &[0, 1, 2, 3],
        marker: true,
        observer: None,
        costs: None,
    },
    Expected {
        choices: &[0, 1, 2, 3],
        marker: false,
        observer: None,
        costs: Some(&[(3, 0)]),
    },
    Expected {
        choices: &[0, 1, 2, 3, 4, 5, 6, 7],
        marker: false,
        observer: Some(Observer::Direct),
        costs: None,
    },
    Expected {
        choices: &[0, 1, 2, 3, 4, 5, 6, 7],
        marker: true,
        observer: Some(Observer::Forwarded),
        costs: Some(&[(3, 0)]),
    },
];

fn atom(name: &str, values: Vec<Value>) -> Atom {
    Atom::new(Predicate::new(name, values.len()).unwrap(), values).unwrap()
}

fn records(expected: &Expected) -> BTreeSet<Record> {
    expected
        .choices
        .iter()
        .map(|mask| {
            let mut atoms = BTreeSet::new();
            if mask & 1 != 0 {
                atoms.insert(atom("p", vec![Value::Number(1), Value::Number(2)]));
                atoms.insert(atom("q", vec![]));
            }
            if mask & 2 != 0 {
                atoms.insert(atom("p", vec![Value::Number(2), Value::Number(4)]));
            }
            if expected.marker {
                atoms.insert(atom("marker", vec![]));
            }
            if let Some(observer) = &expected.observer {
                atoms.insert(atom("b", vec![]));
                atoms.insert(atom("n", vec![Value::Symbol("foo".into())]));
                if mask & 4 != 0 {
                    atoms.insert(atom("a", vec![]));
                }
                if matches!(observer, Observer::Forwarded) {
                    atoms.insert(atom("v", vec![Value::Symbol("foo".into())]));
                }
            }
            (atoms, expected.costs.map(<[_]>::to_vec))
        })
        .collect()
}

#[test]
fn source_extensions_preserve_complete_session_records() {
    let mut observed_batched_work = false;
    let mut observed_pruning = false;
    for (source, expected) in language_value_sources::SOURCES.into_iter().zip(&EXPECTED) {
        let input = admit_formula(
            source.into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap();
        for workers in [1, 4] {
            for batch in [1, 7] {
                for pruning in [false, true] {
                    let mut config = SolveConfig {
                        backend: Backend::Cpu,
                        oracle: Oracle::Countermodel,
                        models: 0,
                        workers: NonZeroUsize::new(workers).unwrap(),
                        completion_workers: NonZeroUsize::new(workers).unwrap(),
                        batch_size: NonZeroUsize::new(batch).unwrap(),
                        ..Default::default()
                    };
                    if !pruning {
                        config.max_objective_bound_work = 0;
                    }
                    let mut session =
                        Session::new(PreparedInput::formula(&input), config, Control::default())
                            .unwrap();
                    let mut actual = BTreeSet::new();
                    for result in session.by_ref() {
                        let model = result.unwrap();
                        assert!(actual.insert((
                            model.interpretation().atoms().iter().cloned().collect(),
                            model.score().map(|score| score.costs().to_vec()),
                        )));
                    }
                    let outcome = session.outcome().unwrap();
                    assert_eq!(actual, records(expected), "{source}");
                    assert_eq!(outcome.completion(), Some(Completion::Exhausted));
                    assert_eq!(outcome.unsatisfiable(), expected.choices.is_empty());
                    assert_eq!(
                        outcome.optimum_proved(),
                        expected.costs.is_some() && !expected.choices.is_empty()
                    );
                    let restrictions = outcome
                        .countermodel_statistics()
                        .unwrap()
                        .candidate_restrictions;
                    if pruning {
                        observed_pruning |= restrictions > 0;
                    } else {
                        assert_eq!(restrictions, 0);
                    }
                    assert_eq!(outcome.formula_execution().is_some(), workers > 1);
                    if let Some(execution) = outcome.formula_execution() {
                        let completion = execution.completion;
                        assert_eq!(completion.entered, completion.completed);
                        assert_eq!(completion.failed, 0);
                        assert_eq!(execution.pending_candidates, 0);
                        assert_eq!(execution.queued_models, 0);
                        assert!(!completion.overflowed);
                        if completion.entered > 0 {
                            observed_batched_work = true;
                            assert_eq!(completion.requested_workers, workers);
                            assert!(completion.effective_workers <= workers.min(batch));
                        }
                    }
                }
            }
        }
    }
    assert!(observed_batched_work);
    assert!(observed_pruning);
}

#[test]
fn automatic_admission_preserves_typed_json_records() {
    for (source, expected) in language_value_sources::SOURCES.into_iter().zip(&EXPECTED) {
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
            let mut actual = BTreeSet::new();
            for model in answers.records() {
                assert!(actual.insert((
                    model.full_model().iter().cloned().collect(),
                    model.costs().map(<[_]>::to_vec),
                )));
            }
            assert_eq!(actual, records(expected), "{grounder}: {source}");
            assert_eq!(answers.satisfiable(), !expected.choices.is_empty());
        }
    }
}

#[test]
fn stopped_composition_preserves_objective_presence() {
    // Each pair contrasts absent objectives with a retained numeric-zero
    // priority, first for literals and then for proved extremum exclusions.
    for index in [0, 2, 8, 9] {
        let input = source_records::admit(
            language_value_sources::SOURCES[index],
            &FormulaLimits::default(),
        )
        .unwrap();
        let expected = records(&EXPECTED[index]);
        for workers in [2, 4] {
            for batch in [3, 7] {
                let config = SolveConfig {
                    backend: Backend::Cpu,
                    oracle: Oracle::Countermodel,
                    models: 0,
                    completion_workers: NonZeroUsize::new(workers).unwrap(),
                    batch_size: NonZeroUsize::new(batch).unwrap(),
                    max_candidates: 1,
                    ..Default::default()
                };
                let mut session =
                    Session::new(PreparedInput::formula(&input), config, Control::default())
                        .unwrap();
                let mut actual = BTreeSet::new();
                for result in session.by_ref() {
                    let model = result.unwrap();
                    assert!(actual.insert((
                        model.interpretation().atoms().iter().cloned().collect(),
                        model.score().map(|score| score.costs().to_vec()),
                    )));
                }
                assert_eq!(actual.len(), 1);
                assert!(actual.is_subset(&expected));
                let outcome = session.outcome().unwrap();
                assert_eq!(outcome.completion(), Some(Completion::Interrupted));
                assert_eq!(
                    outcome.interruption(),
                    Some(Interruption::Countermodel(Incomplete::CandidateLimit))
                );
                assert_eq!(outcome.candidate_progress(), 1);
                assert_eq!(outcome.verified_models(), 1);
                assert!(!outcome.unsatisfiable());
                assert!(!outcome.optimum_proved());
                assert_eq!(
                    outcome.incumbent().is_some(),
                    EXPECTED[index].costs.is_some()
                );
                assert_eq!(
                    outcome.scored_models(),
                    u64::from(EXPECTED[index].costs.is_some())
                );
                let execution = outcome.formula_execution().unwrap();
                assert_eq!(execution.pending_candidates, 0);
                assert_eq!(execution.queued_models, 0);
                assert_eq!(execution.completion.entered, 1);
                assert_eq!(execution.completion.completed, 1);
                assert_eq!(execution.completion.failed, 0);
            }
        }
    }
}

#[test]
#[ignore = "requires an independently installed clingo executable"]
fn original_sources_match_complete_reference_records() {
    for (source, expected) in language_value_sources::SOURCES.into_iter().zip(&EXPECTED) {
        let reference = source_oracle::records(source);
        let input = source_records::admit(source, &FormulaLimits::default()).unwrap();
        assert_eq!(source_records::exhaustive(&input), reference, "{source}");
        // The external campaign enumerates every model and score. Lexicographic
        // minimum selects all optimum ties; absent scores retain every model.
        let minimum = reference.iter().map(|(_, costs)| costs).min();
        let selected: source_records::Records = reference
            .iter()
            .filter(|(_, costs)| Some(costs) == minimum)
            .cloned()
            .collect();
        let expected = records(expected)
            .into_iter()
            .map(|(atoms, costs)| {
                (
                    atoms.iter().map(source_records::canonical).collect(),
                    costs.map(|values| values.into_iter().map(|(_, value)| value).collect()),
                )
            })
            .collect();
        assert_eq!(selected, expected, "{source}");
    }
}
