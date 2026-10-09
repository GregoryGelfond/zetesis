//! Direct evaluation of a completely checked stratified normal theory.
//!
//! Positive dependency components are saturated in dependency order. Negative
//! literals read only completed predecessors. Original constraints then filter
//! the unique resulting interpretation; they never provide support. This is a
//! ground-theory certificate, independent of source classification.

mod compile;
mod evaluate;
mod order;
#[cfg(test)]
mod tests;

use std::{fmt, mem::size_of};

use crate::{Interpretation, Theory};
use zetesis_cpu::{Cancellation, Stop};

/// Bounds for checked classification, stratification and direct evaluation.
#[derive(Clone, Copy, Debug)]
pub struct StratifiedPlanLimits {
    /// Signed dependency incidences, counting repeated child occurrences.
    pub max_dependencies: usize,
    /// Named simultaneous headers and actual vector capacities. Shared theory,
    /// allocator overhead and other stack temporaries are excluded; this is not RSS.
    pub max_bytes: usize,
    /// Charged classification, graph, component, closure and validation work.
    pub max_work: u64,
}

impl Default for StratifiedPlanLimits {
    fn default() -> Self {
        Self {
            max_dependencies: 4_194_304,
            max_bytes: 256 * 1024 * 1024,
            max_work: 100_000_000,
        }
    }
}

/// Explicit resource whose next complete request could not be admitted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StratifiedResource {
    /// Signed graph incidences.
    Dependencies,
    /// Named simultaneous storage.
    Bytes,
    /// Charged primitive work.
    Work,
}

/// An unsupported class or incomplete attempt, never a satisfiability verdict.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StratifiedError {
    /// An asserted root is not an atomic fact, normal atomic-head rule or constraint.
    UnsupportedRoot {
        /// Original root node.
        root: usize,
    },
    /// A producer body is outside the conjunction-of-signed-atoms grammar.
    UnsupportedBody {
        /// Original producer root.
        root: usize,
        /// Original body node.
        body: usize,
    },
    /// A negative dependency belongs to a recursive component.
    NegativeCycle {
        /// Negated atom whose truth would depend on this component.
        atom: usize,
        /// Original default-negation node.
        body: usize,
    },
    /// Final exact evaluation rejected a producer rather than a constraint.
    InvalidClosure {
        /// Original producer root.
        root: usize,
    },
    /// An explicit resource allowance was insufficient.
    Limit {
        /// Refused resource.
        resource: StratifiedResource,
        /// Complete requested amount.
        observed: u128,
        /// Configured ceiling.
        limit: u128,
    },
    /// A checked count cannot represent the required value.
    Overflow,
    /// Cancellation, deadline or allocation failure.
    Stopped(Stop),
}

impl fmt::Display for StratifiedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedRoot { root } => {
                write!(f, "root {root} has no stratified normal plan")
            }
            Self::UnsupportedBody { root, body } => {
                write!(f, "normal root {root} has an unsupported body at {body}")
            }
            Self::NegativeCycle { atom, body } => {
                write!(
                    f,
                    "negative dependency from atom {atom} to body {body} is recursive"
                )
            }
            Self::InvalidClosure { root } => {
                write!(f, "stratified consequences fail producer {root}")
            }
            Self::Limit {
                resource,
                observed,
                limit,
            } => {
                write!(
                    f,
                    "stratified plan {resource:?} needs {observed}; limit is {limit}"
                )
            }
            Self::Overflow => f.write_str("stratified plan arithmetic overflowed"),
            Self::Stopped(stop) => stop.fmt(f),
        }
    }
}
impl std::error::Error for StratifiedError {}
impl From<Stop> for StratifiedError {
    fn from(stop: Stop) -> Self {
        Self::Stopped(stop)
    }
}

/// Observed work and storage, including an interrupted preparation prefix.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StratifiedPlanStatistics {
    /// Admitted primitive operations; a refused next operation is excluded.
    pub work: u64,
    /// Admitted signed graph incidences.
    pub dependencies: usize,
    /// Completely evaluated dependency components, including isolated vertices.
    pub components: usize,
    /// Atom vertices made true.
    pub derived_atoms: usize,
    /// Formula vertices made true.
    pub activated_nodes: usize,
    /// Outgoing incidences visited from true vertices during evaluation.
    pub propagated_dependencies: usize,
    /// Peak named simultaneous headers and actual capacities.
    pub peak_bytes: u128,
    /// Successful plan header and retained interpretation words; zero on refusal.
    pub retained_bytes: u128,
}

/// A directly evaluated, completely checked stratified normal theory.
///
/// Each original producer has one atomic head and a conjunction of atoms,
/// single default-negated atoms, falsum or canonical truth. Positive recursion
/// is permitted; recursion through default negation and choice heads are not.
/// Arbitrary original constraints filter the result after all strata complete.
/// With no failed constraint, the result is the unique answer set of this
/// grammar. This certificate does not establish source grounding completeness
/// or formal refinement of this Rust implementation.
#[derive(Debug)]
pub struct StratifiedPlan {
    consequences: Interpretation,
    failed_constraint: Option<usize>,
    statistics: StratifiedPlanStatistics,
}

impl StratifiedPlan {
    /// Certify all original roots and compute the stratified consequences once.
    ///
    /// # Errors
    /// Returns class, resource, arithmetic or control refusal. No partial plan
    /// or answer-set conclusion is returned on failure.
    pub fn compile(
        theory: &Theory,
        limits: StratifiedPlanLimits,
        cancellation: &Cancellation,
    ) -> Result<Self, StratifiedError> {
        Self::compile_accounted(theory, limits, cancellation).result
    }

