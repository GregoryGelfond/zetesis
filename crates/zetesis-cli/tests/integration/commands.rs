//! Direct invocation, backend diagnostics, and command parsing.

use std::process::{Command as ProcessCommand, Stdio};

use crate::support::options::plain as options;
use clap::Parser;
use zetesis_cli::{Backend, Command, Completion, Options, Report, RunError, run_with_diagnostics};
use zetesis_cpu::Cancellation;
use zetesis_test_support::io::Closed;

#[test]
fn normal_invocation_defaults_to_the_cpu_and_preserves_explicit_backends() {
    let configured = options(&["input.lp"]);
    assert_eq!(configured.backend, Backend::Cpu);
    assert_eq!(configured.input.to_str(), Some("input.lp"));
    assert_eq!(configured.command, None);
    for backend in Backend::ALL {
        assert_eq!(options(&["--backend", backend.label()]).backend, backend);
    }
    assert_eq!(options(&["devices"]).command, Some(Command::Devices));
}

#[test]
fn retired_backend_values_are_explained() {
    for retired in ["auto", "dx12", "gl", "nvidia"] {
        let error = Options::try_parse_from(["zetesis", "--backend", retired]).unwrap_err();
        assert_eq!(
            error.kind(),
            clap::error::ErrorKind::InvalidValue,
            "{retired}"
        );
        assert!(
            error
                .to_string()
                .contains(&format!("`{retired}` is no longer a backend")),
            "{error}"
        );
    }
}

/// A tiny automatic run, one node and one choice, with its models and its
/// diagnostics as text.
fn tiny_auto_run() -> (Report, String, String) {
    let mut models = Vec::new();
    let mut diagnostics = Vec::new();
    let report = run_with_diagnostics(
        "node(a). {chosen(X)} :- node(X).".into(),
        &options(&[]),
        &mut models,
        &mut diagnostics,
        &Cancellation::default(),
    )
    .unwrap();
    (
        report,
        String::from_utf8(models).unwrap(),
        String::from_utf8(diagnostics).unwrap(),
    )
}

#[test]
fn a_tiny_auto_run_finds_its_first_answer_after_one_check() {
    // The narrowed root holds the one undecided gate atom; its out branch is
    // the first answer, found after one check.
    let (report, models, _) = tiny_auto_run();
    assert_eq!(report.completion, Completion::RequestedModels);
    assert_eq!(report.checked, 1);
    assert_eq!(report.discovered_gate_atoms, 1);
    assert!(models.contains("Answer: 1\nnode(a)\n"));
}

#[test]
fn default_configuration_precedes_the_models() {
    let (_, models, diagnostics) = tiny_auto_run();
    assert!(models.contains("Backend: CPU"));
    assert!(diagnostics.is_empty());
    assert!(!diagnostics.contains("Answer:"));
}

#[test]
fn diagnostics_failure_is_propagated_before_model_output() {
    let mut models = Vec::new();
    let error = run_with_diagnostics(
        "a.".into(),
        &options(&["--backend", "cpu", "--stats"]),
        &mut models,
        &mut Closed,
        &Cancellation::default(),
    )
    .unwrap_err();
    assert!(matches!(error, RunError::Output(_)));
    assert!(crate::support::human::preamble(&String::from_utf8_lossy(
        &models
    )));
}

#[test]
fn finite_expansion_is_automatic_and_respects_its_own_limits() {
    let source = "#const n = 3. p(1..n). {q(X)} :- p(X).";
    let mut models = Vec::new();
    let mut diagnostics = Vec::new();
    let report = run_with_diagnostics(
        source.into(),
        &options(&["--backend", "cpu", "--stats"]),
        &mut models,
        &mut diagnostics,
        &Cancellation::default(),
    )
    .unwrap();
    assert_eq!(report.models, 1);
    assert!(
        String::from_utf8(models)
            .unwrap()
            .contains("p(1) p(2) p(3)")
    );
    assert!(
        String::from_utf8(diagnostics)
            .unwrap()
            .contains("Oracle: reduct closure")
    );
    let error = run_with_diagnostics(
        source.into(),
        &options(&["--max-expanded-templates", "2"]),
        &mut Vec::new(),
        &mut Vec::new(),
        &Cancellation::default(),
    )
    .unwrap_err();
    assert!(matches!(error, RunError::Expansion(_)));
    assert!(error.to_string().contains("bytes "));
}

#[test]
fn help_and_version_are_directly_runnable_without_input() {
    for argument in ["--help", "--version"] {
        let result = ProcessCommand::new(env!("CARGO_BIN_EXE_zetesis"))
            .arg(argument)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert!(result.status.success());
        let output = String::from_utf8(result.stdout).unwrap();
        assert!(output.contains("zetesis"));
        if argument == "--help" {
            assert!(output.contains("devices"));
            assert!(output.contains("solve"));
            assert!(!output.contains("--max-search-work"));
        }
    }
}

