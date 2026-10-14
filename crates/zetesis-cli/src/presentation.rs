//! Human record styling; terminal discovery belongs to the process adapter.

mod diagnostics;
mod source_error;
pub(crate) use diagnostics::{Diagnostics, Label};

pub use zetesis_presentation::ColorMode;

const RESET: &str = "\u{1b}[0m";

/// The error a diagnostic returns when its own rendering fails while its writer
/// did not: a `Display` implementation returned an error of its own.
fn formatting_failure() -> std::io::Error {
    std::io::Error::other("diagnostic formatting failed")
}