    /// Compile with failed-prefix accounting. The iterative component passes
    /// and queued positive closure take O(atoms + nodes + roots + incidences)
    /// work and space. Shared bodies are not expanded per producer. Every vertex
    /// becomes true at most once, and each such outgoing incidence is read once.
    /// Final original evaluation shares the remaining work and storage allowance.
    #[must_use]
    pub fn compile_accounted(
        theory: &Theory,
        limits: StratifiedPlanLimits,
        cancellation: &Cancellation,
    ) -> StratifiedAttempt {
        let mut budget = Budget {
            limits,
            cancellation,
            statistics: StratifiedPlanStatistics::default(),
            current_bytes: 0,
        };
        let result =
            compile::build(theory, &mut budget).map(|(consequences, failed_constraint)| {
                budget.statistics.retained_bytes = size_of::<Self>() as u128
                    + consequences.words.capacity() as u128 * size_of::<u64>() as u128;
                Self {
                    consequences,
                    failed_constraint,
                    statistics: budget.statistics,
                }
            });
        StratifiedAttempt {
            result,
            statistics: budget.statistics,
        }
    }

    /// Exact original owner. Independently equal theories have different atom IDs.
    #[must_use]
    pub fn theory(&self) -> &Theory {
        self.consequences.theory()
    }

    /// Completed stratum-by-stratum consequences. A failed constraint makes this
    /// interpretation unsuitable as an answer of the complete original theory.
    #[must_use]
    pub const fn consequences(&self) -> &Interpretation {
        &self.consequences
    }

    /// First failed original constraint in root order, after complete evaluation.
    #[must_use]
    pub const fn failed_constraint(&self) -> Option<usize> {
        self.failed_constraint
    }

    /// Complete construction, evaluation and storage observations.
    #[must_use]
    pub const fn statistics(&self) -> StratifiedPlanStatistics {
        self.statistics
    }
}

/// A completed plan or refusal, with the same attempt's observed prefix.
#[derive(Debug)]
pub struct StratifiedAttempt {
    /// Completed plan or typed refusal.
    pub result: Result<StratifiedPlan, StratifiedError>,
    /// Actual admitted work and named capacity, including failed attempts.
    pub statistics: StratifiedPlanStatistics,
}

struct Budget<'a> {
    limits: StratifiedPlanLimits,
    cancellation: &'a Cancellation,
    statistics: StratifiedPlanStatistics,
    current_bytes: u128,
}

impl Budget<'_> {
    fn tick(&mut self) -> Result<(), StratifiedError> {
        self.cancellation.poll()?;
        Self::ceiling(
            StratifiedResource::Work,
            u128::from(self.statistics.work) + 1,
            u128::from(self.limits.max_work),
        )?;
        self.statistics.work += 1;
        Ok(())
    }

    fn dependency(&mut self) -> Result<(), StratifiedError> {
        let next = self
            .statistics
            .dependencies
            .checked_add(1)
            .ok_or(StratifiedError::Overflow)?;
        Self::ceiling(
            StratifiedResource::Dependencies,
            next as u128,
            self.limits.max_dependencies as u128,
        )?;
        self.statistics.dependencies = next;
        Ok(())
    }

    fn observe_bytes(&mut self, bytes: u128) -> Result<(), StratifiedError> {
        self.current_bytes = bytes;
        self.statistics.peak_bytes = self.statistics.peak_bytes.max(bytes);
        self.check_bytes(bytes)
    }

    fn check_bytes(&self, bytes: u128) -> Result<(), StratifiedError> {
        Self::ceiling(
            StratifiedResource::Bytes,
            bytes,
            self.limits.max_bytes as u128,
        )
    }

    fn reserve<T>(&mut self, count: usize) -> Result<Vec<T>, StratifiedError> {
        self.cancellation.poll()?;
        let requested = self
            .current_bytes
            .checked_add(count as u128 * size_of::<T>() as u128)
            .ok_or(StratifiedError::Overflow)?;
        self.check_bytes(requested)?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(count)
            .map_err(|_| Stop::Allocation)?;
        let actual = self
            .current_bytes
            .checked_add(values.capacity() as u128 * size_of::<T>() as u128)
            .ok_or(StratifiedError::Overflow)?;
        self.observe_bytes(actual)?;
        Ok(values)
    }

    fn filled<T: Copy>(&mut self, count: usize, value: T) -> Result<Vec<T>, StratifiedError> {
        let mut values = self.reserve(count)?;
        for _ in 0..count {
            self.tick()?;
            values.push(value);
        }
        Ok(values)
    }

    fn release<T>(&mut self, vector: Vec<T>) -> Result<(), StratifiedError> {
        let bytes = vector.capacity() as u128 * size_of::<T>() as u128;
        drop(vector);
        self.current_bytes = self
            .current_bytes
            .checked_sub(bytes)
            .ok_or(StratifiedError::Overflow)?;
        Ok(())
    }

    fn ceiling(
        resource: StratifiedResource,
        observed: u128,
        limit: u128,
    ) -> Result<(), StratifiedError> {
        if observed > limit {
            return Err(StratifiedError::Limit {
                resource,
                observed,
                limit,
            });
        }
        Ok(())
    }
}
