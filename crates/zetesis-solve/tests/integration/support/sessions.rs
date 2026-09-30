//! A CPU session configuration that enumerates every model, and a normal
//! program's admission under the default options and limits.

pub use zetesis_reference_support::normal;
use zetesis_solve::{Backend, SolveConfig};

/// A session configuration on the CPU that enumerates every model.
pub fn config() -> SolveConfig {
    SolveConfig {
        backend: Backend::Cpu,
        models: 0,
        ..Default::default()
    }
}
