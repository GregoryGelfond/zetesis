//! Projection compares classes, not a solver's arbitrary representative.
//!
//! These fixed sources have default complete atom display and no #show forms.
//! The maintained clingo decoder can therefore expose their full selected
//! interpretations. Its ordinary optN replay rule also applies to the scored
//! fixture; no alternative report parser or representative-selection rule is used.

use std::{collections::BTreeSet, num::NonZeroUsize, path::Path, time::Duration};

use zetesis_core::Model;
use zetesis_cpu::Cancellation;
use zetesis_solve::{
    AnswerSelection, Backend, Completion, Grounder, Oracle, PreparedInput, ProjectionLimits,
    Session, SolveConfig,
};
use zetesis_themelios::{
    AdmissionOptions, ExpansionLimits, FormulaLimits, OutputSelection, admit_formula,
    observation::{self, ObservationProgram},
};
use zetesis_validation::{answers, process};

type Family = BTreeSet<Vec<String>>;

const REPORT_BYTES: usize = 64 * 1024;
const CLEANUP_TIMEOUT: Duration = Duration::from_secs(1);
const FOUR: &[&[&str]] = &[&[], &["p"], &["q"], &["p", "q"]];
const Q_REQUIRED: &[&[&str]] = &[&["q"], &["p", "q"]];
const P_IMAGE: &[&[&str]] = &[&[], &["p"]];
const EMPTY_IMAGE: &[&[&str]] = &[&[]];

