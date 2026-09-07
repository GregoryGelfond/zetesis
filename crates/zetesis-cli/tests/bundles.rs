//! Original-file bundle invocation shares solver behavior and keeps refusals located.

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use clap::Parser;
use zetesis_cli::{
    Completion, Options, RunError, run_bundle_with_diagnostics, run_with_diagnostics,
};
use zetesis_cpu::Control;
use zetesis_themelios::{BundleLimits, SourceBundle};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        loop {
            let serial = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "zetesis-cli-bundle-{}-{serial}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => panic!("temporary fixture: {error}"),
            }
        }
    }
    fn write(&self, name: &str, source: &str) {
        fs::write(self.0.join(name), source).unwrap();
    }
    fn bundle(&self) -> SourceBundle {
        SourceBundle::load(self.0.join("entry.lp"), BundleLimits::default()).unwrap()
    }
    fn process(&self, arguments: &[&str]) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_zetesis"))
            .args(["--backend", "cpu", "--models", "0"])
            .args(arguments)
            .arg(self.0.join("entry.lp"))
            .stdin(Stdio::null())
            .output()
            .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        // Remove only the unique directory created by this fixture.
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn options(arguments: &[&str]) -> Options {
    Options::try_parse_from(
        ["zetesis", "--models", "0"]
            .into_iter()
            .chain(arguments.iter().copied()),
    )
    .unwrap()
}

#[test]
fn bundle_and_string_paths_share_exhaustive_solver_results() {
    let fixture = Fixture::new();
    fixture.write(
        "entry.lp",
        "#const upper=lower+1. #include \"data.lp\". {pick(X)} :- d(X).",
    );
    fixture.write("data.lp", "#const lower=1. d(lower..upper).");
    let options = options(&["--backend", "cpu"]);
    let mut original = Vec::new();
    let mut diagnostics = Vec::new();
    let report = run_bundle_with_diagnostics(
        fixture.bundle(),
        &options,
        &mut original,
        &mut diagnostics,
        &Control::default(),
    )
    .unwrap();
    let mut explicit = Vec::new();
    let direct = run_with_diagnostics(
        "d(1). d(2). {pick(X)} :- d(X).".into(),
        &options,
        &mut explicit,
        &mut Vec::new(),
        &Control::default(),
    )
    .unwrap();
    assert_eq!(original, explicit);
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
    assert_eq!(process.stdout, original);
    assert!(
        !String::from_utf8(process.stdout)
            .unwrap()
            .contains("Backend:")
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
        assert!(output.contains("Coverage: exhausted"));
        assert!(output.contains("Models: 2; candidates examined: 2;"));
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
        &Control::default(),
    )
    .unwrap_err();
    assert!(models.is_empty());
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
    assert!(display.contains("unsafe.lp: bytes "), "{display}");
    assert!(!display.contains("UNSATISFIABLE"));
}

#[test]
fn combined_expansion_budget_applies_across_original_files() {
    let fixture = Fixture::new();
    fixture.write("entry.lp", "#include \"data.lp\". p(1..2).");
    fixture.write("data.lp", "q(1..2).");
    let mut models = Vec::new();
    let error = run_bundle_with_diagnostics(
        fixture.bundle(),
        &options(&["--backend", "cpu", "--max-expanded-templates", "3"]),
        &mut models,
        &mut Vec::new(),
        &Control::default(),
    )
    .unwrap_err();
    assert!(matches!(error, RunError::BundleAdmission(_)));
    assert!(models.is_empty());
    assert!(error.to_string().contains("Templates"));
}

#[test]
fn process_file_byte_limit_applies_to_included_files() {
    let fixture = Fixture::new();
    fixture.write("entry.lp", "#include \"large.lp\".");
    fixture.write("large.lp", &format!("% {}\np.", "padding ".repeat(10)));
    let result = fixture.process(&["--max-source-bytes", "32"]);
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
    let error = String::from_utf8(result.stderr).unwrap();
    assert!(error.contains("FileBytes"));
    assert!(error.contains("large.lp"));
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
        fs::read_to_string(fixture.0.join("entry.lp")).unwrap(),
        &options(&["--backend", "cpu"]),
        &mut Vec::new(),
        &mut Vec::new(),
        &Control::default(),
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
        &Control::default(),
    )
    .unwrap_err();
    assert!(
        matches!(error, RunError::UnsupportedOracle { .. }),
        "{error}"
    );
}
