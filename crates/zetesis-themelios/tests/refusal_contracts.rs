//! Located API failures retain source evidence, and observations preserve typed values.

use std::error::Error as _;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use themelios_base::source::{FromBytesRefusal, SourceId};
use zetesis_core::{Atom, AtomPattern, ConstructionError, Model, Predicate, Value};
use zetesis_cpu::Cancellation;
use zetesis_themelios::{
    AdmissionFailure, AdmissionOptions, BundleAdmissionError, BundleAdmissionOptions, BundleError,
    BundleLimits, ExpansionFailure, ExpansionLimits, FormulaFailure, FormulaLimits, InputLimit,
    SourceBundle, admit, admit_bundle_extended, admit_bundle_formula, admit_extended,
    admit_formula, observation,
};

const SOURCE: SourceId = SourceId::new(73);

fn options() -> AdmissionOptions {
    AdmissionOptions {
        source_id: SOURCE,
        ..Default::default()
    }
}

fn formula(
    source: &str,
    limits: &FormulaLimits,
) -> Result<zetesis_themelios::AdmittedFormula, FormulaFailure> {
    admit_formula(
        source.to_owned(),
        options(),
        ExpansionLimits::default(),
        *limits,
    )
}

fn located(error: &AdmissionFailure, source: &str) {
    let diagnostics = error.diagnostics();
    assert!(!diagnostics.is_empty());
    assert!(!error.to_string().is_empty());
    for diagnostic in diagnostics {
        let location = diagnostic.primary().location;
        assert_eq!(location.source, SOURCE);
        assert!(usize::try_from(location.span.end().get()).unwrap() <= source.len());
    }
}

