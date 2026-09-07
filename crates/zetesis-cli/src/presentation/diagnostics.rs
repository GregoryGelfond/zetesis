//! Streaming typed metadata actions; arbitrary diagnostic bytes pass unchanged.

use super::{ColorMode, RESET};
use std::{
    fmt,
    io::{self, Write},
};

pub(crate) enum Label {
    Source,
    Oracle,
    Grounding,
    Backend,
    Auto,
}
impl Label {
    const fn text(self) -> &'static str {
        match self {
            Self::Source => "Source",
            Self::Oracle => "Oracle",
            Self::Grounding => "Grounding",
            Self::Backend => "Backend",
            Self::Auto => "Auto",
        }
    }
}

// A borrowed or owned writer plus one explicit policy, with no buffered text,
// environment lookup or terminal discovery. Metadata adds fixed label/style
// bytes and delegates value formatting and I/O costs to their implementations.
// Write calls and their failures remain observable in order.
pub(crate) struct Diagnostics<W> {
    writer: W,
    color: ColorMode,
}
impl<W: Write> Diagnostics<W> {
    pub(crate) const fn new(writer: W, color: ColorMode) -> Self {
        Self { writer, color }
    }

    pub(crate) fn metadata(&mut self, label: Label, value: fmt::Arguments<'_>) -> io::Result<()> {
        let label = label.text();
        if self.color == ColorMode::Always {
            writeln!(self.writer, "{BLUE}{label}:{ITALIC_GRAY} {value}{RESET}")
        } else {
            writeln!(self.writer, "{label}: {value}")
        }
    }
}

const BLUE: &str = "\u{1b}[34m";
const ITALIC_GRAY: &str = "\u{1b}[3;90m";
impl<W: Write> Write for Diagnostics<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.writer.write(bytes)
    }
    fn write_all(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.writer.write_all(bytes)
    }
    fn write_fmt(&mut self, args: fmt::Arguments<'_>) -> io::Result<()> {
        self.writer.write_fmt(args)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}
