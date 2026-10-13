//! Successful admission retains bounded warnings and original source evidence.

use crate::support::admission;
use zetesis_reference_support::BundleFixture;

use zetesis_test_support::fixtures::ZERO_DIVISOR as SOURCE;
use zetesis_themelios::base::{diagnostic::Severity, source::SourceId};
use zetesis_themelios::{
    AdmittedFormula, BundleAdmissionOptions, ExpansionLimits, FormulaFailure, FormulaLimits,
    FormulaResource, FormulaWarning, admit_bundle_formula,
};

const SOURCE_ID: SourceId = SourceId::new(7);

fn admit(source: &str, limits: &FormulaLimits) -> Result<AdmittedFormula, FormulaFailure> {
    admission::admit_with(SOURCE_ID, source, ExpansionLimits::default(), limits)
}

#[test]
fn warnings_retain_their_original_locations() {
    let admitted = admit(SOURCE, &FormulaLimits::default()).unwrap();
    let [warning @ FormulaWarning::ZeroDivisor { location }] = admitted.warnings() else {
        panic!("one typed zero-divisor warning: {:?}", admitted.warnings());
    };
    assert_eq!(
        location.location().expect("parsed source").source,
        SOURCE_ID
    );
    assert_eq!(warning.site(), *location);
    assert_eq!(
        warning.location().expect("parsed source"),
        location.location().expect("parsed source")
    );
    assert!(
        admitted
            .source()
            .expect("source input")
            .slice(location.location().expect("parsed source").span)
            .unwrap()
            .contains("1/X")
    );
    let diagnostic = warning.diagnostic().expect("parsed source diagnostic");
    assert_eq!(
        diagnostic.primary().location,
        location.location().expect("parsed source")
    );
}

#[test]
fn omitted_instances_produce_warning_diagnostics() {
    let admitted = admit(SOURCE, &FormulaLimits::default()).unwrap();
    assert_eq!(
        admitted.warnings()[0]
            .diagnostic()
            .expect("parsed source diagnostic")
            .severity(),
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
    assert!(
        first.location().expect("parsed source").span.start()
            < second.location().expect("parsed source").span.start()
    );
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
    let fixture = BundleFixture::new();
    fixture.write("entry.lp", "#include \"child.lp\".\n");
    fixture.write("child.lp", SOURCE);
    let bundle = fixture.bundle();
    let source_id = bundle
        .sources()
        .iter()
        .find(|source| source.id() != bundle.entry())
        .unwrap()
        .id();
    fixture.write("child.lp", "replacement.\n");
    let admitted = admit_bundle_formula(
        bundle,
        BundleAdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    assert_eq!(admitted.warnings().len(), 1);
    assert_eq!(
        admitted.warnings()[0]
            .location()
            .expect("parsed source")
            .source,
        source_id
    );
    let rendered = admitted.warning_view().to_string();
    assert!(rendered.contains("child.lp:2:"), "{rendered}");
    assert!(rendered.contains("2 | p(X) :- d(X), 1/X=1."), "{rendered}");
    assert!(!rendered.contains("replacement"));
}
