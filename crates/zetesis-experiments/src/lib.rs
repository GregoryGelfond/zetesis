//! Reproducible reduct-oracle and original-source grounding experiments.
//!
//! These experiments compare identical compiled programs and frozen candidates.
//! They do not claim complete solve speedups or include source grounding in the
//! dispatch timing. Static closure results use the dense CPU reference; general
//! formula results use complete native membership and time CPU residual checking.
//! The separate [`grounding`] API times fresh formula admission on CPU, with
//! exact source/subject comparison and complete native solving outside its timer.
//! [`relation_measurement`] compares packed equality selections on the same typed
//! source rows and queries, with retained preparation and common reconstruction.
//! That primitive profile does not perform full pattern matching or solving.
#![forbid(unsafe_code)]

mod backend;
mod fixtures;
mod formula_completion;
mod formula_fixtures;
mod formula_measurement;
mod formula_parallel;
mod measurement;
pub mod relation_fixtures;
pub mod relation_measurement;
pub mod table_measurement;
pub mod grounding;
pub mod lazy_measurement;
pub mod tight_measurement;
pub mod aggregate_measurement;
pub mod feedback_measurement;

pub use backend::Backend;
pub use fixtures::{BenchmarkFixture, Family};
pub use formula_fixtures::{FormulaFamily, FormulaFixture};
pub use formula_measurement::{
    FormulaBenchmarkError, FormulaOptions, run_formula, run_formula_projection,
};
pub use measurement::{BenchmarkError, Options, run};

/// Standalone experiment selection, retaining the existing static command.
#[derive(Debug, clap::Parser)]
#[command(
    name = "zetesis-bench",
    about = "Reproducible reduct, grounding and execution-primitive measurements",
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

/// The experiments, each with its own validation contract.
#[derive(Debug, clap::Subcommand)]
pub enum Experiment {
    /// Compare complete finite-table row survival and projected typed domains on CPU.
    Table(table_measurement::Options),
    /// Study finite conditional countermodel guards without changing ordinary search.
    Feedback(feedback_measurement::Options),
    /// Compare packed relation equality masks on the same typed rows and keys.
    Relation(relation_measurement::Options),
    /// Compare exact native aggregate reductions on matched original/frozen masks.
    Aggregate(aggregate_measurement::Options),
    /// Compare ranked tight certificates with exact CPU residual completion.
    Tight(tight_measurement::Options),
    /// Compare exact lazy source rounds on matched sparse and dense candidates.
    Lazy(lazy_measurement::Options),
    /// Compare resident GPU reduct propagation plus exact CPU residual search.
    Formula(FormulaOptions),
    /// Compare exact gate projections with per-candidate CPU quotas.
    ///
    /// Requires physical Metal or Vulkan. Hybrid residual checks stay serial; ordinary
    /// cumulative-budget scheduling, outer search and grounding are excluded.
    FormulaProjection(FormulaOptions),
    /// Attribute bounded original-source formula admission on CPU.
    Grounding(grounding::Options),
}
