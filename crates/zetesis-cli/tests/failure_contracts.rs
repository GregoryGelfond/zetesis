//! Transport and admission failures must remain typed, located, and incomplete.

use std::error::Error;
use std::io::{self, Write};
use std::num::NonZeroUsize;
use std::process::{Command, Stdio};

use clap::Parser;
use zetesis_cli::{Completion, Options, RunError, run_with_diagnostics};
use zetesis_cpu::Control;

struct CutWriter {
    capacity: usize,
    retained: Vec<u8>,
}
impl CutWriter {
    fn new(capacity: usize) -> Self {
        Self {
            capacity,
            retained: Vec::new(),
        }
    }
}
impl Write for CutWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let count = bytes.len().min(self.capacity - self.retained.len());
        if count == 0 && !bytes.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "fixture output closed",
            ));
        }
        self.retained.extend_from_slice(&bytes[..count]);
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn options(arguments: &[&str]) -> Options {
    Options::try_parse_from(
        [
            "zetesis",
            "--backend",
            "cpu",
            "--workers",
            "1",
            "--models",
            "0",
        ]
        .into_iter()
        .chain(arguments.iter().copied()),
    )
    .unwrap()
}

fn output_error(error: &RunError) {
    let RunError::Output(source) = error else {
        panic!("expected transport failure: {error}")
    };
    assert_eq!(source.kind(), io::ErrorKind::BrokenPipe);
    assert!(error.to_string().contains("fixture output closed"));
    assert_eq!(error.source().unwrap().to_string(), source.to_string());
}

#[test]
fn every_output_truncation_propagates_through_each_cpu_oracle() {
    let scenarios: &[(&str, &[&str])] = &[
        (
            "p(1,foo). q(\"a b\\\"c\\\\d\\ne\").",
            &["--oracle", "closure", "--grounder", "lazy"],
        ),
        ("-q.", &["--oracle", "closure", "--grounder", "lazy"]),
        (
            "{a}. b:-a.",
            &["--oracle", "closure", "--grounder", "eager"],
        ),
        ("a | b.", &["--oracle", "countermodel"]),
        (
            "a. #show. #show f(1). #minimize{2@1,k:a}.",
            &["--oracle", "countermodel"],
        ),
        ("a. #minimize{2@1,k:a}.", &["--oracle", "countermodel"]),
    ];
    for &(source, arguments) in scenarios {
        let options = options(arguments);
        let mut complete = Vec::new();
        let report = run_with_diagnostics(
            source.into(),
            &options,
            &mut complete,
            &mut io::sink(),
            &Control::default(),
        )
        .unwrap();
        assert_eq!(report.completion, Completion::Exhausted);
        for capacity in 0..complete.len() {
            let mut output = CutWriter::new(capacity);
            let error = run_with_diagnostics(
                source.into(),
                &options,
                &mut output,
                &mut io::sink(),
                &Control::default(),
            )
            .unwrap_err();
            output_error(&error);
            assert_eq!(output.retained, complete[..capacity]);
        }
        let mut output = CutWriter::new(complete.len());
        let report = run_with_diagnostics(
            source.into(),
            &options,
            &mut output,
            &mut io::sink(),
            &Control::default(),
        )
        .unwrap();
        assert_eq!(report.completion, Completion::Exhausted);
        assert_eq!(output.retained, complete);
    }
}

#[test]
fn diagnostic_truncation_is_a_transport_failure_before_false_completion() {
    for (source, arguments) in [
        ("a.", vec!["--oracle", "closure", "--grounder", "lazy"]),
        ("{a}.", vec!["--oracle", "closure", "--grounder", "eager"]),
        ("{a;b}. #minimize{1:a}.", vec!["--oracle", "countermodel"]),
    ] {
        let options = options(&arguments);
        let mut complete = Vec::new();
        run_with_diagnostics(
            source.into(),
            &options,
            &mut io::sink(),
            &mut complete,
            &Control::default(),
        )
        .unwrap();
        for capacity in 0..complete.len() {
            let mut diagnostics = CutWriter::new(capacity);
            let mut output = Vec::new();
            let error = run_with_diagnostics(
                source.into(),
                &options,
                &mut output,
                &mut diagnostics,
                &Control::default(),
            )
            .unwrap_err();
            output_error(&error);
            assert_eq!(diagnostics.retained, complete[..capacity]);
            assert!(
                !String::from_utf8(output)
                    .unwrap()
                    .contains("Coverage: exhausted")
            );
        }
    }
}

