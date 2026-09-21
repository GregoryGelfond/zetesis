//! Process adapter for reduct-oracle and source-grounding experiments.
#![forbid(unsafe_code)]
use clap::Parser;
use zetesis_experiments::command::{Completion, execute};

fn main() -> std::process::ExitCode {
    let options = zetesis_experiments::CommandOptions::parse();
    match execute(&options, &mut std::io::stdout().lock()) {
        Ok(Completion::Passed) => std::process::ExitCode::SUCCESS,
        Ok(Completion::Refused) => std::process::ExitCode::FAILURE,
        Err(error) => {
            eprintln!("zetesis-bench: {error}");
            std::process::ExitCode::from(2)
        }
    }
}
