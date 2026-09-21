//! Least consequences of positive producers, checked against original constraints.
//!
//! This optional complete-root certificate admits positive cycles and monotone
//! conjunction/disjunction bodies. It does not translate source or treat a
//! dependency projection as the original theory. Construction computes the least
//! consequences once; no per-candidate propagation graph is retained.

mod compile;
mod propagate;
mod validation;
#[cfg(test)]
mod tests;

use std::{fmt, mem::size_of};

use crate::{Interpretation, Theory};
use zetesis_cpu::{Cancellation, Stop};

/// Independent limits for complete-root classification and least closure.
#[derive(Clone, Copy, Debug)]
pub struct PositivePlanLimits {
    /// Forward incidences, counting both occurrences of an aliased binary child.
    pub max_dependencies: usize,
    /// Named plan header, construction-vector headers and actual vector capacity.
    /// Excludes the shared theory, allocator overhead and other stack temporaries.
    /// This is not RSS or an allocator hard cap; allocation slack is checked after
    /// reservation, and a refused actual allocation contributes to the peak.
    pub max_bytes: usize,
    /// Charged node/root, initialization, incidence and queue operations.
    pub max_work: u64,
}

impl Default for PositivePlanLimits {
    fn default() -> Self {
        Self {
            max_dependencies: 4_194_304,
            max_bytes: 256 * 1024 * 1024,
            max_work: 100_000_000,
        }
    }
}

/// Which explicit preparation resource could not be admitted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PositiveResource {
    /// Forward graph incidences.
    Dependencies,
    /// Named simultaneous construction/retained storage.
    Bytes,
    /// Charged preparation and propagation work.
    Work,
}

/// Refused grammar or incomplete preparation, never a satisfiability result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PositiveError {
    /// This original root is not an atomic fact, positive atomic-head rule or
    /// arbitrary constraint. In particular, choices are not ordinary producers.
    UnsupportedRoot {
        /// Original asserted formula node.
        root: usize,
    },
    /// An original atomic-head producer body contains an unsupported implication.
    UnsupportedBody {
        /// Original asserted formula node.
        root: usize,
        /// Original body node whose complete nested classification failed.
        body: usize,
    },
    /// Completed propagation failed an original producer. This is an invariant
    /// refusal, never a constraint-based conclusion that no answer set exists.
    InvalidClosure {
        /// Original producer root reported false by exact evaluation.
        root: usize,
    },
    /// A named resource's complete next request exceeds its ceiling.
    Limit {
        /// Refused resource.
        resource: PositiveResource,
        /// Complete requested amount, rather than only its increment.
        observed: u128,
        /// Configured ceiling.
        limit: u128,
    },
    /// Checked shape/count arithmetic cannot represent the required value.
    Overflow,
    /// Original cancellation, deadline or allocation failure.
    Stopped(Stop),
}

impl fmt::Display for PositiveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedRoot { root } => {
                write!(f, "root {root} has no positive atomic-head plan")
            }
            Self::UnsupportedBody { root, body } => {
                write!(f, "positive root {root} has an unsupported body at {body}")
            }
            Self::InvalidClosure { root } => {
                write!(f, "least consequences fail original producer {root}")
            }
            Self::Limit {
                resource,
                observed,
                limit,
            } => write!(
                f,
                "positive plan {resource:?} needs {observed}; limit is {limit}"
            ),
            Self::Overflow => f.write_str("positive plan arithmetic overflowed"),
            Self::Stopped(stop) => stop.fmt(f),
        }
    }
}
impl std::error::Error for PositiveError {}
impl From<Stop> for PositiveError {
    fn from(stop: Stop) -> Self {
        Self::Stopped(stop)
    }
}

