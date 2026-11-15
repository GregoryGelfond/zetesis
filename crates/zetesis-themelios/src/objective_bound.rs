//! Reusable objective eligibility, score reads and exact candidate bounds.
//!
//! A plan compiles complete positive objective joins over the supplied completed
//! possible relation. Its atom order must match the original theory, and the
//! caller establishes coverage of every stable model. Score reads reuse that
//! eligibility independently of candidate pruning. Constructing an incumbent
//! bound additionally requires a verified incumbent. Neither operation solves
//! a source, establishes answer-set membership or changes its frozen reduct.

mod bound;
mod choices;
mod join;
mod projected;
mod score;
#[cfg(test)]
mod tests;
pub use projected::{
    ObjectiveProjectionAttempt, ObjectiveProjectionError, ObjectiveProjectionErrorKind,
    ObjectiveProjectionReceipt, ObjectiveSource,
};
pub use score::{ObjectiveScore, ObjectiveScoreError, ObjectiveScoreErrorKind};

use std::collections::BTreeMap;
use std::fmt;

use zetesis_algebra::{Cancellation, Meter, MeterCountError, Stop};
use zetesis_core::catalog::Atoms;
use zetesis_ferraris::{
    AggregateElement, AggregateError, AggregateLimits, FormulaNodes, FormulaParts, NodeView, Theory,
};
use zetesis_objective::{ObjectiveProgram, PlanIdentity, Score};

/// Inclusive construction limits; zero is a real ceiling.
#[derive(Clone, Copy, Debug)]
pub struct ObjectivePlanLimits {
    /// Supplied original semantic atoms for [`ObjectivePlan::new`], or selected
    /// objective-readable atoms for [`ObjectivePlan::new_projected`]. Also bounds the requested lengths of
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
    /// Eligibility DAG nodes, contribution rows and threshold descriptors, each.
    pub max_nodes: usize,
    /// Logical child occurrences, Boolean operands, conditions and weights, each.
    pub max_operands: usize,
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
            max_operands: 2_097_152,
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
    /// Semantic atom count under the selected constructor: the whole supplied
    /// catalog for `new`, or objective-readable rows for `new_projected`.
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
    /// Logical operand occurrences or retained arena cells.
    Operands,
    /// Local variable slots.
    Variables,
    /// Positive body width.
    BodyAtoms,
    /// Optional choice-proof storage, including source premises and scratch.
    ChoiceBytes,
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
    /// Choice premises belong to a different admitted theory.
    ChoiceOwner,
    /// Optional source choice capture was incomplete. The complete source
    /// receipt remains available through [`crate::RequiredChoices::capture_failure`].
    ChoiceCapture,
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
    identity: PlanIdentity,
    original: Theory,
    objectives: ObjectiveProgram,
    nodes: FormulaParts,
    levels: BTreeMap<i32, Vec<AggregateElement>>,
    choices: Option<choices::Costs>,
    statistics: ObjectiveBoundStatistics,
}
impl ObjectivePlan {
    /// Prepare necessary nonnegative choice costs without changing exact scores.
    ///
    /// A required group prepays its least certified objective weight under its
    /// activation; allocated keys retain their excess over that weight. The same
    /// checked allocation also supplies a minimum-layer cost that excludes cheaper
    /// keys. Independent lower-cost comparisons are conjoined, never added.
    /// Groups never share a coalesced objective key.
    /// Unsupported bridges remain unproved, not false. The result is optional:
    /// refusal preserves the exact plan and any preceding successful preparation.
    /// Success and error statistics count this attempt separately from [`Self::statistics`].
    /// `max_bytes` bounds source premises, retained group costs and temporary
    /// proof/index capacities together. It is an independent allowance, not RSS.
    ///
    /// # Errors
    /// Reports foreign premises, cancellation, allocation or preparation limits.
    pub fn prepare_choice_bounds(
        &mut self,
        required: &crate::RequiredChoices,
        limits: ObjectivePlanLimits,
        max_bytes: u128,
        cancellation: &Cancellation,
    ) -> Result<ObjectiveBoundStatistics, ObjectiveBoundError> {
        let (costs, statistics) =
            choices::prepare(self, required, limits, max_bytes, cancellation)?;
        self.choices = costs;
        Ok(statistics)
    }

