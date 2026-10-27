//! Source ownership, canonical admission and cooperative control.

use super::*;

#[test]
fn refusal_recovery_transfers_original_owners() {
    struct Cause(Box<u8>);

    let source = ParsedSource::new("p.".into(), AdmissionOptions::default()).unwrap();
    let source_bytes = source.source().text().as_ptr();
    let error = Cause(Box::new(7));
    let error_payload = std::ptr::from_ref(error.0.as_ref());
    let failure = SourceFailure::new(source, error);
    assert_eq!(failure.source().source().text().as_ptr(), source_bytes);
    assert_eq!(
        std::ptr::from_ref(failure.error().0.as_ref()),
        error_payload
    );
    let (source, error) = failure.into_parts();
    assert_eq!(source.source().text().as_ptr(), source_bytes);
    assert_eq!(std::ptr::from_ref(error.0.as_ref()), error_payload);
    let error = SourceFailure::new(source, error).into_error();
    assert_eq!(std::ptr::from_ref(error.0.as_ref()), error_payload);
    assert_eq!(*error.0, 7);
}
#[test]
fn formula_control_retains_the_refused_source() {
    let source = ParsedSource::new("p(1..3).".into(), AdmissionOptions::default()).unwrap();
    let pointer = source.source().text().as_ptr();
    let cancellation = zetesis_cpu::Cancellation::default();
    cancellation.cancel();
    let failure = source
        .prepare_formula_with_cancellation(
            ExpansionLimits::default(),
            FormulaLimits::default(),
            &cancellation,
        )
        .unwrap_err();
    assert_eq!(
        failure.error().interruption(),
        Some(zetesis_cpu::Stop::Cancelled)
    );
    assert_eq!(failure.source().source().text().as_ptr(), pointer);
}

#[test]
fn formula_preparation_observes_its_supplied_control() {
    let source = ParsedSource::new("a | b.".into(), AdmissionOptions::default()).unwrap();
    let cancellation = zetesis_cpu::Cancellation::default();
    cancellation.cancel();
    let failure = source
        .prepare_formula_with_cancellation(
            ExpansionLimits::default(),
            FormulaLimits::default(),
            &cancellation,
        )
        .unwrap_err();
    assert_eq!(
        failure.error().interruption(),
        Some(zetesis_cpu::Stop::Cancelled)
    );
}

#[test]
fn formula_materialization_retains_preparation_control() {
    let source = ParsedSource::new("a | b.".into(), AdmissionOptions::default()).unwrap();
    let cancellation = zetesis_cpu::Cancellation::default();
    let prepared = source
        .prepare_formula_with_cancellation(
            ExpansionLimits::default(),
            FormulaLimits::default(),
            &cancellation,
        )
        .unwrap();
    cancellation.cancel();
    let failure = prepared.ground().unwrap_err();
    assert_eq!(failure.interruption(), Some(zetesis_cpu::Stop::Cancelled));
}

#[test]
fn bundle_formula_preparation_observes_control() {
    // The shared bundle fixture is built over this crate, so its unit tests write their own.
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("entry.lp");
    std::fs::write(&path, "p(1..3).").unwrap();
    let bundle = crate::SourceBundle::load(&path, crate::BundleLimits::default()).unwrap();
    let cancellation = zetesis_cpu::Cancellation::default();
    cancellation.cancel();
    let failure = crate::prepare_bundle_formula_with_cancellation(
        bundle,
        crate::BundleAdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
        &cancellation,
    )
    .unwrap_err();
    assert_eq!(
        failure.error().interruption(),
        Some(zetesis_cpu::Stop::Cancelled)
    );
    assert_eq!(failure.bundle().sources().len(), 1);
}

