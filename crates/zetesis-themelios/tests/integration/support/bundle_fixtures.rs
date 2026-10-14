//! Admission of a source bundle written by the shared bundle fixture.

use zetesis_reference_support::BundleFixture;
use zetesis_themelios::{
    AdmittedBundle, BundleAdmissionFailure, BundleAdmissionOptions, ExpansionLimits,
    admit_bundle_extended,
};

/// `fixture`'s bundle admitted under the default options and expansion
/// limits.
pub(crate) fn admit(fixture: &BundleFixture) -> Result<AdmittedBundle, BundleAdmissionFailure> {
    admit_bundle_extended(
        fixture.bundle(),
        BundleAdmissionOptions::default(),
        ExpansionLimits::default(),
    )
}
