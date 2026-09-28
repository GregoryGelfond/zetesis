//! Human presentation with explicit capabilities and no environment discovery.
//!
//! Logical results and measurements belong to their domain libraries. This
//! module renders caller-supplied text without interpreting solver semantics.
//! Tables retain all cell content and use a vertical layout when the columns do
//! not fit. Callers bound the number and size of rows before constructing a view.
#![forbid(unsafe_code)]

mod streams;
mod table;
pub use streams::{Streams, TrackedWriter, color_disabled, terminal_width};
pub use table::{Alignment, Column, Layout, Row, Table, TableError};

use std::fmt;
use std::io::{self, Write};

/// Styling policy. Process adapters resolve terminal and environment evidence.
/// Generic library writers leave automatic styling disabled.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "clap", derive(clap::ValueEnum))]
pub enum ColorMode {
    /// Style supported terminals; generic writers remain plain.
    #[default]
    Auto,
    /// Emit ANSI color and emphasis.
    Always,
    /// Emit plain text.
    Never,
}

/// Meaning of a styled fragment, independent of its content.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// A table heading or metadata label, in blue.
    Label,
    /// Auxiliary values and units, in italic gray.
    Metadata,
    /// A total or terminal result, in bold italic gray.
    Conclusion,
    /// An optimization label or value, in italic green.
    Objective,
    /// A recoverable diagnostic, in yellow.
    Warning,
    /// A failed operation, in red.
    Error,
}

impl ColorMode {
    /// Resolve explicit capabilities. An explicit override takes precedence over
    /// environment conventions. This function performs no I/O.
    #[must_use]
    pub const fn resolve(self, terminal: bool, disabled: bool) -> Self {
        match self {
            Self::Auto if terminal && !disabled => Self::Always,
            Self::Auto => Self::Never,
            mode => mode,
        }
    }

    /// Machine-readable output has no terminal styling.
    #[must_use]
    pub const fn human(self, json: bool) -> Self {
        if json { Self::Never } else { self }
    }

    /// Write a styled fragment. Content is caller-owned and is not interpreted.
    ///
    /// # Errors
    /// Returns the writer's error, including a partial write.
    pub fn styled(
        self,
        output: &mut impl Write,
        role: Role,
        value: fmt::Arguments<'_>,
    ) -> io::Result<()> {
        if self == Self::Always {
            write!(output, "{}{value}{RESET}", role.prefix())
        } else {
            output.write_fmt(value)
        }
    }

    /// Write a numbered answer heading.
    ///
    /// # Errors
    /// Returns the writer's error.
    pub fn answer(self, output: &mut impl Write, number: usize) -> io::Result<()> {
        if self == Self::Always {
            writeln!(output, "\u{1b}[1;36mAnswer:\u{1b}[22;36m {number}{RESET}")
        } else {
            writeln!(output, "Answer: {number}")
        }
    }

    /// Begin an optimization line; finish it with `Self::objective_end`.
    ///
    /// # Errors
    /// Returns the writer's error.
    pub fn objective(self, output: &mut impl Write) -> io::Result<()> {
        if self == Self::Always {
            write!(output, "{}Optimization:", Role::Objective.prefix())
        } else {
            write!(output, "Optimization:")
        }
    }

    /// End a previously begun optimization line.
    ///
    /// # Errors
    /// Returns the writer's error.
    pub fn objective_end(self, output: &mut impl Write) -> io::Result<()> {
        if self == Self::Always {
            write!(output, "{RESET}")?;
        }
        writeln!(output)
    }

    /// Write an untagged terminal result.
    ///
    /// # Errors
    /// Returns the writer's error.
    pub fn status(self, output: &mut impl Write, text: &str) -> io::Result<()> {
        self.styled(output, Role::Conclusion, format_args!("{text}"))?;
        writeln!(output)
    }

    /// Write a blue label followed by italic gray metadata.
    ///
    /// # Errors
    /// Returns the writer's error.
    pub fn metadata(
        self,
        output: &mut impl Write,
        label: &str,
        value: fmt::Arguments<'_>,
    ) -> io::Result<()> {
        if self == Self::Always {
            writeln!(
                output,
                "{}{label}:{} {value}{RESET}",
                Role::Label.prefix(),
                Role::Metadata.prefix()
            )
        } else {
            writeln!(output, "{label}: {value}")
        }
    }
}

impl Role {
    const fn prefix(self) -> &'static str {
        match self {
            Self::Label => "\u{1b}[34m",
            Self::Metadata => "\u{1b}[3;90m",
            Self::Conclusion => "\u{1b}[1;3;90m",
            Self::Objective => "\u{1b}[3;32m",
            Self::Warning => "\u{1b}[33m",
            Self::Error => "\u{1b}[31m",
        }
    }
}

const RESET: &str = "\u{1b}[0m";
