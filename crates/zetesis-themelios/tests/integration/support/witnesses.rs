//! Admission under the default options with given expansion and formula limits,
//! for the witness propositions.

use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionLimits, FormulaFailure, FormulaLimits,
    admit_formula,
};

/// `source` admitted under the default options with `expansion` and `limits`.
pub fn limited(
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
