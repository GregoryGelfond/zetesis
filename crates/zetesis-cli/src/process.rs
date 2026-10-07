use crate::presentation::Diagnostics;
use crate::{ColorMode, Completion, Invocation, Options, PublicationOutcome, RunError, RunFailure};
use std::io::{self, BufWriter, IsTerminal, Read, Write};
use std::process::ExitCode;
use std::time::{Duration, Instant};
use zetesis_presentation::{Streams, color_disabled, terminal_width};
use zetesis_themelios::SourceBundle;

/// Process adapter. Exit 0 means a completed request; test uses 1 for a
/// completed check with non-passing evidence. Exit 2 is an input/backend/output
/// error, and 3 interrupted preparation, search or publication. Satisfiability
/// and coverage are printed independently; this is not clingo's numeric exit-code protocol.
/// Standard output is explicitly flushed before returning. A flush failure is
/// an output error, independently of any established semantic outcome.
#[must_use]
pub fn entry() -> ExitCode {
    let mut invocation = match Invocation::try_parse_from(std::env::args_os()) {
        Ok(invocation) => invocation,
        Err(error) => {
            let status = if error.use_stderr() {
                ExitCode::from(2)
            } else {
                ExitCode::SUCCESS
            };
            return if error.print().is_ok() {
                status
            } else {
                ExitCode::from(2)
            };
        }
    };
    let output = io::stdout().lock();
    let output_terminal = output.is_terminal();
    let no_color = std::env::var_os("NO_COLOR");
    let term = std::env::var_os("TERM");
    let disabled = color_disabled(no_color.as_deref(), term.as_deref());
    let diagnostics = io::stderr().lock();
    let (mode, json) = match &invocation {
        Invocation::Solve(options) => (options.color, options.json),
        Invocation::Devices => (ColorMode::Auto, false),
        Invocation::Test(command) => (command.color(), command.json()),
    };
    let colors = Streams::resolve(
        mode.human(json),
        output_terminal,
        diagnostics.is_terminal(),
        disabled,
    );
    let columns = std::env::var("COLUMNS").ok();
    let width = terminal_width(columns.as_deref());
    let layout = zetesis_presentation::Layout::new(width, colors.output);
    let mut output = buffered_output(output, output_terminal);
    let mut diagnostics = match &invocation {
        Invocation::Solve(options) => {
            Diagnostics::for_solve(diagnostics, colors.diagnostics, options)
        }
        Invocation::Devices | Invocation::Test(_) => {
            Diagnostics::new(diagnostics, colors.diagnostics)
        }
    }
    .with_width(width);
    let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    // Only conformance checks own child solvers that an interrupt must settle.
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    let _interrupts = if matches!(invocation, Invocation::Test(_)) {
        match zetesis_validation::process::interrupts::Interrupts::install(&cancelled) {
            Ok(interrupts) => Some(interrupts),
            Err(error) => {
                let _ = diagnostics.diagnostic(&error);
                return ExitCode::from(2);
            }
        }
    } else {
        None
    };
    let result = match &mut invocation {
        Invocation::Solve(options) => {
            options.color = colors.output;
            run_input(options, &mut output, &mut diagnostics)
                .map(|outcome| publication_status(&outcome))
        }
        Invocation::Devices => crate::devices::devices_with_color(&mut output, colors.output)
            .map(|()| ExitCode::SUCCESS)
            .map_err(RunFailure::from),
        Invocation::Test(command) => {
            let result = crate::testing::execute_with_cancellation(
                command,
                layout,
                &mut output,
                &mut diagnostics,
                cancelled.as_ref(),
            );
            Ok(command_status(
                result.map(|passed| passed == crate::testing::Completion::Passed),
                &mut diagnostics,
            ))
        }
    };
    finish_output(output, result, &mut diagnostics)
}

fn command_status<E: std::fmt::Display>(
    result: Result<bool, E>,
    diagnostics: &mut Diagnostics<impl Write>,
) -> ExitCode {
    match result {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(error) => {
            let _ = diagnostics.diagnostic(&error);
            ExitCode::from(2)
        }
    }
}

fn publication_status(outcome: &PublicationOutcome) -> ExitCode {
    if matches!(outcome, PublicationOutcome::Stopped(_))
        || outcome.semantic().completion() == Some(Completion::Interrupted)
    {
        ExitCode::from(3)
    } else {
        ExitCode::SUCCESS
    }
}