/// Work and named-storage observations, including a failed preparation prefix.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PositivePlanStatistics {
    /// Admitted primitive operations. A refused next operation is not included.
    pub work: u64,
    /// Admitted forward incidences during graph construction.
    pub dependencies: usize,
    /// Atom vertices that became true, including facts.
    pub derived_atoms: usize,
    /// Formula vertices that became true, including canonical truth constants.
    pub activated_nodes: usize,
    /// Outgoing incidences visited during propagation.
    pub propagated_dependencies: usize,
    /// Maximum named simultaneous bytes, using observed vector capacities and
    /// reserving the final plan header throughout construction. Refused proposed
    /// allocations do not increase this; actual slack before refusal does.
    pub peak_bytes: u128,
    /// Final plan header and actual interpretation-word capacity on success;
    /// zero on failure, because no prepared owner is returned.
    pub retained_bytes: u128,
}

/// A complete least-consequence computation for one exact original theory.
///
/// Every root was checked. The least interpretation satisfies all positive
/// producers. If no constraint failed, it is the unique answer set of this
/// admitted grammar; a failed constraint rules out every answer set, but need
/// not rule out larger classical models when its body is nonmonotone. This
/// class certificate is not the solve crate's subject-associated `AnswerSet`.
/// It does not establish source completeness or verify the Rust implementation.
///
/// ```
/// use zetesis_cpu::Cancellation;
/// use zetesis_ferraris::{AdmissionLimits, Node, PositivePlan, PositivePlanLimits, Theory};
///
/// // a. b :- a. a :- b. The positive cycle is seeded by the fact a.
/// let theory = Theory::new(
///     2,
///     vec![Node::Atom(0), Node::Atom(1), Node::Implies(0, 1), Node::Implies(1, 0)],
///     vec![0, 2, 3],
///     AdmissionLimits::default(),
/// )?;
/// let plan = PositivePlan::compile(&theory, PositivePlanLimits::default(), &Cancellation::default())?;
/// assert_eq!(plan.least_consequences().atoms().collect::<Vec<_>>(), [0, 1]);
/// assert_eq!(plan.failed_constraint(), None);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug)]
pub struct PositivePlan {
    least: Interpretation,
    failed_constraint: Option<usize>,
    statistics: PositivePlanStatistics,
}

impl PositivePlan {
    /// Compute once using the same implementation as [`Self::compile_accounted`].
    ///
    /// # Errors
    /// Returns unsupported-root/body, resource, arithmetic, control or allocation
    /// refusal. No partial closure or certificate is returned.
    pub fn compile(
        theory: &Theory,
        limits: PositivePlanLimits,
        cancellation: &Cancellation,
    ) -> Result<Self, PositiveError> {
        Self::compile_accounted(theory, limits, cancellation).result
    }

    /// Inspect all original roots and compute least consequences with forward
    /// incidences. Producer bodies admit atoms, falsum, And, Or and exactly
    /// False→False truth. Roots admit atomic facts, positive body→atom,
    /// arbitrary body→False and False. Other producer implication bodies,
    /// default negation in producers and choice heads are refused.
    ///
    /// Each atom/formula vertex becomes true at most once; each outgoing edge
    /// is then visited once. Total work is linear in atoms, nodes, roots and
    /// incidences; no DNF expansion or repeated full fixpoint scan occurs.
    /// After propagation, exact original evaluation checks the least result.
    /// Evaluation shares the remaining work and simultaneous byte allowance;
    /// its temporary truth vector replaces the released incidence/work vectors.
    /// Failed attempts retain actual work and peak observations, not a closure.
    #[must_use]
    pub fn compile_accounted(
        theory: &Theory,
        limits: PositivePlanLimits,
        cancellation: &Cancellation,
    ) -> PositiveAttempt {
        let mut budget = Budget {
            limits,
            cancellation,
            statistics: PositivePlanStatistics::default(),
            current_bytes: 0,
        };
        let result = compile::build(theory, &mut budget);
        let result = result.map(|(least, failed_constraint)| {
            budget.statistics.retained_bytes = size_of::<Self>() as u128
                + least.words.capacity() as u128 * size_of::<u64>() as u128;
            Self {
                least,
                failed_constraint,
                statistics: budget.statistics,
            }
        });
        PositiveAttempt {
            result,
            statistics: budget.statistics,
        }
    }

