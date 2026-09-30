//! Successful admission retains bounded warnings and original source evidence.

use std::fs;

use zetesis_test_support::fixtures::ZERO_DIVISOR as SOURCE;
use zetesis_themelios::base::{
    diagnostic::{Severity, ToDiagnostic},
    source::SourceId,
};
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, BundleAdmissionOptions, BundleLimits, ExpansionLimits,
    FormulaFailure, FormulaLimits, FormulaResource, FormulaWarning, SourceBundle,
    admit_bundle_formula, admit_formula,
};

fn admit(source: &str, limits: &FormulaLimits) -> Result<AdmittedFormula, FormulaFailure> {
    admit_formula(
        source.into(),
        AdmissionOptions {
            source_id: SourceId::new(7),
            ..AdmissionOptions::default()
        },
        ExpansionLimits::default(),
        *limits,
    )
}

#[test]
fn warnings_retain_their_original_locations() {
    let admitted = admit(SOURCE, &FormulaLimits::default()).unwrap();
    let [warning @ FormulaWarning::ZeroDivisor { location }] = admitted.warnings() else {
        panic!("one typed zero-divisor warning: {:?}", admitted.warnings());
    };
    assert_eq!(location.source, SourceId::new(7));
    assert_eq!(warning.location(), *location);
    assert!(
        admitted
            .source()
            .slice(location.span)
            .unwrap()
            .contains("1/X")
    );
    let diagnostic = warning.to_diagnostic();
    assert_eq!(diagnostic.primary().location, *location);
}

#[test]
fn omitted_instances_produce_warning_diagnostics() {
    let admitted = admit(SOURCE, &FormulaLimits::default()).unwrap();
    assert_eq!(
        admitted.warnings()[0].to_diagnostic().severity(),
        Severity::Warning
    );
    let rendered = admitted.warning_view().to_string();
    assert_eq!(
        rendered.matches("warning[zetesis::zero-divisor]").count(),
        1
    );
    assert!(rendered.contains("<input>:2:"), "{rendered}");
    assert!(rendered.contains("2 | p(X) :- d(X), 1/X=1."), "{rendered}");
    assert!(rendered.contains("= help: guard the denominator to exclude zero"));
    assert!(!rendered.contains('\u{1b}'));
}

#[test]
fn unreached_zero_divisors_retain_no_warnings() {
    for source in [
        "d(0..2). p(X) :- d(X), X!=0, 1/X=1.",
        "d(0..2). p(X) :- d(X), 1/X=1, X!=0.",
        "p(X) :- unknown(X), 1/X=1.",
    ] {
        let admitted = admit(
            source,
            &FormulaLimits {
                max_warnings: 0,
                ..FormulaLimits::default()
            },
        )
        .unwrap();
        assert!(admitted.warnings().is_empty(), "{source}");
        assert!(admitted.warning_view().to_string().is_empty(), "{source}");
    }
}

#[test]
fn repeated_evaluations_retain_one_warning_location() {
    let admitted = admit(
        "d(0..4). p(X,Y) :- d(X), d(Y), 1/X=1.",
        &FormulaLimits {
            max_warnings: 1,
            ..FormulaLimits::default()
        },
    )
    .unwrap();
    assert_eq!(admitted.warnings().len(), 1);
}

#[test]
fn distinct_warning_locations_follow_source_order() {
    let admitted = admit(
        "d(0..2). p(X) :- d(X), 1/X=1. q(X) :- d(X), 2\\X=0.",
        &FormulaLimits::default(),
    )
    .unwrap();
    let [first, second] = admitted.warnings() else {
        panic!("two warning locations: {:?}", admitted.warnings());
    };
    assert!(first.location().span.start() < second.location().span.start());
}

#[test]
fn warning_retention_exhaustion_refuses_admission() {
    for (source, limit) in [
        (SOURCE, 0),
        ("d(0..2). p(X) :- d(X), 1/X=1. q(X) :- d(X), 2\\X=0.", 1),
    ] {
        let failure = admit(
            source,
            &FormulaLimits {
                max_warnings: limit,
                ..FormulaLimits::default()
            },
        )
        .unwrap_err();
        assert!(
            matches!(failure, FormulaFailure::Limit {
                resource: FormulaResource::Warnings,
                limit: ceiling,
                observed,
                ..
            } if ceiling == limit as u128 && observed == limit as u128 + 1),
            "{failure}"
        );
    }
}

#[test]
fn bundle_warning_view_uses_retained_original_bytes() {
    let directory = tempfile::tempdir().unwrap();
    let entry = directory.path().join("entry.lp");
    let child = directory.path().join("child.lp");
    fs::write(&entry, "#include \"child.lp\".\n").unwrap();
    fs::write(&child, SOURCE).unwrap();
    let bundle = SourceBundle::load(&entry, BundleLimits::default()).unwrap();
    let source_id = bundle
        .sources()
        .iter()
        .find(|source| source.id() != bundle.entry())
        .unwrap()
        .id();
    fs::write(&child, "replacement.\n").unwrap();
    let admitted = admit_bundle_formula(
        bundle,
        BundleAdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    assert_eq!(admitted.warnings().len(), 1);
    assert_eq!(admitted.warnings()[0].location().source, source_id);
    let rendered = admitted.warning_view().to_string();
    assert!(rendered.contains("child.lp:2:"), "{rendered}");
    assert!(rendered.contains("2 | p(X) :- d(X), 1/X=1."), "{rendered}");
    assert!(!rendered.contains("replacement"));
}
