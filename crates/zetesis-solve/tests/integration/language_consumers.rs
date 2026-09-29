//! Full answer identity survives aggregate contributions, costs and observations.

use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsStr,
    num::NonZeroUsize,
    path::Path,
};

use crate::support::reports::REPORT_BYTES;
use zetesis_clingo_support as oracle;
use zetesis_core::{Atom, Model, Predicate, Value};
use zetesis_cpu::Cancellation;
use zetesis_solve::{
    AnswerSelection, AnswerSet, Backend, Completion, ExecutionResources, Grounder, Oracle,
    PreparedInput, Session, SolveConfig, WorldViewLimits,
};
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionLimits, FormulaLimits, admit_formula, observation,
};

mod ordinary_composition;
mod projected_families;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Record {
    atoms: Model,
    costs: Vec<(i32, i64)>,
    display: String,
}

fn record(atoms: &[&str], priority: i32, cost: i64, display: &str) -> Record {
    Record {
        atoms: Model::new(atoms.iter().map(|name| atom(name, vec![]))).unwrap(),
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

fn cases() -> Vec<Case> {
    [
        Case {
            source: include_str!("../fixtures/language-consumers/symbolic-weight.lp"),
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
            source: include_str!("../fixtures/language-consumers/neutral-weight.lp"),
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
            source: include_str!("../fixtures/language-consumers/missing-measure.lp"),
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
        filtered_priority(),
        Case {
            source: include_str!("../fixtures/language-consumers/arithmetic-keys.lp"),
            file: "arithmetic-keys.lp",
            reference_difference: None,
            answers: [(1, 2, "chosen(11)"), (2, 4, "chosen(12)")]
                .map(|(choice, cost, display)| Record {
                    atoms: Model::new([
                        atom("data", vec![Value::Number(1)]),
                        atom("data", vec![Value::Number(2)]),
                        atom("pick", vec![Value::Number(choice)]),
                    ])
                    .unwrap(),
                    costs: vec![(1, cost)],
                    display: display.into(),
                })
                .into(),
        },
        independent_objectives(),
        recursive_alternatives(),
        neutral_minimum(),
        cyclic_observer(),
        Case {
            source: include_str!("../fixtures/language-consumers/scoped-objectives.lp"),
            file: "scoped-objectives.lp",
            reference_difference: None,
            answers: [
                (&["a"][..], [(2, 1), (1, 1)], "score(1)"),
                (&["b"][..], [(2, 1), (1, 0)], "score(1)"),
                (&["a", "b"][..], [(2, 2), (1, 1)], "score(2)"),
            ]
            .map(|(atoms, costs, display)| Record {
                atoms: Model::new(atoms.iter().map(|name| atom(name, vec![]))).unwrap(),
                costs: costs.into(),
                display: display.into(),
            })
            .into(),
        },
        Case {
            source: include_str!("../fixtures/language-consumers/shared-tuple.lp"),
            file: "shared-tuple.lp",
            reference_difference: None,
            answers: BTreeSet::from([
                record(&["a"], 1, 1, "choice(6)"),
                record(&["b"], 1, 1, "choice(6)"),
                record(&["a", "b"], 1, 0, "choice(6)"),
            ]),
        },
    ]
    .into_iter()
    .chain(ordinary_composition::cases())
    .collect()
}

fn filtered_priority() -> Case {
    Case {
        source: include_str!("../fixtures/language-consumers/filtered-priority.lp"),
        file: "filtered-priority.lp",
        reference_difference: None,
        answers: BTreeSet::from([
            Record {
                atoms: Model::new([atom("n", vec![Value::Number(0)])]).unwrap(),
                costs: vec![(1, 0)],
                display: "score(1)".into(),
            },
            Record {
                atoms: Model::new([atom("a", vec![]), atom("n", vec![Value::Number(1)])]).unwrap(),
                costs: vec![(1, 1)],
                display: "score(2)".into(),
            },
        ]),
    }
}

fn recursive_alternatives() -> Case {
    // Each node has one supported alternative. The constraint excludes choosing
    // both left(0) and left(1); left(2) does not affect the objective.
    let alternatives: [(&[i32], i64); 6] = [
        (&[], 0),
        (&[0], 1),
        (&[1], 1),
        (&[2], 0),
        (&[0, 2], 1),
        (&[1, 2], 1),
    ];
    let answers = alternatives
        .into_iter()
        .map(|(left, cost)| {
            let mut atoms = Vec::new();
            for node in 0..=2 {
                atoms.push(atom("node", vec![Value::Number(node)]));
                let choice = if left.contains(&node) {
                    "left"
                } else {
                    "right"
                };
                atoms.push(atom(choice, vec![Value::Number(node)]));
            }
            atoms.sort();
            Record {
                atoms: Model::new(atoms).unwrap(),
                costs: vec![(1, cost)],
                display: left
                    .iter()
                    .map(|node| format!("left({node})"))
                    .collect::<Vec<_>>()
                    .join(" "),
            }
        })
        .collect();
    Case {
        source: include_str!("../fixtures/language-consumers/recursive-alternatives.lp"),
        file: "recursive-alternatives.lp",
        answers,
        reference_difference: None,
    }
}

fn neutral_minimum() -> Case {
    Case {
        source: include_str!("../fixtures/language-consumers/neutral-minimum.lp"),
        file: "neutral-minimum.lp",
        answers: BTreeSet::from([
            record(&["b"], 2, 1, "value(1)"),
            record(&["a", "b"], 2, 0, "value(1)"),
        ]),
        // A missing extremum measure contributes no value but retains its
        // independent head permission. clingo 5.8.2 drops that permission.
        reference_difference: Some(DisplayRecord {
            costs: vec![1],
            displays: vec![(vec!["value(1)".into()], 1)],
        }),
    }
}

fn cyclic_observer() -> Case {
    Case {
        source: include_str!("../fixtures/language-consumers/cyclic-observer.lp"),
        file: "cyclic-observer.lp",
        reference_difference: None,
        answers: [(1, "count(1)"), (2, "count(2)")]
            .map(|(count, display)| {
                let mut atoms = vec![atom("n", vec![Value::Number(count)]), atom("p", vec![])];
                if count == 2 {
                    atoms.insert(0, atom("a", vec![]));
                }
                Record {
                    atoms: Model::new(atoms).unwrap(),
                    costs: vec![(2, 1), (1, i64::from(count))],
                    display: display.into(),
                }
            })
            .into(),
    }
}

fn independent_objectives() -> Case {
    Case {
        source: include_str!("../fixtures/language-consumers/independent-objectives.lp"),
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
            atoms: Model::new(atoms.iter().map(|name| atom(name, vec![]))).unwrap(),
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
    let atoms = model.clone();
    let score = answer.score().expect("the numeric objective is present");
    assert!(score.is_present());
    let display = owner
        .metadata()
        .observations()
        .render(
            model,
            owner.metadata().output(),
            observation::Limits::default(),
            &Cancellation::default(),
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
    projected_families::check(AnswerSelection::All, config, resources);
    for case in cases() {
        let owner = admit(&case);
        let world_view = Session::builder(
            PreparedInput::formula(&owner),
            config,
            Cancellation::default(),
        )
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
    projected_families::check(AnswerSelection::Optimal, config, resources);
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
        let mut session = Session::builder(
            PreparedInput::formula(&owner),
            config,
            Cancellation::default(),
        )
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
    // These sources have several complete candidates. Requiring actual device
    // work makes their composed semantic contracts part of device qualification.
    if let Backend::Gpu(Some(api)) = backend
        && matches!(
            case.file,
            "shared-tuple.lp"
                | "neutral-minimum.lp"
                | "cyclic-observer.lp"
                | "recursive-alternatives.lp"
                | "pooled-disjuncts.lp"
                | "pooled-conditions.lp"
        )
    {
        let execution = outcome
            .formula_execution()
            .expect("formula device execution");
        assert!(
            execution.adapter.contains(api.name()),
            "{}",
            execution.adapter
        );
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
        Cancellation::default(),
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
                    &Cancellation::default(),
                )
                .unwrap_err();
            assert_eq!(
                error.kind(),
                &observation::ErrorKind::Evaluation(observation::EvaluationError::Undefined)
            );
            (model.clone(), answer.score().unwrap().costs().to_vec())
        })
        .collect();
    assert_eq!(actual, expected);
    assert_eq!(family.outcome().completion(), Some(Completion::Exhausted));
}

#[cfg(feature = "gpu")]
mod physical {
    use super::*;
    use zetesis_solve::GpuApi;
    use zetesis_wgpu::{AdapterBackend, GpuContext, GpuOptions, GpuSelection};

    fn resources(api: GpuApi, kind: AdapterBackend) -> ExecutionResources {
        let context =
            GpuContext::new_selected(GpuOptions::default(), GpuSelection { api }).unwrap();
        assert_eq!(context.info().backend_kind(), kind);
        assert!(context.info().is_hardware_gpu());
        eprintln!("language consumers adapter={:?}", context.info().metadata());
        ExecutionResources::with_gpu(&context)
    }

    #[test]
    #[ignore = "requires physical Metal; checks complete original identities"]
    fn metal_families_retain_scored_observations() {
        families(
            SolveConfig {
                backend: Backend::Gpu(Some(GpuApi::Metal)),
                ..config(4, 3)
            },
            &resources(GpuApi::Metal, AdapterBackend::Metal),
        );
    }

    #[test]
    #[ignore = "requires actual Vulkan; checks complete original identities"]
    fn vulkan_families_retain_scored_observations() {
        families(
            SolveConfig {
                backend: Backend::Gpu(Some(GpuApi::Vulkan)),
                ..config(4, 3)
            },
            &resources(GpuApi::Vulkan, AdapterBackend::Vulkan),
        );
    }

    #[test]
    #[ignore = "requires physical Metal; checks complete optimum identities"]
    fn metal_optimum_ties_retain_full_answers() {
        optimum(
            SolveConfig {
                backend: Backend::Gpu(Some(GpuApi::Metal)),
                ..config(4, 3)
            },
            &resources(GpuApi::Metal, AdapterBackend::Metal),
        );
    }

    #[test]
    #[ignore = "requires actual Vulkan; checks complete optimum identities"]
    fn vulkan_optimum_ties_retain_full_answers() {
        optimum(
            SolveConfig {
                backend: Backend::Gpu(Some(GpuApi::Vulkan)),
                ..config(4, 3)
            },
            &resources(GpuApi::Vulkan, AdapterBackend::Vulkan),
        );
    }
}

#[test]
#[ignore = "requires independently installed clingo"]
fn original_sources_retain_declared_reference_results() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR"));
    for case in cases() {
        let source = directory
            .join("tests/fixtures/language-consumers")
            .join(case.file);
        assert_eq!(std::fs::read_to_string(&source).unwrap(), case.source);
        let run = oracle::run_in(
            directory,
            [
                OsStr::new("0"),
                OsStr::new("--outf=2"),
                OsStr::new("--opt-mode=optN"),
                source.as_os_str(),
            ],
            &oracle::DECIDED,
            oracle::Limits {
                max_output_bytes: REPORT_BYTES,
                ..oracle::Limits::default()
            },
        );
        println!(
            "source={}\nexit={}\nstdout={}\nstderr={}",
            case.file,
            run.code(),
            std::str::from_utf8(run.stdout()).unwrap(),
            std::str::from_utf8(run.stderr()).unwrap()
        );
        let report = oracle::answers(&run);
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
