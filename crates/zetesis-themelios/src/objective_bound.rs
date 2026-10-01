//! Optional exact objective constraints for the outer candidate search only.
//!
//! A plan compiles complete positive objective joins over the supplied completed
//! possible relation. Its atom order must match the original theory. The caller
//! establishes coverage of every stable model and verifies the incumbent; this
//! module neither solves a source nor changes its original frozen reduct.

mod bound;
mod join;
mod score;
pub use score::{ObjectiveScore, ObjectiveScoreError, ObjectiveScoreErrorKind};

use std::collections::BTreeMap;
use std::fmt;

use zetesis_core::catalog::Atoms;
use zetesis_cpu::{Cancellation, Stop};
use zetesis_ferraris::{AggregateElement, AggregateError, AggregateLimits, Node, Theory};
use zetesis_objective::{ObjectiveProgram, Score};

/// Inclusive construction limits; zero is a real ceiling.
#[derive(Clone, Copy, Debug)]
pub struct ObjectivePlanLimits {
    /// Supplied original semantic atoms. Also bounds the requested lengths of
    /// two integer lookup orders and one reusable merge-scratch vector. Each
    /// reservation is fallible; allocator slack and atom payload are not a
    /// process memory ceiling.
    pub max_atoms: usize,
    /// Complete positive relational bindings before filters.
    pub max_bindings: u64,
    /// Distinct global priority/weight/tuple keys.
    pub max_keys: usize,
    /// Logical retained key payload, including scalar tags and string bytes.
    pub max_key_bytes: usize,
    /// Explicit scalar tuple width per objective key.
    pub max_tuple_width: usize,
    /// Eligibility DAG nodes.
    pub max_nodes: usize,
    /// Local binding slots per lifted objective.
    pub max_variables: usize,
    /// Positive conditions per lifted objective.
    pub max_body_atoms: usize,
    /// Charged construction operations, including prepared-index comparisons
    /// and writes, query probes and visited scalar descriptors/text bytes.
    pub max_work: u64,
}
impl Default for ObjectivePlanLimits {
    fn default() -> Self {
        Self {
            max_atoms: 65_536,
            max_bindings: 1_000_000,
            max_keys: 65_536,
            max_key_bytes: 16_777_216,
            max_tuple_width: 64,
            max_nodes: 1_048_576,
            max_variables: 64,
            max_body_atoms: 1_024,
            max_work: 10_000_000,
        }
    }
}

/// Independent ceilings for each optional incumbent constraint.
#[derive(Clone, Copy, Debug)]
pub struct ObjectiveBoundLimits {
    /// Absolute node and temporary-state bounds for each exact aggregate family.
    pub aggregate: AggregateLimits,
    /// Cumulative work across copying, all priorities and final theory admission.
    pub max_work: u64,
}
impl Default for ObjectiveBoundLimits {
    fn default() -> Self {
        Self {
            aggregate: AggregateLimits::default(),
            max_work: 10_000_000,
        }
    }
}

/// Refused optional planning resource.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectiveBoundResource {
    /// Semantic atom count.
    Atoms,
    /// Complete join rows.
    Bindings,
    /// Distinct contribution keys.
    Keys,
    /// Retained scalar key payload.
    KeyBytes,
    /// Explicit scalar tuple width.
    TupleWidth,
    /// Formula nodes.
    Nodes,
    /// Local variable slots.
    Variables,
    /// Positive body width.
    BodyAtoms,
    /// Charged work.
    Work,
}

/// Logical accounting, available on success and refusal.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ObjectiveBoundStatistics {
    /// Charged operations.
    pub work: u64,
    /// Complete relational bindings.
    pub bindings: u64,
    /// Distinct globally coalesced keys.
    pub keys: usize,
    /// Logical retained key bytes.
    pub key_bytes: usize,
    /// Constructed DAG nodes.
    pub nodes: usize,
}

