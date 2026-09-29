//! Preparation preserves source evidence and budgets before materialization.

use std::fs;

use crate::support::grounding_observers::Observer;
use zetesis_themelios::{
    AdmissionFailure, AdmissionOptions, BundleAdmissionOptions, BundleLimits, ExpansionFailure,
    ExpansionLimits, ExpansionResource, FormulaBundleFailure, FormulaFailure, FormulaLimits,
    FormulaResource, InputLimit, SourceBundle, admit_bundle_formula, admit_formula,
    prepare_bundle_formula, prepare_formula,
};

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
    let source = include_str!(
        "../../../../validation/corpus/kr-domains/standalone/send-money/send-money.lp"
    );
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
    // Preparation charges source metadata; structural matching then reserves
    // capture-delta cells. Canonical term lookups do not copy scalar payloads.
    let source = "p(f(\"x\")). q(X) :- p(f(X)).";
    let baseline = admit_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    let exact_bytes = baseline.expansion_usage().scalar_bytes;
    assert!(matches!(
        prepare_formula(
            source.into(),
            AdmissionOptions::default(),
            ExpansionLimits {
                max_scalar_bytes: 0,
                ..ExpansionLimits::default()
            },
            FormulaLimits::default(),
        ),
        Err(FormulaFailure::Expansion(ExpansionFailure::Limit {
            resource: ExpansionResource::ScalarBytes,
            limit: 0,
            observed: 1..,
            ..
        }))
    ));
    // Preparation succeeds below the complete charge. Both entry points must
    // agree at this inclusive boundary; the private handoff regression anchors
    // an accepted preparation charge independently of this shared baseline.
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
    assert_eq!(resource, ExpansionResource::ScalarBytes);
    assert_eq!(limit, (exact_bytes - 1) as u128);
    assert_eq!(observed, exact_bytes as u128);
    assert_eq!(location.source, AdmissionOptions::default().source_id);
    let exact = prepare_formula(
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
    assert_eq!(exact.expansion_usage(), baseline.expansion_usage());
    assert_eq!(exact.atoms(), baseline.atoms());
    assert_eq!(exact.theory().nodes(), baseline.theory().nodes());
    assert_eq!(exact.theory().roots(), baseline.theory().roots());
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
fn explicit_base_preserves_prepared_bundle_semantics() {
    let fixture = bundle_fixture();
    fs::write(
        fixture.path().join("entry.lp"),
        "#program base. #include \"child.lp\". #show q/1. q(X):-p(X). #program base.",
    )
    .unwrap();
    fs::write(fixture.path().join("child.lp"), "#program base. p(1..2).").unwrap();
    let prepared = prepare_bundle_formula(
        load(&fixture),
        BundleAdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    assert_eq!(prepared.bundle().sources().len(), 2);
    let actual = prepared.ground().unwrap();
    let expected = admit_formula(
        "p(1..2). #show q/1. q(X):-p(X).".into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    assert_eq!(actual.atoms(), expected.atoms());
    assert_eq!(actual.theory().nodes(), expected.theory().nodes());
    assert_eq!(actual.theory().roots(), expected.theory().roots());
    assert_eq!(actual.metadata().output(), expected.metadata().output());
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

fn preparation_fixture(entry: &str, child: &str) -> tempfile::TempDir {
    let fixture = tempfile::tempdir().unwrap();
    fs::write(fixture.path().join("entry.lp"), entry).unwrap();
    fs::write(fixture.path().join("child.lp"), child).unwrap();
    fixture
}

fn retained_preparation_failure(error: &FormulaBundleFailure, entry: &str, child: &str) {
    assert_eq!(error.bundle().sources().len(), 2);
    assert_eq!(error.bundle().sources()[0].source().text(), entry);
    assert_eq!(error.bundle().sources()[1].source().text(), child);
    let diagnostics = error.diagnostics();
    assert!(!diagnostics.is_empty());
    for diagnostic in diagnostics {
        let location = diagnostic.primary().location;
        assert!(
            !error
                .bundle()
                .get(location.source)
                .unwrap()
                .source()
                .slice(location.span)
                .unwrap()
                .is_empty()
        );
    }
    // The bundle wrapper preserves the typed cause and its ordinary human view.
    assert_eq!(error.to_string(), error.error().to_string());
    assert_eq!(
        std::error::Error::source(error).unwrap().to_string(),
        error.error().to_string()
    );
}

#[test]
fn bundle_definitions_share_one_preparation_budget() {
    let entry = "#const first=1. #include \"child.lp\". p(first,second).";
    let child = "#const second=2.";
    let fixture = preparation_fixture(entry, child);
    let limited = |max_constants| {
        prepare_bundle_formula(
            load(&fixture),
            BundleAdmissionOptions::default(),
            ExpansionLimits {
                max_constants,
                ..ExpansionLimits::default()
            },
            FormulaLimits::default(),
        )
    };
    let error = limited(1).unwrap_err();
    assert!(matches!(
        error.error(),
        FormulaFailure::Expansion(ExpansionFailure::Limit {
            resource: ExpansionResource::Constants,
            limit: 1,
            observed: 2,
            ..
        })
    ));
    retained_preparation_failure(&error, entry, child);
    let admitted = limited(2).unwrap().ground().unwrap();
    assert_eq!(
        admitted.atoms(),
        admit_formula(
            "p(1,2).".into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap()
        .atoms()
    );
}

#[test]
fn bundle_metadata_counts_original_occurrences() {
    let entry = "#show p/0. #include \"child.lp\". p.";
    let child = "#show p/0.";
    let fixture = preparation_fixture(entry, child);
    let limited = |max_metadata_statements| {
        prepare_bundle_formula(
            load(&fixture),
            BundleAdmissionOptions::default(),
            ExpansionLimits {
                max_metadata_statements,
                ..ExpansionLimits::default()
            },
            FormulaLimits::default(),
        )
    };
    let error = limited(1).unwrap_err();
    assert!(matches!(
        error.error(),
        FormulaFailure::Expansion(ExpansionFailure::Limit {
            resource: ExpansionResource::MetadataStatements,
            limit: 1,
            observed: 2,
            ..
        })
    ));
    retained_preparation_failure(&error, entry, child);
    let prepared = limited(2).unwrap();
    assert_eq!(prepared.metadata().directives().len(), 2);
    assert_eq!(prepared.metadata().atom_selection().signatures().len(), 1);
}

#[test]
fn bundle_objectives_share_one_preparation_budget() {
    let entry = "#minimize{1@0,a}. #include \"child.lp\".";
    let child = ":~ #true. [2@0,b]";
    let fixture = preparation_fixture(entry, child);
    let limited = |max_templates| {
        prepare_bundle_formula(
            load(&fixture),
            BundleAdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits {
                objective: zetesis_objective::AdmissionLimits {
                    max_templates,
                    ..zetesis_objective::AdmissionLimits::default()
                },
                ..FormulaLimits::default()
            },
        )
    };
    let error = limited(1).unwrap_err();
    assert!(matches!(
        error.error(),
        FormulaFailure::Limit {
            resource: FormulaResource::ObjectiveElements,
            limit: 1,
            observed: 2,
            ..
        }
    ));
    retained_preparation_failure(&error, entry, child);
    assert_eq!(
        limited(2)
            .unwrap()
            .ground()
            .unwrap()
            .objectives()
            .templates()
            .len(),
        2
    );
}

#[test]
fn duplicate_definitions_precede_later_metadata_refusal() {
    let entry = "#const same=1. #include \"child.lp\".";
    let child = "#const same=1. #show p/0.";
    let fixture = preparation_fixture(entry, child);
    let error = prepare_bundle_formula(
        load(&fixture),
        BundleAdmissionOptions::default(),
        ExpansionLimits {
            max_metadata_statements: 0,
            ..ExpansionLimits::default()
        },
        FormulaLimits::default(),
    )
    .unwrap_err();
    let FormulaFailure::Expansion(ExpansionFailure::DuplicateConstant {
        first, duplicate, ..
    }) = error.error()
    else {
        panic!("{error}");
    };
    assert_ne!(first.source, duplicate.source);
    for location in [first, duplicate] {
        assert_eq!(
            error
                .bundle()
                .get(location.source)
                .unwrap()
                .source()
                .slice(location.span)
                .unwrap(),
            "#const same=1."
        );
    }
    retained_preparation_failure(&error, entry, child);
}

#[test]
fn bundle_raise_failure_retains_the_loaded_catalog() {
    let entry = "#include \"child.lp\". p.";
    let child = "q(2147483648).";
    let fixture = preparation_fixture(entry, child);
    let error = prepare_bundle_formula(
        load(&fixture),
        BundleAdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap_err();
    assert!(matches!(
        error.error(),
        FormulaFailure::Expansion(ExpansionFailure::Admission(AdmissionFailure::Raise(_)))
    ));
    retained_preparation_failure(&error, entry, child);
    assert_eq!(
        error.diagnostics()[0].primary().location.source,
        error.bundle().sources()[1].id()
    );
}

#[test]
fn a_formula_admission_reports_the_charges_its_preparation_made() {
    // Three expanded facts are charged three templates, and three values for
    // their argument plus three for their sizes; the rule is charged one
    // template. The receipt is the one the extended profile reports, from
    // the one budget.
    let admitted = admit_formula(
        "p(1..3). q :- p(X).".into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    let usage = admitted.expansion_usage();
    assert_eq!(usage.templates, 4);
    assert_eq!(usage.values, 6);
}