    /// Exact immutable original theory; equality of independently built theories
    /// does not authorize substituting their local atom IDs.
    #[must_use]
    pub fn theory(&self) -> &Theory {
        self.least.theory()
    }

    /// Least consequences of the positive producers. When a constraint failed,
    /// this interpretation is not a model of the complete theory.
    #[must_use]
    pub const fn least_consequences(&self) -> &Interpretation {
        &self.least
    }

    /// First violated original constraint in root order, after complete closure.
    #[must_use]
    pub const fn failed_constraint(&self) -> Option<usize> {
        self.failed_constraint
    }

    /// Complete construction/propagation/original-evaluation and owner observations.
    #[must_use]
    pub const fn statistics(&self) -> PositivePlanStatistics {
        self.statistics
    }
}

/// One preparation attempt; an error supplies no positive semantic certificate.
#[derive(Debug)]
pub struct PositiveAttempt {
    /// Completed plan or original typed refusal.
    pub result: Result<PositivePlan, PositiveError>,
    /// Actual admitted work and named capacity, including a failure prefix.
    pub statistics: PositivePlanStatistics,
}

struct Budget<'a> {
    limits: PositivePlanLimits,
    cancellation: &'a Cancellation,
    statistics: PositivePlanStatistics,
    current_bytes: u128,
}

impl Budget<'_> {
    fn tick(&mut self) -> Result<(), PositiveError> {
        self.cancellation.poll()?;
        Self::ceiling(
            PositiveResource::Work,
            u128::from(self.statistics.work) + 1,
            u128::from(self.limits.max_work),
        )?;
        self.statistics.work += 1;
        Ok(())
    }

    fn dependency(&mut self) -> Result<(), PositiveError> {
        let next = self
            .statistics
            .dependencies
            .checked_add(1)
            .ok_or(PositiveError::Overflow)?;
        Self::ceiling(
            PositiveResource::Dependencies,
            next as u128,
            self.limits.max_dependencies as u128,
        )?;
        self.statistics.dependencies = next;
        Ok(())
    }

    fn observe_bytes(&mut self, bytes: u128) -> Result<(), PositiveError> {
        self.current_bytes = bytes;
        self.statistics.peak_bytes = self.statistics.peak_bytes.max(bytes);
        self.check_bytes(bytes)
    }

    fn check_bytes(&self, bytes: u128) -> Result<(), PositiveError> {
        Self::ceiling(
            PositiveResource::Bytes,
            bytes,
            self.limits.max_bytes as u128,
        )
    }

    fn reserve<T>(&mut self, count: usize) -> Result<Vec<T>, PositiveError> {
        self.cancellation.poll()?;
        let requested = self
            .current_bytes
            .checked_add(count as u128 * size_of::<T>() as u128)
            .ok_or(PositiveError::Overflow)?;
        self.check_bytes(requested)?;
        let mut result = Vec::new();
        result
            .try_reserve_exact(count)
            .map_err(|_| Stop::Allocation)?;
        let actual = self
            .current_bytes
            .checked_add(result.capacity() as u128 * size_of::<T>() as u128)
            .ok_or(PositiveError::Overflow)?;
        self.observe_bytes(actual)?;
        Ok(result)
    }

    fn release<T>(&mut self, vector: Vec<T>) -> Result<(), PositiveError> {
        let bytes = vector.capacity() as u128 * size_of::<T>() as u128;
        drop(vector);
        self.current_bytes = self
            .current_bytes
            .checked_sub(bytes)
            .ok_or(PositiveError::Overflow)?;
        Ok(())
    }

    fn ceiling(
        resource: PositiveResource,
        observed: u128,
        limit: u128,
    ) -> Result<(), PositiveError> {
        if observed > limit {
            return Err(PositiveError::Limit {
                resource,
                observed,
                limit,
            });
        }
        Ok(())
    }
}
