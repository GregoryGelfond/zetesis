//! Ordinary formula invocations select the hybrid before answer-set publication.

use clap::Parser;
use zetesis_cli::{Completion, Options, RunError, run_with_diagnostics};
use zetesis_cpu::Control;

#[path = "support/formula_records.rs"]
mod formula_records;

#[path = "support/head_element_sources.rs"]
mod head_element_sources;

#[path = "support/projected_conditional_sources.rs"]
mod projected_conditional_sources;

#[path = "support/logical_extremum_sources.rs"]
mod logical_extremum_sources;

#[cfg(feature = "gpu")]
#[path = "support/contribution_sources.rs"]
mod contribution_sources;

#[path = "support/bound_priority_sources.rs"]
mod bound_priority_sources;

#[cfg(feature = "gpu")]
#[path = "support/bounded_writer.rs"]
mod bounded_writer;

fn options(arguments: &[&str]) -> Options {
    Options::try_parse_from(
        ["zetesis", "--models", "0"]
            .into_iter()
            .chain(arguments.iter().copied()),
    )
    .unwrap()
}

fn cpu_output(source: &str, json: bool) -> Vec<u8> {
    let mut configuration = options(&["--backend", "cpu"]);
    configuration.json = json;
    let mut output = Vec::new();
    let report = run_with_diagnostics(
        source.into(),
        &configuration,
        &mut output,
        &mut Vec::new(),
        &Control::default(),
    )
    .unwrap();
    assert_eq!(report.completion, Completion::Exhausted);
    output
}

/// Compare the complete answer family of a source with no objective.
fn assert_answer_family(source: &str, answers: Vec<Vec<zetesis_core::Atom>>) {
    let mut expected: Vec<_> = answers
        .into_iter()
        .map(|mut atoms| {
            atoms.sort();
            (atoms, None)
        })
        .collect();
    expected.sort();
    assert_eq!(
        formula_records::full_records(&cpu_output(source, true)),
        expected,
        "{source}"
    );
}

#[test]
fn complete_records_distinguish_hidden_atoms() {
    let left = "visible. hidden(left). #show visible/0.";
    let right = "visible. hidden(right). #show visible/0.";
    // Identical displayed records cannot certify the hidden model identity.
    assert_eq!(
        formula_records::displayed_records(&cpu_output(left, false)),
        formula_records::displayed_records(&cpu_output(right, false))
    );
    assert_ne!(
        formula_records::full_records(&cpu_output(left, true)),
        formula_records::full_records(&cpu_output(right, true))
    );
}

#[test]
fn complete_records_preserve_objective_presence() {
    let absent = formula_records::full_records(&cpu_output("a. #show.", true));
    let present = formula_records::full_records(&cpu_output("a. #show. #minimize{0@3}.", true));
    assert_eq!(absent.len(), 1);
    assert_eq!(present.len(), 1);
    assert_eq!(absent[0].0, present[0].0);
    assert_eq!(absent[0].1, None);
    assert_eq!(present[0].1, Some(vec![(3, 0)]));
}

#[test]
fn head_elements_preserve_complete_source_answers() {
    use zetesis_core::{Atom, Predicate, Value};

    let atom = |name| Atom::new(Predicate::new(name, 0).unwrap(), Vec::new()).unwrap();
    let a = atom("a");
    let b = atom("b");
    let d =
        |number| Atom::new(Predicate::new("d", 1).unwrap(), vec![Value::Number(number)]).unwrap();
    let expected = [
        vec![vec![a.clone()], vec![b.clone()], vec![a.clone(), b.clone()]],
        vec![vec![a.clone()]],
        vec![vec![], vec![a.clone()]],
        vec![vec![a.clone()]],
        vec![vec![d(1), d(2)]],
        vec![vec![a.clone()], vec![b]],
        vec![vec![], vec![a.clone()]],
        vec![vec![]],
        vec![],
        vec![vec![], vec![a.clone()]],
        vec![vec![]],
        vec![vec![a.clone()]],
        vec![],
        vec![vec![]],
        vec![vec![d(1), d(2)]],
        vec![vec![], vec![a.clone()]],
        vec![vec![], vec![a.clone()]],
        vec![vec![], vec![a.clone()]],
        vec![vec![], vec![a.clone()]],
        vec![vec![], vec![a]],
        vec![],
    ];
    assert_eq!(expected.len(), head_element_sources::SOURCES.len());
    for (source, answers) in head_element_sources::SOURCES.into_iter().zip(expected) {
        assert_answer_family(source, answers);
    }
}

