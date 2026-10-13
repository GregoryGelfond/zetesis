//! Helpers shared by the integration test modules.

use zetesis_core::{GroundProgram, Program, StaticLimits};

/// `program` compiled under the default static limits.
pub(crate) fn compile(program: &Program) -> GroundProgram {
    GroundProgram::compile(program, StaticLimits::default()).unwrap()
}