#[test]
fn input_quota_diagnostics_preserve_observed_count_source_and_typed_boundary() {
    let source = "p(a) :- q(a), q(a).";
    for (options, expected, phrase) in [
        (
            AdmissionOptions {
                max_source_bytes: source.len() - 1,
                ..options()
            },
            InputLimit::SourceBytes,
            "source bytes",
        ),
        (
            AdmissionOptions {
                max_syntax_nodes: 0,
                ..options()
            },
            InputLimit::SyntaxNodes,
            "syntax nodes",
        ),
        (
            AdmissionOptions {
                max_syntax_depth: 1,
                ..options()
            },
            InputLimit::SyntaxDepth,
            "syntax depth",
        ),
        (
            AdmissionOptions {
                max_body_elements: 1,
                ..options()
            },
            InputLimit::BodyElements,
            "body elements",
        ),
    ] {
        let error = admit(source.to_owned(), options).unwrap_err();
        located(&error, source);
        let AdmissionFailure::Limit {
            resource,
            limit,
            observed,
            location,
        } = &error
        else {
            panic!("expected input ceiling: {error}")
        };
        assert_eq!(*resource, expected);
        assert_eq!(location.source, SOURCE);
        assert!(observed > limit);
        let text = error.to_string();
        assert!(text.contains(phrase));
        assert!(text.contains(&format!("limit {limit}")));
        assert!(text.contains(&format!("observed {observed}")));
        assert!(error.source().is_none());
    }
    let error = admit_formula(
        source.to_owned(),
        AdmissionOptions {
            max_source_bytes: 0,
            ..options()
        },
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap_err();
    let expansion = error
        .source()
        .unwrap()
        .downcast_ref::<ExpansionFailure>()
        .unwrap();
    let admission = expansion
        .source()
        .unwrap()
        .downcast_ref::<AdmissionFailure>()
        .unwrap();
    assert!(matches!(
        admission,
        AdmissionFailure::Limit {
            resource: InputLimit::SourceBytes,
            ..
        }
    ));
    located(admission, source);
    assert_eq!(error.diagnostics()[0].primary().location.source, SOURCE);
    assert!(admit(source.to_owned(), options()).is_ok());
}

#[test]
fn real_s0_refusals_explain_the_feature_without_discarding_the_source() {
    for (source, phrase) in [
        ("p | q.", "head"),
        ("1 {p}.", "bounded choice"),
        ("{p;q}.", "other than one element"),
        ("{p:q}.", "conditional choice"),
        ("p :- q:r.", "body element"),
        ("p :- 1=1=1.", "comparison chain"),
        ("p :- 1<2.", "comparison relation"),
        ("p(f(X)):-q(X).", "non-scalar"),
        ("p :- #true.", "Boolean"),
    ] {
        let error = admit(source.to_owned(), options()).unwrap_err();
        located(&error, source);
        assert!(error.to_string().contains(phrase), "{source}: {error}");
        assert!(error.source().is_none());
    }
    let source = "p(2147483648).";
    let error = admit(source.to_owned(), options()).unwrap_err();
    assert!(matches!(error, AdmissionFailure::Raise(_)));
    assert!(error.to_string().contains("source raising"));
    located(&error, source);
    let source = "p(X).";
    let error = admit(source.to_owned(), options()).unwrap_err();
    assert!(
        error
            .source()
            .unwrap()
            .downcast_ref::<zetesis_core::AdmissionError>()
            .is_some()
    );
    located(&error, source);
}

#[test]
fn profile_diagnostics_do_not_invent_a_source_stage() {
    let source = "a.p:-#min{:a}=#sup.";
    let error = formula(source, &FormulaLimits::default()).unwrap_err();
    let text = error.to_string();
    assert!(text.contains("source profile does not admit"), "{text}");
    assert!(!text.contains("S0"), "{text}");
    assert_eq!(error.diagnostics()[0].primary().location.source, SOURCE);
}

#[test]
fn extremum_diagnostics_explain_the_retained_guard() {
    for (endpoint, value) in [("(-2147483647-1)", i32::MIN), ("2147483647", i32::MAX)] {
        let source = format!("n(N):-N=#min{{{endpoint}:#true}}.");
        let error = formula(&source, &FormulaLimits::default()).unwrap_err();
        let FormulaFailure::Expansion(ExpansionFailure::Admission(admission)) = &error else {
            panic!("expected a located endpoint refusal: {error}")
        };
        assert!(matches!(admission, AdmissionFailure::ExtremumEndpoint {
            value: actual, ..
        } if *actual == value));
        located(admission, &source);
        assert_eq!(
            admission.to_string(),
            format!("numeric extremum endpoint {value} is excluded by the zetesis endpoint guard")
        );
    }
}

#[test]
fn core_shape_and_program_limits_remain_actionable_through_source_admission() {
    let empty = Predicate::new("", 0).unwrap_err();
    assert_eq!(empty, ConstructionError::EmptyPredicateName);
    assert!(empty.to_string().contains("empty"));
    let arity = AtomPattern::new(Predicate::new("p", 2).unwrap(), vec![]).unwrap_err();
    assert_eq!(
        arity,
        ConstructionError::ArityMismatch {
            expected: 2,
            actual: 0
        }
    );
    assert!(arity.to_string().contains('2') && arity.to_string().contains('0'));
    let mut limits = options();
    limits.core_limits.max_templates = 1;
    let source = "p. q.";
    let error = admit(source.to_owned(), limits).unwrap_err();
    let cause = error
        .source()
        .unwrap()
        .downcast_ref::<zetesis_core::AdmissionError>()
        .unwrap();
    assert!(cause.to_string().contains("Templates"), "{cause}");
    assert_eq!(error.to_string(), cause.to_string());
    located(&error, source);
    assert_eq!(
        admit(source.to_owned(), options())
            .unwrap()
            .program()
            .templates()
            .len(),
        2
    );
}

#[test]
fn constant_refusals_retain_duplicate_or_policy_locations_and_evaluation_causes() {
    for source in [
        "#const n=1. #const n=2. p(n).",
        "#const n=2. [default] p(n).",
    ] {
        let error =
            admit_extended(source.to_owned(), options(), ExpansionLimits::default()).unwrap_err();
        assert!(matches!(
            error,
            ExpansionFailure::DuplicateConstant { .. } | ExpansionFailure::ConstantPolicy { .. }
        ));
        assert!(!error.to_string().is_empty());
        assert!(error.source().is_none());
        let diagnostics = error.diagnostics();
        assert!(!diagnostics.is_empty());
        assert!(
            diagnostics
                .iter()
                .all(|diagnostic| diagnostic.primary().location.source == SOURCE)
        );
    }
    let source = "#const n=1/0. p(n).";
    let error =
        admit_extended(source.to_owned(), options(), ExpansionLimits::default()).unwrap_err();
    assert!(matches!(error, ExpansionFailure::Evaluation { .. }));
    assert!(error.source().is_some());
    assert!(!error.to_string().is_empty());
    assert!(
        error
            .diagnostics()
            .iter()
            .all(|diagnostic| diagnostic.primary().location.source == SOURCE)
    );
    assert!(
        admit_extended(
            "#const n=1. p(n).".to_owned(),
            options(),
            ExpansionLimits::default()
        )
        .is_ok()
    );
}

#[test]
fn formula_resource_failures_keep_the_actual_nested_admission_cause() {
    let mut objective = FormulaLimits::default();
    objective.objective.max_filters = 0;
    let error = formula("p(1). #minimize{X:p(X),X!=0}.", &objective).unwrap_err();
    assert!(
        matches!(error, FormulaFailure::Objective { .. }),
        "{error:?}"
    );
    assert!(
        error
            .source()
            .unwrap()
            .downcast_ref::<zetesis_objective::AdmissionError>()
            .is_some()
    );
    assert!(error.to_string().contains("Filters"));
    assert_eq!(error.diagnostics()[0].primary().location.source, SOURCE);
    let mut aggregate = FormulaLimits::default();
    aggregate.aggregate.max_states = 0;
    let error = formula("{q}. p :- #count{1:q}=1.", &aggregate).unwrap_err();
    assert!(
        matches!(error, FormulaFailure::Aggregate { .. }),
        "{error:?}"
    );
    assert!(
        error
            .source()
            .unwrap()
            .downcast_ref::<zetesis_ferraris::AggregateError>()
            .is_some()
    );
    assert!(error.to_string().contains("state limit"));
    assert_eq!(error.diagnostics()[0].primary().location.source, SOURCE);
    let mut observation = FormulaLimits::default();
    observation.observation.max_nodes = 0;
    let error = formula("p. #show f(1).", &observation).unwrap_err();
    assert!(
        matches!(error, FormulaFailure::Observation { .. }),
        "{error:?}"
    );
    assert!(
        error
            .source()
            .unwrap()
            .downcast_ref::<observation::Error>()
            .is_some()
    );
    assert!(!error.to_string().is_empty());
    assert_eq!(error.diagnostics()[0].primary().location.source, SOURCE);
}

static TEMPORARY: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "zetesis-refusal-{}-{}",
            std::process::id(),
            TEMPORARY.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(fs::canonicalize(path).unwrap())
    }
    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
    fn write(&self, name: &str, bytes: impl AsRef<[u8]>) -> PathBuf {
        let path = self.path(name);
        fs::write(&path, bytes).unwrap();
        path
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn include(path: &Path) -> String {
    format!(
        "#include {}.\n",
        serde_json::to_string(&path.to_string_lossy()).unwrap()
    )
}

#[test]
fn file_failures_retain_real_operating_system_and_byte_decoding_causes() {
    let fixture = Fixture::new();
    let path = fixture.path("missing.lp");
    let error = SourceBundle::load(&path, BundleLimits::default()).unwrap_err();
    assert!(matches!(error, BundleError::Io { .. }));
    assert_eq!(
        error
            .source()
            .unwrap()
            .downcast_ref::<io::Error>()
            .unwrap()
            .kind(),
        io::ErrorKind::NotFound
    );
    assert!(error.to_string().contains("missing.lp"));
    let bad = fixture.write("invalid.lp", [0xff, 0xfe]);
    let root = fixture.write("root.lp", include(&bad));
    let error = SourceBundle::load(root, BundleLimits::default()).unwrap_err();
    assert!(matches!(
        error,
        BundleError::Source {
            including: Some(_),
            ..
        }
    ));
    assert!(
        error
            .source()
            .unwrap()
            .downcast_ref::<FromBytesRefusal>()
            .is_some()
    );
    assert!(error.to_string().contains("invalid.lp"));
    let error =
        SourceBundle::load_many(Vec::<PathBuf>::new(), BundleLimits::default()).unwrap_err();
    assert!(matches!(error, BundleError::EmptyRoots));
    assert!(error.to_string().contains("at least one input root"));
    assert!(error.source().is_none());
}

#[test]
fn cycle_and_library_refusals_keep_the_closing_source_and_name() {
    let fixture = Fixture::new();
    let first = fixture.write("first.lp", include(&fixture.path("second.lp")));
    fixture.write("second.lp", include(&first));
    let error = SourceBundle::load(first, BundleLimits::default()).unwrap_err();
    let BundleError::Cycle { chain, location } = &error else {
        panic!("{error}")
    };
    assert_eq!(chain.len(), 3);
    assert_eq!(chain.first(), chain.last());
    assert_eq!(location.source, SourceId::new(1));
    assert!(error.to_string().contains("3 path entries"));
    assert!(error.source().is_none());
    let path = fixture.write("library.lp", "#include <incmode>.");
    let error = SourceBundle::load(path, BundleLimits::default()).unwrap_err();
    assert!(matches!(error, BundleError::LibraryInclude { .. }));
    assert!(error.to_string().contains("<incmode>"));
    assert!(error.source().is_none());
}

#[test]
fn bundled_admission_keeps_the_catalog_after_failure_and_moves_program_identity_on_success() {
    let fixture = Fixture::new();
    let child = fixture.write("child.lp", "#const n=2. q(n).");
    let root = fixture.write("root.lp", format!("#const n=1.\n{}", include(&child)));
    let bundle = SourceBundle::load(&root, BundleLimits::default()).unwrap();
    assert_eq!(bundle.sources()[0].loaded_path(), root);
    let failure = admit_bundle_extended(
        bundle,
        BundleAdmissionOptions::default(),
        ExpansionLimits::default(),
    )
    .unwrap_err();
    let cause = failure
        .source()
        .unwrap()
        .downcast_ref::<BundleAdmissionError>()
        .unwrap();
    assert!(
        cause
            .source()
            .unwrap()
            .downcast_ref::<ExpansionFailure>()
            .is_some()
    );
    assert_eq!(failure.to_string(), cause.to_string());
    assert!(failure.to_string().contains('n'));
    for diagnostic in failure.diagnostics() {
        assert!(
            failure
                .bundle()
                .get(diagnostic.primary().location.source)
                .is_some()
        );
    }
    let bundle = failure.into_bundle();
    assert_eq!(bundle.sources().len(), 2);
    fs::write(&child, "q.").unwrap();
    fs::write(&root, format!("p.\n{}", include(&child))).unwrap();
    let admitted = admit_bundle_extended(
        SourceBundle::load(&root, BundleLimits::default()).unwrap(),
        BundleAdmissionOptions::default(),
        ExpansionLimits::default(),
    )
    .unwrap();
    let identity = admitted.program().clone();
    let program = admitted.into_program();
    assert!(program.same_instance(&identity));
    assert_eq!(program.templates().len(), 2);
}

#[test]
fn formula_bundle_alias_failure_keeps_a_downcastable_include_cause() {
    let fixture = Fixture::new();
    let child = fixture.write("child.lp", "q.");
    fs::create_dir(fixture.path("sub")).unwrap();
    let alias = fixture.path("sub/../child.lp");
    let root = fixture.write("root.lp", format!("{}{}", include(&child), include(&alias)));
    let bundle = SourceBundle::load(&root, BundleLimits::default()).unwrap();
    let failure = admit_bundle_formula(
        bundle,
        BundleAdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap_err();
    let cause = failure
        .source()
        .unwrap()
        .downcast_ref::<FormulaFailure>()
        .unwrap();
    assert!(matches!(
        cause,
        FormulaFailure::Include(error) if matches!(error.as_ref(), BundleAdmissionError::IncludeAlias { .. })
    ));
    assert!(
        cause
            .source()
            .unwrap()
            .downcast_ref::<BundleAdmissionError>()
            .is_some()
    );
    assert_eq!(failure.to_string(), cause.to_string());
    assert!(!failure.to_string().is_empty());
    for diagnostic in failure.diagnostics() {
        assert!(
            failure
                .bundle()
                .get(diagnostic.primary().location.source)
                .is_some()
        );
    }
}

#[cfg(unix)]
#[test]
fn a_redirected_include_has_both_lexical_and_canonical_diagnostic_evidence() {
    let fixture = Fixture::new();
    let actual = fixture.write("actual.lp", "q.");
    let alias = fixture.path("alias.lp");
    std::os::unix::fs::symlink(&actual, &alias).unwrap();
    let root = fixture.write("root.lp", include(&alias));
    let failure = admit_bundle_extended(
        SourceBundle::load(root, BundleLimits::default()).unwrap(),
        BundleAdmissionOptions::default(),
        ExpansionLimits::default(),
    )
    .unwrap_err();
    assert!(matches!(
        failure.error(),
        BundleAdmissionError::IncludeRedirection { .. }
    ));
    let text = failure.to_string();
    assert!(text.contains("alias.lp") && text.contains("actual.lp"));
    assert!(failure.error().source().is_none());
    assert_eq!(
        failure.diagnostics()[0].primary().location.source,
        SourceId::new(0)
    );
}

#[test]
fn observation_bindings_preserve_symbol_string_and_infinite_value_identity() {
    let input = formula("#show p/1. #show item(X): p(X).", &FormulaLimits::default()).unwrap();
    let observations = input.metadata().observations();
    // Observation evaluation intentionally accepts a caller-supplied model;
    // it neither admits these values as source terms nor decides stability.
    for (value, expected) in [
        (Value::Symbol("a".to_owned()), "p(a) item(a)"),
        (
            Value::String("a\\b\nc\"".to_owned()),
            "p(\"a\\\\b\\nc\\\"\") item(\"a\\\\b\\nc\\\"\")",
        ),
        (Value::Infimum, "p(#inf) item(#inf)"),
        (Value::Supremum, "p(#sup) item(#sup)"),
    ] {
        let atom = Atom::new(Predicate::new("p", 1).unwrap(), vec![value]).unwrap();
        let model = Model::new([atom]);
        let original = model.clone();
        let rendered = observations
            .render(
                &model,
                input.metadata().output(),
                observation::Limits {
                    max_symbol_nodes: 2,
                    ..Default::default()
                },
                &Cancellation::default(),
            )
            .unwrap();
        assert_eq!(rendered.text(), expected);
        let error = observations
            .render(
                &model,
                input.metadata().output(),
                observation::Limits {
                    max_symbol_nodes: 1,
                    ..Default::default()
                },
                &Cancellation::default(),
            )
            .unwrap_err();
        assert!(matches!(
            error.kind(),
            observation::ErrorKind::Limit {
                resource: observation::Resource::Nodes,
                ..
            }
        ));
        assert_eq!(error.location().unwrap().source, SOURCE);
        assert_eq!(model, original);
        assert_eq!(
            observations
                .render(
                    &model,
                    input.metadata().output(),
                    observation::Limits::default(),
                    &Cancellation::default()
                )
                .unwrap()
                .text(),
            expected
        );
    }
    let malformed = Model::new([Atom::new(
        Predicate::new("p", 1).unwrap(),
        vec![Value::Symbol(String::new())],
    )
    .unwrap()]);
    let error = observations
        .evaluate(
            &malformed,
            observation::Limits::default(),
            &Cancellation::default(),
        )
        .unwrap_err();
    assert_eq!(error.kind(), &observation::ErrorKind::InvalidSymbol);
    assert_eq!(error.location().unwrap().source, SOURCE);
}

#[test]
fn nonbinding_aggregate_guards_require_safe_inputs() {
    let source = "p(1). n(N) :- N<=#count{X:p(X)}.";
    let error = formula(source, &FormulaLimits::default()).unwrap_err();
    assert!(matches!(error, FormulaFailure::UnsafeVariable { .. }));
    assert!(
        error
            .diagnostics()
            .iter()
            .all(|diagnostic| diagnostic.primary().location.source == SOURCE)
    );
}
