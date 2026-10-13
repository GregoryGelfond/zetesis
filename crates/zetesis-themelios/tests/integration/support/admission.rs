//! Formula admission under a test module's own source identity and the
//! default limits.

use themelios_base::source::SourceId;
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionLimits, FormulaFailure, FormulaLimits,
    admit_formula,
};

/// The default admission options under `source_id`.
pub(crate) fn options(source_id: SourceId) -> AdmissionOptions {
    AdmissionOptions {
        source_id,
        ..Default::default()
    }
}

/// `source` admitted under `source_id` and the default limits.
pub(crate) fn admit(source_id: SourceId, source: &str) -> Result<AdmittedFormula, FormulaFailure> {
    admit_with(
        source_id,
        source,
        ExpansionLimits::default(),
        &FormulaLimits::default(),
    )
}

/// `source` admitted under `source_id` with `expansion` and `limits`.
pub(crate) fn admit_with(
    source_id: SourceId,
    source: &str,
    expansion: ExpansionLimits,
    limits: &FormulaLimits,
) -> Result<AdmittedFormula, FormulaFailure> {
    admit_formula(source.into(), options(source_id), expansion, *limits)
}

/// `source` admitted under `source_id` and the default limits, which the test
/// requires: a refusal fails it, naming the source.
pub(crate) fn admitted(source_id: SourceId, source: &str) -> AdmittedFormula {
    admit(source_id, source).unwrap_or_else(|error| panic!("{source}: {error}"))
}
