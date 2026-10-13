//! Admission under the default options with given expansion and formula limits,
//! for the witness propositions.

use super::finite_bindings::native;
use zetesis_reference_support::{formula, resource_limits};
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

/// The least ceiling at which `run` admits `bounded`, searched from
/// `default`, the resource's default limit: admission is monotone in each
/// isolated resource ceiling, and every lower ceiling the search tries is
/// refused as `refused` accepts. The least ceiling admits the formula the
/// default limits admit.
pub(crate) fn inclusive_ceiling(
    bounded: &str,
    default: u64,
    mut run: impl FnMut(u64) -> Result<AdmittedFormula, FormulaFailure>,
    refused: impl Fn(&FormulaFailure) -> bool,
) -> u64 {
    let least = first_success(default, |cap| match run(cap) {
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
pub(crate) fn term_work_ceiling(bounded: &str) -> u64 {
    inclusive_ceiling(
        bounded,
        resource_limits::expansion_default(ExpansionResource::TermWork),
        |cap| {
            limited(
                bounded,
                resource_limits::expansion(ExpansionResource::TermWork, cap),
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
pub(crate) fn scalar_bytes_ceiling(bounded: &str) -> u64 {
    inclusive_ceiling(
        bounded,
        resource_limits::expansion_default(ExpansionResource::ScalarBytes),
        |cap| {
            limited(
                bounded,
                resource_limits::expansion(ExpansionResource::ScalarBytes, cap),
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
pub(crate) fn substitution_ceiling(bounded: &str) -> u64 {
    inclusive_ceiling(
        bounded,
        resource_limits::formula_default(FormulaResource::Substitutions),
        |cap| {
            limited(
                bounded,
                ExpansionLimits::default(),
                &resource_limits::formula(FormulaResource::Substitutions, cap),
            )
        },
        |error| matches!(error, FormulaFailure::Limit { resource: FormulaResource::Substitutions, observed, limit, .. } if *observed == limit + 1),
    )
}
