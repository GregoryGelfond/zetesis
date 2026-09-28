//! Standard-stream policy from observed capabilities, with no discovery here.
//!
//! A process adapter observes whether each stream is a terminal and reads the
//! environment; these pure functions turn those observations into styling and
//! width, so every zetesis tool resolves them the same way.

use std::ffi::OsStr;
use std::io::{self, Write};
use std::num::NonZeroUsize;

use crate::ColorMode;

/// Styling for standard output and for diagnostics, each resolved from its own
/// terminal capability; neither stream borrows the other's policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Streams {
    /// Styling of standard output.
    pub output: ColorMode,
    /// Styling of diagnostics.
    pub diagnostics: ColorMode,
}

impl Streams {
    /// Resolve `mode` for each stream from that stream's capability, with
    /// automatic styling disabled when `disabled` is set.
    #[must_use]
    pub const fn resolve(
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

/// Whether the environment disables automatic styling: a nonempty `NO_COLOR`,
/// or `TERM=dumb`. The adapter passes the variables' values as it read them.
#[must_use]
pub fn color_disabled(no_color: Option<&OsStr>, term: Option<&OsStr>) -> bool {
    no_color.is_some_and(|value| !value.is_empty()) || term.is_some_and(|value| value == "dumb")
}

/// Table width for an observed `COLUMNS` value: a positive width of at most
/// 512, otherwise 80. A terminal-provided width is presentation evidence only;
/// the cap keeps an environment from requesting arbitrarily wide padding.
#[must_use]
pub fn terminal_width(columns: Option<&str>) -> NonZeroUsize {
    const DEFAULT: NonZeroUsize = NonZeroUsize::new(80).expect("80 is nonzero");
    columns
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|width| *width <= 512)
        .and_then(NonZeroUsize::new)
        .unwrap_or(DEFAULT)
}

/// A writer that records whether any write was attempted.
///
/// A failed writer can already have modified its sink, so tracking calls
/// rather than accepted bytes lets a tool avoid appending a second machine
/// document to a partly written one.
pub struct TrackedWriter<'a, W> {
    output: &'a mut W,
    attempted: bool,
}

impl<'a, W> TrackedWriter<'a, W> {
    /// Track writes to `output`; none has been attempted yet.
    pub const fn new(output: &'a mut W) -> Self {
        Self {
            output,
            attempted: false,
        }
    }

    /// Whether any write was attempted, successful or not.
    #[must_use]
    pub const fn write_attempted(&self) -> bool {
        self.attempted
    }
}

impl<W: Write> Write for TrackedWriter<'_, W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.attempted = true;
        self.output.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.output.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::{ColorMode, Streams, color_disabled, terminal_width};
    use std::ffi::OsStr;

    #[test]
    fn automatic_stream_styles_follow_separate_capabilities() {
        for (output, diagnostics, expected_output, expected_diagnostics) in [
            (true, false, ColorMode::Always, ColorMode::Never),
            (false, true, ColorMode::Never, ColorMode::Always),
            (true, true, ColorMode::Always, ColorMode::Always),
            (false, false, ColorMode::Never, ColorMode::Never),
        ] {
            let actual = Streams::resolve(ColorMode::Auto, output, diagnostics, false);
            assert_eq!(actual.output, expected_output);
            assert_eq!(actual.diagnostics, expected_diagnostics);
        }
    }

    #[test]
    fn disabled_automatic_streams_remain_plain() {
        let actual = Streams::resolve(ColorMode::Auto, true, true, true);
        assert_eq!(actual.output, ColorMode::Never);
        assert_eq!(actual.diagnostics, ColorMode::Never);
    }

    #[test]
    fn explicit_stream_styles_override_capabilities() {
        for mode in [ColorMode::Always, ColorMode::Never] {
            let actual = Streams::resolve(mode, false, true, true);
            assert_eq!(actual.output, mode);
            assert_eq!(actual.diagnostics, mode);
        }
    }

    #[test]
    fn json_streams_remain_plain_under_always() {
        let actual = Streams::resolve(ColorMode::Always.human(true), true, true, false);
        assert_eq!(actual.output, ColorMode::Never);
        assert_eq!(actual.diagnostics, ColorMode::Never);
    }

    #[test]
    fn terminal_conventions_disable_only_automatic_color() {
        for (no_color, term, expected) in [
            (None, None, false),
            (Some(""), Some("xterm"), false),
            (Some("0"), Some("xterm"), true),
            (Some("1"), None, true),
            (None, Some("dumb"), true),
            (None, Some("xterm-256color"), false),
        ] {
            assert_eq!(
                color_disabled(no_color.map(OsStr::new), term.map(OsStr::new)),
                expected
            );
        }
    }

    #[test]
    fn terminal_width_accepts_positive_widths_up_to_512() {
        for (columns, expected) in [
            (None, 80),
            (Some("120"), 120),
            (Some("512"), 512),
            (Some("513"), 80),
            (Some("0"), 80),
            (Some("wide"), 80),
        ] {
            assert_eq!(terminal_width(columns).get(), expected, "{columns:?}");
        }
    }
}
