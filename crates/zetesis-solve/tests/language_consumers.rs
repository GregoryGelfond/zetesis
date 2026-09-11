//! Full answer identity survives aggregate contributions, costs and observations.

use std::{
    collections::{BTreeMap, BTreeSet},
    num::NonZeroUsize,
    path::{Path, PathBuf},
    time::Duration,
};

use zetesis_core::{Atom, Predicate, Value};
use zetesis_cpu::Control;
use zetesis_solve::{
    AnswerSelection, AnswerSet, Backend, Completion, ExecutionResources, Grounder, Oracle,
    PreparedInput, Session, SolveConfig, WorldViewLimits,
};
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionLimits, FormulaLimits, admit_formula, observation,
};
use zetesis_validation::{answers, process};

const REPORT_BYTES: usize = 64 * 1024;
const CLEANUP_TIMEOUT: Duration = Duration::from_secs(1);

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Record {
    atoms: Vec<Atom>,
    costs: Vec<(i32, i64)>,
    display: String,
}

fn record(atoms: &[&str], priority: i32, cost: i64, display: &str) -> Record {
    Record {
        atoms: atoms.iter().map(|name| atom(name, vec![])).collect(),
        costs: vec![(priority, cost)],
        display: display.into(),
    }
}

fn atom(name: &str, values: Vec<Value>) -> Atom {
    Atom::new(Predicate::new(name, values.len()).unwrap(), values).unwrap()
}

struct Case {
    source: &'static str,
    file: &'static str,
    answers: BTreeSet<Record>,
    reference_difference: Option<DisplayRecord>,
}

#[derive(Debug, PartialEq, Eq)]
struct DisplayRecord {
    costs: Vec<i64>,
    displays: Vec<(Vec<String>, u64)>,
}

fn cases() -> [Case; 8] {
    [
        Case {
            source: include_str!("fixtures/language-consumers/symbolic-weight.lp"),
            file: "symbolic-weight.lp",
            answers: BTreeSet::from([record(&[], 1, 1, "value(2)"), record(&["a"], 1, 0, "")]),
            // Abstract Gringo gives the symbolic tuple zero weight and retains
            // its independent head permission. clingo 5.8.2 drops that element.
            reference_difference: Some(DisplayRecord {
                costs: vec![1],
                displays: vec![(vec!["value(2)".into()], 1)],
            }),
        },
        Case {
            source: include_str!("fixtures/language-consumers/neutral-weight.lp"),
            file: "neutral-weight.lp",
            reference_difference: None,
            answers: BTreeSet::from([
                record(&[], 2, 1, ""),
                record(&["a"], 2, 0, ""),
                record(&["b"], 2, 1, "value(2)"),
                record(&["a", "b"], 2, 0, "value(2)"),
            ]),
        },
        Case {
            source: include_str!("fixtures/language-consumers/missing-measure.lp"),
            file: "missing-measure.lp",
            answers: BTreeSet::from([
                record(&[], 3, 0, ""),
                record(&["a"], 3, 0, ""),
                record(&["b"], 3, 0, "a"),
                record(&["a", "b"], 3, 0, "a"),
            ]),
            reference_difference: Some(DisplayRecord {
                costs: vec![0],
                displays: vec![(vec![], 1), (vec!["a".into()], 1)],
            }),
        },
        Case {
            source: include_str!("fixtures/language-consumers/filtered-priority.lp"),
            file: "filtered-priority.lp",
            reference_difference: None,
            answers: BTreeSet::from([
                Record {
                    atoms: vec![atom("n", vec![Value::Number(0)])],
                    costs: vec![(1, 0)],
                    display: "score(1)".into(),
                },
                Record {
                    atoms: vec![atom("a", vec![]), atom("n", vec![Value::Number(1)])],
                    costs: vec![(1, 1)],
                    display: "score(2)".into(),
                },
            ]),
        },
        Case {
            source: include_str!("fixtures/language-consumers/arithmetic-keys.lp"),
            file: "arithmetic-keys.lp",
            reference_difference: None,
            answers: [(1, 2, "chosen(11)"), (2, 4, "chosen(12)")]
                .map(|(choice, cost, display)| Record {
                    atoms: vec![
                        atom("data", vec![Value::Number(1)]),
                        atom("data", vec![Value::Number(2)]),
                        atom("pick", vec![Value::Number(choice)]),
                    ],
                    costs: vec![(1, cost)],
                    display: display.into(),
                })
                .into(),
        },
        independent_objectives(),
        Case {
            source: include_str!("fixtures/language-consumers/scoped-objectives.lp"),
            file: "scoped-objectives.lp",
            reference_difference: None,
            answers: [
                (&["a"][..], [(2, 1), (1, 1)], "score(1)"),
                (&["b"][..], [(2, 1), (1, 0)], "score(1)"),
                (&["a", "b"][..], [(2, 2), (1, 1)], "score(2)"),
            ]
            .map(|(atoms, costs, display)| Record {
                atoms: atoms.iter().map(|name| atom(name, vec![])).collect(),
                costs: costs.into(),
                display: display.into(),
            })
            .into(),
        },
        Case {
            source: include_str!("fixtures/language-consumers/shared-tuple.lp"),
            file: "shared-tuple.lp",
            reference_difference: None,
            answers: BTreeSet::from([
                record(&["a"], 1, 1, "choice(6)"),
                record(&["b"], 1, 1, "choice(6)"),
                record(&["a", "b"], 1, 0, "choice(6)"),
            ]),
        },
    ]
}

