//! Backend policy, separate from the questions asked of a lowered program.

use std::num::NonZeroUsize;

use crate::OutputLimits;
use zetesis_solve::{Resources, SolveConfig};

/// Which native preparation supplies an answer-set run.
///
/// This is a preparation choice: native sessions distinguish their retained
/// relational, complete-formula, hybrid and terminal-definition inputs
/// separately. The three choices are the CLI's `--grounder` values, with the
/// same mapping.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Grounder {
    /// Use the checked formula preparation's adaptive terminal-definition plan.
    /// Ineligible programs retain complete eager formula grounding.
    #[default]
    Auto,
    /// Materialize the complete formula theory before search.
    Eager,
    /// Instantiate on demand. A program inside the relational profile is
    /// joined from source without a complete ground-rule store. Any other
    /// program, refused by that profile only for a construct it lacks, is
    /// grounded as formulas: an eager producer core, eligible integrity
    /// constraints streamed during search, and certified terminal definitions
    /// reconstructed per answer. Every other refusal is returned, never
    /// retried under another mode.
    Lazy,
}

/// Native resource and execution policy for [`crate::Solver`].
///
/// The backend is CPU-only and always enumerates unscored, unprojected answer
/// sets. Its workers use the host's available parallelism by default, or one if
/// the host cannot report it. Limits remain independent named allowances;
/// neither their sum nor [`Self::memory`] is a process-memory guarantee.
#[derive(Clone, Copy, Debug)]
pub struct Config {
    /// Preparation policy; explicit modes preserve their admission boundaries.
    pub grounder: Grounder,
    /// Maximum CPU workers assigned to the native session.
    pub workers: NonZeroUsize,
    /// Memory allowance used by the shared ordinary resource policy at each
    /// preparation and session opening. Source, solve and output capacities are
    /// derived together, so changing this field cannot leave stale defaults.
    /// Independent named capacities exclude allocator/stack overhead; not RSS.
    pub memory: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            grounder: Grounder::Auto,
            workers: std::thread::available_parallelism().unwrap_or(NonZeroUsize::MIN),
            memory: SolveConfig::REFERENCE_MEMORY,
        }
    }
}

impl Config {
    pub(crate) fn resources(self) -> Resources {
        Resources::new(self.memory, self.workers)
    }

    pub(crate) fn output(self) -> OutputLimits {
        OutputLimits::for_resources(self.resources())
    }
    pub(crate) fn session(self) -> SolveConfig {
        SolveConfig {
            backend: zetesis_solve::Backend::Cpu,
            search: zetesis_solve::SearchMethod::Regions,
            grounder: match self.grounder {
                Grounder::Auto => zetesis_solve::Grounder::Auto,
                Grounder::Eager => zetesis_solve::Grounder::Eager,
                Grounder::Lazy => zetesis_solve::Grounder::Lazy,
            },
            models: 0,
            ..self.resources().solve_config()
        }
    }
}

#[cfg(test)]
mod tests;