struct Case {
    file: &'static str,
    source: &'static str,
    domain: &'static [&'static str],
    image: &'static [&'static [&'static str]],
    selected: &'static [&'static [&'static str]],
    costs: Option<&'static [i64]>,
}

const CASES: &[Case] = &[
    Case {
        file: "empty.lp",
        source: include_str!("../fixtures/projected-reference/empty.lp"),
        domain: &[],
        image: EMPTY_IMAGE,
        selected: FOUR,
        costs: None,
    },
    Case {
        file: "signature.lp",
        source: include_str!("../fixtures/projected-reference/signature.lp"),
        domain: &["p"],
        image: P_IMAGE,
        selected: FOUR,
        costs: None,
    },
    Case {
        file: "required.lp",
        source: include_str!("../fixtures/projected-reference/required.lp"),
        domain: &[],
        image: EMPTY_IMAGE,
        selected: Q_REQUIRED,
        costs: None,
    },
    Case {
        file: "rich-required.lp",
        source: include_str!("../fixtures/projected-reference/rich-required.lp"),
        domain: &[],
        image: EMPTY_IMAGE,
        selected: Q_REQUIRED,
        costs: None,
    },
    Case {
        file: "cyclic-required.lp",
        source: include_str!("../fixtures/projected-reference/cyclic-required.lp"),
        domain: &[],
        image: EMPTY_IMAGE,
        selected: &[&["q", "r"], &["p", "q", "r"]],
        costs: None,
    },
    Case {
        file: "optional.lp",
        source: include_str!("../fixtures/projected-reference/optional.lp"),
        domain: &["p"],
        image: P_IMAGE,
        selected: FOUR,
        costs: None,
    },
    Case {
        file: "contradictory-optional.lp",
        source: include_str!("../fixtures/projected-reference/contradictory-optional.lp"),
        // Source activity retains the optional p identity despite the
        // impossible correlation in this declaration's condition.
        domain: &["p"],
        image: P_IMAGE,
        selected: FOUR,
        costs: None,
    },
    Case {
        file: "signed-binding.lp",
        source: include_str!("../fixtures/projected-reference/signed-binding.lp"),
        domain: &["-p(1)"],
        image: &[&[], &["-p(1)"]],
        selected: &[
            &["d(1)", "d(2)"],
            &["d(1)", "d(2)", "q"],
            &["-p(1)", "d(1)", "d(2)"],
            &["-p(1)", "d(1)", "d(2)", "q"],
            &["-p(2)", "d(1)", "d(2)"],
            &["-p(2)", "d(1)", "d(2)", "q"],
            &["-p(1)", "-p(2)", "d(1)", "d(2)"],
            &["-p(1)", "-p(2)", "d(1)", "d(2)", "q"],
        ],
        costs: None,
    },
    Case {
        file: "inconsistent.lp",
        source: include_str!("../fixtures/projected-reference/inconsistent.lp"),
        domain: &[],
        image: &[],
        selected: &[],
        costs: None,
    },
    Case {
        file: "optimal.lp",
        source: include_str!("../fixtures/projected-reference/optimal.lp"),
        domain: &["p"],
        image: P_IMAGE,
        selected: Q_REQUIRED,
        costs: Some(&[0]),
    },
];

fn symbols(values: &[&str]) -> Vec<String> {
    let mut result: Vec<_> = values.iter().map(|value| (*value).to_owned()).collect();
    result.sort_unstable();
    result
}

fn family(rows: &[&[&str]]) -> Family {
    rows.iter().map(|row| symbols(row)).collect()
}

fn key(model: &[String], domain: &[&str]) -> Vec<String> {
    model
        .iter()
        .filter(|atom| domain.contains(&atom.as_str()))
        .cloned()
        .collect()
}

fn model_symbols(model: &Model) -> Vec<String> {
    // An empty term channel and default atom selection render the complete
    // supplied interpretation through the maintained bounded library view.
    let rendered = ObservationProgram::default()
        .render(
            model,
            &OutputSelection::default(),
            observation::Limits {
                max_output_bytes: REPORT_BYTES,
                ..observation::Limits::default()
            },
            &Cancellation::default(),
        )
        .unwrap();
    let mut symbols = answers::split_display(
        rendered.text(),
        false,
        answers::Limits::for_bytes(REPORT_BYTES),
    )
    .unwrap();
    symbols.sort_unstable();
    symbols
}

fn records(report: &answers::ReportedAnswers) -> Family {
    let mut result = Family::new();
    for (model, multiplicity) in report.displays() {
        assert_eq!(*multiplicity, 1, "default display retains full identity");
        assert!(result.insert(model.clone()));
    }
    assert_eq!(usize::try_from(report.model_count()).unwrap(), result.len());
    result
}

fn reference(case: &Case, executable: &Path, projected: bool) -> answers::ReportedAnswers {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = directory
        .join("tests/fixtures/projected-reference")
        .join(case.file);
    assert_eq!(std::fs::read_to_string(&source).unwrap(), case.source);
    let mut arguments = vec![
        "--models=0".into(),
        "--outf=2".into(),
        "--opt-mode=optN".into(),
    ];
    if projected {
        arguments.push("--project=project".into());
    }
    arguments.push(source.into_os_string());
    let (capture, pending) = process::invoke(
        process::Invocation {
            executable,
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
        "source={} projected={projected}\narguments={arguments:?}\nexit={:?}\nstdout={}\nstderr={}",
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
    let report = answers::clingo_json(
        capture.stdout(),
        answers::Limits {
            max_input_bytes: REPORT_BYTES,
            max_witnesses: 32,
            max_symbols: 256,
            max_cost_dimensions: 2,
        },
    )
    .unwrap();
    assert_eq!(report.solver(), "clingo version 5.8.2");
    assert_eq!(report.cost(), case.costs);
    report
}

fn check(case: &Case, executable: &Path) {
    let selected = records(&reference(case, executable, false));
    assert_eq!(
        selected,
        family(case.selected),
        "{}: full selected family",
        case.file
    );
    let expected = family(case.image);
    let reference = records(&reference(case, executable, true));
    let mut observed = Family::new();
    for model in reference {
        assert!(
            selected.contains(&model),
            "{}: full reference membership",
            case.file
        );
        assert!(
            observed.insert(key(&model, case.domain)),
            "{}: unique reference key",
            case.file
        );
    }
    assert_eq!(observed, expected, "{}: reference key image", case.file);
    check_native(case, &selected, &expected);
}

fn check_native(case: &Case, selected: &Family, expected: &Family) {
    let owner = admit_formula(
        case.source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    assert_eq!(owner.source().text(), case.source);
    assert!(owner.projection().is_explicit());
    // Explicit fixture export feeds the bounded renderer used by the external
    // oracle comparison. This fixed-domain model makes no answer-set claim.
    let domain = model_symbols(
        &Model::new(
            owner
                .projection()
                .atoms()
                .iter()
                .map(|atom| atom.to_atom(zetesis_core::ValueLimits::default()).unwrap()),
        )
        .unwrap(),
    );
    assert_eq!(
        domain,
        symbols(case.domain),
        "{}: fixed source domain",
        case.file
    );
    let config = SolveConfig {
        backend: Backend::Cpu,
        grounder: Grounder::Eager,
        oracle: Oracle::Countermodel,
        models: 0,
        workers: NonZeroUsize::new(1).unwrap(),
        completion_workers: NonZeroUsize::new(1).unwrap(),
        batch_size: NonZeroUsize::new(1).unwrap(),
        ..SolveConfig::default()
    };
    let selection = if case.costs.is_some() {
        AnswerSelection::Optimal
    } else {
        AnswerSelection::All
    };
    let mut session = Session::builder(
        PreparedInput::formula(&owner),
        config,
        Cancellation::default(),
    )
    .selection(selection)
    .projected(ProjectionLimits::default())
    .start()
    .unwrap();
    let mut native = Family::new();
    for answer in session.by_ref() {
        let answer = answer.unwrap();
        let model = model_symbols(answer.interpretation());
        assert!(
            selected.contains(&model),
            "{}: full native membership",
            case.file
        );
        let costs = answer.score().map(|score| {
            score
                .costs()
                .iter()
                .map(|&(_, cost)| cost)
                .collect::<Vec<_>>()
        });
        assert_eq!(costs.as_deref(), case.costs);
        assert!(
            native.insert(key(&model, case.domain)),
            "{}: unique native key",
            case.file
        );
    }
    assert_eq!(&native, expected, "{}: native key image", case.file);
    let outcome = session.outcome().unwrap();
    assert_eq!(outcome.completion(), Some(Completion::Exhausted));
    if case.costs.is_some() {
        assert!(outcome.optimum_proved());
    }
    let projection = outcome.projection().unwrap();
    assert!(projection.complete);
    assert_eq!(projection.representatives, expected.len());
    assert_eq!(
        usize::try_from(projection.duplicates).unwrap(),
        selected.len() - expected.len()
    );
}

#[test]
#[ignore = "requires independently installed clingo 5.8.2"]
fn projected_classes_match_complete_reference_families() {
    let executable = zetesis_clingo_support::executable();
    for case in CASES {
        check(case, &executable);
    }
    println!(
        "complete_sources={} bounded_reference_calls={}",
        CASES.len(),
        CASES.len() * 2
    );
}
