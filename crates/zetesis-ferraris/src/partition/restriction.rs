//! Bounded presentation of conditional lower bounds as a candidate-only theory.

use std::fmt;

use crate::{
    AdmissionLimits, AggregateComparison, AggregateElement, AggregateError, AggregateLimits, Node,
    Theory, append_aggregate,
};
use zetesis_cpu::{Cancellation, Stop};

use super::Plan;

/// Bounds for emitting a separate consequence theory, excluding the borrowed plan.
/// The existing aggregate translator also bounds each temporary state frontier.
#[derive(Clone, Copy, Debug)]
pub struct RestrictionLimits {
    /// Atom, final node and root ceilings; node limits also constrain translation.
    pub theory: AdmissionLimits,
    /// Per-group element/frontier ceilings. Its work ceiling applies separately
    /// to each translation, within the cumulative ceiling below.
    pub aggregate: AggregateLimits,
    /// Cumulative atom-node, root and aggregate-translation operations.
    pub max_work: u64,
}

impl Default for RestrictionLimits {
    fn default() -> Self {
        Self {
            theory: AdmissionLimits::default(),
            aggregate: AggregateLimits::default(),
            max_work: 10_000_000,
        }
    }
}

/// An incomplete consequence-theory construction; no partial theory is exposed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RestrictionErrorKind {
    /// The declared semantic universe exceeds the emission ceiling.
    Atoms,
    /// A formula node would exceed the emission ceiling.
    Nodes,
    /// An asserted consequence would exceed the emission ceiling.
    Roots,
    /// One group's temporary element vector exceeds its ceiling.
    Elements,
    /// Cumulative emission work reached its inclusive ceiling.
    Work,
    /// A bound or accounting quantity cannot be represented.
    Overflow,
    /// The independent exact cardinality translator refused its bounded work.
    Aggregate(AggregateError),
    /// Final theory admission refused the constructed shape.
    Theory(crate::AdmissionError),
    /// Cancellation or fallible allocation stopped emission.
    Stopped(Stop),
}

/// Typed emission refusal with its charged work prefix.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RestrictionError {
    kind: RestrictionErrorKind,
    work: u64,
}

impl RestrictionError {
    /// Specific refusal, never an accepted consequence or logical UNSAT.
    #[must_use]
    pub const fn kind(self) -> RestrictionErrorKind {
        self.kind
    }

    /// Charged emission and translation operations before refusal.
    #[must_use]
    pub const fn work(self) -> u64 {
        self.work
    }
}

impl fmt::Display for RestrictionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            RestrictionErrorKind::Aggregate(error) => error.fmt(f),
            RestrictionErrorKind::Theory(error) => error.fmt(f),
            RestrictionErrorKind::Stopped(stop) => stop.fmt(f),
            kind => write!(f, "partition consequence emission refused: {kind:?}"),
        }
    }
}

impl std::error::Error for RestrictionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.kind {
            RestrictionErrorKind::Aggregate(error) => Some(error),
            RestrictionErrorKind::Theory(error) => Some(error),
            RestrictionErrorKind::Stopped(stop) => Some(stop),
            _ => None,
        }
    }
}

/// An emitted lower-bound view, with no original-theory entailment certificate.
#[derive(Debug)]
pub struct Restriction {
    theory: Theory,
    work: u64,
}

impl Restriction {
    /// Consequence formulas over the caller's declared semantic atom indices.
    #[must_use]
    pub fn theory(&self) -> &Theory {
        &self.theory
    }

    /// Charged atom-node, root and aggregate-translation operations.
    #[must_use]
    pub const fn work(&self) -> u64 {
        self.work
    }
}

impl Plan {
    /// Emit only the derived local lower bounds as a separate Boolean theory.
    ///
    /// Inconsistent stated premises emit falsum, even for an empty partition.
    /// Otherwise zero lower bounds emit no root. The original theory is neither
    /// read nor changed. Applying this view to a candidate generator preserves
    /// complete original enumeration only if the original theory entails the
    /// supplied premises with exactly the same atom index meanings. The original
    /// theory must remain the subject of every frozen-reduct check.
    ///
    /// Emission reuses exact cardinality lowering. Temporary elements are bounded
    /// per group; nodes, roots, translator states and total work have independent
    /// ceilings. Translation costs include revalidating its existing DAG prefix.
    /// Final theory shape validation scans those bounded nodes and roots once
    /// more; it is not included in the logical work counter. No search occurs.
    ///
    /// # Errors
    /// Refuses emission/translation bounds, allocation or control. Failure drops
    /// the entire partial output and retains its charged work count.
    pub fn restriction(
        &self,
        limits: RestrictionLimits,
        cancellation: &Cancellation,
    ) -> Result<Restriction, RestrictionError> {
        let mut builder = Builder {
            nodes: Vec::new(),
            roots: Vec::new(),
            work: 0,
            limits,
            cancellation,
        };
        let result = builder.build(self);
        result
            .map(|theory| Restriction {
                theory,
                work: builder.work,
            })
            .map_err(|kind| RestrictionError {
                kind,
                work: builder.work,
            })
    }
}

