//! Process adapter for reduct-oracle and source-grounding experiments.
#![forbid(unsafe_code)]
use clap::Parser;

fn main() -> std::process::ExitCode {
    let options = zetesis_experiments::CommandOptions::parse();
    let mut output = std::io::stdout().lock();
    let outcome = match options.command {
        Some(zetesis_experiments::Experiment::Table(options)) => {
            return match zetesis_experiments::table_measurement::run(&options, &mut output) {
                Ok(true) => std::process::ExitCode::SUCCESS,
                Ok(false) => std::process::ExitCode::FAILURE,
                Err(error) => {
                    eprintln!("zetesis-bench: {error}");
                    std::process::ExitCode::from(2)
                }
            };
        }
        Some(zetesis_experiments::Experiment::Feedback(options)) => {
            zetesis_experiments::feedback_measurement::run(&options, &mut output)
                .map_err(|error| error.to_string())
        }
        Some(zetesis_experiments::Experiment::Relation(options)) => {
            zetesis_experiments::relation_measurement::run(&options, &mut output)
                .map_err(|error| error.to_string())
        }
        Some(zetesis_experiments::Experiment::Aggregate(options)) => {
            zetesis_experiments::aggregate_measurement::run(&options, &mut output)
                .map_err(|error| error.to_string())
        }
        Some(zetesis_experiments::Experiment::Tight(options)) => {
            zetesis_experiments::tight_measurement::run(&options, &mut output)
                .map_err(|error| error.to_string())
        }
        Some(zetesis_experiments::Experiment::Lazy(options)) => {
            zetesis_experiments::lazy_measurement::run(&options, &mut output)
                .map_err(|error| error.to_string())
        }
        Some(zetesis_experiments::Experiment::FormulaProjection(options)) => {
            zetesis_experiments::run_formula_projection(&options, &mut output)
                .map_err(|error| error.to_string())
        }
        Some(zetesis_experiments::Experiment::Grounding(options)) => {
            zetesis_experiments::grounding::run(&options, &mut output)
                .map_err(|error| error.to_string())
        }
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
