//! Unit spellings are checked before they reach byte or second limits.

use clap::{Parser, error::ErrorKind};
use zetesis_cli::Options;

#[test]
fn memory_units_map_to_exact_byte_counts() {
    for (value, expected) in [
        ("0", 0),
        ("17B", 17),
        ("3KiB", 3 << 10),
        ("2MiB", 2 << 20),
        ("4GiB", 4 << 30),
        ("2TiB", 2 << 40),
        ("18446744073709551615", u64::MAX),
    ] {
        let options = Options::try_parse_from(["zetesis", "--memory-budget", value]).unwrap();
        assert_eq!(options.memory, expected);
    }
}

#[test]
fn memory_units_reject_lossy_or_unrepresentable_values() {
    for value in [
        "",
        "-1",
        "+1",
        "1.5GiB",
        "4GB",
        "2 GiB",
        "18446744073709551616",
        "16777216TiB",
    ] {
        let error = Options::try_parse_from(["zetesis", "--memory", value]).unwrap_err();
        assert!(matches!(
            error.kind(),
            ErrorKind::ValueValidation | ErrorKind::UnknownArgument
        ));
    }
}

#[test]
fn time_units_preserve_whole_seconds() {
    for (value, expected) in [
        ("0", 0),
        ("30", 30),
        ("30s", 30),
        ("2m", 120),
        ("3h", 10_800),
        ("18446744073709551615", u64::MAX),
    ] {
        let options = Options::try_parse_from(["zetesis", "--time-limit", value]).unwrap();
        assert_eq!(options.time_limit, Some(expected));
    }
}

#[test]
fn time_units_reject_lossy_or_unrepresentable_values() {
    for value in ["1.5s", "100ms", "1d", "+1", "18446744073709551615h"] {
        let error = Options::try_parse_from(["zetesis", "--time-limit", value]).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::ValueValidation);
    }
}