#[test]
fn admission_and_materialization_failures_retain_causes_and_locations() {
    for (source, arguments, expected) in [
        ("p(X).", vec!["--oracle", "closure"], "expansion"),
        ("#project a/0.", vec!["--oracle", "countermodel"], "formula"),
        (
            "a.",
            vec![
                "--oracle",
                "closure",
                "--grounder",
                "eager",
                "--max-ground-rules",
                "0",
            ],
            "static",
        ),
        (
            "a. #show a.",
            vec!["--max-observation-work", "0"],
            "observation",
        ),
    ] {
        let mut output = Vec::new();
        let error = run_with_diagnostics(
            source.into(),
            &options(&arguments),
            &mut output,
            &mut io::sink(),
            &Control::default(),
        )
        .unwrap_err();
        match (&error, expected) {
            (RunError::Expansion(_), "expansion")
            | (RunError::FormulaAdmission(_), "formula")
            | (RunError::Static(_), "static")
            | (RunError::Observation(_), "observation") => (),
            _ => panic!("unexpected {expected} error: {error:?}"),
        }
        assert!(error.source().is_some(), "{error}");
        let message = error.to_string();
        if matches!(expected, "expansion" | "formula") {
            assert!(message.starts_with("source admission:"), "{message}");
            assert!(message.contains("bytes "), "{message}");
        }
        assert!(output.is_empty());
    }
    let mut options = options(&[]);
    options.max_observation_bytes = 20;
    let mut output = Vec::new();
    let error = run_with_diagnostics(
        "a.b.c.d.e.f.g.h. #show x.".into(),
        &options,
        &mut output,
        &mut io::sink(),
        &Control::default(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        RunError::ObservationOutputLimit { limit: 20, .. }
    ));
    assert!(error.source().is_none());
    assert!(error.to_string().contains("human Answer requires at least"));
    assert!(output.is_empty());
}

#[test]
fn lower_layer_failures_preserve_typed_causes_at_the_public_cli_boundary() {
    let admission = zetesis_themelios::admit(
        "p(.".to_owned(),
        zetesis_themelios::AdmissionOptions::default(),
    )
    .unwrap_err();
    assert!(!admission.diagnostics().is_empty());
    let error = RunError::Admission(admission);
    assert!(
        error
            .source()
            .unwrap()
            .downcast_ref::<zetesis_themelios::AdmissionFailure>()
            .is_some()
    );
    let diagnostic = error.to_string();
    assert!(diagnostic.starts_with("source admission:"));
    assert!(diagnostic.contains("bytes "), "{diagnostic}");

    let admitted = zetesis_themelios::admit(
        "p.".to_owned(),
        zetesis_themelios::AdmissionOptions::default(),
    )
    .unwrap();
    let seed = zetesis_core::Seed::new(admitted.program(), []).unwrap();
    let pool = zetesis_cpu::BatchOracle::new(NonZeroUsize::MIN, NonZeroUsize::MIN).unwrap();
    let failure = pool
        .check_batch(
            admitted.program(),
            &[seed.clone(), seed],
            zetesis_cpu::Limits::default(),
            &Control::default(),
        )
        .unwrap_err();
    let error = RunError::Batch(failure);
    assert!(matches!(
        error
            .source()
            .unwrap()
            .downcast_ref::<zetesis_cpu::BatchError>(),
        Some(zetesis_cpu::BatchError::Capacity {
            limit: 1,
            actual: 2
        })
    ));
    assert_eq!(error.to_string(), "batch of 2 exceeds capacity 1");

    let failure = zetesis_ferraris::Theory::new(
        1,
        vec![zetesis_ferraris::Node::Atom(1)],
        vec![0],
        zetesis_ferraris::AdmissionLimits::default(),
    )
    .unwrap_err();
    let error = RunError::Formula(failure);
    assert_eq!(
        error
            .source()
            .unwrap()
            .downcast_ref::<zetesis_ferraris::AdmissionError>(),
        Some(&zetesis_ferraris::AdmissionError::Atom)
    );
    assert_eq!(error.to_string(), "atom is outside the formula universe");

    let ground = zetesis_core::GroundProgram::compile(
        admitted.program(),
        zetesis_core::StaticLimits::default(),
    )
    .unwrap();
    assert_eq!(ground.atom_count(), 1);
    for (words, expected) in [
        (vec![], "expected 1 result words, received 0"),
        (
            vec![2],
            "result contains nonzero bits outside the atom carrier",
        ),
    ] {
        let error = RunError::Words(ground.model_from_words(&words).unwrap_err());
        assert!(
            error
                .source()
                .unwrap()
                .downcast_ref::<zetesis_core::WordError>()
                .is_some()
        );
        assert_eq!(error.to_string(), expected);
        assert_eq!(error.source().unwrap().to_string(), expected);
    }
}

