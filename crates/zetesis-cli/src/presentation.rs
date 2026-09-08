//! Human record styling; terminal discovery belongs to the process adapter.

use std::io::{self, Write};

mod diagnostics;
mod source_error;
pub(crate) use diagnostics::{Diagnostics, Label};

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

/// Color policy for human output. JSON always ignores this policy.
///
/// The process resolves `Auto` independently for each standard stream, a nonempty
/// `NO_COLOR`, and `TERM=dumb`. Library calls with injected writers keep `Auto`
/// plain because a generic writer carries no terminal capability. Use `Always`
/// to request ANSI styling explicitly from a library call.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, clap::ValueEnum)]
pub enum ColorMode {
    /// Style an eligible terminal; leave generic library writers plain.
    #[default]
    Auto,
    /// Emit ANSI styling in human headings, metadata and source diagnostics.
    Always,
    /// Emit plain text.
    Never,
}

impl ColorMode {
    pub(crate) const fn human(self, json: bool) -> Self {
        if json { Self::Never } else { self }
    }
    /// Resolve the process policy from explicit terminal capabilities.
    /// Environment inspection is the caller's responsibility. `disabled` means
    /// automatic color is disabled by terminal/environment conventions; an
    /// explicit `Always` request takes precedence.
    #[must_use]
    pub const fn resolve(self, terminal: bool, disabled: bool) -> Self {
        match self {
            Self::Auto if terminal && !disabled => Self::Always,
            Self::Auto => Self::Never,
            mode => mode,
        }
    }

    pub(crate) fn answer(self, output: &mut impl Write, number: usize) -> io::Result<()> {
        if self == Self::Always {
            writeln!(output, "{BOLD_CYAN}Answer:{CYAN} {number}{RESET}")
        } else {
            writeln!(output, "Answer: {number}")
        }
    }

    pub(crate) fn objective(self, output: &mut impl Write) -> io::Result<()> {
        if self == Self::Always {
            write!(output, "{ITALIC_GREEN}Optimization:")
        } else {
            write!(output, "Optimization:")
        }
    }

    pub(crate) fn objective_end(self, output: &mut impl Write) -> io::Result<()> {
        if self == Self::Always {
            write!(output, "{RESET}")?;
        }
        writeln!(output)
    }

    pub(crate) fn status(self, output: &mut impl Write, text: &str) -> io::Result<()> {
        if self == Self::Always {
            writeln!(output, "{BOLD_ITALIC_GRAY}{text}{RESET}")
        } else {
            writeln!(output, "{text}")
        }
    }
}

const BOLD_CYAN: &str = "\u{1b}[1;36m";
const CYAN: &str = "\u{1b}[22;36m";
const ITALIC_GREEN: &str = "\u{1b}[3;32m";
const BOLD_ITALIC_GRAY: &str = "\u{1b}[1;3;90m";
const RESET: &str = "\u{1b}[0m";
