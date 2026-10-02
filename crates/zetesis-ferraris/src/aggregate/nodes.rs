use std::ops::Deref;

use zetesis_core::{Value, catalog::TermRef};
use zetesis_cpu::Cancellation;

use super::lower::Destination;
use super::{
    AggregateBuild, AggregateComparison, AggregateElement, AggregateError, AggregateExtremum,
    AggregateFamilyBuild, AggregateFamilyLimits, AggregateGuard, AggregateLimits, ExtremumBound,
    ValueExtremumElement, extremum, family, lower, value_extremum,
};
use crate::Node;

/// Owned formula nodes with reusable, completed aggregate-prefix checks.
///
/// Construction and [`Self::push`] accept unchecked nodes. Aggregate compilation
/// validates backward edges in the unchecked suffix before using any condition;
/// atom-universe and root admission remain [`crate::Theory::new`]'s responsibility.
/// The buffer owns one vector and a scan frontier. Read-only slice access cannot
/// change checked nodes, and removing a suffix clamps the frontier to what remains.
/// Extracting the vector discards this evidence; wrapping it again starts unchecked.
///
/// The compiler methods have the corresponding free functions' reduct semantics,
/// input obligations, node ceilings and transactional failure behavior. They save
/// one work unit per previously checked node. Each call still charges its initial
/// control poll/node-ceiling check, scans every unchecked node, and charges all
/// aggregate-specific work. A scan publishes its frontier only on completion;
/// that completed check can survive a later compilation failure. Appended compiler
/// nodes are checked on the next call, with no privileged topology assumption.
/// Refusals retain their actual work and restore the original nodes and length;
/// reserved vector capacity may change.
///
/// The ordinary vector editing operations below have vector allocation behavior
/// and do not consume an aggregate budget. Compiler appends remain fallible and
/// metered. No mutable node access is exposed:
///
/// ```compile_fail
/// use zetesis_ferraris::{FormulaNodes, Node};
/// let mut nodes = FormulaNodes::new(vec![Node::False]);
/// nodes[0] = Node::And(0, 0);
/// ```
#[derive(Debug, Default)]
pub struct FormulaNodes {
    nodes: Vec<Node>,
    validated: usize,
}

impl FormulaNodes {
    /// Take ownership without scanning or copying the node vector.
    #[must_use]
    pub const fn new(nodes: Vec<Node>) -> Self {
        Self {
            nodes,
            validated: 0,
        }
    }

    /// Allocated node capacity, excluding this owner's inline scan frontier.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.nodes.capacity()
    }

    /// Append one unchecked node, preserving the unchanged checked prefix.
    pub fn push(&mut self, node: Node) {
        self.nodes.push(node);
    }

    /// Remove nodes after `len`, discarding their validation evidence.
    pub fn truncate(&mut self, len: usize) {
        self.nodes.truncate(len);
        self.validated = self.validated.min(self.nodes.len());
    }

    /// Move the suffix out, retaining evidence only for the unchanged prefix.
    /// The returned nodes retain their original absolute child indices.
    ///
    /// # Panics
    /// Panics if `at` exceeds the current length, as [`Vec::split_off`] does.
    #[must_use]
    pub fn split_off(&mut self, at: usize) -> Vec<Node> {
        let suffix = self.nodes.split_off(at);
        self.validated = self.validated.min(at);
        suffix
    }

    /// Return the vector without copying, discarding its validation frontier.
    #[must_use]
    pub fn into_vec(self) -> Vec<Node> {
        self.nodes
    }

    /// Append the scalar formula of [`super::append_aggregate`], reusing checked
    /// prefix nodes under this owner's exclusive mutation boundary.
    ///
    /// # Errors
    /// Returns the free function's typed failures with actual work accounting.
    pub fn append_aggregate(
        &mut self,
        elements: &[AggregateElement],
        comparison: AggregateComparison,
        bound: i64,
        limits: AggregateLimits,
        cancellation: &Cancellation,
    ) -> Result<AggregateBuild, AggregateError> {
        lower::append(
            self.destination(),
            elements,
            comparison,
            bound,
            limits,
            cancellation,
        )
    }

    /// Append the ordered roots of [`super::append_aggregate_family`], reusing
    /// completed prefix validation across independent families.
    ///
    /// # Errors
    /// Returns the free function's typed failures. Its limits remain cumulative
    /// across this family; completed earlier calls do not spend this call's budget.
    pub fn append_aggregate_family(
        &mut self,
        elements: &[AggregateElement],
        guards: &[AggregateGuard],
        limits: AggregateFamilyLimits,
        cancellation: &Cancellation,
    ) -> Result<AggregateFamilyBuild, AggregateError> {
        family::append(self.destination(), elements, guards, limits, cancellation)
    }

    /// Append the numeric extremum of [`super::append_extremum`], reusing
    /// completed prefix validation.
    ///
    /// # Errors
    /// Returns the free function's typed failures with actual work accounting.
    pub fn append_extremum(
        &mut self,
        elements: &[AggregateElement],
        extremum: AggregateExtremum,
        comparison: AggregateComparison,
        bound: impl Into<ExtremumBound>,
        limits: AggregateLimits,
        cancellation: &Cancellation,
    ) -> Result<AggregateBuild, AggregateError> {
        extremum::append(
            self.destination(),
            elements,
            extremum,
            comparison,
            bound.into(),
            limits,
            cancellation,
        )
    }

    /// Append the typed extremum of [`super::append_value_extremum`], reusing
    /// completed prefix validation.
    ///
    /// # Errors
    /// Returns the free function's typed failures with actual work accounting.
    pub fn append_value_extremum(
        &mut self,
        elements: &[ValueExtremumElement],
        extremum: AggregateExtremum,
        comparison: AggregateComparison,
        bound: &Value,
        limits: AggregateLimits,
        cancellation: &Cancellation,
    ) -> Result<AggregateBuild, AggregateError> {
        self.append_value_extremum_refs(
            elements.iter().map(|element| ValueExtremumElement {
                value: (&element.value).into(),
                condition: element.condition,
            }),
            extremum,
            comparison,
            bound.into(),
            limits,
            cancellation,
        )
    }

    /// Append the borrowed typed extremum of [`super::append_value_extremum_refs`].
    /// Canonical terms keep their original authority; only node-prefix validation
    /// is reused, with every typed comparison still charged.
    ///
    /// # Errors
    /// Returns the free function's typed failures with actual work accounting.
    pub fn append_value_extremum_refs<'a>(
        &mut self,
        elements: impl Iterator<Item = ValueExtremumElement<TermRef<'a>>>,
        extremum: AggregateExtremum,
        comparison: AggregateComparison,
        bound: TermRef<'_>,
        limits: AggregateLimits,
        cancellation: &Cancellation,
    ) -> Result<AggregateBuild, AggregateError> {
        value_extremum::append(
            self.destination(),
            elements,
            extremum,
            comparison,
            bound,
            limits,
            cancellation,
        )
    }

    fn destination(&mut self) -> Destination<'_> {
        Destination::retained(&mut self.nodes, &mut self.validated)
    }
}

impl Deref for FormulaNodes {
    type Target = [Node];

    fn deref(&self) -> &Self::Target {
        &self.nodes
    }
}
