//! Process entry point for zetesis.
#![forbid(unsafe_code)]

fn main() -> std::process::ExitCode {
    zetesis_cli::entry()
}
