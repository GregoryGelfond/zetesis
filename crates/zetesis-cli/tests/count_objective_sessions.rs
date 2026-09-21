//! Count-head activity composes with forwarded objectives in prepared sessions.

#[path = "support/clingo_report.rs"]
mod clingo_report;
#[path = "support/count_objective_sources.rs"]
mod count_objective_sources;
#[path = "support/count_objective_dependencies.rs"]
mod count_objective_dependencies;

use count_objective_sources::{INCONSISTENT, SATISFIABLE};

use std::collections::BTreeSet;
use std::num::NonZeroUsize;

use zetesis_cli::{
    Backend, Completion, Interruption, Oracle, PreparedInput, SearchMethod, Session, SolveConfig,
};
use zetesis_core::{Atom, Predicate, Value};
use zetesis_cpu::Cancellation;
use zetesis_sat::Incomplete;
use zetesis_themelios::{AdmissionOptions, ExpansionLimits, FormulaLimits, admit_formula};

type Record = (BTreeSet<Atom>, Option<Vec<(i32, i64)>>);

struct Case {
    source: &'static str,
    models: &'static [(&'static [&'static str], Value)],
    costs: Option<&'static [(i32, i64)]>,
}

// The independent objective producer selects these constant-cost optimum ties.
// The dependency contracts separately enumerate costs derived from head activity.
// Both include every hidden derived atom.
const CASES: &[Case] = &[
    Case {
        source: SATISFIABLE[0],
        models: &[
            (&["a"], Value::Number(0)),
            (&["b"], Value::Number(0)),
            (&["a", "b"], Value::Number(0)),
        ],
        costs: Some(&[(7, 0)]),
    },
    Case {
        source: SATISFIABLE[1],
        models: &[(&["a"], Value::Number(0))],
        costs: Some(&[(7, 0)]),
    },
    Case {
        source: SATISFIABLE[2],
        models: &[(&["d"], Value::Number(2)), (&["a", "d"], Value::Number(2))],
        costs: Some(&[(7, -2)]),
    },
    Case {
        source: SATISFIABLE[3],
        models: &[
            (&[], Value::Number(0)),
            (&["a", "b"], Value::Number(0)),
            (&["b", "c"], Value::Number(0)),
            (&["a", "b", "c"], Value::Number(0)),
        ],
        costs: Some(&[(7, 0)]),
    },
    Case {
        source: SATISFIABLE[4],
        models: &[
            (&["a"], Value::Supremum),
            (&["b"], Value::Supremum),
            (&["a", "b"], Value::Supremum),
        ],
        costs: None,
    },
    Case {
        source: INCONSISTENT,
        models: &[],
        costs: Some(&[(7, 0)]),
    },
];

fn atom(name: &str, values: Vec<Value>) -> Atom {
    Atom::new(Predicate::new(name, values.len()).unwrap(), values).unwrap()
}

fn expected(case: &Case) -> BTreeSet<Record> {
    case.models
        .iter()
        .map(|(selected, value)| {
            let atoms = selected
                .iter()
                .map(|name| atom(name, vec![]))
                .chain(["n", "p"].map(|name| atom(name, vec![value.clone()])))
                .collect();
            (atoms, case.costs.map(<[_]>::to_vec))
        })
        .collect()
}

#[test]
fn prepared_sessions_preserve_complete_model_records() {
    let mut observed_pruning = false;
    let mut observed_batched_work = false;
    for case in CASES {
        let input = admit_formula(
            case.source.into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap();
        for workers in [1, 4] {
            for batch in [1, 8] {
                for pruning in [false, true] {
                    let mut config = SolveConfig {
                        backend: Backend::Cpu,
                        // The batched completion and its accounting belong to the clause search.
                        search: SearchMethod::Clauses,
                        models: 0,
                        workers: NonZeroUsize::new(workers).unwrap(),
                        completion_workers: NonZeroUsize::new(workers).unwrap(),
                        batch_size: NonZeroUsize::new(batch).unwrap(),
                        ..Default::default()
                    };
                    if !pruning {
                        config.max_objective_bound_work = 0;
                    }
                    let mut session = Session::new(
                        PreparedInput::formula(&input),
                        config,
                        Cancellation::default(),
                    )
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
                    assert_eq!(outcome.completion(), Some(Completion::Exhausted));
                    assert_eq!(actual, expected(case), "{}", case.source);
                    assert_eq!(
                        outcome.optimum_proved(),
                        case.costs.is_some() && !case.models.is_empty()
                    );
                    assert_eq!(outcome.unsatisfiable(), case.models.is_empty());
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
                        let accounting = execution.completion;
                        assert_eq!(accounting.entered, accounting.completed);
                        assert_eq!(accounting.failed, 0);
                        assert_eq!(execution.pending_candidates, 0);
                        assert_eq!(execution.queued_models, 0);
                        if accounting.entered > 0 {
                            observed_batched_work = true;
                            assert_eq!(accounting.requested_workers, workers);
                            assert!(accounting.effective_workers <= workers.min(batch));
                        }
                    }
                }
            }
        }
    }
    assert!(observed_pruning);
    assert!(observed_batched_work);
}

fn bounded_config(workers: usize, batch: usize, pruning: bool) -> SolveConfig {
    let mut config = SolveConfig {
        backend: Backend::Cpu,
        // The batched completion and its accounting belong to the clause search.
        search: SearchMethod::Clauses,
        oracle: Oracle::Countermodel,
        models: 0,
        completion_workers: NonZeroUsize::new(workers).unwrap(),
        batch_size: NonZeroUsize::new(batch).unwrap(),
        ..Default::default()
    };
    if !pruning {
        config.max_objective_bound_work = 0;
    }
    config
}

#[test]
fn completion_refusal_retains_pending_candidates() {
    let input = admit_formula(
        SATISFIABLE[0].into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    for workers in [2, 4] {
        for batch in [3, 5] {
            for pruning in [false, true] {
                let config = SolveConfig {
                    max_completion_scratch_bytes: 0,
                    ..bounded_config(workers, batch, pruning)
                };
                let mut session = Session::new(
                    PreparedInput::formula(&input),
                    config,
                    Cancellation::default(),
                )
                .unwrap();
                assert!(session.next().is_none());
                let outcome = session.outcome().unwrap();
                assert_eq!(outcome.completion(), Some(Completion::Interrupted));
                assert_eq!(
                    outcome.interruption(),
                    Some(Interruption::Countermodel(Incomplete::CompletionScratch))
                );
                assert!(!outcome.optimum_proved());
                assert!(!outcome.unsatisfiable());
                assert!(outcome.incumbent().is_none());
                assert_eq!(outcome.verified_models(), 0);
                assert_eq!(outcome.scored_models(), 0);
                assert_eq!(outcome.retained_models(), 0);
                assert_eq!(outcome.candidate_progress(), u64::try_from(batch).unwrap());
                let execution = outcome.formula_execution().unwrap();
                assert_eq!(execution.pending_candidates, batch);
                assert_eq!(execution.queued_models, 0);
                let accounting = execution.completion;
                assert_eq!(accounting.entered, 0);
                assert_eq!(accounting.completed, 0);
                assert_eq!(accounting.failed, 0);
                assert_eq!(accounting.peak_scratch_bytes, 0);
                assert!(!accounting.overflowed);
            }
        }
    }
}

#[test]
fn candidate_ceiling_retains_an_unproved_incumbent() {
    let input = admit_formula(
        SATISFIABLE[0].into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    // The independent atom d selects n(0) or n(1); all three nonempty
    // subsets of {a,b} remain allowed. Two candidates cannot exhaust them.
    let original_models: BTreeSet<Record> = [0, 1]
        .into_iter()
        .flat_map(|value| {
            [vec!["a"], vec!["b"], vec!["a", "b"]]
                .into_iter()
                .map(move |mut selected| {
                    if value == 1 {
                        selected.push("d");
                    }
                    let atoms = selected
                        .into_iter()
                        .map(|name| atom(name, vec![]))
                        .chain(["n", "p"].map(|name| atom(name, vec![Value::Number(value)])))
                        .collect();
                    (atoms, Some(vec![(7, i64::from(value))]))
                })
        })
        .collect();
    for workers in [2, 4] {
        for batch in [3, 5] {
            for pruning in [false, true] {
                let config = SolveConfig {
                    max_candidates: 2,
                    ..bounded_config(workers, batch, pruning)
                };
                let mut session = Session::new(
                    PreparedInput::formula(&input),
                    config,
                    Cancellation::default(),
                )
                .unwrap();
                let mut delivered = BTreeSet::new();
                for result in session.by_ref() {
                    let model = result.unwrap();
                    assert!(delivered.insert((
                        model.interpretation().atoms().iter().cloned().collect(),
                        model.score().map(|score| score.costs().to_vec()),
                    )));
                }
                let outcome = session.outcome().unwrap();
                assert_eq!(outcome.completion(), Some(Completion::Interrupted));
                assert_eq!(
                    outcome.interruption(),
                    Some(Interruption::Countermodel(Incomplete::CandidateLimit))
                );
                assert!(!outcome.optimum_proved());
                assert!(!outcome.unsatisfiable());
                assert_eq!(outcome.candidate_progress(), 2);
                assert_eq!(outcome.verified_models(), 2);
                assert_eq!(outcome.scored_models(), 2);
                assert_eq!(outcome.retained_models(), delivered.len());
                assert!(!delivered.is_empty());
                assert!(delivered.is_subset(&original_models));
                let incumbent = outcome.incumbent().unwrap();
                assert_eq!(
                    incumbent.tied_models,
                    u64::try_from(delivered.len()).unwrap()
                );
                for (_, costs) in &delivered {
                    assert_eq!(costs.as_deref(), Some(incumbent.score.costs()));
                }
                let execution = outcome.formula_execution().unwrap();
                assert_eq!(execution.pending_candidates, 0);
                assert_eq!(execution.queued_models, 0);
                assert_eq!(execution.cpu_residuals, 2);
                let accounting = execution.completion;
                assert_eq!(accounting.entered, 2);
                assert_eq!(accounting.completed, 2);
                assert_eq!(accounting.residual_completed, 2);
                assert_eq!(accounting.failed, 0);
                assert_eq!(accounting.requested_workers, workers);
                assert_eq!(accounting.effective_workers, 2);
                assert!(!accounting.overflowed);
                if !pruning {
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
}
