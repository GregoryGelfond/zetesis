//! Direct invocation, backend diagnostics, and command parsing.

use std::process::{Command as ProcessCommand, Stdio};

use clap::Parser;
use zetesis_cli::{Backend, Command, Completion, Options, RunError, run_with_diagnostics};
use zetesis_cpu::Control;

fn options(arguments: &[&str]) -> Options {
    Options::try_parse_from(["zetesis"].into_iter().chain(arguments.iter().copied())).unwrap()
}

#[test]
fn normal_invocation_defaults_to_auto_and_preserves_explicit_backends() {
    let configured = options(&["input.lp"]);
    assert_eq!(configured.backend, Backend::Auto);
    assert_eq!(configured.input.to_str(), Some("input.lp"));
    assert_eq!(configured.command, None);
    for (argument, backend) in [
        ("auto", Backend::Auto),
        ("cpu", Backend::Cpu),
        ("gpu", Backend::Gpu),
        ("metal", Backend::Metal),
        ("vulkan", Backend::Vulkan),
        ("dx12", Backend::Dx12),
        ("gl", Backend::Gl),
        ("nvidia", Backend::Nvidia),
    ] {
        assert_eq!(options(&["--backend", argument]).backend, backend);
    }
    assert_eq!(options(&["devices"]).command, Some(Command::Devices));
}

#[test]
fn tiny_auto_run_keeps_lazy_first_seed_and_separates_diagnostics() {
    let mut models = Vec::new();
    let mut diagnostics = Vec::new();
    let report = run_with_diagnostics(
        "node(a). {chosen(X)} :- node(X).".into(),
        &options(&[]),
        &mut models,
        &mut diagnostics,
        &Control::default(),
    )
    .unwrap();
    assert_eq!(report.completion, Completion::RequestedModels);
    assert_eq!(report.checked, 1);
    assert_eq!(report.discovered_gate_atoms, 0);
    let models = String::from_utf8(models).unwrap();
    let diagnostics = String::from_utf8(diagnostics).unwrap();
    assert!(models.starts_with("Answer: 1\nnode(a)\n"));
    assert!(!models.contains("Backend:"));
    assert!(!models.contains("Auto:"));
    assert!(diagnostics.contains("Backend: cpu"));
    assert!(!diagnostics.contains("Answer:"));
    #[cfg(feature = "gpu")]
    assert!(diagnostics.contains("no measured GPU crossover"));
    #[cfg(not(feature = "gpu"))]
    assert!(diagnostics.contains("without device discovery"));
}

#[test]
fn diagnostics_failure_is_propagated_before_model_output() {
    struct Broken;
    impl std::io::Write for Broken {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::ErrorKind::BrokenPipe.into())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut models = Vec::new();
    let error = run_with_diagnostics(
        "a.".into(),
        &options(&["--backend", "cpu"]),
        &mut models,
        &mut Broken,
        &Control::default(),
    )
    .unwrap_err();
    assert!(matches!(error, RunError::Output(_)));
    assert!(models.is_empty());
}

#[test]
fn finite_expansion_is_automatic_and_respects_its_own_limits() {
    let source = "#const n = 3. p(1..n). {q(X)} :- p(X).";
    let mut models = Vec::new();
    let mut diagnostics = Vec::new();
    let report = run_with_diagnostics(
        source.into(),
        &options(&["--backend", "cpu"]),
        &mut models,
        &mut diagnostics,
        &Control::default(),
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
        &Control::default(),
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
            assert!(output.contains("[default: auto]"));
        }
    }
}

#[test]
fn process_keeps_backend_reporting_on_stderr() {
    let source = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/kr-domains/accepted/task-allocation-projections.lp"
    );
    let result = ProcessCommand::new(env!("CARGO_BIN_EXE_zetesis"))
        .args(["--backend", "cpu", source])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(result.status.success());
    let output = String::from_utf8(result.stdout).unwrap();
    let errors = String::from_utf8(result.stderr).unwrap();
    assert!(output.starts_with("Answer: 1\n"));
    assert!(!output.contains("Backend:"));
    assert!(errors.contains("Backend: cpu"));
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
    assert!(output.contains("GPU: support not compiled"));
    assert!(!output.contains("Answer:"));
    for backend in ["gpu", "metal", "vulkan", "dx12", "gl", "nvidia"] {
        let mut output = Vec::new();
        let error = run_with_diagnostics(
            "a.".into(),
            &options(&["--backend", backend]),
            &mut output,
            &mut Vec::new(),
            &Control::default(),
        )
        .unwrap_err();
        assert!(matches!(error, RunError::BackendUnavailable));
        assert!(output.is_empty());
    }
}

#[cfg(feature = "gpu")]
#[test]
fn automatic_execution_ignores_device_transport_limits() {
    let mut models = Vec::new();
    let mut diagnostics = Vec::new();
    // Device transport has no producer on this automatic CPU route. A zero
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
        &Control::default(),
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
    assert!(diagnostics.contains("no measured GPU crossover"));
    assert!(diagnostics.contains("Backend: cpu"));
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
        &Control::default(),
    )
    .unwrap_err();
    assert!(matches!(error, RunError::Gpu(_)));
    assert!(models.is_empty());
    assert!(
        !String::from_utf8(diagnostics)
            .unwrap()
            .contains("Backend: cpu")
    );
    println!("explicit device refusal: {error}");
}
