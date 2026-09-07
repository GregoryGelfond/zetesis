//! Independent standard-stream capabilities determine their own policies.

use super::{ColorMode, Streams};

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
