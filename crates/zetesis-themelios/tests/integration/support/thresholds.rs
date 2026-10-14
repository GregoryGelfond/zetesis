//! Exact preparation ceilings and owner-local full-admission work contracts.

use zetesis_reference_support::resource_limits;
use zetesis_test_support::thresholds::first_success;
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionFailure, ExpansionResource, FormulaFailure,
    FormulaLimits, prepare_formula,
};

/// The least `max_scalar_bytes` under which `source` prepares; one byte
/// less refuses it for its scalar bytes.
pub(crate) fn minimum_preparation_bytes(source: &str) -> u64 {
    let prepare = |cap| {
        prepare_formula(
            source.into(),
            AdmissionOptions::default(),
            resource_limits::expansion(ExpansionResource::ScalarBytes, cap),
            FormulaLimits::default(),
        )
    };
    first_success(
        resource_limits::expansion_default(ExpansionResource::ScalarBytes),
        |cap| match prepare(cap) {
            Ok(_) => true,
            Err(FormulaFailure::Expansion(ExpansionFailure::Limit {
                resource: ExpansionResource::ScalarBytes,
                ..
            })) => false,
            Err(other) => panic!("unexpected refusal: {other}"),
        },
    )
}

/// Check full admissions whose actual local-plan lookup work can depend on the
/// newly allocated frame owners. Each success supplies its own work receipt;
/// each refusal supplies its own inclusive ceiling and next required amount.
/// The generous run anchors completion and `check` retains the semantic oracle.
/// Exact local-table inclusivity is tested separately with fixed borrowed frames.
pub(crate) fn bounded_term_work(
    mut run: impl FnMut(u64) -> Result<AdmittedFormula, FormulaFailure>,
    mut check: impl FnMut(&AdmittedFormula),
) -> u64 {
    let ceiling = resource_limits::expansion_default(ExpansionResource::TermWork);
    let complete = run(ceiling).expect("the generous allowance completes admission");
    let work = u64::try_from(complete.expansion_usage().term_work).unwrap();
    assert!(work > 0 && work <= ceiling);
    check(&complete);
    let source = complete.source().expect("source fixture");
    let mut probes = vec![
        0,
        work / 2,
        work.saturating_sub(2),
        work - 1,
        work,
        work.saturating_add(1).min(ceiling),
    ];
    probes.sort_unstable();
    probes.dedup();
    for maximum in probes {
        match run(maximum) {
            Ok(actual) => {
                assert_ne!(maximum, 0, "the source must consume preparation work");
                assert!(u64::try_from(actual.expansion_usage().term_work).unwrap() <= maximum);
                check(&actual);
            }
            Err(error @ FormulaFailure::Expansion(ExpansionFailure::Limit { .. })) => {
                let FormulaFailure::Expansion(ExpansionFailure::Limit {
                    resource,
                    limit,
                    observed,
                    location,
                }) = &error
                else {
                    unreachable!("matched expansion limit")
                };
                assert_eq!(*resource, ExpansionResource::TermWork, "{error}");
                assert_eq!(*limit, u128::from(maximum), "{error}");
                assert!(*observed > *limit, "{error}");
                let location = location.location().expect("source refusal has a location");
                assert_eq!(location.source, source.id());
                assert!(!location.span.is_empty());
                assert!(source.slice(location.span).is_ok());
            }
            Err(error) => panic!("unexpected refusal at work={maximum}: {error}"),
        }
    }
    work
}
