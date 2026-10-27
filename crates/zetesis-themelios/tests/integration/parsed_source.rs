//! Original source ownership across explicit formula preparation attempts.

use crate::support::admission;
use std::time::Instant;
use themelios_base::source::SourceId;
use zetesis_cpu::{Cancellation, Stop};
use zetesis_reference_support::resource_limits;
use zetesis_themelios::logical::term::{EvalError, Variable};
use zetesis_themelios::{
    AdmissionFailure, AdmissionOptions, ExpansionFailure, ExpansionLimits, FormulaFailure,
    FormulaLimits, InputLimit, ParsedSource, PreparedFormula, SourceFailure, prepare_formula,
};

fn input(text: &str) -> ParsedSource {
    ParsedSource::new(text.into(), admission::options(SourceId::new(19))).unwrap()
}

#[test]
fn construction_takes_the_original_byte_allocation() {
    let text = String::from("p(\"λ\").\r\n");
    let address = text.as_ptr();
    let parsed = ParsedSource::new(text, AdmissionOptions::default()).unwrap();
    assert_eq!(parsed.source().text().as_ptr(), address);
}

#[test]
fn formula_admission_transfers_original_bytes() {
    let parsed = input("p(\"λ\").\r\n");
    let address = parsed.source().text().as_ptr();
    let prepared = parsed
        .prepare_formula(ExpansionLimits::default(), FormulaLimits::default())
        .unwrap();
    assert_eq!(prepared.source().unwrap().text().as_ptr(), address);
    assert_eq!(prepared.source().unwrap().id(), SourceId::new(19));
    let admitted = prepared.ground().unwrap();
    assert_eq!(admitted.source().unwrap().text().as_ptr(), address);
    assert_eq!(admitted.source().unwrap().id(), SourceId::new(19));
}

#[test]
fn interrupted_preparation_preserves_the_original_parse_and_reason() {
    let cancelled = Cancellation::default();
    cancelled.cancel();
    let expired = Cancellation::with_deadline(Instant::now()).unwrap();
    for (control, reason) in [(cancelled, Stop::Cancelled), (expired, Stop::Deadline)] {
        let parsed = input("1{a;b}1.");
        let address = parsed.source().text().as_ptr();
        let original = parsed.parsed().syntax().green().to_owned();
        let failure = parsed
            .prepare_formula_with_cancellation(
                ExpansionLimits::default(),
                FormulaLimits::default(),
                &control,
            )
            .unwrap_err();
        assert_eq!(failure.error().interruption(), Some(reason));
        let (recovered, error) = failure.into_parts();
        assert_eq!(error.interruption(), Some(reason));
        assert_eq!(recovered.source().text().as_ptr(), address);
        assert_eq!(recovered.source().id(), SourceId::new(19));
        assert!(std::ptr::eq(
            std::ptr::from_ref(&*original),
            std::ptr::from_ref(recovered.parsed().syntax().green()),
        ));
    }
}

#[test]
fn formula_retry_transfers_original_bytes() {
    let parsed = input("1{a;b}1.");
    let address = parsed.source().text().as_ptr();
    let cancelled = Cancellation::default();
    cancelled.cancel();
    let failure = parsed
        .prepare_formula_with_cancellation(
            ExpansionLimits::default(),
            FormulaLimits::default(),
            &cancelled,
        )
        .unwrap_err();
    assert_eq!(failure.error().interruption(), Some(Stop::Cancelled));
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
    assert_eq!(admitted.source().unwrap().id(), SourceId::new(19));
    assert_eq!(
        crate::support::atom_models::answer_sets(&admitted),
        [
            std::collections::BTreeSet::from([zetesis_test_support::programs::nullary("a")]),
            std::collections::BTreeSet::from([zetesis_test_support::programs::nullary("b")])
        ]
        .into(),
    );
}

