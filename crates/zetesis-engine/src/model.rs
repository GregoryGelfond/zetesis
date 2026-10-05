//! Transfer a checked native answer to the canonical model without text or caches.
//!
//! Symbol construction has one independent byte allowance per atom and a
//! cumulative symbol-walk allowance across this run. The atom count separately bounds
//! the result container. Observation limits are per answer. These are named
//! logical/construction limits, not a resident-memory ceiling. Upstream set
//! insertion and Model display unions use infallible allocation and comparison;
//! polls bracket those bounded operations, but cannot interrupt within them.

use std::{error::Error, fmt};

use themelios_program::{AnswerSet, program::Program};
use themelios_solve::{
    contract::Fault,
    outcome::{Conclusion, Model},
};
use zetesis_cpu::{Cancellation, Stop};
use zetesis_themelios::{SourceMetadata, observation, symbols};

use crate::faults;

/// Independent bounds on exporting complete atoms and evaluated display terms.
///
/// Atom count bounds each output set, byte capacity bounds one constructed atom,
/// and symbol work accumulates across the whole run. Observation limits reset
/// for each delivered model. These limits do not account for allocator overhead,
/// upstream ordered-set comparisons or the caller's retained models.
#[derive(Clone, Copy, Debug)]
pub struct OutputLimits {
    /// Full atom cardinality in one model.
    pub max_atoms: usize,
    /// Independent construction capacity for one signed atom.
    pub max_atom_bytes: u128,
    /// Cumulative symbol-walk callbacks; excludes ordered-set comparisons.
    pub max_symbol_work: u64,
    /// Evaluation allowances for each model’s `#show` terms.
    pub observation: observation::Limits,
    /// Construction allowances for each model’s displayed symbols.
    pub construction: observation::ConstructionLimits,
}

impl Default for OutputLimits {
    fn default() -> Self {
        Self {
            max_atoms: zetesis_solve::SolveConfig::DEFAULT.max_atoms,
            max_atom_bytes: observation::ConstructionLimits::default().max_bytes as u128,
            max_symbol_work: zetesis_solve::SolveConfig::DEFAULT.max_model_work,
            observation: observation::Limits::default(),
            construction: observation::ConstructionLimits::default(),
        }
    }
}

pub(crate) struct Exporter {
    limits: OutputLimits,
    work: u128,
}

impl Exporter {
    pub(crate) fn new(limits: OutputLimits) -> Self {
        Self { limits, work: 0 }
    }

    pub(crate) fn convert(
        &mut self,
        answer: &zetesis_solve::AnswerSet,
        metadata: &SourceMetadata,
        cancellation: &Cancellation,
    ) -> Result<Model, OutputError> {
        if answer.score().is_some() {
            return Err(OutputError::ScoredAnswer);
        }
        cancellation.poll().map_err(OutputError::Control)?;
        let native = answer.interpretation();
        let count = native.atoms().len();
        if count > self.limits.max_atoms {
            return Err(OutputError::Atoms {
                observed: count,
                limit: self.limits.max_atoms,
            });
        }
        let mut atoms = AnswerSet::new();
        for atom in native.atoms() {
            let symbol = symbols::atom_with(atom, self.limits.max_atom_bytes, || {
                cancellation.poll().map_err(SymbolStop::Control)?;
                self.work += 1;
                if self.work > u128::from(self.limits.max_symbol_work) {
                    return Err(SymbolStop::Work {
                        observed: self.work,
                        limit: self.limits.max_symbol_work,
                    });
                }
                Ok(())
            })
            .map_err(OutputError::Symbol)?;
            atoms.insert(symbol);
        }
        cancellation.poll().map_err(OutputError::Control)?;
        let terms = metadata
            .observations()
            .evaluate_with_construction_limits(
                native,
                self.limits.observation,
                self.limits.construction,
                cancellation,
            )
            .map_err(OutputError::Observation)?
            .into_symbols();
        cancellation.poll().map_err(OutputError::Control)?;
        let model = Model::of(atoms).with_terms(terms);
        cancellation.poll().map_err(OutputError::Control)?;
        Ok(model)
    }
}

/// Why construction of one canonical output symbol stopped.
#[derive(Debug)]
pub enum SymbolStop {
    /// Cancellation or the request's deadline was observed during construction.
    Control(Stop),
    /// Cumulative symbol traversal exceeded the run's configured work allowance.
    Work {
        /// Traversal callbacks attempted across the run, including this refusal.
        observed: u128,
        /// Configured maximum traversal callbacks.
        limit: u64,
    },
}

impl fmt::Display for SymbolStop {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Control(stop) => stop.fmt(f),
            Self::Work { observed, limit } => write!(
                f,
                "symbol export requires {observed} work units, allowance is {limit}"
            ),
        }
    }
}
impl Error for SymbolStop {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Control(stop) => Some(stop),
            Self::Work { .. } => None,
        }
    }
}

/// A typed failure while exporting a native answer through the Rust solve API.
#[derive(Debug)]
pub enum OutputError {
    /// Request control observed at an output boundary.
    Control(Stop),
    /// The complete answer exceeds the configured output atom count.
    Atoms {
        /// Atoms in the native answer before export.
        observed: usize,
        /// Configured maximum atoms in one exported answer.
        limit: usize,
    },
    /// Canonical symbol construction failed or reached its separate work limit.
    Symbol(symbols::Failure<SymbolStop>),
    /// Evaluation or construction of a `#show` term failed.
    Observation(observation::Error),
    /// The unscored backend received a scored native answer: an internal defect.
    ScoredAnswer,
}

impl OutputError {
    pub(crate) fn finish(self, original: &Program) -> Result<Conclusion, Fault> {
        if let Self::Observation(error) = self {
            return faults::observation(error, original);
        }
        let stop = match &self {
            Self::Control(stop)
            | Self::Symbol(symbols::Failure::Stopped(SymbolStop::Control(stop))) => Some(*stop),
            _ => None,
        };
        if let Some(conclusion) = stop.and_then(faults::control) {
            return Ok(conclusion);
        }
        let fault = match &self {
            Self::Atoms { .. }
            | Self::Symbol(
                symbols::Failure::Bridge(
                    symbols::Error::Allocation | symbols::Error::Storage { .. },
                )
                | symbols::Failure::Stopped(SymbolStop::Work { .. }),
            ) => Fault::resource(self.to_string()),
            _ => Fault::adapter_bug(self.to_string()),
        };
        Err(fault.caused_by(self))
    }
}

impl fmt::Display for OutputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Control(stop) => stop.fmt(f),
            Self::Atoms { observed, limit } => {
                write!(f, "model export has {observed} atoms, allowance is {limit}")
            }
            Self::Symbol(error) => error.fmt(f),
            Self::Observation(error) => error.fmt(f),
            Self::ScoredAnswer => f.write_str("unscored enumeration yielded an objective score"),
        }
    }
}
impl Error for OutputError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Control(stop) => Some(stop),
            Self::Symbol(error) => Some(error),
            Self::Observation(error) => Some(error),
            Self::Atoms { .. } | Self::ScoredAnswer => None,
        }
    }
}
