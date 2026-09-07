//! Reproducible reduct-oracle and original-source grounding experiments.
//!
//! These experiments compare identical compiled programs and frozen candidates.
//! They do not claim complete solve speedups or include source grounding in the
//! dispatch timing. Static closure results use the dense CPU reference; general
//! formula results use complete native membership and time CPU residual checking.
//! The separate [`grounding`] API times fresh formula admission on CPU, with
//! exact source/subject comparison and complete native solving outside its timer.
#![forbid(unsafe_code)]

mod fixtures;
mod formula_completion;
mod formula_fixtures;
mod formula_measurement;
mod formula_parallel;
mod measurement;
pub mod grounding;

pub use fixtures::{BenchmarkFixture, Family};
pub use formula_fixtures::{FormulaFamily, FormulaFixture};
pub use formula_measurement::{
    FormulaBenchmarkError, FormulaOptions, run_formula, run_formula_projection,
};
pub use measurement::{Backend, BenchmarkError, Options, run};

/// Standalone experiment selection, retaining the existing static command.
#[derive(Debug, clap::Parser)]
#[command(
    name = "zetesis-bench",
    about = "Exact reduct oracle and grounding qualification measurements",
    args_conflicts_with_subcommands = true
)]
pub struct CommandOptions {
    /// Formula or source-grounding experiment; omission runs static closure tests.
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
    /// Compare exact gate projections with per-candidate CPU quotas.
    ///
    /// Requires physical Metal. Hybrid residual checks stay serial; ordinary
    /// cumulative-budget scheduling, outer search and grounding are excluded.
    FormulaProjection(FormulaOptions),
    /// Attribute bounded original-source formula admission on CPU.
    Grounding(grounding::Options),
}
