//! Styling over canonical diagnostic text, preserving its source and wording.
//!
//! Only unnumbered diagnostic headings, location rows and label gutters receive
//! styling. Numbered source rows pass unchanged, even when their text resembles
//! a diagnostic. No semantic result is inferred from rendered error text.

use std::{
    fmt,
    io::{self, Write},
};

use super::{ColorMode, RESET};

const BOLD_RED: &str = "\u{1b}[1;31m";
const BOLD_YELLOW: &str = "\u{1b}[1;33m";
const BOLD_BLUE: &str = "\u{1b}[1;34m";
const ITALIC_GRAY: &str = "\u{1b}[3;90m";
const BLUE: &str = "\u{1b}[34m";

/// Style one rendered line at a time. Temporary storage follows the longest
/// line, independently of diagnostic count; the underlying human view may also
/// allocate one diagnostic's rendering. Sink failure stops further formatting.
pub(super) fn write(
    output: &mut impl Write,
    mode: ColorMode,
    error: &(impl fmt::Display + ?Sized),
) -> io::Result<()> {
    if mode != ColorMode::Always {
        output.write_all(b"zetesis: ")?;
        let mut view = Plain {
            output,
            failure: None,
        };
        if fmt::write(&mut view, format_args!("{error}")).is_err() {
            return Err(format_failure(view.failure));
        }
        return writeln!(view.output);
    }
    write!(output, "{BLUE}zetesis:{RESET} ")?;
    let mut view = Lines {
        output,
        line: String::new(),
        first: true,
        severity: BOLD_RED,
        failure: None,
    };
    if fmt::write(&mut view, format_args!("{error}")).is_err() {
        return Err(format_failure(view.failure));
    }
    if !view.line.is_empty() {
        view.emit("")?;
    }
    writeln!(view.output)
}

fn format_failure(failure: Option<io::Error>) -> io::Error {
    failure.unwrap_or_else(|| io::Error::other("diagnostic formatting failed"))
}

struct Plain<'a, W> {
    output: &'a mut W,
    failure: Option<io::Error>,
}

impl<W: Write> fmt::Write for Plain<'_, W> {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        self.output.write_all(text.as_bytes()).map_err(|error| {
            self.failure = Some(error);
            fmt::Error
        })
    }
}

struct Lines<'a, W> {
    output: &'a mut W,
    line: String,
    first: bool,
    severity: &'static str,
    failure: Option<io::Error>,
}

impl<W: Write> Lines<'_, W> {
    fn emit(&mut self, newline: &str) -> io::Result<()> {
        let style = if self.first {
            Some(BOLD_RED)
        } else if let Some(style) = header(&self.line) {
            self.severity = style;
            Some(style)
        } else {
            detail(&self.line, self.severity)
        };
        if let Some(style) = style {
            write!(self.output, "{style}{}{RESET}{newline}", self.line)?;
        } else {
            write!(self.output, "{}{newline}", self.line)?;
        }
        self.first = false;
        self.line.clear();
        Ok(())
    }
}

impl<W: Write> fmt::Write for Lines<'_, W> {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        for chunk in text.split_inclusive('\n') {
            if let Some(line) = chunk.strip_suffix('\n') {
                self.line.push_str(line);
                if let Err(error) = self.emit("\n") {
                    self.failure = Some(error);
                    return Err(fmt::Error);
                }
            } else {
                self.line.push_str(chunk);
            }
        }
        Ok(())
    }
}

fn header(line: &str) -> Option<&'static str> {
    if line.starts_with("error[") {
        Some(BOLD_RED)
    } else if line.starts_with("warning[") {
        Some(BOLD_YELLOW)
    } else if line.starts_with("note[") {
        Some(BOLD_BLUE)
    } else {
        None
    }
}

