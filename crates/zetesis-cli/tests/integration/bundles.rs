//! Original-file bundle invocation shares solver behavior and keeps refusals located.

use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};

use crate::support::human::before_timing;
use crate::support::options::enumerating as options;
use zetesis_cli::{Completion, RunError, run_bundle_with_diagnostics, run_with_diagnostics};
use zetesis_cpu::Cancellation;
use zetesis_themelios::{BundleLimits, SourceBundle};

struct Fixture(tempfile::TempDir);
impl Fixture {
    fn new() -> Self {
        Self(tempfile::tempdir().expect("temporary fixture"))
    }
    fn write(&self, name: &str, source: &str) {
        fs::write(self.0.path().join(name), source).unwrap();
    }
    fn bundle(&self) -> SourceBundle {
        SourceBundle::load(self.0.path().join("entry.lp"), BundleLimits::default()).unwrap()
    }
    fn process(&self, arguments: &[&str]) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_zetesis"))
            .args(["--backend", "cpu", "--models", "0"])
            .args(arguments)
            .arg(self.0.path().join("entry.lp"))
            .stdin(Stdio::null())
            .output()
            .unwrap()
    }
}

const MALFORMED_CHOICE: &str = "{a,b :- q.\nq :- c.\n";

#[test]
fn typed_include_depth_bounds_the_loaded_graph() {
    let fixture = Fixture::new();
    fixture.write("entry.lp", "#include \"child.lp\".");
    fixture.write("child.lp", "#include \"leaf.lp\".");
    fixture.write("leaf.lp", "a.");
    let refused = SourceBundle::load(
        fixture.0.path().join("entry.lp"),
        BundleLimits {
            max_include_depth: 1,
            ..BundleLimits::default()
        },
    )
    .unwrap_err();
    assert!(matches!(
        refused,
        zetesis_themelios::BundleError::Limit {
            resource: zetesis_themelios::BundleResource::IncludeDepth,
            ..
        }
    ));
    let complete_bundle = SourceBundle::load(
        fixture.0.path().join("entry.lp"),
        BundleLimits {
            max_include_depth: 2,
            ..BundleLimits::default()
        },
    )
    .unwrap();
    assert_eq!(complete_bundle.sources().len(), 3);
    let complete = fixture.process(&["--color", "never"]);
    assert!(complete.status.success());
    let output = String::from_utf8(complete.stdout).unwrap();
    assert!(output.contains("a\n"), "{output}");
}

#[test]
fn stdin_syntax_failure_renders_each_diagnostic_once() {
    for arguments in [
        vec!["--backend", "cpu", "--color", "never"],
        vec![
            "--backend",
            "cpu",
            "--color",
            "never",
            "--oracle",
            "countermodel",
        ],
    ] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_zetesis"))
            .args(arguments)
            .arg("-")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(MALFORMED_CHOICE.as_bytes())
            .unwrap();
        let result = child.wait_with_output().unwrap();
        assert_eq!(result.status.code(), Some(2));
        assert!(
            !std::str::from_utf8(&result.stdout)
                .unwrap()
                .contains("Answer:")
        );
        let text = String::from_utf8(result.stderr).unwrap();
        assert_eq!(text.matches("error[syntax::").count(), 3, "{text}");
        assert!(text.contains("<input>:1:3"), "{text}");
        assert!(text.contains("1 | {a,b :- q."), "{text}");
        assert!(!text.contains("bytes "), "{text}");
    }
}

#[test]
fn syntax_failure_exposes_each_original_diagnostic() {
    let fixture = Fixture::new();
    fixture.write("entry.lp", MALFORMED_CHOICE);
    let result = fixture.process(&["--color", "never"]);
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
    let text = String::from_utf8(result.stderr).unwrap();
    assert_eq!(text.matches("error[syntax::").count(), 3, "{text}");
    assert!(text.contains("entry.lp:1:"), "{text}");
    assert!(text.contains("1 | {a,b :- q."), "{text}");
    assert!(text.contains('^'), "{text}");
    assert!(!text.contains("UNSATISFIABLE"));
}

#[test]
fn included_syntax_failure_resolves_the_child_source() {
    let fixture = Fixture::new();
    fixture.write("entry.lp", "#include \"child.lp\". q.");
    fixture.write("child.lp", "% original é\r\n{a,b :- q.\r\n");
    let result = fixture.process(&["--color", "never"]);
    let text = String::from_utf8(result.stderr).unwrap();
    assert_eq!(result.status.code(), Some(2));
    assert!(text.contains("child.lp:2:"), "{text}");
    assert!(text.contains("2 | {a,b :- q."), "{text}");
    assert!(!text.contains("unknown source"), "{text}");
}

