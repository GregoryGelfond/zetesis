//! Located API failures retain source evidence, and observations preserve typed values.

use crate::support::{admission, atom_models::bundle_answer_sets};
use std::collections::BTreeSet;
use std::error::Error as _;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use zetesis_test_support::programs::nullary;

use themelios_base::source::{FromBytesRefusal, SourceId};
use zetesis_algebra::Cancellation;
use zetesis_core::{Atom, AtomPattern, ConstructionError, Model, Predicate, Value};
use zetesis_reference_support::resource_limits;
use zetesis_themelios::logical::term::{EvalError, Variable};
use zetesis_themelios::{
    AdmissionFailure, AdmissionOptions, BundleAdmissionError, BundleAdmissionOptions, BundleError,
    BundleLimits, ExpansionFailure, ExpansionLimits, ExpansionResource, FormulaFailure,
    FormulaLimits, InputLimit, SourceBundle, admit_bundle_formula, admit_formula, observation,
};

const SOURCE: SourceId = SourceId::new(73);

fn options() -> AdmissionOptions {
    admission::options(SOURCE)
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
    for (expected, bound, phrase) in [
        (InputLimit::SourceBytes, source.len() - 1, "source bytes"),
        (InputLimit::SyntaxNodes, 0, "syntax nodes"),
        (InputLimit::SyntaxDepth, 1, "syntax depth"),
        (InputLimit::BodyElements, 1, "body elements"),
    ] {
        let options = AdmissionOptions {
            source_id: SOURCE,
            ..resource_limits::admission(expected, u64::try_from(bound).unwrap())
        };
        let error = admit_formula(
            source.to_owned(),
            options,
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap_err();
        let FormulaFailure::Expansion(ExpansionFailure::Admission(error)) = error else {
            panic!("expected original input ceiling");
        };
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
        assert_eq!(location.location().unwrap().source, SOURCE);
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
            source_id: SOURCE,
            ..resource_limits::admission(InputLimit::SourceBytes, 0)
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
    assert!(formula(source, &FormulaLimits::default()).is_ok());
}

#[test]
fn source_raise_and_safety_refusals_keep_original_evidence() {
    let source = "p(2147483648).";
    let failure = formula(source, &FormulaLimits::default()).unwrap_err();
    let FormulaFailure::Expansion(ExpansionFailure::Admission(error)) = &failure else {
        panic!("expected original raising refusal: {failure}");
    };
    assert!(matches!(error, AdmissionFailure::Raise(_)));
    assert!(error.to_string().contains("source raising"));
    located(error, source);
    let source = "p(X).";
    let error = formula(source, &FormulaLimits::default()).unwrap_err();
    // Fact expansion reports the named variable before formula safety runs.
    let FormulaFailure::Expansion(expansion) = &error else {
        panic!("expected fact expansion refusal: {error}");
    };
    let ExpansionFailure::Evaluation {
        error: evaluation,
        location,
    } = expansion
    else {
        panic!("expected checked scalar refusal: {expansion}");
    };
    assert!(
        matches!(evaluation, EvalError::NotGround { variable: Variable::Named(name) }
        if name.as_str() == "X")
    );
    assert_eq!(location.statement_id().unwrap().index(), 0);
    let location = location.location().unwrap();
    assert_eq!(location.source, SOURCE);
    assert_eq!(location.span.start().get(), 0);
    assert_eq!(
        usize::try_from(location.span.end().get()).unwrap(),
        source.len()
    );
    let expansion_cause = error
        .source()
        .unwrap()
        .downcast_ref::<ExpansionFailure>()
        .unwrap();
    assert!(std::ptr::eq(expansion_cause, expansion));
    let evaluation_cause = expansion
        .source()
        .unwrap()
        .downcast_ref::<EvalError>()
        .unwrap();
    assert!(std::ptr::eq(evaluation_cause, evaluation));
    assert!(!error.to_string().is_empty());
    assert_eq!(error.interruption(), None);
    let diagnostics = error.diagnostics();
    assert!(!diagnostics.is_empty());
    for diagnostic in diagnostics {
        assert_eq!(diagnostic.primary().location.source, SOURCE);
        assert!(
            usize::try_from(diagnostic.primary().location.span.end().get()).unwrap()
                <= source.len()
        );
    }
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
            format!("numeric extremum endpoint {value} is excluded by the numeric endpoint guard")
        );
    }
}

