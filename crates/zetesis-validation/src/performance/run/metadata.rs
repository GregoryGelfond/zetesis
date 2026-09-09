//! Bounded producer configuration evidence, outside measured solves.
use super::{Fault, Report, Request, invoke};
use std::path::Path;
use std::time::Instant;

pub(super) fn collect(
    request: &Request<'_>,
    directory: &Path,
    deadline: Instant,
    report: &mut Report,
) -> bool {
    let native_help = if request.schedule.extended() {
        "--help"
    } else {
        "--help-all"
    };
    for (executable, argument) in [
        (request.native, "--version"),
        (request.native, native_help),
        (request.reference, "--version"),
    ] {
        if !query(executable, argument, directory, deadline, report) {
            return false;
        }
        if argument == "--help" {
            let capture = report.metadata.last().expect("successful query retained");
            let advertised = full_help(capture.stdout())
                .and_then(|stdout| full_help(capture.stderr()).map(|stderr| stdout || stderr));
            match advertised {
                Ok(true) if !query(executable, "--help-all", directory, deadline, report) => {
                    return false;
                }
                Err(_) => {
                    report.faults.push(Fault::Metadata);
                    return false;
                }
                Ok(_) => {}
            }
        }
    }
    if request.schedule.extended()
        && !query(request.reference, "--help", directory, deadline, report)
    {
        return false;
    }
    report.metadata_complete = true;
    true
}

fn query(
    executable: &Path,
    argument: &str,
    directory: &Path,
    deadline: Instant,
    report: &mut Report,
) -> bool {
    let Some(capture) = invoke(
        executable,
        vec![argument.into()],
        directory,
        deadline,
        report,
    ) else {
        return false;
    };
    let complete = capture.complete(false);
    report.metadata.push(capture);
    if !complete {
        report.faults.push(Fault::Metadata);
    }
    complete
}

fn full_help(bytes: &[u8]) -> Result<bool, std::str::Utf8Error> {
    let text = std::str::from_utf8(bytes)?;
    let mut clean = String::new();
    let mut characters = text.chars().peekable();
    while let Some(character) = characters.next() {
        if character == '\u{1b}' && characters.peek() == Some(&'[') {
            characters.next();
            for character in characters.by_ref() {
                if ('@'..='~').contains(&character) {
                    break;
                }
            }
        } else {
            clean.push(character);
        }
    }
    Ok(clean.split_whitespace().any(|word| word == "--help-all"))
}