#[test]
fn later_root_syntax_failure_keeps_its_source_identity() {
    let fixture = Fixture::new();
    fixture.write("entry.lp", "q.");
    fixture.write("later.lp", MALFORMED_CHOICE);
    let result = Command::new(env!("CARGO_BIN_EXE_zetesis"))
        .args(["--backend", "cpu", "--color", "never"])
        .arg(fixture.0.path().join("entry.lp"))
        .arg(fixture.0.path().join("later.lp"))
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let text = String::from_utf8(result.stderr).unwrap();
    assert_eq!(result.status.code(), Some(2));
    assert!(text.contains("later.lp:1:"), "{text}");
    assert!(text.contains("1 | {a,b :- q."), "{text}");
}

#[test]
fn syntax_error_styles_preserve_the_canonical_text() {
    let fixture = Fixture::new();
    fixture.write("entry.lp", MALFORMED_CHOICE);
    let plain = fixture.process(&["--color", "never"]);
    let painted = fixture.process(&["--color", "always"]);
    let plain = String::from_utf8(plain.stderr).unwrap();
    let painted = String::from_utf8(painted.stderr).unwrap();
    assert!(painted.contains("\u{1b}[1;31merror[syntax::"), "{painted}");
    assert!(painted.contains("\u{1b}[3;90m -->"), "{painted}");
    let mut unpainted = String::new();
    let mut chunks = painted.split("\u{1b}[");
    unpainted.push_str(chunks.next().unwrap());
    for chunk in chunks {
        let (style, rest) = chunk.split_once('m').expect("terminated SGR");
        assert!(
            style
                .bytes()
                .all(|byte| byte.is_ascii_digit() || byte == b';')
        );
        unpainted.push_str(rest);
    }
    assert_eq!(unpainted, plain);
}

#[test]
fn redirected_syntax_errors_are_plain_by_default() {
    let fixture = Fixture::new();
    fixture.write("entry.lp", MALFORMED_CHOICE);
    let result = fixture.process(&[]);
    assert!(!result.stderr.contains(&0x1b));
}

#[test]
fn json_syntax_failure_remains_machine_readable() {
    let fixture = Fixture::new();
    fixture.write("entry.lp", MALFORMED_CHOICE);
    let result = fixture.process(&["--json", "--color", "always"]);
    assert_eq!(result.status.code(), Some(2));
    let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert!(report["models"].as_array().unwrap().is_empty());
    assert_eq!(report["outcome"]["error"]["kind"], "bundle_load");
    assert!(String::from_utf8_lossy(&result.stderr).contains("error[syntax::"));
    assert!(!result.stdout.contains(&0x1b));
    assert!(!result.stderr.contains(&0x1b));
}

#[test]
fn bundle_and_string_paths_share_exhaustive_solver_results() {
    let fixture = Fixture::new();
    fixture.write(
        "entry.lp",
        "#const upper=lower+1. #include \"data.lp\". {pick(X)} :- d(X).",
    );
    fixture.write("data.lp", "#const lower=1. d(lower..upper).");
    let mut options = options(&["--backend", "cpu", "--stats"]);
    options.statistics_view = zetesis_cli::StatisticsView::Records;
    let mut original = Vec::new();
    let mut diagnostics = Vec::new();
    let report = run_bundle_with_diagnostics(
        fixture.bundle(),
        &options,
        &mut original,
        &mut diagnostics,
        &Cancellation::default(),
    )
    .unwrap();
    let mut explicit = Vec::new();
    let direct = run_with_diagnostics(
        "d(1). d(2). {pick(X)} :- d(X).".into(),
        &options,
        &mut explicit,
        &mut Vec::new(),
        &Cancellation::default(),
    )
    .unwrap();
    assert_eq!(
        before_timing(std::str::from_utf8(&original).unwrap()),
        before_timing(std::str::from_utf8(&explicit).unwrap())
    );
    assert_eq!(report.completion, Completion::Exhausted);
    assert_eq!((report.models, report.checked), (4, 4));
    assert_eq!(report.checked, direct.checked);
    assert!(
        String::from_utf8(diagnostics)
            .unwrap()
            .contains("2 original files")
    );
    let process = fixture.process(&[]);
    assert!(process.status.success());
    assert_eq!(
        before_timing(std::str::from_utf8(&process.stdout).unwrap()),
        before_timing(std::str::from_utf8(&original).unwrap())
    );
    assert!(
        String::from_utf8(process.stdout)
            .unwrap()
            .contains("Backend: CPU")
    );
}

