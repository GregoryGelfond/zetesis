//! Admission under the source identity the aggregate-assignment and
//! ground-guard propositions share.

use themelios_base::source::SourceId;
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionLimits, FormulaFailure, FormulaLimits,
    admit_formula,
};

/// The source identity both modules admit under.
pub const SOURCE: SourceId = SourceId::new(83);

/// The default admission options under [`SOURCE`].
pub fn options() -> AdmissionOptions {
    AdmissionOptions {
        source_id: SOURCE,
        ..Default::default()
    }
}

/// `source` admitted under [`options`] and the default limits.
pub fn input(source: &str) -> Result<AdmittedFormula, FormulaFailure> {
    admit_formula(
        source.into(),
        options(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
}
