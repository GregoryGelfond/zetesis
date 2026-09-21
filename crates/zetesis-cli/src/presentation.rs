//! Human record styling; terminal discovery belongs to the process adapter.

mod diagnostics;
mod source_error;
mod tracked_writer;
pub(crate) use diagnostics::{Diagnostics, Label};
pub(crate) use tracked_writer::TrackedWriter;

pub(crate) struct Streams {
    pub(crate) output: ColorMode,
    pub(crate) diagnostics: ColorMode,
}
impl Streams {
    // Resolution is pure. The process supplies each stream's actual capability;
    // neither stream borrows the other's policy or terminal observation.
    pub(crate) const fn resolve(
        mode: ColorMode,
        output_terminal: bool,
        diagnostics_terminal: bool,
        disabled: bool,
    ) -> Self {
        Self {
            output: mode.resolve(output_terminal, disabled),
            diagnostics: mode.resolve(diagnostics_terminal, disabled),
        }
    }
}

#[cfg(test)]
mod tests;

pub use zetesis_presentation::ColorMode;

const RESET: &str = "\u{1b}[0m";