fn independent_objectives() -> Case {
    Case {
        source: include_str!("fixtures/language-consumers/independent-objectives.lp"),
        file: "independent-objectives.lp",
        reference_difference: None,
        answers: [
            (&[][..], [(2, 1), (1, 0)], ""),
            (&["a"][..], [(2, 0), (1, 0)], "flag(a)"),
            (&["b", "p", "q"][..], [(2, 1), (1, 1)], "flag(b)"),
            (
                &["a", "b", "p", "q"][..],
                [(2, 0), (1, 1)],
                "flag(a) flag(b)",
            ),
        ]
        .map(|(atoms, costs, display)| Record {
            atoms: atoms.iter().map(|name| atom(name, vec![])).collect(),
            costs: costs.into(),
            display: display.into(),
        })
        .into(),
    }
}

fn admit(case: &Case) -> AdmittedFormula {
    admit_formula(
        case.source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap_or_else(|error| panic!("{}: {error}", case.source))
}

fn config(workers: usize, batch: usize) -> SolveConfig {
    SolveConfig {
        backend: Backend::Cpu,
        grounder: Grounder::Eager,
        oracle: Oracle::Countermodel,
        models: 0,
        workers: NonZeroUsize::new(workers).unwrap(),
        completion_workers: NonZeroUsize::new(workers).unwrap(),
        batch_size: NonZeroUsize::new(batch).unwrap(),
        ..Default::default()
    }
}

fn capture(owner: &AdmittedFormula, answer: &AnswerSet) -> Record {
    let model = answer.interpretation();
    let atoms = model.atoms().iter().cloned().collect();
    let score = answer.score().expect("the numeric objective is present");
    assert!(score.is_present());
    let display = owner
        .metadata()
        .observations()
        .render(
            model,
            owner.metadata().output(),
            observation::Limits::default(),
            &Control::default(),
        )
        .unwrap();
    Record {
        atoms,
        costs: score.costs().to_vec(),
        display: display.text().to_owned(),
    }
}

#[test]
fn original_families_retain_their_scored_observations() {
    for (workers, batch) in [(1, 1), (4, 3)] {
        families(config(workers, batch), &ExecutionResources::default());
    }
}

fn families(config: SolveConfig, resources: &ExecutionResources) {
    for case in cases() {
        let owner = admit(&case);
        let world_view =
            Session::builder(PreparedInput::formula(&owner), config, Control::default())
                .resources(resources)
                .collect(WorldViewLimits::default())
                .unwrap();
        assert_eq!(world_view.len(), case.answers.len(), "{}", case.source);
        let actual: BTreeSet<_> = world_view
            .answer_sets()
            .iter()
            .map(|answer| capture(&owner, answer))
            .collect();
        assert_eq!(actual, case.answers, "{}", case.source);
        assert_eq!(
            world_view.outcome().completion(),
            Some(Completion::Exhausted)
        );
        device_evidence(world_view.outcome(), config.backend, &case);
    }
}

#[test]
fn optimum_ties_retain_distinct_full_answers() {
    for (workers, batch) in [(1, 1), (4, 3)] {
        optimum(config(workers, batch), &ExecutionResources::default());
    }
}

fn optimum(config: SolveConfig, resources: &ExecutionResources) {
    for case in cases() {
        let owner = admit(&case);
        let best = case
            .answers
            .iter()
            .map(|answer| &answer.costs)
            .min()
            .unwrap();
        let expected: BTreeSet<_> = case
            .answers
            .iter()
            .filter(|answer| &answer.costs == best)
            .collect();
        let mut session =
            Session::builder(PreparedInput::formula(&owner), config, Control::default())
                .resources(resources)
                .selection(AnswerSelection::Optimal)
                .start()
                .unwrap();
        let answers: Vec<_> = session
            .by_ref()
            .map(|answer| capture(&owner, &answer.unwrap()))
            .collect();
        assert_eq!(answers.len(), expected.len(), "{}", case.source);
        assert_eq!(answers.iter().collect::<BTreeSet<_>>(), expected);
        assert!(session.outcome().unwrap().optimum_proved());
        device_evidence(&session.outcome().unwrap(), config.backend, &case);
    }
}

fn device_evidence(outcome: &zetesis_solve::SemanticOutcome, backend: Backend, case: &Case) {
    // This source necessarily has several complete candidates. Requiring
    // actual device work prevents a forced-device test from qualifying CPU-only
    // completion or an empty candidate path.
    if backend == Backend::Metal && case.file == "shared-tuple.lp" {
        let execution = outcome
            .formula_execution()
            .expect("formula device execution");
        assert!(execution.adapter.contains("Metal"));
        assert!(execution.gpu_batches > 0);
        assert!(execution.gpu_work > 0);
        assert!(execution.gpu_candidates > 0);
        assert_eq!(
            execution.gpu_decided + execution.cpu_residuals,
            execution.gpu_candidates
        );
        assert_eq!(
            (execution.pending_candidates, execution.queued_models),
            (0, 0)
        );
    }
}

#[test]
fn observation_failure_preserves_the_complete_family() {
    let source = "1#sum{1:a;1:b}1. #minimize{1@1,k:not a;1@1,l:not b}. #show. #show broken(1/0).";
    let owner = admit_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    let family = Session::builder(
        PreparedInput::formula(&owner),
        config(4, 3),
        Control::default(),
    )
    .collect(WorldViewLimits::default())
    .unwrap();
    let expected: BTreeSet<_> = cases()
        .into_iter()
        .find(|case| case.file == "shared-tuple.lp")
        .unwrap()
        .answers
        .into_iter()
        .map(|answer| (answer.atoms, answer.costs))
        .collect();
    let actual: BTreeSet<_> = family
        .answer_sets()
        .iter()
        .map(|answer| {
            let model = answer.interpretation();
            let error = owner
                .metadata()
                .observations()
                .render(
                    model,
                    owner.metadata().output(),
                    observation::Limits::default(),
                    &Control::default(),
                )
                .unwrap_err();
            assert_eq!(
                error.kind(),
                &observation::ErrorKind::Evaluation(observation::EvaluationError::Undefined)
            );
            (
                model.atoms().iter().cloned().collect(),
                answer.score().unwrap().costs().to_vec(),
            )
        })
        .collect();
    assert_eq!(actual, expected);
    assert_eq!(family.outcome().completion(), Some(Completion::Exhausted));
}

#[cfg(feature = "gpu")]
mod physical {
    use super::*;
    use zetesis_wgpu::{
        AdapterBackend, GpuBackendPreference, GpuContext, GpuOptions, GpuSelection,
    };

    fn resources() -> ExecutionResources {
        let context = GpuContext::new_selected(
            GpuOptions::default(),
            GpuSelection {
                backend: GpuBackendPreference::Metal,
                vendor_id: None,
            },
        )
        .unwrap();
        assert_eq!(context.info().backend_kind(), AdapterBackend::Metal);
        assert!(context.info().is_hardware_gpu());
        eprintln!("language consumers adapter={:?}", context.info().metadata());
        ExecutionResources::with_gpu(&context)
    }

    #[test]
    #[ignore = "requires physical Metal; checks complete original identities"]
    fn metal_families_retain_scored_observations() {
        families(
            SolveConfig {
                backend: Backend::Metal,
                ..config(4, 3)
            },
            &resources(),
        );
    }

    #[test]
    #[ignore = "requires physical Metal; checks complete optimum identities"]
    fn metal_optimum_ties_retain_full_answers() {
        optimum(
            SolveConfig {
                backend: Backend::Metal,
                ..config(4, 3)
            },
            &resources(),
        );
    }
}

fn clingo() -> PathBuf {
    std::env::var_os("CLINGO")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::split_paths(&std::env::var_os("PATH")?)
                .map(|directory| directory.join("clingo"))
                .find(|path| path.is_file())
        })
        .expect("independently installed clingo")
        .canonicalize()
        .unwrap()
}

