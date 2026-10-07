//! The command lines the tests parse into options, and the CPU session
//! configuration they compare with.

use clap::Parser;
use zetesis_cli::{Options, SolveConfig};

/// `zetesis ARGUMENTS`.
pub fn plain(arguments: &[&str]) -> Options {
    Options::try_parse_from(["zetesis"].into_iter().chain(arguments.iter().copied())).unwrap()
}

/// `zetesis --models 0 ARGUMENTS`: every model.
pub fn enumerating(arguments: &[&str]) -> Options {
    Options::try_parse_from(
        ["zetesis", "--models", "0"]
            .into_iter()
            .chain(arguments.iter().copied()),
    )
    .unwrap()
}

/// `zetesis --backend cpu --workers 1 --models 0 ARGUMENTS`: every model, from
/// one CPU worker.
pub fn serial(arguments: &[&str]) -> Options {
    Options::try_parse_from(
        [
            "zetesis",
            "--backend",
            "cpu",
            "--workers",
            "1",
            "--models",
            "0",
        ]
        .into_iter()
        .chain(arguments.iter().copied()),
    )
    .unwrap()
}

/// [`serial`], with statistics when `stats` holds.
pub fn serial_with_statistics(arguments: &[&str], stats: bool) -> Options {
    let mut options = serial(arguments);
    options.stats = stats;
    options
}

/// The session configuration of [`serial`]: every model, from one CPU worker.
pub fn serial_config() -> SolveConfig {
    SolveConfig::from(&serial(&[]))
}
