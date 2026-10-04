//! Runtime observation diagnostics retain original sources and publication evidence.

use std::error::Error as _;
use std::fs;
use std::process::{Command, Output, Stdio};

use clap::Parser;
use zetesis_cli::{
    Options, PublicationFailure, RunError, run_bundle_finalized_with_diagnostics,
    run_finalized_with_diagnostics,
};
use zetesis_cpu::Cancellation;
use zetesis_themelios::base::source::{Source, SourceId};
use zetesis_themelios::observation::{Error, ErrorKind, EvaluationError};
use zetesis_themelios::{
    AdmissionOptions, BundleLimits, ExpansionLimits, FormulaLimits, SourceBundle,
};

const SOURCE: &str = "p(k).\n#show ok : p(_).\n#show sh(X+1) : p(X).\n";
const DIRECTIVE: &str = "#show sh(X+1) : p(X).";

fn options() -> Options {
    Options::try_parse_from(["zetesis", "--backend", "cpu", "--models", "0"]).unwrap()
}

fn observation(failure: &PublicationFailure) -> &Error {
    let RunError::Observation(error) = failure.cause.as_ref() else {
        panic!("expected typed observation failure: {failure}");
    };
    error
}

fn fail_source(source: &str) -> (PublicationFailure, Vec<u8>) {
    let mut output = Vec::new();
    let failure = run_finalized_with_diagnostics(
        source.into(),
        &options(),
        &mut output,
        &mut Vec::new(),
        &Cancellation::default(),
    )
    .unwrap_err();
    (failure, output)
}

#[test]
fn runtime_error_resolves_the_original_directive() {
    let (failure, _) = fail_source(SOURCE);
    let error = observation(&failure);
    assert_eq!(
        error.kind(),
        &ErrorKind::Evaluation(EvaluationError::Undefined)
    );
    let source = error.diagnostic_source().expect("attached original input");
    let location = error.location().unwrap();
    assert_eq!(source.id(), location.source);
    assert_eq!(source.text(), SOURCE);
    assert_eq!(source.slice(location.span).unwrap(), DIRECTIVE);
    assert_eq!(error.diagnostic_source_name(), Some("<input>"));
    assert_eq!(error.diagnostic().unwrap().primary().location, location);
    assert!(failure.cause.source().unwrap().is::<Error>());
    let message = failure.to_string();
    assert_eq!(message.matches("error[zetesis::observation]").count(), 1);
    assert!(message.contains("<input>:3:1"), "{message}");
    assert!(message.contains("3 | #show sh(X+1) : p(X)."), "{message}");
    assert!(message.contains('^'), "{message}");
}

#[test]
fn original_one_line_refusal_points_to_the_failing_show() {
    let (failure, _) = fail_source("p(k). #show ok : p(_). #show sh(X+1) : p(X).");
    let error = observation(&failure);
    assert_eq!(
        error.kind(),
        &ErrorKind::Evaluation(EvaluationError::Undefined)
    );
    assert_eq!(
        error
            .diagnostic_source()
            .unwrap()
            .slice(error.location().unwrap().span)
            .unwrap(),
        DIRECTIVE
    );
    let message = failure.to_string();
    assert!(message.contains("<input>:1:24"), "{message}");
}

#[test]
fn observation_failure_preserves_verified_unpublished_evidence() {
    let (failure, output) = fail_source(SOURCE);
    // The first #show succeeds locally; the later refusal still publishes no
    // partial model or completion and never erases checked membership evidence.
    assert!(crate::support::human::preamble(
        std::str::from_utf8(&output).unwrap()
    ));
    assert!(failure.subject().is_some());
    assert!(failure.semantic().is_some());
    assert_eq!(failure.publication().unwrap().models(), 0);
    assert!(!failure.publication().unwrap().summary());
    let partial = failure.partial_report.as_ref().unwrap();
    assert_eq!((partial.verified_models, partial.published_models), (1, 0));
    assert!(!partial.summary_published);
    assert!(observation(&failure).statistics().work > 0);
    assert!(failure.secondary_output.is_none());
}

struct BundleFixture(tempfile::TempDir);
impl BundleFixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        fs::write(
            directory.path().join("entry.lp"),
            "#include \"child.lp\".\n",
        )
        .unwrap();
        fs::write(directory.path().join("child.lp"), SOURCE).unwrap();
        Self(directory)
    }
    fn process(&self, arguments: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_zetesis"))
            .args(["--backend", "cpu", "--models", "0"])
            .args(arguments)
            .arg(self.0.path().join("entry.lp"))
            .stdin(Stdio::null())
            .output()
            .unwrap()
    }
}