#[test]
#[ignore = "requires independently installed clingo"]
fn original_sources_retain_declared_reference_results() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR"));
    let executable = clingo();
    for case in cases() {
        let source = directory
            .join("tests/fixtures/language-consumers")
            .join(case.file);
        assert_eq!(std::fs::read_to_string(&source).unwrap(), case.source);
        let arguments = [
            "0".into(),
            "--outf=2".into(),
            "--opt-mode=optN".into(),
            source.into_os_string(),
        ];
        let (capture, pending) = process::invoke(
            process::Invocation {
                executable: &executable,
                arguments: &arguments,
                directory,
            },
            process::Limits {
                timeout: Duration::from_secs(5),
                max_output_bytes: REPORT_BYTES,
                cleanup_timeout: CLEANUP_TIMEOUT,
            },
        )
        .unwrap()
        .into_parts();
        if let Some(pending) = pending {
            let cleanup = pending.retry(CLEANUP_TIMEOUT);
            if let Some(pending) = cleanup.pending {
                panic!("unresolved oracle child {}", pending.abandon());
            }
            assert!(cleanup.failure.is_none());
        }
        println!(
            "source={}\nexit={:?}\nstdout={}\nstderr={}",
            case.file,
            capture.exit(),
            capture.stdout_text().unwrap(),
            capture.stderr_text().unwrap()
        );
        assert_eq!(capture.stop(), process::Stop::Completed);
        assert!(capture.failure().is_none());
        assert!(capture.cleanup_failure().is_none());
        assert!(matches!(
            capture.exit().and_then(|exit| exit.code),
            Some(10 | 20 | 30)
        ));
        let report = answers::clingo_json(capture.stdout(), answers::Limits::default()).unwrap();
        assert_eq!(report.solver(), "clingo version 5.8.2");
        let best = case
            .answers
            .iter()
            .map(|answer| &answer.costs)
            .min()
            .unwrap();
        let costs: Vec<_> = best.iter().map(|&(_, cost)| cost).collect();
        let mut displays = BTreeMap::new();
        for answer in case.answers.iter().filter(|answer| &answer.costs == best) {
            let terms: Vec<String> = answer
                .display
                .split_whitespace()
                .map(str::to_owned)
                .collect();
            *displays.entry(terms).or_insert(0_u64) += 1;
        }
        let native = DisplayRecord {
            costs,
            displays: displays.into_iter().collect(),
        };
        let expected = if let Some(reference) = &case.reference_difference {
            assert_ne!(
                reference, &native,
                "a known disagreement must not masquerade as parity"
            );
            reference
        } else {
            &native
        };
        // Preserve every unchanged-source reference result. Explicit differences
        // are checked against both recorded families, never counted as parity.
        assert_eq!(
            report.cost(),
            Some(expected.costs.as_slice()),
            "{}",
            case.file
        );
        assert_eq!(report.displays(), expected.displays, "{}", case.file);
    }
}