#[test]
fn bundled_show_filters_output_without_merging_hidden_models() {
    let fixture = Fixture::new();
    fixture.write(
        "entry.lp",
        "#include \"data.lp\". {hidden}. #show p/1. #show.",
    );
    fixture.write("data.lp", "#defined unused/2. p(1). q. #show q/0.");
    for grounder in ["lazy", "eager"] {
        let result = fixture.process(&["--grounder", grounder]);
        assert!(result.status.success(), "{:?}", result.stderr);
        let output = String::from_utf8(result.stdout).unwrap();
        assert!(output.contains("Answer: 1\np(1) q\nAnswer: 2\np(1) q\n"));
        assert!(output.contains("SATISFIABLE\nModels: 2\n"));
    }
}

#[test]
fn unsafe_included_rule_keeps_original_file_and_span_in_typed_failure() {
    let fixture = Fixture::new();
    fixture.write("entry.lp", "#include \"unsafe.lp\". d(1).");
    let original = "% source evidence\np(X) :- not q(X).\n";
    fixture.write("unsafe.lp", original);
    let mut models = Vec::new();
    let error = run_bundle_with_diagnostics(
        fixture.bundle(),
        &options(&["--backend", "cpu"]),
        &mut models,
        &mut Vec::new(),
        &Cancellation::default(),
    )
    .unwrap_err();
    assert!(!std::str::from_utf8(&models).unwrap().contains("Answer:"));
    let RunError::BundleAdmission(failure) = &error else {
        panic!("expected located semantic refusal: {error}");
    };
    assert_eq!(failure.bundle().sources().len(), 2);
    let diagnostic = failure.diagnostics().remove(0);
    let location = diagnostic.primary().location;
    let source = failure.bundle().get(location.source).unwrap();
    assert_eq!(source.source().text(), original);
    assert_eq!(
        source.source().slice(location.span).unwrap(),
        "p(X) :- not q(X)."
    );
    let display = error.to_string();
    assert!(display.contains("unsafe.lp:2:1"), "{display}");
    assert!(!display.contains("UNSATISFIABLE"));
}

#[test]
fn combined_expansion_budget_applies_across_original_files() {
    let fixture = Fixture::new();
    fixture.write("entry.lp", "#include \"data.lp\". p(1..2).");
    fixture.write("data.lp", "q(1..2).");
    let error = zetesis_themelios::admit_bundle_extended(
        fixture.bundle(),
        zetesis_themelios::BundleAdmissionOptions::default(),
        zetesis_themelios::ExpansionLimits {
            max_templates: 3,
            ..Default::default()
        },
    )
    .unwrap_err();
    assert!(matches!(
        error.error(),
        zetesis_themelios::BundleAdmissionError::Expansion(
            zetesis_themelios::ExpansionFailure::Limit {
                resource: zetesis_themelios::ExpansionResource::Templates,
                ..
            }
        )
    ));
    assert!(error.to_string().contains("Templates"));
}

#[test]
fn typed_file_byte_limit_applies_to_included_files() {
    let fixture = Fixture::new();
    fixture.write("entry.lp", "#include \"large.lp\".");
    fixture.write("large.lp", &format!("% {}\np.", "padding ".repeat(10)));
    let error = SourceBundle::load(
        fixture.0.path().join("entry.lp"),
        BundleLimits {
            max_file_bytes: 32,
            ..BundleLimits::default()
        },
    )
    .unwrap_err();
    assert!(matches!(
        error,
        zetesis_themelios::BundleError::Limit {
            resource: zetesis_themelios::BundleResource::FileBytes,
            ..
        }
    ));
    assert!(error.to_string().contains("large.lp"));
}

#[test]
fn files_resolve_original_includes_but_strings_have_no_implicit_base_path() {
    let fixture = Fixture::new();
    fixture.write("entry.lp", "#include \"data.lp\".");
    fixture.write("data.lp", "p.");
    let file = fixture.process(&[]);
    assert!(file.status.success());
    assert!(
        String::from_utf8(file.stdout)
            .unwrap()
            .contains("Answer: 1\np\n")
    );
    let error = run_with_diagnostics(
        fs::read_to_string(fixture.0.path().join("entry.lp")).unwrap(),
        &options(&["--backend", "cpu"]),
        &mut Vec::new(),
        &mut Vec::new(),
        &Cancellation::default(),
    )
    .unwrap_err();
    assert!(matches!(error, RunError::FormulaAdmission(_)));
}

#[test]
fn lazy_formula_bundle_is_refused_before_device_discovery() {
    let fixture = Fixture::new();
    fixture.write("entry.lp", "a | b.");
    let error = run_bundle_with_diagnostics(
        fixture.bundle(),
        &options(&["--backend", "metal", "--grounder", "lazy"]),
        &mut Vec::new(),
        &mut Vec::new(),
        &Cancellation::default(),
    )
    .unwrap_err();
    assert!(
        matches!(
            error,
            RunError::HybridBackend {
                backend: zetesis_cli::Backend::Gpu(Some(zetesis_backend::GpuApi::Metal))
            }
        ),
        "{error}"
    );
}