#[test]
fn preparation_failure_exposes_its_original_error() {
    let parsed = input("p(X).");
    let address = parsed.source().text().as_ptr();
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
    // A variable in a fact refuses during fact expansion, before formula safety.
    let FormulaFailure::Expansion(expansion) = failure.error() else {
        panic!("expected fact expansion refusal: {failure}");
    };
    let ExpansionFailure::Evaluation { error, location } = expansion else {
        panic!("expected checked scalar refusal: {expansion}");
    };
    assert!(
        matches!(error, EvalError::NotGround { variable: Variable::Named(name) }
        if name.as_str() == "X")
    );
    assert_eq!(location.statement_id().unwrap().index(), 0);
    let location = location.location().unwrap();
    assert_eq!(location.source, SourceId::new(19));
    assert_eq!(
        failure.source().source().slice(location.span).unwrap(),
        "p(X)."
    );
    let expansion_cause = std::error::Error::source(failure.error())
        .unwrap()
        .downcast_ref::<ExpansionFailure>()
        .unwrap();
    assert!(std::ptr::eq(expansion_cause, expansion));
    let evaluation_cause = std::error::Error::source(expansion)
        .unwrap()
        .downcast_ref::<EvalError>()
        .unwrap();
    assert!(std::ptr::eq(evaluation_cause, error));
    assert_eq!(failure.error().diagnostics(), expected.diagnostics());
    assert_eq!(failure.to_string(), expected.to_string());
    let cause = std::error::Error::source(&failure)
        .unwrap()
        .downcast_ref::<FormulaFailure>()
        .unwrap();
    assert!(std::ptr::eq(cause, failure.error()));
    assert_eq!(failure.source().source().id(), SourceId::new(19));
    assert_eq!(failure.source().source().text().as_ptr(), address);
    assert!(std::ptr::eq(
        std::ptr::from_ref(&*original),
        std::ptr::from_ref(failure.source().parsed().syntax().green()),
    ));
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
        std::ptr::from_ref(recovered.parsed().syntax().green()),
    ));
}

#[test]
fn source_limits_precede_parsing() {
    let error = ParsedSource::new(
        "{ malformed".into(),
        resource_limits::admission(InputLimit::SourceBytes, 0),
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
    let options = admission::options(SourceId::new(19));
    let error = ParsedSource::new(text.into(), options).unwrap_err();
    let AdmissionFailure::Syntax(syntax) = error else {
        panic!("expected syntax refusal");
    };
    assert_eq!(syntax.diagnostics().len(), 3);
    assert_eq!(syntax.source().text(), text);
    assert_eq!(syntax.source().id(), options.source_id);
}

#[test]
fn formula_attempts_keep_the_original_syntax_limit() {
    let source = ParsedSource::new(
        "p.".into(),
        resource_limits::admission(InputLimit::SyntaxNodes, 0),
    )
    .unwrap();
    let failed = source
        .prepare_formula(ExpansionLimits::default(), FormulaLimits::default())
        .unwrap_err();
    assert!(matches!(
        failed.error(),
        FormulaFailure::Expansion(ExpansionFailure::Admission(AdmissionFailure::Limit {
            resource: InputLimit::SyntaxNodes,
            ..
        }))
    ));
    let original_diagnostics = failed.error().diagnostics();
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
    assert_eq!(failed.error().diagnostics(), original_diagnostics);
}

#[test]
fn arithmetic_failure_remains_a_typed_evaluation_refusal() {
    let failure = input("p(1/0).")
        .prepare_formula(ExpansionLimits::default(), FormulaLimits::default())
        .map_err(SourceFailure::into_error)
        .and_then(PreparedFormula::ground)
        .unwrap_err();
    assert!(matches!(
        failure,
        FormulaFailure::Expansion(ExpansionFailure::Evaluation { .. })
    ));
    assert_eq!(failure.interruption(), None);
    let cause = std::error::Error::source(&failure)
        .unwrap()
        .downcast_ref::<ExpansionFailure>()
        .unwrap();
    assert!(std::error::Error::source(cause).is_some());
    assert!(
        failure
            .diagnostics()
            .iter()
            .all(|diagnostic| diagnostic.primary().location.source == SourceId::new(19))
    );
}

#[test]
fn source_recovery_does_not_reconstruct_bytes() {
    let parsed = input("p.");
    let address = parsed.source().text().as_ptr();
    let source = parsed.into_source();
    assert_eq!(source.text().as_ptr(), address);
    assert_eq!(source.id(), SourceId::new(19));
}
