//! Ordinary formula invocations select the hybrid before stable-model publication.

use clap::Parser;
use zetesis_cli::{Completion, Options, RunError, run_with_diagnostics};
use zetesis_cpu::Control;

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
fn explicit_formula_route_admits_source_before_device_failure_and_still_refuses_lazy() {
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
mod physical {
    use super::physical_backend::Backend;
    use super::{Completion, Control, options, run_with_diagnostics};

    fn records(output: &[u8]) -> Vec<(Vec<String>, Option<String>)> {
        let text = std::str::from_utf8(output).unwrap();
        let mut rows = Vec::new();
        let mut lines = text.lines().peekable();
        while let Some(line) = lines.next() {
            if line.starts_with("Answer:") {
                // These named tiny fixtures deliberately contain no whitespace in symbols.
                let mut atoms: Vec<_> = lines
                    .next()
                    .unwrap()
                    .split_whitespace()
                    .map(str::to_owned)
                    .collect();
                atoms.sort();
                let score = lines
                    .peek()
                    .filter(|line| line.starts_with("Optimization:"))
                    .map(|_| ())
                    .and_then(|()| lines.next().map(str::to_owned));
                rows.push((atoms, score));
            }
        }
        rows.sort();
        rows
    }

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
        ]
        .into_iter()
        .chain(super::count_objective_sources::SATISFIABLE)
        .chain([super::count_objective_sources::INCONSISTENT])
        {
            let mut expected = Vec::new();
            let cpu = run_with_diagnostics(
                source.into(),
                &options(&["--backend", "cpu"]),
                &mut expected,
                &mut Vec::new(),
                &Control::default(),
            )
            .unwrap();
            assert_eq!(cpu.completion, Completion::Exhausted);
            for batch in ["3", "7", "16"] {
                let mut actual = Vec::new();
                let mut diagnostics = Vec::new();
                let report = run_with_diagnostics(
                    source.into(),
                    &options(&[
                        "--backend",
                        backend.argument(),
                        "--oracle",
                        "countermodel",
                        "--batch-size",
                        batch,
                        "--stats",
                    ]),
                    &mut actual,
                    &mut diagnostics,
                    &Control::default(),
                )
                .unwrap();
                assert_eq!(report.completion, Completion::Exhausted);
                assert_eq!(report.models, cpu.models);
                assert_eq!(
                    records(&actual),
                    records(&expected),
                    "{source} batch={batch}"
                );
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
}