#[test]
fn lazy_countermodel_requests_are_refused_before_parsing() {
    let mut options = options(&["--oracle", "countermodel", "--grounder", "lazy"]);
    options.backend = zetesis_cli::Backend::Metal;
    let mut output = Vec::new();
    let mut diagnostics = Vec::new();
    let error = run_with_diagnostics(
        "this source cannot parse".into(),
        &options,
        &mut output,
        &mut diagnostics,
        &Control::default(),
    )
    .unwrap_err();
    match &error {
        RunError::UnsupportedOracle { backend, grounder } => {
            assert_eq!(*backend, options.backend);
            assert_eq!(*grounder, options.grounder);
        }
        _ => panic!("route validation must precede source admission: {error}"),
    }
    let message = error.to_string();
    assert!(
        message.contains("the countermodel oracle requires --grounder eager or auto"),
        "{message}"
    );
    assert!(message.contains("Metal"), "{message}");
    assert!(message.contains("lazy"), "{message}");
    assert!(error.source().is_none());
    assert!(output.is_empty());
    assert!(diagnostics.is_empty(), "no backend may be initialized");
}

#[test]
fn process_rejects_non_utf8_and_excessive_stdin_without_claiming_unsat() {
    for (arguments, input, expected) in [
        (
            vec!["--backend", "cpu", "--max-source-bytes", "1"],
            b"a.".as_slice(),
            "source byte limit",
        ),
        (vec!["--backend", "cpu"], b"\xff".as_slice(), "UTF-8"),
        (
            vec!["--backend", "cpu", "-", "another.lp"],
            b"".as_slice(),
            "must be the only input",
        ),
    ] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_zetesis"))
            .args(arguments)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(input).unwrap();
        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        let error = String::from_utf8(output.stderr).unwrap();
        assert!(error.contains(expected), "{error}");
        assert!(!error.contains("UNSATISFIABLE"));
    }
}

#[test]
fn device_output_failure_is_reported_before_adapter_discovery() {
    let error = zetesis_cli::devices(&mut CutWriter::new(0)).unwrap_err();
    output_error(&error);
}

#[test]
fn partial_and_interrupted_summaries_propagate_every_output_failure() {
    let mut requested = options(&["--oracle", "closure"]);
    requested.models = 1;
    for options in [
        requested,
        options(&["--oracle", "closure", "--max-candidates", "0"]),
        options(&["--oracle", "countermodel", "--max-search-work", "0"]),
    ] {
        let mut complete = Vec::new();
        let report = run_with_diagnostics(
            "{a}.".into(),
            &options,
            &mut complete,
            &mut io::sink(),
            &Control::default(),
        )
        .unwrap();
        assert_ne!(report.completion, Completion::Exhausted);
        let text = std::str::from_utf8(&complete).unwrap();
        assert!(text.contains("Coverage: partial"));
        assert!(!text.contains("UNSATISFIABLE"));
        for capacity in 0..complete.len() {
            let mut output = CutWriter::new(capacity);
            let error = run_with_diagnostics(
                "{a}.".into(),
                &options,
                &mut output,
                &mut io::sink(),
                &Control::default(),
            )
            .unwrap_err();
            output_error(&error);
            assert_eq!(output.retained, complete[..capacity]);
        }
    }
}

