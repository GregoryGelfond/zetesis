use crate::presentation::{Diagnostics, Streams};
use crate::{Command, Completion, Options, Report, RunError, RunFailure, devices};
use clap::Parser;
use std::io::{self, IsTerminal, Read, Write};
use std::process::ExitCode;
use zetesis_themelios::{BundleLimits, SourceBundle};

/// Process adapter. Exit 0 means a completed request, 2 an input/backend/output
/// error, and 3 an interrupted search. Satisfiability and coverage are printed
/// independently; this is not clingo's numeric exit-code protocol.
#[must_use]
pub fn entry() -> ExitCode {
    let mut options = Options::parse();
    let mut output = io::stdout().lock();
    let no_color = std::env::var_os("NO_COLOR");
    let term = std::env::var_os("TERM");
    let disabled = color_disabled(no_color.as_deref(), term.as_deref());
    let diagnostics = io::stderr().lock();
    let colors = Streams::resolve(
        options.color.human(options.json),
        output.is_terminal(),
        diagnostics.is_terminal(),
        disabled,
    );
    options.color = colors.output;
    let mut diagnostics = Diagnostics::new(diagnostics, colors.diagnostics);
    if options.command == Some(Command::Devices) {
        return match devices(&mut output) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                let _ = writeln!(diagnostics, "zetesis: {error}");
                ExitCode::from(2)
            }
        };
    }
    let result = run_input(&options, &mut output, &mut diagnostics);
    match result {
        Ok(report) if report.completion == Completion::Interrupted => ExitCode::from(3),
        Ok(_) => ExitCode::SUCCESS,
        Err(error) => {
            let _ = writeln!(diagnostics, "zetesis: {error}");
            ExitCode::from(2)
        }
    }
}

fn run_input(
    options: &Options,
    output: &mut impl Write,
    diagnostics: &mut Diagnostics<impl Write>,
) -> Result<Report, RunFailure> {
    let input = match load_input(options) {
        Ok(input) => input,
        Err(error) => {
            let failure = error.into();
            return Err(if options.json {
                crate::output::input_failure(output, failure, options)
            } else {
                failure
            });
        }
    };
    let control = zetesis_cpu::Control::default();
    let result = match input {
        Input::Source(source) => {
            crate::driver::run_source_with_writer(source, options, output, diagnostics, &control)
        }
        Input::Bundle(bundle) => {
            crate::driver::run_bundle_with_writer(bundle, options, output, diagnostics, &control)
        }
    };
    result
        .map(crate::SolveReport::into_report)
        .map_err(crate::SolveFailure::into_legacy)
}

enum Input {
    Source(String),
    Bundle(SourceBundle),
}

fn color_disabled(no_color: Option<&std::ffi::OsStr>, term: Option<&std::ffi::OsStr>) -> bool {
    no_color.is_some_and(|value| !value.is_empty()) || term.is_some_and(|value| value == "dumb")
}

fn load_input(options: &Options) -> Result<Input, RunError> {
    if !options.additional_inputs.is_empty()
        && std::iter::once(&options.input)
            .chain(&options.additional_inputs)
            .any(|path| path.as_os_str() == "-")
    {
        return Err(RunError::MixedStandardInput);
    }
    crate::engine::validate_combination(&options.into())?;
    if options.input.as_os_str() == "-" {
        let source = read_source(options).map_err(RunError::Input)?;
        Ok(Input::Source(source))
    } else {
        let bundle = SourceBundle::load_many(
            std::iter::once(&options.input).chain(&options.additional_inputs),
            BundleLimits {
                max_roots: options.max_source_roots,
                max_files: options.max_source_files,
                max_file_bytes: options.max_source_bytes,
                max_total_bytes: options.max_total_source_bytes,
                max_include_depth: options.max_include_depth,
            },
        )
        .map_err(RunError::BundleLoad)?;
        Ok(Input::Bundle(bundle))
    }
}

fn read_source(options: &Options) -> io::Result<String> {
    let reader = io::stdin().lock();
    let maximum = u64::try_from(options.max_source_bytes)
        .unwrap_or(u64::MAX)
        .saturating_add(1);
    let mut source = String::new();
    reader.take(maximum).read_to_string(&mut source)?;
    if source.len() > options.max_source_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "source byte limit exceeded",
        ));
    }
    Ok(source)
}

#[cfg(test)]
mod tests {
    use super::color_disabled;
    use std::ffi::OsStr;

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
}