#[test]
fn bundle_formula_materialization_retains_preparation_control() {
    // The shared bundle fixture is built over this crate, so its unit tests write their own.
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("entry.lp");
    std::fs::write(&path, "a | b.").unwrap();
    let bundle = crate::SourceBundle::load(&path, crate::BundleLimits::default()).unwrap();
    let cancellation = zetesis_cpu::Cancellation::default();
    let prepared = crate::prepare_bundle_formula_with_cancellation(
        bundle,
        crate::BundleAdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
        &cancellation,
    )
    .unwrap();
    cancellation.cancel();
    let failure = prepared.ground().unwrap_err();
    assert_eq!(
        failure.error().interruption(),
        Some(zetesis_cpu::Stop::Cancelled)
    );
}

fn source_preparation_retains_input(text: &str, expansion: ExpansionLimits) {
    let source = ParsedSource::new(text.into(), AdmissionOptions::default()).unwrap();
    let bytes = source.source().text().as_ptr();
    let id = source.source().id();
    let span = source.source().span();
    let original = themelios_program::raise::raise(source.parsed());
    match source.prepare_formula(expansion, FormulaLimits::default()) {
        Ok(prepared) => {
            let retained = prepared
                .source()
                .expect("parsed preparation retains its source");
            assert_eq!(retained.text().as_ptr(), bytes);
            assert_eq!(retained.id(), id);
            assert_eq!(retained.span(), span);
            assert_eq!(retained.text(), text);
            assert!(original.diagnostics().is_empty());
            assert_eq!(prepared.original_program(), original.program());
            assert_eq!(
                format!("{:?}", prepared.original_program()),
                format!("{:?}", original.program()),
                "nested provenance: {text}"
            );
            assert!(
                prepared.preparation().options.domains.is_none(),
                "library domain analysis stays opt-in"
            );
        }
        Err(failure) => {
            let retained = failure.source().source();
            assert_eq!(retained.text().as_ptr(), bytes);
            assert_eq!(retained.id(), id);
            assert_eq!(retained.span(), span);
            assert_eq!(retained.text(), text);
            // Recovering a refused input preserves the parsed owner as well.
            let recovered = failure.into_source();
            assert!(recovered.parsed().diagnostics().is_empty());
            let raised = themelios_program::raise::raise(recovered.parsed());
            assert_eq!(raised.diagnostics(), original.diagnostics());
            assert_eq!(raised.program(), original.program());
            assert_eq!(
                format!("{:?}", raised.program()),
                format!("{:?}", original.program())
            );
        }
    }
}

#[test]
fn preparation_preserves_original_source_and_program_at_each_expansion_boundary() {
    for text in crate::test_support::PREPARATION_CASES {
        for max_term_work in [0, 1, 8, 100, ExpansionLimits::default().max_term_work] {
            source_preparation_retains_input(
                text,
                ExpansionLimits {
                    max_term_work,
                    ..ExpansionLimits::default()
                },
            );
        }
    }
}

fn no_metadata_storage() -> FormulaLimits {
    FormulaLimits {
        metadata_storage: crate::MetadataStorageLimits { max_bytes: 0 },
        ..FormulaLimits::default()
    }
}

#[test]
fn source_metadata_refusal_retains_its_typed_cause_and_input() {
    let text = "p(1). q(X) :- p(X), X+1=2. #show p/1.";
    let source = ParsedSource::new(text.into(), AdmissionOptions::default()).unwrap();
    let bytes = source.source().text().as_ptr();
    let id = source.source().id();
    let error = source
        .prepare_formula(ExpansionLimits::default(), no_metadata_storage())
        .unwrap_err();
    let FormulaFailure::Expansion(crate::ExpansionFailure::Admission(
        crate::AdmissionFailure::Metadata { location, .. },
    )) = error.error()
    else {
        panic!("formula metadata refusal: {error:?}")
    };
    assert_eq!(location.location().unwrap().source, id);
    assert_eq!(error.source().source().text().as_ptr(), bytes);
    assert_eq!(error.source().source().text(), text);
}

/// Debug includes the ordered roots, captured working directory, paths, source
/// identities, spans, text and include occurrences. Addresses separately prove
/// that preparation retains their original byte allocations.
fn bundle_snapshot(bundle: &crate::SourceBundle) -> (String, Vec<*const u8>) {
    (
        format!("{bundle:?}"),
        bundle
            .sources()
            .iter()
            .map(|source| source.source().text().as_ptr())
            .collect(),
    )
}