#[test]
fn projected_conditionals_preserve_complete_answers() {
    use zetesis_core::{Atom, Predicate, Value};

    let p =
        |number| Atom::new(Predicate::new("p", 1).unwrap(), vec![Value::Number(number)]).unwrap();
    let q = Atom::new(Predicate::new("q", 0).unwrap(), Vec::new()).unwrap();
    // With no witnesses, default negation succeeds. Double negation succeeds
    // precisely when a witness exists; it can also support the recursive pair.
    let expected = [
        vec![vec![q.clone()], vec![p(1)], vec![p(2)], vec![p(1), p(2)]],
        vec![
            vec![],
            vec![p(1), q.clone()],
            vec![p(2), q.clone()],
            vec![p(1), p(2), q.clone()],
        ],
        vec![],
        vec![vec![], vec![p(1), q]],
    ];
    assert_eq!(expected.len(), projected_conditional_sources::SOURCES.len());
    for (source, answers) in projected_conditional_sources::SOURCES
        .into_iter()
        .zip(expected)
    {
        assert_answer_family(source, answers);
    }
}

#[test]
fn logical_extrema_preserve_complete_answers() {
    use zetesis_core::{Atom, Predicate};

    let a = Atom::new(Predicate::new("a", 0).unwrap(), Vec::new()).unwrap();
    let b = Atom::new(Predicate::new("b", 0).unwrap(), Vec::new()).unwrap();
    let expected = [
        vec![vec![a.clone()], vec![a.clone(), b.clone()]],
        vec![vec![b.clone()], vec![a.clone(), b.clone()]],
        // Symbol z precedes string "a" in logical term order.
        vec![vec![b.clone()], vec![a.clone(), b.clone()]],
        vec![vec![a.clone()], vec![a.clone(), b.clone()]],
        vec![vec![a.clone()], vec![a.clone(), b]],
        // An empty minimum and a selected #sup both satisfy the bound.
        vec![vec![], vec![a]],
    ];
    assert_eq!(expected.len(), logical_extremum_sources::SOURCES.len());
    for (source, answers) in logical_extremum_sources::SOURCES.into_iter().zip(expected) {
        assert_answer_family(source, answers);
    }
}

#[test]
fn cancellation_precedes_explicit_formula_device_initialization() {
    for arguments in [
        vec!["--backend", "metal"],
        vec!["--backend", "gpu", "--oracle", "countermodel"],
    ] {
        let control = Control::default();
        control.cancel();
        let mut output = Vec::new();
        let mut diagnostics = Vec::new();
        let report = run_with_diagnostics(
            "1 {a;b} 1.".into(),
            &options(&arguments),
            &mut output,
            &mut diagnostics,
            &control,
        )
        .unwrap();
        assert_eq!(report.completion, Completion::Interrupted);
        assert_eq!((report.models, report.checked), (0, 0));
        assert!(report.formula_execution.is_none());
        assert!(!String::from_utf8(diagnostics).unwrap().contains("Backend:"));
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("INCOMPLETE"));
        assert!(!text.contains("UNSATISFIABLE"));
    }
}

#[test]
fn formula_admission_precedes_device_initialization() {
    let mut output = Vec::new();
    let error = run_with_diagnostics(
        "invalid ? source".into(),
        &options(&["--backend", "metal", "--oracle", "countermodel"]),
        &mut output,
        &mut Vec::new(),
        &Control::default(),
    )
    .unwrap_err();
    assert!(matches!(error, RunError::FormulaAdmission(_)));
    assert!(output.is_empty());
}

#[test]
fn formula_device_route_refuses_explicit_lazy_grounding() {
    let mut output = Vec::new();
    let error = run_with_diagnostics(
        "1 {a;b} 1.".into(),
        &options(&["--backend", "metal", "--grounder", "lazy"]),
        &mut output,
        &mut Vec::new(),
        &Control::default(),
    )
    .unwrap_err();
    assert!(matches!(error, RunError::UnsupportedOracle { .. }));
    assert!(output.is_empty());
}

#[test]
fn automatic_formula_route_reports_the_checked_cpu_specialization() {
    let mut output = Vec::new();
    let mut diagnostics = Vec::new();
    let report = run_with_diagnostics(
        "1 {a;b} 1. #minimize{1,a:a;1,b:b}.".into(),
        &options(&["--stats"]),
        &mut output,
        &mut diagnostics,
        &Control::default(),
    )
    .unwrap();
    assert_eq!(report.completion, Completion::Exhausted);
    assert_eq!(report.models, 2);
    assert!(report.formula_execution.is_none());
    let text = String::from_utf8(diagnostics).unwrap();
    assert!(text.contains("backend=cpu; oracle=tight-support"));
    assert!(!text.contains("hybrid GPU"));
}

