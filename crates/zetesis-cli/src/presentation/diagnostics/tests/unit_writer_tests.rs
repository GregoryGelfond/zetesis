//! Source diagnostics reach the diagnostics writer in their write units, and a
//! unit whose rendering fails writes nothing.

use std::{cell::Cell, fmt, io};

use crate::ColorMode;
use crate::presentation::{Diagnostics, source_error};
use zetesis_test_support::io::{BoundedWriter, Chunks};

/// A diagnostic rendered in several fragments, as a `Display` implementation
/// may write it.
struct Fragmented(&'static [&'static str]);

impl fmt::Display for Fragmented {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.iter().try_for_each(|part| output.write_str(part))
    }
}

const SOURCE: &str =
    "failed\nerror[syntax::expected]: token\n --> demo.lp:1:1\n1 | a\n  | ^\n  = help: fix\n";

#[test]
fn a_plain_diagnostic_arrives_in_its_rendering_fragments() {
    // The plain view passes its prefix and each fragment through: the chunks
    // through `Diagnostics` are the ones written directly into the sink.
    let diagnostic = Fragmented(&[
        "failed\nerr",
        "or[syntax::expected]: token\n",
        " --> demo.lp:1:1\n",
    ]);
    for mode in [ColorMode::Never, ColorMode::Auto] {
        let mut direct = Chunks::new();
        source_error::write(&mut direct, mode, &diagnostic).unwrap();
        assert!(direct.chunks().len() > 3, "{:?}", direct.chunks());
        let mut through = Chunks::new();
        Diagnostics::new(&mut through, mode)
            .diagnostic(&diagnostic)
            .unwrap();
        assert_eq!(through.chunks(), direct.chunks());
    }
}

#[test]
fn a_styled_diagnostic_arrives_as_its_heading_lines_and_newline() {
    let lines = SOURCE.lines().count();
    assert!(lines >= 2);
    let mut chunks = Chunks::new();
    Diagnostics::new(&mut chunks, ColorMode::Always)
        .diagnostic(&SOURCE)
        .unwrap();
    let chunks = chunks.chunks();
    assert_eq!(chunks.len(), lines + 2, "{chunks:?}");
    assert!(chunks[0].ends_with(b"zetesis:\x1b[0m "), "{chunks:?}");
    assert_eq!(chunks[lines + 1], b"\n");
}

#[test]
fn a_diagnostic_stops_rendering_after_its_writer_fails() {
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
    let mut writer = BoundedWriter::new(64);
    assert!(
        Diagnostics::new(&mut writer, ColorMode::Always)
            .diagnostic(&Many(&rendered))
            .is_err()
    );
    assert!(rendered.get() < 100);
}

#[test]
fn a_record_whose_rendering_fails_writes_nothing() {
    struct Failing;
    impl fmt::Display for Failing {
        fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
            Err(fmt::Error)
        }
    }
    let mut chunks = Chunks::new();
    let error = Diagnostics::new(&mut chunks, ColorMode::Never)
        .information(format_args!("Reduct search: {Failing}"))
        .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::Other);
    assert!(chunks.chunks().is_empty(), "{:?}", chunks.chunks());
}
