//! Admission of a source bundle written by the shared bundle fixture.

use zetesis_reference_support::BundleFixture;
use zetesis_themelios::{
    AdmittedFormulaBundle, BundleAdmissionOptions, ExpansionLimits, FormulaBundleFailure,
    FormulaLimits, admit_bundle_formula,
};

/// `fixture`'s bundle admitted under the default options and expansion
/// limits.
pub(crate) fn admit(
    fixture: &BundleFixture,
) -> Result<AdmittedFormulaBundle, FormulaBundleFailure> {
    admit_bundle_formula(
        fixture.bundle(),
        BundleAdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
}