#[cfg(not(feature = "gpu"))]
#[test]
fn cpu_only_formula_hardware_request_is_explicitly_unavailable() {
    let mut output = Vec::new();
    let error = run_with_diagnostics(
        "1 {a;b} 1.".into(),
        &options(&["--backend", "metal"]),
        &mut output,
        &mut Vec::new(),
        &Control::default(),
    )
    .unwrap_err();
    assert!(matches!(error, RunError::BackendUnavailable));
    assert!(output.is_empty());
}

#[cfg(feature = "gpu")]
#[path = "support/physical_backend.rs"]
mod physical_backend;

#[cfg(feature = "gpu")]
#[path = "support/count_objective_sources.rs"]
mod count_objective_sources;

#[cfg(feature = "gpu")]
#[path = "support/language_value_sources.rs"]
mod language_value_sources;

#[cfg(feature = "gpu")]
mod physical {
    use super::formula_records::{displayed_records, full_records};
    use super::physical_backend::Backend;
    use super::{Completion, Control, options, run_with_diagnostics};

    #[test]
    #[ignore = "requires actual Metal; executes the ordinary solver and never substitutes CPU"]
    fn ordinary_metal_formula_batches_match_complete_cpu_models_costs_and_displays() {
        qualify_formula_results(Backend::Metal);
    }

    #[test]
    #[ignore = "requires actual Vulkan through the ordinary solver"]
    fn ordinary_vulkan_formula_results_match_cpu() {
        qualify_formula_results(Backend::Vulkan);
    }

    fn qualify_formula_results(backend: Backend) {
        for source in [
            "a | b.",
            "{a;b;c;d}. x:-x. :-a,b. #show.",
            "1 {a;b;c} 1. #minimize{1@2,a:a;1@2,b:b;2@2,c:c}.",
            "{a;b;c;d}. #maximize{2@1,k:a;2@1,k:b;1@1,c:c}. #show a/0.",
            "{p;-p}. q:-not p,not -p. #show x:q.",
            "{e(1);e(2)}. n(N):-N=#sum{X:e(X)}. #show n/1.",
            // Original sources also occur in weighted_heads' external clingo corpus.
            "1#sum{-1:a;2:b}1.",
            "{d}.0#sum{0:a:not d}0.",
            "{d}.1#sum+{1:a:not d}1.",
            "0#sum+{0:a}0.",
            "{x;y}.1#count{0:not a;0:a}1.#minimize{1@3,k:x;0@1,l:y}.",
        ]
        .into_iter()
        .chain(super::count_objective_sources::SATISFIABLE)
        .chain([super::count_objective_sources::INCONSISTENT])
        .chain(super::language_value_sources::SOURCES)
        .chain(super::head_element_sources::SOURCES)
        .chain(super::projected_conditional_sources::SOURCES)
        .chain(super::logical_extremum_sources::SOURCES)
        .chain(super::contribution_sources::SOURCES)
        .chain(super::bound_priority_sources::SOURCES)
        {
            for json in [false, true] {
                qualify_formula_output(source, backend, json);
            }
        }
    }

    fn qualify_formula_output(source: &str, backend: Backend, json: bool) {
        let mut expected = Vec::new();
        let mut configuration = options(&["--backend", "cpu"]);
        configuration.json = json;
        let cpu = run_with_diagnostics(
            source.into(),
            &configuration,
            &mut expected,
            &mut Vec::new(),
            &Control::default(),
        )
        .unwrap();
        assert_eq!(cpu.completion, Completion::Exhausted);
        for batch in ["3", "7", "16"] {
            let mut actual = Vec::new();
            let mut diagnostics = Vec::new();
            let mut configuration = options(&[
                "--backend",
                backend.argument(),
                "--oracle",
                "countermodel",
                "--batch-size",
                batch,
                "--stats",
            ]);
            configuration.json = json;
            let report = run_with_diagnostics(
                source.into(),
                &configuration,
                &mut actual,
                &mut diagnostics,
                &Control::default(),
            )
            .unwrap();
            assert_eq!(report.completion, Completion::Exhausted);
            assert_eq!(report.models, cpu.models);
            if json {
                assert_eq!(
                    full_records(&actual),
                    full_records(&expected),
                    "{source} batch={batch}: complete typed models and costs"
                );
            } else {
                assert_eq!(
                    displayed_records(&actual),
                    displayed_records(&expected),
                    "{source} batch={batch}: displayed records"
                );
            }
            let stats = report.formula_execution.unwrap();
            assert!(stats.adapter.contains(backend.name()));
            if report.checked > 0 {
                assert!(stats.gpu_batches > 0);
            } else {
                // An inconsistent source can exhaust before proposing a world.
                assert_eq!(report.models, 0);
                assert_eq!(stats.gpu_batches, 0);
            }
            assert_eq!(stats.gpu_candidates, report.checked);
            assert_eq!(stats.gpu_decided + stats.cpu_residuals, report.checked);
            assert_eq!((stats.pending_candidates, stats.queued_models), (0, 0));
            let text = String::from_utf8(diagnostics).unwrap();
            assert!(text.contains("hybrid GPU propagation + exact CPU residual search"));
            assert!(text.contains("GPU kernel timing=unavailable"));
        }
    }

