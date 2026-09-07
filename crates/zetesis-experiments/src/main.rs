//! Process adapter for the explicitly selected CPU or Metal benchmark.
#![forbid(unsafe_code)]
use clap::Parser;

fn main() -> std::process::ExitCode {
    let options = zetesis_experiments::CommandOptions::parse();
    let mut output = std::io::stdout().lock();
    let outcome = match options.command {
        Some(zetesis_experiments::Experiment::Formula(options)) => {
            zetesis_experiments::run_formula(&options, &mut output)
                .map_err(|error| error.to_string())
        }
        None => zetesis_experiments::run(&options.static_options, &mut output)
            .map_err(|error| error.to_string()),
    };
    match outcome {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("zetesis-bench: {error}");
            std::process::ExitCode::from(2)
        }
    }
}