/// A refusal to construct an optional optimization, never a logical result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectiveBoundErrorKind {
    /// Cancellation or deadline.
    Control(Stop),
    /// Explicit construction ceiling.
    Limit(ObjectiveBoundResource),
    /// The atom catalog length differs from the original theory or contains duplicate atoms.
    AtomCatalog,
    /// An admitted objective variable unexpectedly remained unbound.
    UnboundVariable,
    /// A negated numeric contribution cannot be represented as i32.
    WeightNormalizationOverflow,
    /// A checked size cannot be represented.
    Overflow,
    /// Fallible allocation failed.
    Allocation,
    /// Exact aggregate lowering declined within its own explicit ceilings.
    Aggregate(AggregateError),
    /// Final candidate formula admission failed.
    Theory(zetesis_ferraris::AdmissionError),
}

/// Failure evidence; template indices address the caller's source-origin catalog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ObjectiveBoundError {
    kind: ObjectiveBoundErrorKind,
    template: Option<usize>,
    statistics: ObjectiveBoundStatistics,
}
impl ObjectiveBoundError {
    /// Structured reason; callers may decline the optional optimization.
    #[must_use]
    pub const fn kind(self) -> ObjectiveBoundErrorKind {
        self.kind
    }
    /// Original lifted template, when the failure occurred during its join.
    #[must_use]
    pub const fn template_index(self) -> Option<usize> {
        self.template
    }
    /// Partial logical accounting without a completed constraint.
    #[must_use]
    pub const fn statistics(self) -> ObjectiveBoundStatistics {
        self.statistics
    }
}
impl fmt::Display for ObjectiveBoundError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "objective candidate bound {:?} (template {:?})",
            self.kind, self.template
        )
    }
}
impl std::error::Error for ObjectiveBoundError {}

/// Reusable exact positive eligibility formulas, independent of any incumbent.
#[derive(Debug)]
pub struct ObjectivePlan {
    original: Theory,
    objectives: ObjectiveProgram,
    nodes: Vec<Node>,
    levels: BTreeMap<i32, Vec<AggregateElement>>,
    statistics: ObjectiveBoundStatistics,
}
impl ObjectivePlan {
    /// Compile all possible numeric objective keys and their exact eligibility.
    /// `atoms` must be the original theory's complete, unique atom
    /// catalog; each catalog index is reused verbatim. This API does not prove
    /// that the supplied relation covers all stable models.
    ///
    /// Preparation borrows the atoms and builds an `AtomIndex` containing only
    /// row positions. The atom ceiling is checked before its fallible linear
    /// storage reservations. Both index construction and later binary searches
    /// charge the same work owner; preparation is not free. Predicate ranges
    /// preserve original catalog order within a predicate, while full-key
    /// membership returns the original dense identity. The temporary index is
    /// dropped after planning; the resulting plan retains its eligibility DAG.
    ///
    /// All comparisons use checked canonical typed identity, including sign,
    /// arity and structural values. This is separate from ASP term ordering.
    /// A missing closed-condition atom is false without extending the catalog.
    /// Work cutoffs reflect actual preparation/traversal, so a changed algorithm
    /// can refuse at a different numeric cutoff while preserving typed failure
    /// and the performed prefix. No partly constructed plan is returned.
    ///
    /// # Errors
    /// Refuses invalid catalogs, numeric normalization overflow, resource ceilings,
    /// cancellation or allocation.
    pub fn new(
        original: &Theory,
        atoms: Atoms<'_>,
        objectives: &ObjectiveProgram,
        limits: ObjectivePlanLimits,
        cancellation: &Cancellation,
    ) -> Result<Self, ObjectiveBoundError> {
        join::compile(original, atoms, objectives, limits, cancellation)
    }
    /// Original theory instance, unchanged by planning.
    #[must_use]
    pub fn original(&self) -> &Theory {
        &self.original
    }
    /// Complete planning accounting.
    #[must_use]
    pub const fn statistics(&self) -> ObjectiveBoundStatistics {
        self.statistics
    }
    /// Compile lexicographic cost less than or equal to `incumbent`, retaining
    /// all ties. Missing priority slots mean zero, as in `Score::compare_costs`.
    /// The caller must independently verify the incumbent before using this to
    /// prune a search. Apply the result only to classical candidates, never to
    /// the original theory or its proper-subset reduct query.
    ///
    /// # Errors
    /// Refuses bounded exact lowering, cancellation, storage or work limits.
    pub fn bound(
        &self,
        incumbent: &Score,
        limits: ObjectiveBoundLimits,
        cancellation: &Cancellation,
    ) -> Result<ObjectiveBound, ObjectiveBoundError> {
        bound::compile(self, incumbent, limits, cancellation)
    }
}