    #[test]
    #[ignore = "requires actual Metal; checks resource and diagnostic failures in ordinary solving"]
    fn ordinary_metal_formula_limits_preserve_partial_coverage_and_writer_errors() {
        qualify_formula_limits(Backend::Metal);
    }

    #[test]
    #[ignore = "requires actual Vulkan through the ordinary solver"]
    fn ordinary_vulkan_formula_retains_bounded_outcomes() {
        qualify_formula_limits(Backend::Vulkan);
    }

    fn qualify_formula_limits(backend: Backend) {
        qualify_optimization_stop(backend);
        let source = "{a;b;c;d}.";
        let mut output = Vec::new();
        let report = run_with_diagnostics(
            source.into(),
            &options(&[
                "--backend",
                backend.argument(),
                "--oracle",
                "countermodel",
                "--batch-size",
                "7",
                "--max-candidates",
                "3",
            ]),
            &mut output,
            &mut Vec::new(),
            &Control::default(),
        )
        .unwrap();
        assert_eq!(report.completion, Completion::Interrupted);
        assert_eq!(report.models, 3);
        assert!(
            !std::str::from_utf8(&output)
                .unwrap()
                .contains("coverage=exhausted")
        );
        let mut limited = options(&["--backend", backend.argument(), "--oracle", "countermodel"]);
        limited.max_batch_bytes = 0;
        let report = run_with_diagnostics(
            source.into(),
            &limited,
            &mut Vec::new(),
            &mut Vec::new(),
            &Control::default(),
        )
        .unwrap();
        assert_eq!(report.completion, Completion::Interrupted);
        assert_eq!(report.checked, 0);
        assert_eq!(report.formula_execution.unwrap().gpu_batches, 0);
        let mut output = Vec::new();
        let mut broken = super::bounded_writer::BoundedWriter::new(0);
        let error = run_with_diagnostics(
            source.into(),
            &options(&["--backend", backend.argument(), "--oracle", "countermodel"]),
            &mut output,
            &mut broken,
            &Control::default(),
        )
        .unwrap_err();
        assert!(
            matches!(error, super::RunError::Output(ref cause) if cause.kind() == std::io::ErrorKind::BrokenPipe)
        );
        assert!(output.is_empty());
        assert!(broken.bytes().is_empty());
    }

    fn qualify_optimization_stop(backend: Backend) {
        for pruning in [false, true] {
            let mut configuration = options(&[
                "--backend",
                backend.argument(),
                "--oracle",
                "countermodel",
                "--batch-size",
                "7",
                "--max-candidates",
                "1",
            ]);
            if !pruning {
                configuration.max_objective_bound_work = 0;
            }
            let mut output = Vec::new();
            let report = run_with_diagnostics(
                super::count_objective_sources::SATISFIABLE[0].into(),
                &configuration,
                &mut output,
                &mut Vec::new(),
                &Control::default(),
            )
            .unwrap();
            assert_eq!(report.completion, Completion::Interrupted);
            assert_eq!(report.checked, 1);
            assert_eq!(report.optimization.unwrap().scored_models, 1);
            let execution = report.formula_execution.unwrap();
            assert!(execution.adapter.contains(backend.name()));
            assert_eq!(execution.gpu_batches, 1);
            assert_eq!(execution.gpu_candidates, 1);
            assert_eq!(execution.gpu_decided + execution.cpu_residuals, 1);
            assert_eq!(
                (execution.pending_candidates, execution.queued_models),
                (0, 0)
            );
            let text = std::str::from_utf8(&output).unwrap();
            assert!(text.contains("INCOMPLETE"));
            assert!(text.contains("Coverage: partial"));
            assert!(!text.contains("OPTIMUM FOUND"));
            assert!(!text.contains("Coverage: exhausted"));
        }
    }
}
