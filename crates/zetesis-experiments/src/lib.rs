//! Reproducible reduct-oracle measurements with explicit physical backend choice.
//!
//! These experiments compare identical compiled programs and frozen candidates.
//! They do not claim complete solve speedups or include source grounding in the
//! dispatch timing. Static closure results use the dense CPU reference; general
//! formula results use complete native membership and time CPU residual checking.
#![forbid(unsafe_code)]

mod fixtures;
mod formula_completion;
mod formula_fixtures;
mod formula_measurement;
mod formula_parallel;
mod measurement;

pub use fixtures::{BenchmarkFixture, Family};
pub use formula_fixtures::{FormulaFamily, FormulaFixture};
pub use formula_measurement::{FormulaBenchmarkError, FormulaOptions, run_formula};
pub use measurement::{Backend, BenchmarkError, Options, run};

/// Standalone experiment selection, retaining the existing static command.
#[derive(Debug, clap::Parser)]
#[command(
    name = "zetesis-bench",
    about = "Exact reduct oracle qualification and measurements",
    args_conflicts_with_subcommands = true
)]
pub struct CommandOptions {
    /// Optional general-formula experiment; omission runs static closure tests.
    #[command(subcommand)]
    pub command: Option<Experiment>,
    /// Existing static closure experiment options.
    #[command(flatten)]
    pub static_options: Options,
}

/// Independently qualified execution profiles.
#[derive(Debug, clap::Subcommand)]
pub enum Experiment {
    /// Compare resident GPU reduct propagation plus exact CPU residual search.
    Formula(FormulaOptions),
}