struct Builder<'a> {
    nodes: Vec<Node>,
    roots: Vec<usize>,
    work: u64,
    limits: RestrictionLimits,
    cancellation: &'a Cancellation,
}

impl Builder<'_> {
    fn build(&mut self, plan: &Plan) -> Result<Theory, RestrictionErrorKind> {
        self.poll()?;
        if plan.atom_count() > self.limits.theory.max_atoms {
            return Err(RestrictionErrorKind::Atoms);
        }
        if plan.inconsistent() {
            let falsum = self.node(Node::False)?;
            self.root(falsum)?;
        } else {
            for group in plan.consequences() {
                self.tick()?;
                if group.lower != 0 {
                    let root = self.lower(group.members, group.lower)?;
                    self.root(root)?;
                }
            }
        }
        self.poll()?;
        let theory = Theory::new(
            plan.atom_count(),
            std::mem::take(&mut self.nodes),
            std::mem::take(&mut self.roots),
            self.limits.theory,
        )
        .map_err(RestrictionErrorKind::Theory)?;
        self.poll()?;
        Ok(theory)
    }

    fn lower(&mut self, members: &[usize], lower: usize) -> Result<usize, RestrictionErrorKind> {
        if members.len() > self.limits.aggregate.max_elements {
            return Err(RestrictionErrorKind::Elements);
        }
        let mut elements = Vec::new();
        elements
            .try_reserve_exact(members.len())
            .map_err(|_| RestrictionErrorKind::Stopped(Stop::Allocation))?;
        for &atom in members {
            let condition = self.node(Node::Atom(atom))?;
            elements.push(AggregateElement {
                condition,
                weight: 1,
            });
        }
        let bound = i64::try_from(lower).map_err(|_| RestrictionErrorKind::Overflow)?;
        let mut limits = self.limits.aggregate;
        limits.max_nodes = limits.max_nodes.min(self.limits.theory.max_nodes);
        limits.max_work = limits.max_work.min(self.limits.max_work - self.work);
        let result = append_aggregate(
            &mut self.nodes,
            &elements,
            AggregateComparison::Ge,
            bound,
            limits,
            self.cancellation,
        );
        let work = match result {
            Ok(build) => build.statistics().work,
            Err(error) => error.statistics().work,
        };
        self.work = self
            .work
            .checked_add(work)
            .ok_or(RestrictionErrorKind::Overflow)?;
        result
            .map(crate::AggregateBuild::root)
            .map_err(RestrictionErrorKind::Aggregate)
    }

    fn poll(&self) -> Result<(), RestrictionErrorKind> {
        self.cancellation
            .poll()
            .map_err(RestrictionErrorKind::Stopped)
    }

    fn tick(&mut self) -> Result<(), RestrictionErrorKind> {
        self.poll()?;
        if self.work == self.limits.max_work {
            return Err(RestrictionErrorKind::Work);
        }
        self.work += 1;
        Ok(())
    }

    fn node(&mut self, node: Node) -> Result<usize, RestrictionErrorKind> {
        self.tick()?;
        if self.nodes.len() == self.limits.theory.max_nodes {
            return Err(RestrictionErrorKind::Nodes);
        }
        self.nodes
            .try_reserve(1)
            .map_err(|_| RestrictionErrorKind::Stopped(Stop::Allocation))?;
        let index = self.nodes.len();
        self.nodes.push(node);
        Ok(index)
    }

    fn root(&mut self, root: usize) -> Result<(), RestrictionErrorKind> {
        self.tick()?;
        if self.roots.len() == self.limits.theory.max_roots {
            return Err(RestrictionErrorKind::Roots);
        }
        self.roots
            .try_reserve(1)
            .map_err(|_| RestrictionErrorKind::Stopped(Stop::Allocation))?;
        self.roots.push(root);
        Ok(())
    }
}
