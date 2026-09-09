//! Count-head activity composes with forwarded objectives in prepared sessions.

#[path = "support/count_objective_sources.rs"]
mod count_objective_sources;

use count_objective_sources::{INCONSISTENT, SATISFIABLE};

use std::collections::BTreeSet;
use std::num::NonZeroUsize;

use zetesis_cli::{Backend, Completion, PreparedInput, Session, SolveConfig};
use zetesis_core::{Atom, Predicate, Value};
use zetesis_cpu::Control;
use zetesis_themelios::{
    AdmissionFailure, AdmissionOptions, ExpansionFailure, ExpansionLimits, FormulaFailure,
    FormulaLimits, ProfileFeature, admit_formula,
};

type Record = (BTreeSet<Atom>, Option<Vec<(i32, i64)>>);

struct Case {
    source: &'static str,
    models: &'static [(&'static [&'static str], i32)],
    direction: i64,
}

// Original programs are retained with fresh clingo results in the tranche
// verification record. The aliased group and objective producer are independent:
// an objective-relevant count head retains a separate admission obligation.
// Expected models include every hidden derived atom.
const CASES: &[Case] = &[
    Case {
        source: SATISFIABLE[0],
        models: &[(&["a"], 0), (&["b"], 0), (&["a", "b"], 0)],
        direction: 1,
    },
    Case {
        source: SATISFIABLE[1],
        models: &[(&["a"], 0)],
        direction: 1,
    },
    Case {
        source: SATISFIABLE[2],
        models: &[(&["d"], 2), (&["a", "d"], 2)],
        direction: -1,
    },
    Case {
        source: SATISFIABLE[3],
        models: &[
            (&[], 0),
            (&["a", "b"], 0),
            (&["b", "c"], 0),
            (&["a", "b", "c"], 0),
        ],
        direction: 1,
    },
    Case {
        source: INCONSISTENT,
        models: &[],
        direction: 1,
    },
];

#[test]
fn objective_dependent_count_heads_remain_refused() {
    for source in [
        "1#count{1:a;1:b}1.n(N):-N=#count{1:a;2:b}.p(X):-n(X).#minimize{X@7:p(X)}.",
        "2#count{1:a;2:a}2.n(N):-N=#count{1:a}.p(X):-n(X).#minimize{X@7:p(X)}.",
        "0#count{1:a;2:a}2.n(N):-N=#count{1:a;2:a}.p(X):-n(X).#maximize{X@7:p(X)}.",
        "{b}.1#count{1:a;1:c}1:-b.n(N):-N=#count{1:a;2:c}.p(X):-n(X).#minimize{X@7:p(X)}.",
        "1#count{1:a;2:a}1.n(N):-N=#count{1:a}.p(X):-n(X).#minimize{X@7:p(X)}.",
    ] {
        let error = admit_formula(
            source.into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap_err();
        assert!(!error.diagnostics().is_empty());
        assert!(matches!(
            error,
            FormulaFailure::Expansion(ExpansionFailure::Admission(AdmissionFailure::Profile {
                feature: ProfileFeature::ObjectiveAggregateDependency,
                ..
            }))
        ));
    }
}

fn atom(name: &str, values: Vec<Value>) -> Atom {
    Atom::new(Predicate::new(name, values.len()).unwrap(), values).unwrap()
}

fn expected(case: &Case) -> BTreeSet<Record> {
    case.models
        .iter()
        .map(|&(selected, value)| {
            let atoms = selected
                .iter()
                .map(|name| atom(name, vec![]))
                .chain(["n", "p"].map(|name| atom(name, vec![Value::Number(value)])))
                .collect();
            (atoms, Some(vec![(7, i64::from(value) * case.direction)]))
        })
        .collect()
}

#[test]
fn prepared_sessions_preserve_all_optimal_models() {
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
                    assert_eq!(outcome.completion(), Some(Completion::Exhausted));
                    assert_eq!(actual, expected(case), "{}", case.source);
                    assert_eq!(outcome.optimum_proved(), !case.models.is_empty());
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
