//! Complete identities are specified independently of projected presentation.

#[path = "support/contribution_sources.rs"]
mod contribution_sources;

use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::time::Duration;

use clap::Parser;
use zetesis_cli::{
    Backend, Completion, Options, Oracle, PreparedInput, Session, SolveConfig, run_with_diagnostics,
};
use zetesis_core::{Atom, Predicate, Sign, Value, ValueLimits, ValueNode};
use zetesis_cpu::Control;
use zetesis_themelios::{AdmissionOptions, ExpansionLimits, FormulaLimits, admit_formula};
use zetesis_validation::{answers, process};

const MAX_REPORT_BYTES: usize = 64 * 1024;
const ORACLE_TIMEOUT: Duration = Duration::from_secs(5);
const CLEANUP_TIMEOUT: Duration = Duration::from_secs(1);

struct Expected {
    models: Vec<Vec<Atom>>,
    costs: Vec<(i32, i64)>,
    displays: Vec<(Vec<String>, u64)>,
}

fn atom(name: &str) -> Atom {
    Atom::new(Predicate::new(name, 0).unwrap(), Vec::new()).unwrap()
}

fn expected() -> [Expected; 2] {
    let p = Atom::new(
        Predicate::new("p", 1).unwrap(),
        vec![
            Value::from_nodes(
                vec![
                    ValueNode::Function {
                        name: "f".into(),
                        sign: Sign::Positive,
                        arity: 1,
                    },
                    ValueNode::Number(1),
                ],
                ValueLimits::default(),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    // Maximizing the independent selected choice forces marker, so no p
    // witness may hold. The min head permits each nonempty subset of {a,b};
    // the hidden choice doubles each optimum without changing its display.
    let hidden = Expected {
        models: vec![
            vec![atom("marker"), atom("selected"), atom("a")],
            vec![atom("marker"), atom("selected"), atom("a"), atom("hidden")],
            vec![atom("marker"), atom("selected"), atom("b")],
            vec![atom("marker"), atom("selected"), atom("b"), atom("hidden")],
            vec![atom("marker"), atom("selected"), atom("a"), atom("b")],
            vec![
                atom("marker"),
                atom("selected"),
                atom("a"),
                atom("b"),
                atom("hidden"),
            ],
        ],
        costs: vec![(2, -1), (1, 0)],
        displays: vec![
            (vec!["a".into()], 2),
            (vec!["a".into(), "b".into()], 2),
            (vec!["b".into()], 2),
        ],
    };
    // Double negation admits the q/p cycle. Maximizing selected forces q;
    // the active max head requires b and independently permits a.
    let recursive = Expected {
        models: vec![
            vec![p.clone(), atom("q"), atom("b"), atom("selected")],
            vec![p, atom("q"), atom("a"), atom("b"), atom("selected")],
        ],
        costs: vec![(3, -1)],
        displays: vec![(vec!["q".into()], 2)],
    };
    [hidden, recursive].map(|mut expected| {
        for model in &mut expected.models {
            model.sort();
        }
        expected.models.sort();
        expected
    })
}

#[test]
fn composed_profiles_preserve_complete_optimum_answers() {
    for (source, expected) in contribution_sources::SOURCES.into_iter().zip(expected()) {
        let input = admit_formula(
            source.into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap();
        assert!(input.atoms().len() <= 8);
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
                Session::new(PreparedInput::formula(&input), config, Control::default()).unwrap();
            let mut models = Vec::new();
            for answer in session.by_ref() {
                let answer = answer.unwrap();
                assert_eq!(answer.score().unwrap().costs(), expected.costs);
                models.push(
                    answer
                        .interpretation()
                        .atoms()
                        .iter()
                        .cloned()
                        .collect::<Vec<_>>(),
                );
            }
            models.sort();
            assert_eq!(models, expected.models, "{source}; workers={workers}");
            let outcome = session.outcome().unwrap();
            assert_eq!(outcome.completion(), Some(Completion::Exhausted));
            assert!(outcome.optimum_proved());
        }
    }
}

fn output(source: &str, json: bool) -> Vec<u8> {
    let mut options = Options::try_parse_from([
        "zetesis",
        "--backend",
        "cpu",
        "--oracle",
        "countermodel",
        "--grounder",
        "eager",
        "--models",
        "0",
    ])
    .unwrap();
    options.json = json;
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
    output
}

fn displays(actual: &answers::ReportedAnswers, expected: &Expected) {
    let costs: Vec<_> = expected.costs.iter().map(|&(_, value)| value).collect();
    assert_eq!(actual.cost(), Some(costs.as_slice()));
    assert_eq!(
        actual.model_count(),
        u64::try_from(expected.models.len()).unwrap()
    );
    assert_eq!(actual.displays(), expected.displays);
}

#[test]
fn composed_output_retains_hidden_optimum_multiplicity() {
    for (source, expected) in contribution_sources::SOURCES.into_iter().zip(expected()) {
        let json = answers::native_json::parse(
            &output(source, true),
            answers::native_json::Limits::default(),
        )
        .unwrap();
        let mut models: Vec<_> = json
            .records()
            .iter()
            .map(|record| {
                assert_eq!(record.costs(), Some(expected.costs.as_slice()));
                let mut atoms = record.full_model().to_vec();
                atoms.sort();
                atoms
            })
            .collect();
        models.sort();
        assert_eq!(models, expected.models);
        displays(
            &json.reported_displays(MAX_REPORT_BYTES).unwrap(),
            &expected,
        );
        let text =
            answers::native_text(&output(source, false), true, answers::Limits::default()).unwrap();
        displays(&text, &expected);
    }
}

fn clingo() -> PathBuf {
    let executable = std::env::var_os("CLINGO")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::split_paths(&std::env::var_os("PATH")?)
                .map(|path| path.join("clingo"))
                .find(|path| path.is_file())
        })
        .expect("independently installed clingo");
    executable.canonicalize().unwrap()
}

#[test]
#[ignore = "requires an independently installed clingo executable"]
fn original_sources_preserve_complete_shown_optimum_ties() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR"));
    let executable = clingo();
    for ((source, filename), expected) in contribution_sources::SOURCES
        .into_iter()
        .zip(["hidden-ties.lp", "recursive-maximum.lp"])
        .zip(expected())
    {
        let path = directory
            .join("tests/fixtures/contributions")
            .join(filename);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
        let arguments = [
            "0".into(),
            "--outf=2".into(),
            "--opt-mode=optN".into(),
            path.into_os_string(),
        ];
        let (capture, pending) = process::invoke(
            process::Invocation {
                executable: &executable,
                arguments: &arguments,
                directory,
            },
            process::Limits {
                timeout: ORACLE_TIMEOUT,
                max_output_bytes: MAX_REPORT_BYTES,
                cleanup_timeout: CLEANUP_TIMEOUT,
            },
        )
        .unwrap()
        .into_parts();
        if let Some(pending) = pending {
            let cleanup = pending.retry(CLEANUP_TIMEOUT);
            if let Some(pending) = cleanup.pending {
                panic!(
                    "oracle cleanup remains unresolved for child {}",
                    pending.abandon()
                );
            }
            assert!(cleanup.failure.is_none());
        }
        println!(
            "{}",
            serde_json::json!({"source": source, "exit": capture.exit(), "stdout": capture.stdout_text().unwrap(), "stderr": capture.stderr_text().unwrap()})
        );
        assert_eq!(capture.stop(), process::Stop::Completed);
        assert!(capture.failure().is_none());
        assert!(capture.cleanup_failure().is_none());
        assert!(matches!(
            capture.exit().and_then(|exit| exit.code),
            Some(10 | 20 | 30)
        ));
        let reported = answers::clingo_json(capture.stdout(), answers::Limits::default()).unwrap();
        // #show hides semantic atoms: this assertion certifies original-source
        // shown/cost multiplicity. Complete hidden identities are specified above.
        displays(&reported, &expected);
    }
}