#[test]
fn core_shape_and_template_limits_remain_actionable_through_formula_admission() {
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
    let limits = AdmissionOptions {
        source_id: SOURCE,
        core_limits: zetesis_core::AdmissionLimits {
            max_templates: 1,
            ..Default::default()
        },
        ..AdmissionOptions::default()
    };
    let source = "p. q.";
    let error = admit_formula(
        source.to_owned(),
        limits,
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap_err();
    let cause = error
        .source()
        .unwrap()
        .downcast_ref::<ExpansionFailure>()
        .unwrap();
    assert!(matches!(cause, ExpansionFailure::Limit {
        resource: ExpansionResource::Templates, limit: 1, observed: 2, location,
    } if location.location().unwrap().source == SOURCE));
    assert!(cause.to_string().contains("Templates"), "{cause}");
    assert_eq!(error.to_string(), cause.to_string());
    assert!(!error.diagnostics().is_empty());
    for diagnostic in error.diagnostics() {
        assert_eq!(diagnostic.primary().location.source, SOURCE);
        assert!(
            usize::try_from(diagnostic.primary().location.span.end().get()).unwrap()
                <= source.len()
        );
    }
    let admitted = formula(source, &FormulaLimits::default()).unwrap();
    assert_eq!(admitted.atoms().len(), 2);
    assert_eq!(
        crate::support::atom_models::answer_sets(&admitted),
        [BTreeSet::from([nullary("p"), nullary("q")])].into()
    );
}

#[test]
fn constant_refusals_retain_duplicate_or_policy_locations_and_evaluation_causes() {
    for source in [
        "#const n=1. #const n=2. p(n).",
        "#const n=2. [default] p(n).",
    ] {
        let failure = formula(source, &FormulaLimits::default()).unwrap_err();
        let FormulaFailure::Expansion(error) = failure else {
            panic!("expected original constant expansion refusal");
        };
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
    let failure = formula(source, &FormulaLimits::default()).unwrap_err();
    let FormulaFailure::Expansion(error) = failure else {
        panic!("expected original constant evaluation refusal");
    };
    assert!(matches!(error, ExpansionFailure::Evaluation { .. }));
    assert!(error.source().is_some());
    assert!(!error.to_string().is_empty());
    assert!(
        error
            .diagnostics()
            .iter()
            .all(|diagnostic| diagnostic.primary().location.source == SOURCE)
    );
    assert!(formula("#const n=1. p(n).", &FormulaLimits::default()).is_ok());
}

#[test]
fn formula_resource_failures_keep_the_actual_nested_admission_cause() {
    let objective =
        resource_limits::formula_objective(zetesis_objective::AdmissionResource::Filters, 0);
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
    // Two of three contributions is a genuine threshold: neither the empty,
    // singleton, conjunction nor disjunction boundary can erase its state cost.
    let source = "{q(1..3)}. p :- #count{X:q(X)}>=2.";
    let admitted = formula(source, &FormulaLimits::default()).unwrap();
    assert!(!admitted.theory().parts().thresholds().is_empty());
    let aggregate =
        resource_limits::formula_aggregate(zetesis_ferraris::AggregateErrorKind::StateLimit, 0);
    let error = formula(source, &aggregate).unwrap_err();
    assert!(
        matches!(error, FormulaFailure::Aggregate { .. }),
        "{error:?}"
    );
    assert_eq!(
        error
            .source()
            .unwrap()
            .downcast_ref::<zetesis_ferraris::AggregateError>()
            .unwrap()
            .kind(),
        zetesis_ferraris::AggregateErrorKind::StateLimit
    );
    assert!(error.to_string().contains("state limit"));
    assert_eq!(error.diagnostics()[0].primary().location.source, SOURCE);
    let observation =
        resource_limits::formula_observation(zetesis_themelios::observation::Resource::Nodes, 0);
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

// The refusals compare canonical paths and decode raw bytes, which the shared
// bundle fixture's temporary directory and text sources do not provide.
struct Fixture(tempfile::TempDir);
impl Fixture {
    /// A directory under the canonical temporary directory, so its paths are
    /// canonical too.
    fn new() -> Self {
        Self(tempfile::tempdir_in(fs::canonicalize(std::env::temp_dir()).unwrap()).unwrap())
    }
    fn path(&self, name: &str) -> PathBuf {
        self.0.path().join(name)
    }
    fn write(&self, name: &str, bytes: impl AsRef<[u8]>) -> PathBuf {
        let path = self.path(name);
        fs::write(&path, bytes).unwrap();
        path
    }
    /// Remove the directory, failing the test if it cannot be removed.
    fn close(self) {
        self.0.close().unwrap();
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
    fixture.close();
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
    fixture.close();
}

#[test]
fn bundled_formula_admission_retains_failed_sources_and_moved_owner_identity() {
    let fixture = Fixture::new();
    let child = fixture.write("child.lp", "#const n=2. q(n).");
    let root = fixture.write("root.lp", format!("#const n=1.\n{}", include(&child)));
    let bundle = SourceBundle::load(&root, BundleLimits::default()).unwrap();
    assert_eq!(bundle.sources()[0].loaded_path(), root);
    let original_bytes = bundle.sources()[0].source().text().as_ptr();
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
        FormulaFailure::Expansion(ExpansionFailure::DuplicateConstant { .. })
    ));
    assert!(
        cause
            .source()
            .unwrap()
            .downcast_ref::<ExpansionFailure>()
            .is_some()
    );
    assert_eq!(failure.to_string(), cause.to_string());
    assert!(failure.to_string().contains('n'));
    assert!(!failure.diagnostics().is_empty());
    for diagnostic in failure.diagnostics() {
        let location = diagnostic.primary().location;
        let source = failure.bundle().get(location.source).unwrap().source();
        assert!(source.slice(location.span).is_ok());
    }
    assert_eq!(failure.bundle().sources().len(), 2);
    assert_eq!(
        failure.bundle().sources()[0].source().text().as_ptr(),
        original_bytes
    );
    fs::write(&child, "q.").unwrap();
    fs::write(&root, format!("p.\n{}", include(&child))).unwrap();
    // Filesystem edits cannot rewrite the immutable failed source graph.
    assert_eq!(
        failure.bundle().sources()[1].source().text(),
        "#const n=2. q(n)."
    );
    let admitted = admit_bundle_formula(
        SourceBundle::load(&root, BundleLimits::default()).unwrap(),
        BundleAdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    let identity = std::ptr::from_ref(admitted.original_program());
    let retained = admitted;
    fixture.close();
    assert!(std::ptr::eq(retained.original_program(), identity));
    assert_eq!(retained.bundle().sources().len(), 2);
    assert_eq!(retained.atoms().len(), 2);
    assert_eq!(
        bundle_answer_sets(&retained),
        [BTreeSet::from([nullary("p"), nullary("q")])].into()
    );
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
    fixture.close();
}

#[cfg(unix)]
#[test]
fn a_redirected_include_has_both_lexical_and_canonical_diagnostic_evidence() {
    let fixture = Fixture::new();
    let actual = fixture.write("actual.lp", "q.");
    let alias = fixture.path("alias.lp");
    std::os::unix::fs::symlink(&actual, &alias).unwrap();
    let root = fixture.write("root.lp", include(&alias));
    let failure = admit_bundle_formula(
        SourceBundle::load(root, BundleLimits::default()).unwrap(),
        BundleAdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap_err();
    let FormulaFailure::Include(cause) = failure.error() else {
        panic!("expected original include identity refusal");
    };
    assert!(matches!(
        cause.as_ref(),
        BundleAdmissionError::IncludeRedirection { .. }
    ));
    let text = failure.to_string();
    assert!(text.contains("alias.lp") && text.contains("actual.lp"));
    assert!(cause.source().is_none());
    assert_eq!(
        failure.diagnostics()[0].primary().location.source,
        SourceId::new(0)
    );
    fixture.close();
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
        let model = Model::new([atom]).unwrap();
        let original = model.clone();
        let rendered = observations
            .render(
                &model,
                input.metadata().output(),
                resource_limits::observation_evaluation(observation::Resource::Nodes, 2),
                &Cancellation::default(),
            )
            .unwrap();
        assert_eq!(rendered.text(), expected);
        let error = observations
            .render(
                &model,
                input.metadata().output(),
                resource_limits::observation_evaluation(observation::Resource::Nodes, 1),
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
    .unwrap()])
    .unwrap();
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
