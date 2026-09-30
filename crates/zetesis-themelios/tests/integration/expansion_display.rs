//! Diagnostic views preserve exact text and propagate the caller's refusal.

use std::fmt::{self, Write};

use themelios_base::{
    source::SourceId,
    span::{ByteOffset, Location, Span},
};
use zetesis_test_support::io::BoundedText;
use zetesis_themelios::ExpansionFailure;

fn cycle(names: &[&str]) -> ExpansionFailure {
    ExpansionFailure::ConstantCycle {
        names: names.iter().map(|name| (*name).into()).collect(),
        location: Location {
            source: SourceId::new(3),
            span: Span::empty(ByteOffset::new(0)),
        },
    }
}

#[test]
fn constant_cycle_rendering_preserves_order() {
    for (names, expected) in [
        (&[][..], "constant dependency cycle: "),
        (&["alpha"][..], "constant dependency cycle: alpha"),
        (
            &["alpha", "β雪", "alpha"][..],
            "constant dependency cycle: alpha -> β雪 -> alpha",
        ),
    ] {
        assert_eq!(cycle(names).to_string(), expected);
    }
}

#[test]
fn constant_cycle_rendering_respects_the_byte_ceiling() {
    let failure = cycle(&["alpha", "β雪", "alpha"]);
    let expected = "constant dependency cycle: alpha -> β雪 -> alpha";
    for maximum in [expected.len() - 1, expected.len()] {
        let mut sink = BoundedText::new(maximum);
        let result = write!(sink, "{failure}");
        assert_eq!(result.is_ok(), maximum == expected.len());
        assert!(sink.text().len() <= maximum);
        assert!(expected.starts_with(sink.text()));
        if result.is_ok() {
            assert_eq!(sink.text(), expected);
        }
    }
}

#[test]
fn constant_cycle_rendering_stops_at_a_refused_separator() {
    struct RefuseSeparator {
        writes: Vec<String>,
        refused: bool,
    }
    impl Write for RefuseSeparator {
        fn write_str(&mut self, text: &str) -> fmt::Result {
            assert!(!self.refused, "no writes follow a refused separator");
            self.writes.push(text.into());
            if text == " -> " {
                self.refused = true;
                Err(fmt::Error)
            } else {
                Ok(())
            }
        }
    }

    let mut sink = RefuseSeparator {
        writes: Vec::new(),
        refused: false,
    };
    let failure = cycle(&["first", "second", "first"]);
    assert!(write!(sink, "{failure}").is_err());
    assert_eq!(
        sink.writes,
        ["constant dependency cycle: ", "first", " -> "]
    );
}