/// A separate candidate-only constraint with explicit original-theory identity.
#[derive(Debug)]
pub struct ObjectiveBound {
    original: Theory,
    theory: Theory,
    statistics: ObjectiveBoundStatistics,
}
impl ObjectiveBound {
    /// The immutable semantic theory whose atom indices the constraint uses.
    #[must_use]
    pub fn original(&self) -> &Theory {
        &self.original
    }
    /// Classical candidate constraint; not an alternative semantic theory.
    #[must_use]
    pub fn theory(&self) -> &Theory {
        &self.theory
    }
    /// Completed bound construction accounting, excluding reusable plan work.
    #[must_use]
    pub const fn statistics(&self) -> ObjectiveBoundStatistics {
        self.statistics
    }
}

struct Work<'a> {
    cancellation: &'a Cancellation,
    limits: ObjectivePlanLimits,
    template: Option<usize>,
    statistics: ObjectiveBoundStatistics,
}
impl Work<'_> {
    fn error(&self, kind: ObjectiveBoundErrorKind) -> ObjectiveBoundError {
        ObjectiveBoundError {
            kind,
            template: self.template,
            statistics: self.statistics,
        }
    }
    fn limit(&self, resource: ObjectiveBoundResource) -> ObjectiveBoundError {
        self.error(ObjectiveBoundErrorKind::Limit(resource))
    }
    fn charge(&mut self, amount: u64) -> Result<(), ObjectiveBoundError> {
        self.cancellation
            .poll()
            .map_err(|error| self.error(ObjectiveBoundErrorKind::Control(error)))?;
        self.account(amount)
    }
    // Imported kernel work has already happened, including on cancellation.
    // Record it without another cancellation check discarding that evidence.
    fn account(&mut self, amount: u64) -> Result<(), ObjectiveBoundError> {
        let next = self
            .statistics
            .work
            .checked_add(amount)
            .ok_or_else(|| self.error(ObjectiveBoundErrorKind::Overflow))?;
        if next > self.limits.max_work {
            return Err(self.limit(ObjectiveBoundResource::Work));
        }
        self.statistics.work = next;
        Ok(())
    }
    fn tick(&mut self) -> Result<(), ObjectiveBoundError> {
        self.charge(1)
    }
    fn reserve<T>(&self, count: usize) -> Result<Vec<T>, ObjectiveBoundError> {
        let mut values = Vec::new();
        values
            .try_reserve_exact(count)
            .map_err(|_| self.error(ObjectiveBoundErrorKind::Allocation))?;
        Ok(values)
    }
    fn node(&mut self, nodes: &mut Vec<Node>, node: Node) -> Result<usize, ObjectiveBoundError> {
        self.tick()?;
        if nodes.len() >= self.limits.max_nodes {
            return Err(self.limit(ObjectiveBoundResource::Nodes));
        }
        nodes
            .try_reserve(1)
            .map_err(|_| self.error(ObjectiveBoundErrorKind::Allocation))?;
        let index = nodes.len();
        nodes.push(node);
        self.statistics.nodes = nodes.len();
        Ok(index)
    }
}
