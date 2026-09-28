//! Process adapter: arguments, terminal evidence, interrupts and exit status.

use crate::{Cli, Completion};
use clap::Parser;
use std::fmt;
use std::io::{self, BufWriter, IsTerminal, Write};
use std::process::ExitCode;
use std::sync::{Arc, atomic::AtomicBool};
use zetesis_presentation::{ColorMode, Layout, Role, Streams, color_disabled, terminal_width};

// Fixed staging for redirected output; terminal bytes bypass it.
const OUTPUT_BUFFER_BYTES: usize = 8 * 1024;

/// Run the command named by the process arguments.
///
/// Exit 0 means the command completed and, for a campaign, every position
/// passed; 1 means a completed campaign retained non-passing positions; 2 is a
/// usage, measurement, publication or output error. SIGINT and SIGTERM cancel
/// a corpus campaign, which then publishes its cancelled positions. Standard
/// output is flushed before returning; a flush failure is an output error.
#[must_use]
pub fn entry() -> ExitCode {
    let command = match Cli::try_parse() {
        Ok(cli) => cli.command,
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
    let mut diagnostics = io::stderr().lock();
    let no_color = std::env::var_os("NO_COLOR");
    let term = std::env::var_os("TERM");
    let streams = Streams::resolve(
        command.color().human(command.json()),
        output_terminal,
        diagnostics.is_terminal(),
        color_disabled(no_color.as_deref(), term.as_deref()),
    );
    let columns = std::env::var("COLUMNS").ok();
    let layout = Layout::new(terminal_width(columns.as_deref()), streams.output);
    let mut output = BufWriter::with_capacity(
        if output_terminal {
            0
        } else {
            OUTPUT_BUFFER_BYTES
        },
        output,
    );
    let cancelled = Arc::new(AtomicBool::new(false));
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    let _interrupts = if matches!(command, crate::Command::Corpus(_)) {
        match zetesis_validation::process::interrupts::Interrupts::install(&cancelled) {
            Ok(interrupts) => Some(interrupts),
            Err(error) => {
                let _ = report(&mut diagnostics, streams.diagnostics, &error);
                return ExitCode::from(2);
            }
        }
    } else {
        None
    };
    let status = match crate::execute_with_cancellation(
        &command,
        layout,
        &mut output,
        &mut diagnostics,
        &cancelled,
    ) {
        Ok(Completion::Passed) => ExitCode::SUCCESS,
        Ok(Completion::NonPass) => ExitCode::FAILURE,
        Err(error) => {
            let _ = report(&mut diagnostics, streams.diagnostics, &error);
            ExitCode::from(2)
        }
    };
    let flushed = output.flush();
    // Do not retry a failed buffered write invisibly from BufWriter::drop.
    let _ = output.into_parts();
    match flushed {
        Ok(()) => status,
        Err(error) => {
            let _ = report(
                &mut diagnostics,
                streams.diagnostics,
                &format_args!("output flush: {error}"),
            );
            ExitCode::from(2)
        }
    }
}

fn report(
    diagnostics: &mut impl Write,
    color: ColorMode,
    error: &impl fmt::Display,
) -> io::Result<()> {
    color.styled(diagnostics, Role::Label, format_args!("zetesis-bench:"))?;
    writeln!(diagnostics, " {error}")
}
