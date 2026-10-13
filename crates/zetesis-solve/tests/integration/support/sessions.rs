//! A CPU session configuration that enumerates every model, a normal
//! program's admission under the default options and limits, and a source's
//! hybrid grounding.

pub(crate) use zetesis_reference_support::normal;
use zetesis_solve::{Backend, SolveConfig};
use zetesis_themelios::{
    AdmissionOptions, ExpansionLimits, FormulaLimits, HybridFormula, prepare_formula,
};

/// A session configuration on the CPU that enumerates every model.
pub(crate) fn config() -> SolveConfig {
    SolveConfig {
        backend: Backend::Cpu,
        models: 0,
        ..Default::default()
    }
}

/// `source` prepared under the default options and limits and grounded for
/// hybrid execution, which it must allow.
pub(crate) fn hybrid(source: &str) -> HybridFormula {
    prepare_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap()
    .ground_hybrid()
    .unwrap()
}
