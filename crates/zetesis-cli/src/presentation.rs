//! Human record styling; terminal discovery belongs to the process adapter.

mod diagnostics;
mod source_error;
pub(crate) use diagnostics::{Diagnostics, Label};

pub use zetesis_presentation::ColorMode;

const RESET: &str = "\u{1b}[0m";