#[test]
fn included_error_retains_loaded_bytes_after_disk_changes() {
    let fixture = BundleFixture::new();
    let bundle =
        SourceBundle::load(fixture.0.path().join("entry.lp"), BundleLimits::default()).unwrap();
    let child = bundle
        .sources()
        .iter()
        .find(|source| source.id() != bundle.entry())
        .unwrap();
    let id = child.id();
    let path = child.path().to_str().unwrap().to_owned();
    fs::write(fixture.0.path().join("child.lp"), "replacement.\n").unwrap();
    let failure = run_bundle_finalized_with_diagnostics(
        bundle,
        &options(),
        &mut Vec::new(),
        &mut Vec::new(),
        &Cancellation::default(),
    )
    .unwrap_err();
    let error = observation(&failure);
    assert_eq!(error.location().unwrap().source, id);
    assert_eq!(error.diagnostic_source().unwrap().id(), id);
    assert_eq!(error.diagnostic_source().unwrap().text(), SOURCE);
    assert_eq!(error.diagnostic_source_name(), Some(path.as_str()));
    let message = failure.to_string();
    assert!(message.contains("child.lp:3:1"), "{message}");
    assert!(message.contains("3 | #show sh(X+1) : p(X)."), "{message}");
    assert!(!message.contains("replacement"), "{message}");
}

#[test]
fn unrelated_source_cannot_replace_the_retained_context() {
    let (failure, _) = fail_source(SOURCE);
    let original = observation(&failure);
    let mut error = original.clone();
    let other = Source::new(SourceId::new(42), "other.".into()).unwrap();
    error.retain_source("other.lp", &other);
    assert_eq!(&error, original);
}

#[test]
fn process_observation_diagnostic_uses_the_existing_color_policy() {
    let fixture = BundleFixture::new();
    let plain = fixture.process(&["--color", "never"]);
    let colored = fixture.process(&["--color", "always"]);
    for result in [&plain, &colored] {
        assert_eq!(result.status.code(), Some(2));
        assert!(crate::support::human::preamble(&without_styles(
            std::str::from_utf8(&result.stdout).unwrap()
        )));
        assert!(String::from_utf8_lossy(&result.stderr).contains("child.lp:3:1"));
    }
    assert!(!plain.stderr.contains(&0x1b));
    let colored = String::from_utf8(colored.stderr).unwrap();
    assert!(
        colored.contains("\u{1b}[1;31merror[zetesis::observation]"),
        "{colored}"
    );
    assert_eq!(without_styles(&colored).as_bytes(), plain.stderr);
}

fn without_styles(text: &str) -> String {
    let mut chunks = text.split("\u{1b}[");
    let mut unpainted = chunks.next().unwrap().to_owned();
    for chunk in chunks {
        let (style, rest) = chunk.split_once('m').expect("terminated SGR");
        assert!(
            style
                .bytes()
                .all(|byte| byte.is_ascii_digit() || byte == b';')
        );
        unpainted.push_str(rest);
    }
    unpainted
}

#[test]
fn json_observation_failure_keeps_models_empty_and_diagnostics_separate() {
    let result = BundleFixture::new().process(&["--json", "--color", "always"]);
    assert_eq!(result.status.code(), Some(2));
    let document: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert!(document["models"].as_array().unwrap().is_empty());
    assert_eq!(document["outcome"]["status"], "failed");
    assert_eq!(document["outcome"]["error"]["kind"], "observation");
    let diagnostics = String::from_utf8(result.stderr).unwrap();
    assert!(diagnostics.contains("child.lp:3:1"), "{diagnostics}");
    assert!(
        diagnostics.contains("3 | #show sh(X+1) : p(X)."),
        "{diagnostics}"
    );
    assert!(!result.stdout.contains(&0x1b));
    assert!(!diagnostics.contains('\u{1b}'));
}

#[test]
fn unlocated_cancellation_never_acquires_an_invented_source() {
    use zetesis_themelios::observation::{Limits, ObservationProgram};

    let cancellation = Cancellation::default();
    cancellation.cancel();
    let mut error = ObservationProgram::default()
        .evaluate(
            &zetesis_core::Model::default(),
            Limits::default(),
            &cancellation,
        )
        .unwrap_err();
    let original = error.clone();
    error.retain_source(
        "input.lp",
        &Source::new(SourceId::new(0), SOURCE.into()).unwrap(),
    );
    assert_eq!(error, original);
    assert!(error.location().is_none());
    assert!(error.diagnostic().is_none());
    assert!(error.diagnostic_source().is_none());
    assert!(error.diagnostic_source_name().is_none());
    assert_eq!(error.to_string(), "observation refused: Stopped(Cancelled)");
}

#[test]
fn source_attachment_preserves_the_library_refusal() {
    use zetesis_themelios::observation::Limits;

    let admitted = zetesis_themelios::admit_formula(
        "#show 1/0.".into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    let mut error = admitted
        .metadata()
        .observations()
        .evaluate(
            &zetesis_core::Model::default(),
            Limits::default(),
            &Cancellation::default(),
        )
        .unwrap_err();
    let original = error.clone();
    assert!(original.diagnostic_source().is_none());
    let location = original.location().unwrap();
    assert_eq!(
        original.to_string(),
        format!(
            "observation refused: Evaluation(Undefined) at source {}, bytes {}..{}",
            location.source.get(),
            location.span.start().get(),
            location.span.end().get(),
        )
    );
    error.retain_source("input.lp", admitted.source().expect("source input"));
    assert_eq!(error.kind(), original.kind());
    assert_eq!(error.location(), original.location());
    assert_eq!(error.statistics(), original.statistics());
    assert_eq!(error.diagnostic(), original.diagnostic());
    assert_eq!(
        error.diagnostic_source(),
        Some(admitted.source().expect("source input"))
    );
}