#[test]
fn second_of_three_sources_retains_its_metadata_refusal() {
    let directory = tempfile::tempdir().unwrap();
    let paths: Vec<_> = ["p(1). q(X) :- p(X), X+1=2.", "#show p/1.", "r(1)."]
        .iter()
        .enumerate()
        .map(|(index, text)| {
            let path = directory.path().join(format!("{index}.lp"));
            std::fs::write(&path, text).unwrap();
            path
        })
        .collect();
    let bundle = crate::SourceBundle::load_many(&paths, crate::BundleLimits::default()).unwrap();
    let original = bundle_snapshot(&bundle);
    let error = crate::prepare_bundle_formula(
        bundle,
        crate::BundleAdmissionOptions::default(),
        ExpansionLimits::default(),
        no_metadata_storage(),
    )
    .unwrap_err();
    let FormulaFailure::Expansion(crate::ExpansionFailure::Admission(
        crate::AdmissionFailure::Metadata { location, .. },
    )) = error.error()
    else {
        panic!("metadata failure: {error:?}")
    };
    assert_eq!(
        location.location().unwrap().source,
        error.bundle().sources()[1].id()
    );
    assert_eq!(bundle_snapshot(error.bundle()), original);
}

#[test]
fn program_preparation_keeps_original_identity_and_one_limit_set() {
    use std::sync::Arc;
    for text in ["p.", "p|q."] {
        let source = ParsedSource::new(text.into(), AdmissionOptions::default()).unwrap();
        let original = Arc::new(themelios_program::raise::raise(source.parsed()).into_program());
        let prepared = crate::prepare_program_formula_with(
            Arc::clone(&original),
            crate::ProgramFormulaOptions {
                purpose: crate::FormulaPurpose::AnswerSets,
                ..crate::ProgramFormulaOptions::default()
            },
        )
        .unwrap();
        assert!(std::ptr::eq(prepared.original_program(), original.as_ref()));
        assert!(prepared.source().is_none());
        assert!(prepared.preparation().options.domains.is_none());
    }
    let source = ParsedSource::new("#show p/0. p.".into(), AdmissionOptions::default()).unwrap();
    let original = Arc::new(themelios_program::raise::raise(source.parsed()).into_program());
    let error = crate::prepare_program_formula_with(
        Arc::clone(&original),
        crate::ProgramFormulaOptions {
            formula: no_metadata_storage(),
            ..crate::ProgramFormulaOptions::default()
        },
    )
    .unwrap_err();
    assert!(std::ptr::eq(
        error.original_program().unwrap(),
        original.as_ref()
    ));
    assert!(matches!(
        error.cause(),
        FormulaFailure::Expansion(crate::ExpansionFailure::Admission(
            crate::AdmissionFailure::Metadata { .. }
        ))
    ));
}

#[test]
fn all_corpus_preparations_retain_the_original_bundle() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../validation/corpus");
    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("manifest.json")).unwrap())
            .unwrap();
    let cases = manifest["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 94);
    for case in cases {
        let path = root.join("kr-domains").join(case["path"].as_str().unwrap());
        let bundle = crate::SourceBundle::load(&path, crate::BundleLimits::default()).unwrap();
        let original = bundle_snapshot(&bundle);
        let result = crate::prepare_bundle_formula(
            bundle,
            crate::BundleAdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        );
        let retained = match &result {
            Ok(prepared) => prepared.bundle(),
            Err(failure) => failure.bundle(),
        };
        assert_eq!(bundle_snapshot(retained), original, "{}", path.display());
    }
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config::with_cases(32))]
    #[test]
    fn generated_fact_and_comparison_preparations_retain_source_ownership(values in proptest::collection::vec(-20i32..20, 0..25)) {
        use std::fmt::Write;
        let mut text = String::new();
        for value in values {
            write!(&mut text, "p({value}).").unwrap();
        }
        text.push_str("q(X) :- p(X), X+1=2.");
        source_preparation_retains_input(&text, ExpansionLimits::default());
    }
}
