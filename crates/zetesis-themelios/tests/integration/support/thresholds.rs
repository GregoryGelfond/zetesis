//! The least scalar-byte ceiling under which a source prepares.

use zetesis_test_support::thresholds::first_success;
use zetesis_themelios::{
    AdmissionOptions, ExpansionFailure, ExpansionLimits, ExpansionResource, FormulaFailure,
    FormulaLimits, prepare_formula,
};

/// The least `max_scalar_bytes` under which `source` prepares; one byte
/// less refuses it for its scalar bytes.
pub(crate) fn minimum_preparation_bytes(source: &str) -> usize {
    let prepare = |cap| {
        prepare_formula(
            source.into(),
            AdmissionOptions::default(),
            ExpansionLimits {
                max_scalar_bytes: cap,
                ..ExpansionLimits::default()
            },
            FormulaLimits::default(),
        )
    };
    first_success(
        ExpansionLimits::default().max_scalar_bytes,
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