fn detail(line: &str, severity: &'static str) -> Option<&'static str> {
    let trimmed = line.trim_start();
    if trimmed.starts_with("-->")
        || trimmed.starts_with("= note:")
        || trimmed.starts_with("= help:")
    {
        return Some(ITALIC_GRAY);
    }
    let label = trimmed.strip_prefix('|')?.trim_start();
    match label.as_bytes().first() {
        Some(b'^') => Some(severity),
        Some(b'-') => Some(ITALIC_GRAY),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{ColorMode, write};
    use std::io;

    #[test]
    fn diagnostic_styles_leave_numbered_source_rows_plain() {
        let source = "failed\nerror[syntax::expected]: expected token\n --> demo.lp:1:2\n1 | error[not-a-diagnostic]: ^\n  |  ^ here\n  = help: add the token";
        let mut bytes = Vec::new();
        write(&mut bytes, ColorMode::Always, source).unwrap();
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.contains("\n1 | error[not-a-diagnostic]: ^\n"));
        assert!(text.contains("\u{1b}[1;31merror[syntax::expected]"));
        assert!(text.contains("\u{1b}[1;31m  |  ^ here\u{1b}[0m"));
        assert!(text.contains("\u{1b}[3;90m --> demo.lp:1:2\u{1b}[0m"));
    }

    #[test]
    fn secondary_diagnostics_keep_their_severity() {
        let mut bytes = Vec::new();
        write(
            &mut bytes,
            ColorMode::Always,
            "failed\nwarning[warning]: review\n  | ^ warning\nnote[note]: context\n  | ^ note\n  | - related\n  = note: more\n  |",
        )
        .unwrap();
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.contains("\u{1b}[1;33mwarning["));
        assert!(text.contains("\u{1b}[1;33m  | ^ warning\u{1b}[0m"));
        assert!(text.contains("\u{1b}[1;34mnote["));
        assert!(text.contains("\u{1b}[1;34m  | ^ note\u{1b}[0m"));
        assert!(text.contains("\u{1b}[3;90m  | - related\u{1b}[0m"));
    }

    #[test]
    fn plain_error_view_preserves_original_bytes() {
        for source in ["", "failed", "failed\n", "failed\r\n\n"] {
            for mode in [ColorMode::Auto, ColorMode::Never] {
                let mut bytes = Vec::new();
                write(&mut bytes, mode, source).unwrap();
                assert_eq!(bytes, format!("zetesis: {source}\n").as_bytes());
            }
        }
    }

    #[test]
    fn diagnostic_writer_propagates_broken_pipe() {
        struct Closed;
        impl io::Write for Closed {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::Error::new(io::ErrorKind::BrokenPipe, "closed"))
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        for mode in [ColorMode::Never, ColorMode::Always] {
            assert_eq!(
                write(&mut Closed, mode, "failed").unwrap_err().kind(),
                io::ErrorKind::BrokenPipe
            );
        }
    }

    #[test]
    fn every_diagnostic_prefix_preserves_writer_failure() {
        use crate::test_writer::BoundedWriter;

        let source = "failed\nerror[syntax::expected]: token\n --> demo.lp:1:1\n1 | a\n  | ^\n  = help: fix\n";
        for mode in [ColorMode::Never, ColorMode::Always] {
            let mut complete = Vec::new();
            write(&mut complete, mode, source).unwrap();
            for capacity in 0..complete.len() {
                let mut writer = BoundedWriter::new(capacity);
                assert_eq!(
                    write(&mut writer, mode, source).unwrap_err().kind(),
                    io::ErrorKind::BrokenPipe
                );
                assert_eq!(writer.bytes(), &complete[..capacity]);
            }
            let mut writer = BoundedWriter::new(complete.len());
            write(&mut writer, mode, source).unwrap();
            assert_eq!(writer.bytes(), complete);
        }
    }

    #[test]
    fn fragmented_formatting_preserves_diagnostic_styles() {
        struct Fragmented;
        impl std::fmt::Display for Fragmented {
            fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                for part in [
                    "failed\ner",
                    "ror[syntax::",
                    "expected]: token\n ",
                    "--> demo.lp:1:1\n",
                ] {
                    output.write_str(part)?;
                }
                Ok(())
            }
        }
        let mut fragmented = Vec::new();
        write(&mut fragmented, ColorMode::Always, &Fragmented).unwrap();
        let mut contiguous = Vec::new();
        write(
            &mut contiguous,
            ColorMode::Always,
            "failed\nerror[syntax::expected]: token\n --> demo.lp:1:1\n",
        )
        .unwrap();
        assert_eq!(fragmented, contiguous);
    }

    #[test]
    fn sink_failure_stops_rendering_remaining_diagnostics() {
        use crate::test_writer::BoundedWriter;
        use std::{cell::Cell, fmt};

        struct Many<'a>(&'a Cell<usize>);
        impl fmt::Display for Many<'_> {
            fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
                for _ in 0..100 {
                    self.0.set(self.0.get() + 1);
                    output.write_str("error[syntax::expected]: token\n")?;
                }
                Ok(())
            }
        }
        let rendered = Cell::new(0);
        let mut output = BoundedWriter::new(64);
        assert!(write(&mut output, ColorMode::Always, &Many(&rendered)).is_err());
        assert!(rendered.get() < 100);
    }

    #[test]
    fn formatter_failure_becomes_an_output_error() {
        struct Invalid;
        impl std::fmt::Display for Invalid {
            fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                Err(std::fmt::Error)
            }
        }
        for mode in [ColorMode::Auto, ColorMode::Always, ColorMode::Never] {
            assert_eq!(
                write(&mut Vec::new(), mode, &Invalid).unwrap_err().kind(),
                io::ErrorKind::Other
            );
        }
    }
}
