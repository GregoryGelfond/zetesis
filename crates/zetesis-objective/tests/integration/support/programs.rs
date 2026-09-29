//! Objective programs under the default admission limits.

use zetesis_objective::{AdmissionLimits, ObjectiveProgram, ObjectiveTemplate};

/// The objective program of `rows` under the default admission limits.
pub fn program(rows: Vec<ObjectiveTemplate>) -> ObjectiveProgram {
    ObjectiveProgram::new(rows, AdmissionLimits::default()).unwrap()
}
