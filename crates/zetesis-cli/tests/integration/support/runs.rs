//! Runs of the command's library entry points, with their output and
//! diagnostics as text.

use clap::Parser;
use zetesis_cli::{Options, Report, RunError, run_detailed_with_diagnostics, run_with_diagnostics};
use zetesis_cpu::Cancellation;

/// The detailed run of `source` under `options`, which must succeed, with its
/// output and diagnostics.
pub fn detailed(source: &str, options: &Options) -> (Report, String, String) {
    let mut output = Vec::new();
    let mut diagnostics = Vec::new();
    let report = run_detailed_with_diagnostics(
        source.into(),
        options,
        &mut output,
        &mut diagnostics,
        &Cancellation::default(),
    )
    .unwrap();
    (
        report,
        String::from_utf8(output).unwrap(),
        String::from_utf8(diagnostics).unwrap(),
    )
}

/// The run of `source` enumerating every model under `arguments`, with its
/// output and diagnostics.
pub fn enumerated(source: &str, arguments: &[&str]) -> (Result<Report, RunError>, String, String) {
    let options = Options::try_parse_from(
        ["zetesis", "--models", "0"]
            .into_iter()
            .chain(arguments.iter().copied()),
    )
    .unwrap();
    let mut output = Vec::new();
    let mut diagnostics = Vec::new();
    let result = run_with_diagnostics(
        source.into(),
        &options,
        &mut output,
        &mut diagnostics,
        &Cancellation::default(),
    );
    (
        result,
        String::from_utf8(output).unwrap(),
        String::from_utf8(diagnostics).unwrap(),
    )
}
