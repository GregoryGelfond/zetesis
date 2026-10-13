//! Admission under the default options with given expansion and formula limits,
//! for the witness propositions.

use super::finite_bindings::native;
use zetesis_reference_support::formula;
use zetesis_test_support::thresholds::first_success;
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionFailure, ExpansionLimits, ExpansionResource,
    FormulaFailure, FormulaLimits, FormulaResource, admit_formula,
};

/// `source` admitted under the default options with `expansion` and `limits`.
pub(crate) fn limited(
    source: &str,
    expansion: ExpansionLimits,
    limits: &FormulaLimits,
) -> Result<AdmittedFormula, FormulaFailure> {
    admit_formula(
        source.into(),
        AdmissionOptions::default(),
        expansion,
        *limits,
    )
}

/// The least ceiling at which `run` admits `bounded`: admission is monotone in
/// each isolated resource ceiling, and every lower ceiling the search tries is
/// refused as `refused` accepts. The least ceiling admits the formula the
/// default limits admit.
pub(crate) fn inclusive_ceiling(
    bounded: &str,
    mut run: impl FnMut(usize) -> Result<AdmittedFormula, FormulaFailure>,
    refused: impl Fn(&FormulaFailure) -> bool,
) -> usize {
    let least = first_success(1_000_000, |cap| match run(cap) {
        Ok(_) => true,
        Err(error) => {
            assert!(refused(&error), "{error}");
            false
        }
    });
    assert_eq!(native(&run(least).unwrap()), native(&formula(bounded)));
    least
}

/// The least term-work ceiling that admits `bounded`, refused below it for
/// its term work.
pub(crate) fn term_work_ceiling(bounded: &str) -> usize {
    inclusive_ceiling(
        bounded,
        |cap| {
            limited(
                bounded,
                ExpansionLimits {
                    max_term_work: cap,
                    ..Default::default()
                },
                &FormulaLimits::default(),
            )
        },
        |error| {
            matches!(
                error,
                FormulaFailure::Expansion(ExpansionFailure::Limit {
                    resource: ExpansionResource::TermWork,
                    ..
                })
            )
        },
    )
}

/// The least scalar-storage ceiling that admits `bounded`, refused below it
/// for its scalar bytes.
pub(crate) fn scalar_bytes_ceiling(bounded: &str) -> usize {
    inclusive_ceiling(
        bounded,
        |cap| {
            limited(
                bounded,
                ExpansionLimits {
                    max_scalar_bytes: cap,
                    ..Default::default()
                },
                &FormulaLimits::default(),
            )
        },
        |error| {
            matches!(
                error,
                FormulaFailure::Expansion(ExpansionFailure::Limit {
                    resource: ExpansionResource::ScalarBytes,
                    ..
                })
            )
        },
    )
}

/// The least substitution ceiling that admits `bounded`, refused below it by
/// exactly one substitution.
pub(crate) fn substitution_ceiling(bounded: &str) -> usize {
    inclusive_ceiling(
        bounded,
        |cap| {
            limited(
                bounded,
                ExpansionLimits::default(),
                &FormulaLimits {
                    max_substitutions: cap as u64,
                    ..Default::default()
                },
            )
        },
        |error| matches!(error, FormulaFailure::Limit { resource: FormulaResource::Substitutions, observed, limit, .. } if *observed == limit + 1),
    )
}