// Fixed process-only staging for redirected output, independent of model count
// and per-record ceilings. Terminal bytes bypass staging to retain prompt output.
const OUTPUT_BUFFER_BYTES: usize = 8 * 1024;

fn buffered_output<W: Write>(output: W, terminal: bool) -> BufWriter<W> {
    BufWriter::with_capacity(if terminal { 0 } else { OUTPUT_BUFFER_BYTES }, output)
}

fn finish_output(
    mut output: BufWriter<impl Write>,
    result: Result<ExitCode, RunFailure>,
    diagnostics: &mut Diagnostics<impl Write>,
) -> ExitCode {
    let flushed = output.flush();
    // Do not retry a failed buffered write invisibly from BufWriter::drop.
    let _ = output.into_parts();
    let status = match result {
        Ok(status) => status,
        Err(error) => {
            let _ = diagnostics.diagnostic(&error);
            if let Some(secondary) = error.secondary_output {
                let _ = diagnostics.diagnostic(&format_args!("secondary output: {secondary}"));
            }
            ExitCode::from(2)
        }
    };
    match flushed {
        Ok(()) => status,
        Err(error) => {
            let _ = diagnostics.diagnostic(&format_args!("output flush: {error}"));
            ExitCode::from(2)
        }
    }
}

fn run_input(
    options: &Options,
    output: &mut impl Write,
    diagnostics: &mut Diagnostics<impl Write>,
) -> Result<PublicationOutcome, RunFailure> {
    let (input, cancellation) = match load_input(options).and_then(|input| {
        process_cancellation(options.time_limit, Instant::now())
            .map(|cancellation| (input, cancellation))
    }) {
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
    let result = match input {
        Input::Source(source) => crate::driver::run_source_with_writer(
            source,
            options,
            output,
            diagnostics,
            &cancellation,
        ),
        Input::Bundle(bundle) => crate::driver::run_bundle_with_writer(
            bundle,
            options,
            output,
            diagnostics,
            &cancellation,
        ),
    };
    result.map_err(crate::PublicationFailure::into_legacy)
}

fn process_cancellation(
    seconds: Option<u64>,
    start: Instant,
) -> Result<zetesis_cpu::Cancellation, RunError> {
    let Some(seconds) = seconds else {
        return Ok(zetesis_cpu::Cancellation::default());
    };
    let deadline = start
        .checked_add(Duration::from_secs(seconds))
        .ok_or(RunError::TimeLimitRange { seconds })?;
    zetesis_cpu::Cancellation::with_deadline(deadline).map_err(RunError::DeadlineTimer)
}

enum Input {
    Source(String),
    Bundle(SourceBundle),
}

fn load_input(options: &Options) -> Result<Input, RunError> {
    if !options.additional_inputs.is_empty()
        && std::iter::once(&options.input)
            .chain(&options.additional_inputs)
            .any(|path| path.as_os_str() == "-")
    {
        return Err(RunError::MixedStandardInput);
    }
    crate::SolveConfig::from(options).validate()?;
    if options.input.as_os_str() == "-" {
        let source = read_source(options).map_err(RunError::Input)?;
        Ok(Input::Source(source))
    } else {
        let bundle = SourceBundle::load_many(
            std::iter::once(&options.input).chain(&options.additional_inputs),
            options.resources().bundle_limits(),
        )
        .map_err(RunError::BundleLoad)?;
        Ok(Input::Bundle(bundle))
    }
}

fn read_source(options: &Options) -> io::Result<String> {
    read_text(
        io::stdin().lock(),
        options.resources().admission_options().max_source_bytes,
    )
}

fn read_text(reader: impl Read, limit: usize) -> io::Result<String> {
    let maximum = u64::try_from(limit).unwrap_or(u64::MAX).saturating_add(1);
    let mut source = Vec::new();
    reader.take(maximum).read_to_end(&mut source)?;
    // Admission counts bytes. Decoding a truncated multibyte character must not
    // replace the already established byte-limit refusal with a UTF-8 error.
    if source.len() > limit {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "source byte limit exceeded",
        ));
    }
    String::from_utf8(source).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "standard input is not valid UTF-8",
        )
    })
}

#[cfg(test)]
mod tests;