#[test]
fn bench_names_the_separate_benchmarking_tool() {
    // `bench` stays reserved, so it is never read as a file-first source; the
    // refusal names the tool and the spelling that solves a file named bench.
    for arguments in [
        &["bench"][..],
        &["bench", "corpus", "--json"],
        &["help", "bench"],
    ] {
        let result = ProcessCommand::new(env!("CARGO_BIN_EXE_zetesis"))
            .args(arguments)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(2), "{arguments:?}");
        assert!(result.stdout.is_empty(), "{arguments:?}");
        let message = String::from_utf8(result.stderr).unwrap();
        assert!(
            message.contains("`bench` is not a zetesis command"),
            "{message}"
        );
        assert!(
            message.contains("the separate `zetesis-bench` tool (see INSTALL.md)"),
            "{message}"
        );
        assert!(message.contains("`zetesis solve bench`"), "{message}");
    }
}

#[test]
fn process_includes_configuration_in_the_human_view() {
    let source = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/correctness/excerpts/task-allocation-projections.lp"
    );
    let result = ProcessCommand::new(env!("CARGO_BIN_EXE_zetesis"))
        .args(["--backend", "cpu", source])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(result.status.success());
    let output = String::from_utf8(result.stdout).unwrap();
    let errors = String::from_utf8(result.stderr).unwrap();
    assert!(output.contains("Answer: 1\n"));
    assert!(output.contains("Backend: CPU"));
    assert!(errors.is_empty());
    assert!(!errors.contains("Answer:"));
}

#[cfg(not(feature = "gpu"))]
#[test]
fn cpu_only_devices_and_explicit_gpu_refusal_are_truthful() {
    let result = ProcessCommand::new(env!("CARGO_BIN_EXE_zetesis"))
        .arg("devices")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(result.status.success());
    let output = String::from_utf8(result.stdout).unwrap();
    assert!(output.contains("GPU: not compiled into this build"));
    assert!(!output.contains("Answer:"));
    for backend in ["gpu", "metal", "vulkan"] {
        let mut output = Vec::new();
        let error = run_with_diagnostics(
            "a.".into(),
            &options(&["--backend", backend]),
            &mut output,
            &mut Vec::new(),
            &Cancellation::default(),
        )
        .unwrap_err();
        assert!(matches!(error, RunError::BackendUnavailable));
        assert!(crate::support::human::preamble(&String::from_utf8_lossy(
            &output
        )));
    }
}

#[cfg(feature = "gpu")]
#[test]
fn default_execution_ignores_device_transport_limits() {
    let mut models = Vec::new();
    let mut diagnostics = Vec::new();
    // Device transport has no producer on the default CPU route. A zero
    // transport ceiling cannot truncate the family or trigger discovery.
    let report = run_with_diagnostics(
        "{a}. {b}. {c}. {d}. {e}. {f}.".into(),
        &options(&[
            "--models",
            "0",
            "--grounder",
            "eager",
            "--max-batch-bytes",
            "0",
        ]),
        &mut models,
        &mut diagnostics,
        &Cancellation::default(),
    )
    .unwrap();
    assert_eq!(report.completion, Completion::Exhausted);
    assert_eq!((report.models, report.checked), (64, 64));
    let models = String::from_utf8(models).unwrap();
    let lines: Vec<_> = models.lines().collect();
    let answers: std::collections::BTreeSet<_> = lines
        .windows(2)
        .filter(|pair| pair[0].starts_with("Answer:"))
        .map(|pair| pair[1].to_owned())
        .collect();
    // Six independent choices have exactly their powerset as answer sets.
    // Construct that family without consulting the solver's carrier or output.
    let expected: std::collections::BTreeSet<_> = (0_u8..64)
        .map(|mask| {
            ["a", "b", "c", "d", "e", "f"]
                .into_iter()
                .enumerate()
                .filter_map(|(index, atom)| (mask & (1 << index) != 0).then_some(atom))
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect();
    assert_eq!(answers, expected);
    let diagnostics = String::from_utf8(diagnostics).unwrap();
    assert!(models.contains("Backend: CPU"));
    assert!(!diagnostics.contains("Backend: gpu"));
}

#[cfg(feature = "gpu")]
#[test]
fn explicit_gpu_failure_cannot_publish_a_cpu_model() {
    let mut models = Vec::new();
    let mut diagnostics = Vec::new();
    let error = run_with_diagnostics(
        "a.".into(),
        &options(&[
            "--backend",
            "gpu",
            "--grounder",
            "eager",
            "--max-batch-bytes",
            "0",
        ]),
        &mut models,
        &mut diagnostics,
        &Cancellation::default(),
    )
    .unwrap_err();
    assert!(matches!(error, RunError::Gpu(_)));
    assert!(crate::support::human::preamble(&String::from_utf8_lossy(
        &models
    )));
    assert!(
        !String::from_utf8(diagnostics)
            .unwrap()
            .contains("Backend: cpu")
    );
    println!("explicit device refusal: {error}");
}