#[cfg(not(feature = "gpu"))]
#[test]
fn cpu_only_inventory_propagates_failures_after_each_capability_record() {
    let mut complete = Vec::new();
    zetesis_cli::devices(&mut complete).unwrap();
    for capacity in 0..complete.len() {
        let mut output = CutWriter::new(capacity);
        let error = zetesis_cli::devices(&mut output).unwrap_err();
        output_error(&error);
        assert_eq!(output.retained, complete[..capacity]);
    }
}

#[cfg(not(feature = "gpu"))]
#[test]
fn cpu_only_auto_eager_routing_reports_its_compiled_capability() {
    let mut options = options(&["--oracle", "closure", "--grounder", "eager"]);
    options.backend = zetesis_cli::Backend::Auto;
    let mut output = Vec::new();
    let mut diagnostics = Vec::new();
    let report = run_with_diagnostics(
        "{a}.".into(),
        &options,
        &mut output,
        &mut diagnostics,
        &Control::default(),
    )
    .unwrap();
    assert_eq!(report.completion, Completion::Exhausted);
    assert_eq!(report.models, 2);
    let diagnostics = String::from_utf8(diagnostics).unwrap();
    assert!(diagnostics.contains("effective=eager"));
    assert!(
        diagnostics.contains("GPU support was not compiled; using CPU without device discovery")
    );
    assert!(!diagnostics.contains("GPU discovery deferred"));
}

#[test]
fn device_inventory_exposes_availability_without_claiming_execution() {
    let mut output = Vec::new();
    let result = zetesis_cli::devices(&mut output);
    let output = String::from_utf8(output).unwrap();
    assert!(output.starts_with("CPU: available"));
    match result {
        Ok(()) => {
            assert!(output.contains("Auto backend:"));
            assert!(output.contains("CUDA is not implemented"));
            assert!(!output.contains("status=PASS"));
        }
        #[cfg(feature = "gpu")]
        Err(RunError::Gpu(error)) => {
            // No native API is a valid inventory result on some targets. Other
            // failures remain test failures; physical devices are never required.
            assert_eq!(error.kind(), zetesis_wgpu::GpuErrorKind::AdapterUnavailable);
            assert!(output.contains("Compiled GPU APIs:"));
            assert!(!output.contains("Auto backend:"));
        }
        Err(error) => panic!("unexpected inventory failure: {error}"),
    }
}

#[test]
fn automatic_lazy_routing_preserves_source_execution_without_device_discovery() {
    let mut options = options(&["--oracle", "closure", "--grounder", "lazy"]);
    options.backend = zetesis_cli::Backend::Auto;
    let mut output = Vec::new();
    let mut diagnostics = Vec::new();
    let report = run_with_diagnostics(
        "a.".into(),
        &options,
        &mut output,
        &mut diagnostics,
        &Control::default(),
    )
    .unwrap();
    assert_eq!(report.completion, Completion::Exhausted);
    assert_eq!(report.models, 1);
    let diagnostics = String::from_utf8(diagnostics).unwrap();
    assert!(diagnostics.contains("using CPU without device discovery"));
    assert!(diagnostics.contains("effective=lazy"));
    assert!(!diagnostics.contains("effective=eager"));
}

#[cfg(not(feature = "gpu"))]
#[test]
fn cpu_only_binary_reports_explicit_gpu_unavailability_without_fallback() {
    for backend in ["gpu", "metal", "nvidia", "vulkan", "dx12", "gl"] {
        let options =
            Options::try_parse_from(["zetesis", "--backend", backend, "--oracle", "closure"])
                .unwrap();
        let mut output = Vec::new();
        let error = run_with_diagnostics(
            "a.".into(),
            &options,
            &mut output,
            &mut io::sink(),
            &Control::default(),
        )
        .unwrap_err();
        assert!(matches!(error, RunError::BackendUnavailable));
        assert!(error.source().is_none());
        assert!(error.to_string().contains("GPU support was not compiled"));
        assert!(output.is_empty());
    }
    let mut output = Vec::new();
    zetesis_cli::devices(&mut output).unwrap();
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains("GPU: support not compiled"));
    assert!(!output.contains("Adapter:"));
    assert!(output.contains("CUDA is not implemented"));
}
