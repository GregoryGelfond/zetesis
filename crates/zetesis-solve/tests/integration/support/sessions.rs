//! A CPU session configuration that enumerates every model, and a normal
//! program's admission under the default options and limits.

use zetesis_solve::{Backend, SolveConfig};
use zetesis_themelios::{AdmissionOptions, Admitted, ExpansionLimits, admit_extended};

/// A session configuration on the CPU that enumerates every model.
pub fn config() -> SolveConfig {
    SolveConfig {
        backend: Backend::Cpu,
        models: 0,
        ..Default::default()
    }
}

/// `source` admitted on the normal-program route, the bounded extension of
/// S0, under the default options and limits.
pub fn normal(source: &str) -> Admitted {
    admit_extended(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
    )
    .unwrap()
}
