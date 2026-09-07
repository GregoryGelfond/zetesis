//! Preparation preserves source evidence and budgets before materialization.

use std::cell::RefCell;
use std::fs;

use zetesis_themelios::{
    AdmissionFailure, AdmissionOptions, BundleAdmissionOptions, BundleLimits, ExpansionFailure,
    ExpansionLimits, FormulaFailure, FormulaLimits, FormulaResource, GroundingObserver, InputLimit,
    SourceBundle, admit_bundle_formula, admit_formula, prepare_bundle_formula, prepare_formula,
};

#[derive(Default)]
struct Observer(RefCell<Vec<bool>>);
impl GroundingObserver for Observer {
    fn enter(&self) {
        self.0.borrow_mut().push(true);
    }
    fn exit(&self) {
        self.0.borrow_mut().push(false);
    }
}

#[test]
fn preparation_exposes_analysis_without_support_completion() {
    let source = "a :- a.";
    let input = prepare_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits {
            max_support_rounds: 0,
            ..FormulaLimits::default()
        },
    )
    .unwrap();
    assert_eq!(input.source().text(), source);
    assert_eq!(
        input.source_analysis(),
        &zetesis_themelios::analysis::Analysis::of(input.analyzed_program())
    );
    assert!(matches!(
        input.source_analysis().classes().tightness(),
        zetesis_themelios::analysis::Verdict::Unknown { .. }
    ));
    assert!(matches!(
        input.ground(),
        Err(FormulaFailure::Limit {
            resource: FormulaResource::SupportRounds,
            ..
        })
    ));
}

#[test]
fn send_analysis_precedes_grounding() {
    let source =
        include_str!("../../../validation/corpus/kr-domains/standalone/send-money/send-money.lp");
    let prepared = prepare_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits {
            max_support_rounds: 0,
            ..FormulaLimits::default()
        },
    )
    .unwrap();
    let analysis = prepared.source_analysis();
    assert!(analysis.classes().uses_choice());
    assert!(analysis.safety().is_safe());
    assert!(matches!(
        analysis.classes().tightness(),
        zetesis_themelios::analysis::Verdict::Holds
    ));
    // These facts do not waive the requested grounding ceiling or implement laziness.
    assert!(matches!(
        prepared.ground(),
        Err(FormulaFailure::Limit {
            resource: FormulaResource::SupportRounds,
            ..
        })
    ));
}

#[test]
fn materialization_checks_dynamic_arithmetic() {
    let prepared = prepare_formula(
        "p(0). q(1/X) :- p(X).".into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    assert!(matches!(
        prepared.ground(),
        Err(FormulaFailure::Expansion(
            ExpansionFailure::Evaluation { .. }
        ))
    ));
}

#[test]
fn grounding_resumes_the_expansion_budget() {
    // This fixture charges 7 selected payload bytes during preparation and 19
    // during grounding. Resetting the budget at ground() would wrongly admit 25.
    let source = "p(\"x\").";
    let exact_bytes = 26;
    let expansion = ExpansionLimits {
        max_scalar_bytes: exact_bytes - 1,
        ..ExpansionLimits::default()
    };
    let prepared = prepare_formula(
        source.into(),
        AdmissionOptions::default(),
        expansion,
        FormulaLimits::default(),
    )
    .unwrap();
    let error = prepared.ground().unwrap_err();
    let legacy = admit_formula(
        source.into(),
        AdmissionOptions::default(),
        expansion,
        FormulaLimits::default(),
    )
    .unwrap_err();
    assert_eq!(error.to_string(), legacy.to_string());
    let FormulaFailure::Expansion(ExpansionFailure::Limit {
        resource,
        limit,
        observed,
        location,
    }) = error
    else {
        panic!("expected cumulative expansion refusal");
    };
    assert_eq!(resource, zetesis_themelios::ExpansionResource::ScalarBytes);
    assert_eq!(limit, (exact_bytes - 1) as u128);
    assert_eq!(observed, exact_bytes as u128);
    assert_eq!(location.source, AdmissionOptions::default().source_id);
    prepare_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits {
            max_scalar_bytes: exact_bytes,
            ..expansion
        },
        FormulaLimits::default(),
    )
    .unwrap()
    .ground()
    .unwrap();
}

#[test]
fn preparation_enforces_the_analysis_budget() {
    let error = prepare_formula(
        "p.".into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits {
            max_analysis_nodes: 0,
            ..FormulaLimits::default()
        },
    )
    .unwrap_err();
    assert!(matches!(
        error,
        FormulaFailure::Limit {
            resource: FormulaResource::AnalysisNodes,
            ..
        }
    ));
}

#[test]
fn preparation_enforces_the_source_budget() {
    let error = prepare_formula(
        "p.".into(),
        AdmissionOptions {
            max_source_bytes: 1,
            ..AdmissionOptions::default()
        },
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        FormulaFailure::Expansion(ExpansionFailure::Admission(AdmissionFailure::Limit {
            resource: InputLimit::SourceBytes,
            limit: 1,
            observed: 2,
            ..
        }))
    ));
}

