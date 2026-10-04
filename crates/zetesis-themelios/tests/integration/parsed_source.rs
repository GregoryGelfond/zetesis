//! Original source ownership across explicit profile attempts.

use themelios_base::source::SourceId;
use zetesis_themelios::{
    AdmissionFailure, AdmissionOptions, ExpansionFailure, ExpansionLimits, FormulaFailure,
    FormulaLimits, InputLimit, ParsedSource, admit_extended, prepare_formula,
};

fn input(text: &str) -> ParsedSource {
    ParsedSource::new(
        text.into(),
        AdmissionOptions {
            source_id: SourceId::new(19),
            ..AdmissionOptions::default()
        },
    )
    .unwrap()
}

#[test]
fn construction_takes_the_original_byte_allocation() {
    let text = String::from("p(\"λ\").\r\n");
    let address = text.as_ptr();
    let parsed = ParsedSource::new(text, AdmissionOptions::default()).unwrap();
    assert_eq!(parsed.source().text().as_ptr(), address);
}

#[test]
fn relational_admission_transfers_original_bytes() {
    let parsed = input("p(\"λ\").\r\n");
    let address = parsed.source().text().as_ptr();
    let admitted = parsed.admit_extended(ExpansionLimits::default()).unwrap();
    assert_eq!(admitted.source().text().as_ptr(), address);
    assert_eq!(admitted.source().id(), SourceId::new(19));
}

#[test]
fn profile_recovery_preserves_the_original_parse() {
    let parsed = input("1{a;b}1.");
    let original = parsed.parsed().syntax().green().to_owned();
    let failure = parsed
        .admit_extended(ExpansionLimits::default())
        .unwrap_err();
    assert!(failure.error().needs_formula_admission());
    let (recovered, _) = failure.into_parts();
    assert!(std::ptr::eq(
        std::ptr::from_ref(&*original),
        std::ptr::from_ref(recovered.parsed().syntax().green()),
    ));
}

#[test]
fn formula_retry_transfers_original_bytes() {
    let parsed = input("1{a;b}1.");
    let address = parsed.source().text().as_ptr();
    let failure = parsed
        .admit_extended(ExpansionLimits::default())
        .unwrap_err();
    let prepared = failure
        .into_source()
        .prepare_formula(ExpansionLimits::default(), FormulaLimits::default())
        .unwrap();
    assert_eq!(
        prepared.source().expect("source input").text().as_ptr(),
        address
    );
    let admitted = prepared.ground().unwrap();
    assert_eq!(
        admitted.source().expect("source input").text().as_ptr(),
        address
    );
}

#[test]
fn profile_failure_exposes_its_original_error() {
    let parsed = input("1{a;b}1.");
    let expected = admit_extended(
        parsed.source().text().into(),
        parsed.options(),
        ExpansionLimits::default(),
    )
    .unwrap_err();
    let failure = parsed
        .admit_extended(ExpansionLimits::default())
        .unwrap_err();
    assert_eq!(failure.error().diagnostics(), expected.diagnostics());
    assert_eq!(failure.to_string(), expected.to_string());
    assert!(std::error::Error::source(&failure).is_some());
    assert_eq!(failure.source().source().id(), SourceId::new(19));
}

#[test]
fn formula_failure_preserves_the_original_parse() {
    let parsed = input("p(X).");
    let original = parsed.parsed().syntax().green().to_owned();
    let expected = prepare_formula(
        parsed.source().text().into(),
        parsed.options(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap_err();
    let failure = parsed
        .prepare_formula(ExpansionLimits::default(), FormulaLimits::default())
        .unwrap_err();
    assert_eq!(failure.error().diagnostics(), expected.diagnostics());
    let recovered = failure.into_source();
    assert!(std::ptr::eq(
        std::ptr::from_ref(&*original),
        std::ptr::from_ref(recovered.parsed().syntax().green())
    ));
}

#[test]
fn source_limits_precede_parsing() {
    let error = ParsedSource::new(
        "{ malformed".into(),
        AdmissionOptions {
            max_source_bytes: 0,
            ..AdmissionOptions::default()
        },
    )
    .unwrap_err();
    assert!(matches!(
        error,
        AdmissionFailure::Limit {
            resource: InputLimit::SourceBytes,
            ..
        }
    ));
}

#[test]
fn syntax_refusal_retains_all_original_diagnostics() {
    let text = "% original é\r\n{a,b :- q.\r\nq :- c.\r\n";
    let options = AdmissionOptions {
        source_id: SourceId::new(19),
        ..AdmissionOptions::default()
    };
    let error = ParsedSource::new(text.into(), options).unwrap_err();
    let AdmissionFailure::Syntax(syntax) = error else {
        panic!("expected syntax refusal")
    };
    assert_eq!(syntax.diagnostics().len(), 3);
    assert_eq!(syntax.source().text(), text);
    assert_eq!(syntax.source().id(), options.source_id);
}

#[test]
fn profile_attempts_keep_the_original_syntax_limit() {
    let source = ParsedSource::new(
        "p.".into(),
        AdmissionOptions {
            max_syntax_nodes: 0,
            ..AdmissionOptions::default()
        },
    )
    .unwrap();
    let failed = source
        .admit_extended(ExpansionLimits::default())
        .unwrap_err();
    assert!(matches!(
        failed.error(),
        ExpansionFailure::Admission(AdmissionFailure::Limit {
            resource: InputLimit::SyntaxNodes,
            ..
        })
    ));
    let failed = failed
        .into_source()
        .prepare_formula(ExpansionLimits::default(), FormulaLimits::default())
        .unwrap_err();
    assert!(matches!(
        failed.error(),
        FormulaFailure::Expansion(ExpansionFailure::Admission(AdmissionFailure::Limit {
            resource: InputLimit::SyntaxNodes,
            ..
        }))
    ));
}

#[test]
fn arithmetic_failure_is_not_a_profile_retry() {
    let failed = input("p(1/0).")
        .admit_extended(ExpansionLimits::default())
        .unwrap_err();
    assert!(!failed.error().needs_formula_admission());
}

#[test]
fn source_recovery_does_not_reconstruct_bytes() {
    let parsed = input("p.");
    let address = parsed.source().text().as_ptr();
    let source = parsed.into_source();
    assert_eq!(source.text().as_ptr(), address);
    assert_eq!(source.id(), SourceId::new(19));
}