    /// Source groups whose costs strengthen the optional incumbent bound.
    #[must_use]
    pub fn choice_bound_groups(&self) -> usize {
        self.choices.as_ref().map_or(0, |costs| costs.groups.len())
    }
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
    /// Compile exact objective eligibility from an authenticated source owner.
    ///
    /// Preparation selects every signed predicate read by a positive objective
    /// pattern or a closed condition, then reuses the ordinary compiler. The
    /// selected atom ceiling applies to this read set, while formula atom nodes
    /// retain original source positions and bounds retain the full universe.
    /// Source admission establishes the complete catalog's unique identities;
    /// this handle cannot be constructed from an arbitrary catalog.
    ///
    /// `max_projection_bytes` bounds simultaneous read-set, selected-position
    /// and index/sort storage, including their headers and actual capacities.
    /// It does not replace the existing eligibility, key or binding limits.
    /// All scans, comparisons, shifts and materialization use `limits.max_work`
    /// and poll cancellation. A refusal retains its performed-work receipt and
    /// observed projection peak; no partially constructed plan is returned.
    /// Optional choice strengthening remains a separate operation and may
    /// decline its full-universe limit without discarding this exact plan.
    ///
    /// The returned attempt retains a receipt on success and refusal. Its small
    /// typed outcome contains a completed plan, an existing compiler refusal or
    /// a projection byte refusal. No partially prepared plan is published.
    pub fn new_projected(
        source: ObjectiveSource<'_>,
        limits: ObjectivePlanLimits,
        max_projection_bytes: u128,
        cancellation: &Cancellation,
    ) -> ObjectiveProjectionAttempt {
        projected::compile(source, limits, max_projection_bytes, cancellation)
    }
    /// Original theory instance, unchanged by planning.
    #[must_use]
    pub fn original(&self) -> &Theory {
        &self.original
    }
    /// This plan's identity, fixed when it was compiled. Its bounds are
    /// installed in a candidate stream under it, which compares their scores
    /// only with other bounds of the same plan.
    #[must_use]
    pub const fn identity(&self) -> PlanIdentity {
        self.identity
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
    /// Prepared choice consequences may exclude additional interpretations
    /// outside the original theory. A better incumbent's bound implies an
    /// earlier bound under that original theory, including when an optional
    /// strengthening fails and returns only the exact cost condition.
    ///
    /// # Errors
    /// Refuses bounded exact lowering, cancellation, storage or work limits.
    pub fn bound(
        &self,
        incumbent: &Score,
        limits: ObjectiveBoundLimits,
        cancellation: &Cancellation,
    ) -> Result<ObjectiveBound, ObjectiveBoundError> {
        bound::compile(
            self,
            incumbent,
            bound::Comparison::AtMost,
            limits,
            cancellation,
        )
    }

    /// Compile lexicographic cost strictly less than `incumbent`, so that no
    /// tie at the incumbent's cost survives, under the same conditions as
    /// [`Self::bound`]. A better incumbent's strict bound implies an earlier
    /// strict or non-strict bound under the original theory, as non-strict
    /// bounds do. The optional choice strengthening compares certified lower
    /// costs strictly too: a model's lower cost never exceeds its cost.
    ///
    /// # Errors
    /// As [`Self::bound`].
    pub fn strict_bound(
        &self,
        incumbent: &Score,
        limits: ObjectiveBoundLimits,
        cancellation: &Cancellation,
    ) -> Result<ObjectiveBound, ObjectiveBoundError> {
        bound::compile(
            self,
            incumbent,
            bound::Comparison::Below,
            limits,
            cancellation,
        )
    }
}

/// A separate candidate-only constraint with explicit original-theory identity.
#[derive(Debug)]
pub struct ObjectiveBound {
    original: Theory,
    theory: Theory,
    statistics: ObjectiveBoundStatistics,
    choice_failure: Option<ObjectiveBoundError>,
}
impl ObjectiveBound {
    /// An optional grouped bound stopped; the exact incumbent bound remains.
    #[must_use]
    pub const fn choice_failure(&self) -> Option<ObjectiveBoundError> {
        self.choice_failure
    }
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
    meter: Meter<'static>,
    bindings: u64,
    keys: usize,
    key_bytes: usize,
    nodes: usize,
}
impl<'a> Work<'a> {
    fn new(limits: ObjectivePlanLimits, cancellation: &'a Cancellation) -> Self {
        Self {
            cancellation,
            limits,
            template: None,
            meter: Meter::local(limits.max_work),
            bindings: 0,
            keys: 0,
            key_bytes: 0,
            nodes: 0,
        }
    }
    fn statistics(&self) -> ObjectiveBoundStatistics {
        ObjectiveBoundStatistics {
            work: self.meter.charged(),
            bindings: self.bindings,
            keys: self.keys,
            key_bytes: self.key_bytes,
            nodes: self.nodes,
        }
    }
    fn error(&self, kind: ObjectiveBoundErrorKind) -> ObjectiveBoundError {
        ObjectiveBoundError {
            kind,
            template: self.template,
            statistics: self.statistics(),
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
        // The native contract distinguishes an unrepresentable total from an
        // exhausted allowance; the shared meter owns the accepted count.
        if self.meter.charged().checked_add(amount).is_none() {
            return Err(self.error(ObjectiveBoundErrorKind::Overflow));
        }
        self.meter
            .bounded(amount, || ((), amount))
            .map_err(|failure| match failure.into_stop() {
                Stop::WorkLimit => self.limit(ObjectiveBoundResource::Work),
                stop => self.error(ObjectiveBoundErrorKind::Control(stop)),
            })
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
    fn node(
        &mut self,
        nodes: &mut FormulaNodes,
        node: NodeView<'_>,
    ) -> Result<usize, ObjectiveBoundError> {
        self.tick()?;
        let (added_nodes, edges) = match node {
            NodeView::And([]) => (2, 2),
            NodeView::And([_]) | NodeView::Or([_]) => (0, 0),
            NodeView::And(row) | NodeView::Or(row) => (1, row.len()),
            NodeView::Threshold(row) => (1, row.conditions.len()),
            NodeView::ExtremumCut(cut) => (1, cut.row.conditions.len()),
            NodeView::Implies(_, _) => (1, 2),
            NodeView::Atom(_) | NodeView::False => (1, 0),
        };
        Meter::check_count(nodes.view().len(), added_nodes, self.limits.max_nodes).map_err(
            |error| match error {
                MeterCountError::Overflow => self.error(ObjectiveBoundErrorKind::Overflow),
                MeterCountError::Limit { .. } => self.limit(ObjectiveBoundResource::Nodes),
            },
        )?;
        Meter::check_count(nodes.parts().occurrences(), edges, self.limits.max_operands).map_err(
            |error| match error {
                MeterCountError::Overflow => self.error(ObjectiveBoundErrorKind::Overflow),
                MeterCountError::Limit { .. } => self.limit(ObjectiveBoundResource::Operands),
            },
        )?;
        // Charge child validation and copied arena cells before publication.
        // Numeric copies include both columns and row/threshold descriptors.
        // The range-returning append allocates no root vector. The builder has
        // no independent work owner.
        let copies = match node {
            NodeView::And([_]) | NodeView::Or([_]) => 1,
            NodeView::And(row) | NodeView::Or(row) if row.len() >= 3 => row.len(),
            NodeView::Threshold(row) => row
                .conditions
                .len()
                .checked_mul(2)
                .and_then(|cells| cells.checked_add(2))
                .ok_or_else(|| self.error(ObjectiveBoundErrorKind::Overflow))?,
            NodeView::ExtremumCut(cut) => cut
                .row
                .conditions
                .len()
                .checked_mul(2)
                .and_then(|cells| cells.checked_add(2))
                .ok_or_else(|| self.error(ObjectiveBoundErrorKind::Overflow))?,
            _ => 0,
        };
        if matches!(node, NodeView::Threshold(_) | NodeView::ExtremumCut(_)) {
            self.charge(
                u64::try_from(nodes.parts().pending_numeric_validation_work())
                    .map_err(|_| self.error(ObjectiveBoundErrorKind::Overflow))?,
            )?;
        }
        // Ordinal publication also validates its independently supplied cut rule.
        if matches!(node, NodeView::ExtremumCut(_)) {
            self.tick()?;
        }
        for _ in 0..edges {
            self.tick()?;
        }
        for _ in 0..copies {
            self.tick()?;
        }
        let mut transaction = nodes.transaction();
        let index = transaction
            .push(node, self.limits.max_nodes, self.limits.max_operands)
            .map_err(|error| {
                self.error(match error {
                    zetesis_ferraris::AdmissionError::Allocation => {
                        ObjectiveBoundErrorKind::Allocation
                    }
                    _ => ObjectiveBoundErrorKind::Theory(error),
                })
            })?;
        transaction.commit();
        self.nodes = nodes.view().len();
        Ok(index)
    }
}