#[test]
fn preparation_retains_the_pinned_safety_verdict() {
    let input = prepare_formula(
        "n(N) :- N=#sum{}.".into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    // zetesis separately checks this aggregate binding extension.
    assert!(!input.source_analysis().safety().is_safe());
    input.ground().unwrap();
}

#[test]
fn explicit_preparation_preserves_the_admitted_theory() {
    for source in [
        "#const n=2. #show p/1. p(1..n).",
        "p :- not q. q :- not p.",
        "1 {p(X):X=1..3} 2.",
        "p(1..2). q(X+1) :- p(X).",
        "n(N) :- N=#sum{}.",
        "{p}. #minimize{1@2:p}.",
    ] {
        let expected = admit_formula(
            source.into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap();
        let prepared = prepare_formula(
            source.into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap();
        let analysis = prepared.source_analysis().clone();
        let metadata = prepared.metadata().clone();
        let actual = prepared.ground().unwrap();
        assert_eq!(actual.atoms(), expected.atoms(), "{source}");
        assert_eq!(
            actual.theory().nodes(),
            expected.theory().nodes(),
            "{source}"
        );
        assert_eq!(
            actual.theory().roots(),
            expected.theory().roots(),
            "{source}"
        );
        assert_eq!(
            actual.formula_origins(),
            expected.formula_origins(),
            "{source}"
        );
        assert_eq!(actual.source_analysis(), &analysis);
        assert_eq!(actual.metadata(), &metadata);
        assert_eq!(actual.objective_origins(), expected.objective_origins());
        assert_eq!(
            actual.objective_declarations(),
            expected.objective_declarations()
        );
    }
}

#[test]
fn grounding_observation_begins_at_materialization() {
    let observer = Observer::default();
    let input = prepare_formula(
        "p.".into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    assert!(observer.0.borrow().is_empty());
    input.ground_with_observer(Some(&observer)).unwrap();
    assert_eq!(*observer.0.borrow(), [true, false]);
}

#[test]
fn grounding_observation_closes_after_a_refusal() {
    let observer = Observer::default();
    let input = prepare_formula(
        "p.".into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits {
            max_support_rounds: 0,
            ..FormulaLimits::default()
        },
    )
    .unwrap();
    assert!(input.ground_with_observer(Some(&observer)).is_err());
    assert_eq!(*observer.0.borrow(), [true, false]);
}

fn bundle_fixture() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    fs::write(
        directory.path().join("entry.lp"),
        "#include \"child.lp\". #show q/1. q(X):-p(X).",
    )
    .unwrap();
    fs::write(directory.path().join("child.lp"), "p(1..2).").unwrap();
    directory
}

fn load(directory: &tempfile::TempDir) -> SourceBundle {
    SourceBundle::load(directory.path().join("entry.lp"), BundleLimits::default()).unwrap()
}

#[test]
fn bundle_preparation_preserves_all_original_sources() {
    let fixture = bundle_fixture();
    let prepared = prepare_bundle_formula(
        load(&fixture),
        BundleAdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    assert_eq!(prepared.bundle().sources().len(), 2);
    assert_eq!(
        prepared.source_analysis(),
        &zetesis_themelios::analysis::Analysis::of(prepared.analyzed_program())
    );
    let metadata = prepared.metadata().clone();
    let actual = prepared.ground().unwrap();
    let expected = admit_bundle_formula(
        load(&fixture),
        BundleAdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    assert_eq!(actual.atoms(), expected.atoms());
    assert_eq!(actual.theory().nodes(), expected.theory().nodes());
    assert_eq!(actual.theory().roots(), expected.theory().roots());
    assert_eq!(actual.formula_origins(), expected.formula_origins());
    assert_eq!(actual.metadata(), &metadata);
    for location in actual.formula_origins().iter().flatten() {
        assert!(
            actual
                .bundle()
                .get(location.source)
                .unwrap()
                .source()
                .slice(location.span)
                .is_ok()
        );
    }
}

#[test]
fn bundle_grounding_refusals_retain_source_evidence() {
    let fixture = bundle_fixture();
    let prepared = prepare_bundle_formula(
        load(&fixture),
        BundleAdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits {
            max_support_rounds: 0,
            ..FormulaLimits::default()
        },
    )
    .unwrap();
    let error = prepared.ground().unwrap_err();
    assert_eq!(error.bundle().sources().len(), 2);
    let FormulaFailure::Limit {
        location,
        resource: FormulaResource::SupportRounds,
        ..
    } = error.error()
    else {
        panic!("unexpected refusal: {error}");
    };
    assert!(
        error
            .bundle()
            .get(location.source)
            .unwrap()
            .source()
            .slice(location.span)
            .is_ok()
    );
}

#[test]
fn bundle_preparation_refusals_retain_source_evidence() {
    let fixture = bundle_fixture();
    let error = prepare_bundle_formula(
        load(&fixture),
        BundleAdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits {
            max_analysis_nodes: 0,
            ..FormulaLimits::default()
        },
    )
    .unwrap_err();
    assert_eq!(error.bundle().sources().len(), 2);
    assert!(matches!(
        error.error(),
        FormulaFailure::Limit {
            resource: FormulaResource::AnalysisNodes,
            ..
        }
    ));
}
