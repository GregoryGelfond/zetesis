//! Closed model queries: owned construction descriptions become ID-only nodes
//! over one canonical atom catalog before objective execution.

use std::{convert::Infallible, fmt, iter::FusedIterator, sync::Arc};

use zetesis_core::atom_interner::{AtomInterner, Failure as InternFailure, Limits as InternLimits};
use zetesis_core::catalog::{AtomRef, Error as CatalogError};
use zetesis_core::{Atom, AtomCatalog};

use crate::{AdmissionError, AdmissionLimits, AdmissionResource};

/// One operation in an acyclic query over a complete model.
/// Every operand index must precede the operation that references it. The
/// default atom operand is an owned construction description; admitted reads
/// borrow an `AtomRef` and canonical storage retains only an occurrence index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConditionNode<A = Atom> {
    /// Constant logical truth.
    Boolean(bool),
    /// Membership of a complete typed atom, including its classical sign.
    Atom(A),
    /// Logical negation of an earlier result.
    Not(usize),
    /// Conjunction of earlier results.
    And(usize, usize),
    /// Disjunction of earlier results.
    Or(usize, usize),
}

/// Atom operands are positions in the exact catalog supplied to construction.
pub type ConditionIndex = ConditionNode<usize>;
/// A logical operation borrowing its complete atom identity.
pub type ConditionNodeRef<'a> = ConditionNode<AtomRef<'a>>;

impl<A> ConditionNode<A> {
    fn as_ref(&self) -> ConditionNode<&A> {
        match self {
            Self::Boolean(value) => ConditionNode::Boolean(*value),
            Self::Atom(atom) => ConditionNode::Atom(atom),
            Self::Not(operand) => ConditionNode::Not(*operand),
            Self::And(left, right) => ConditionNode::And(*left, *right),
            Self::Or(left, right) => ConditionNode::Or(*left, *right),
        }
    }
    fn try_map<B, E>(self, atom: impl FnOnce(A) -> Result<B, E>) -> Result<ConditionNode<B>, E> {
        Ok(match self {
            Self::Boolean(value) => ConditionNode::Boolean(value),
            Self::Atom(value) => ConditionNode::Atom(atom(value)?),
            Self::Not(operand) => ConditionNode::Not(operand),
            Self::And(left, right) => ConditionNode::And(left, right),
            Self::Or(left, right) => ConditionNode::Or(left, right),
        })
    }
}

/// Invalid catalog coordinates, backward references or named storage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConditionError {
    /// An atom occurrence is outside the supplied catalog.
    AtomReference {
        /// Operation containing the occurrence.
        node: usize,
        /// Supplied occurrence position.
        atom: usize,
        /// Catalog occurrence count.
        atoms: usize,
    },
    /// An operation references itself or a later operation.
    Reference {
        /// Operation containing the reference.
        node: usize,
        /// Operand that must precede it.
        operand: usize,
    },
    /// Canonical or node storage could not be admitted.
    Storage(CatalogError),
}
impl fmt::Display for ConditionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AtomReference { node, atom, atoms } => write!(
                f,
                "condition node {node} references atom {atom} outside catalog of {atoms} atoms"
            ),
            Self::Reference { node, operand } => write!(
                f,
                "condition node {node} references nonpreceding operand {operand}"
            ),
            Self::Storage(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for ConditionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Storage(error) => Some(error),
            _ => None,
        }
    }
}

/// Checked condition construction preserves the caller's exact stop cause.
#[derive(Debug)]
pub enum ConditionFailure<E> {
    /// Invalid coordinates or unavailable storage.
    Condition(ConditionError),
    /// The caller refused before the next operation.
    Stopped(E),
}
impl<E: fmt::Display> fmt::Display for ConditionFailure<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Condition(error) => error.fmt(f),
            Self::Stopped(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for ConditionFailure<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Condition(error) => Some(error),
            Self::Stopped(error) => Some(error),
        }
    }
}

#[derive(Clone)]
enum Source {
    Ingress(Vec<ConditionNode>),
    Canonical(Arc<Canonical>),
}
struct Canonical {
    atoms: AtomCatalog,
    nodes: Vec<ConditionIndex>,
}

/// A closed model query whose final node is its result; an empty query is true.
/// `new` retains construction input until `ObjectiveProgram` admission consumes
/// it. Every admitted condition stores ID-only nodes over shared canonical
/// payload. Catalog-backed construction preserves the supplied authority.
/// Cloning an admitted condition shares all its nodes and payload in O(1).
#[derive(Clone)]
pub struct Condition(Source);
impl Default for Condition {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}
impl Condition {
    /// Assemble owned input for subsequent objective-program admission.
    /// This operation does not establish valid references or execution storage.
    #[must_use]
    pub fn new(nodes: Vec<ConditionNode>) -> Self {
        Self(Source::Ingress(nodes))
    }

    /// Validate ID-only nodes over the supplied exact atom occurrence catalog.
    /// No payload is copied. `max_bytes` includes the complete retained catalog,
    /// node capacity and shared condition envelope; allocator bookkeeping and
    /// Arc counters are excluded. Caller input buffers are already allocated.
    /// Work callbacks precede each node, reference and catalog-measure step.
    ///
    /// # Errors
    /// Returns invalid atom/backward coordinates, storage refusal, or the exact
    /// caller stop. No partial condition escapes; the supplied values are consumed.
    pub fn from_catalog_with<E>(
        atoms: AtomCatalog,
        nodes: Vec<ConditionIndex>,
        max_bytes: usize,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<Self, ConditionFailure<E>> {
        for (index, node) in nodes.iter().enumerate() {
            before().map_err(ConditionFailure::Stopped)?;
            let operands = match node {
                ConditionNode::Boolean(_) => 0,
                ConditionNode::Atom(_) | ConditionNode::Not(_) => 1,
                ConditionNode::And(..) | ConditionNode::Or(..) => 2,
            };
            for _ in 0..operands {
                before().map_err(ConditionFailure::Stopped)?;
            }
            check_node(node.as_ref(), index, |id| {
                if *id < atoms.atoms().len() {
                    Ok(())
                } else {
                    Err(ConditionError::AtomReference {
                        node: index,
                        atom: *id,
                        atoms: atoms.atoms().len(),
                    })
                }
            })
            .map_err(ConditionError::from)
            .map_err(ConditionFailure::Condition)?;
        }
        let bytes = atoms
            .storage_with(&mut before)
            .map_err(ConditionFailure::Stopped)?
            .bytes
            + node_bytes(nodes.capacity());
        storage_bound(bytes, max_bytes).map_err(ConditionFailure::Condition)?;
        before().map_err(ConditionFailure::Stopped)?;
        Ok(Self(Source::Canonical(Arc::new(Canonical {
            atoms,
            nodes,
        }))))
    }

    /// Borrow logical operations without materializing canonical atoms.
    #[must_use]
    pub fn nodes(&self) -> ConditionNodes<'_> {
        ConditionNodes(&self.0)
    }

    pub(crate) fn validate(
        &self,
        limits: AdmissionLimits,
        template: usize,
    ) -> Result<(), AdmissionError> {
        super::program::check_bound(
            AdmissionResource::ConditionNodes,
            self.nodes().len(),
            limits.max_condition_nodes,
            Some(template),
        )?;
        for (index, node) in self.nodes().iter().enumerate() {
            check_node(node, index, |atom| {
                super::program::check_bound(
                    AdmissionResource::PredicateArity,
                    atom.values().len(),
                    limits.max_predicate_arity,
                    Some(template),
                )
            })
            .map_err(|error| match error {
                NodeError::Atom(error) => error,
                NodeError::Reference { node, operand } => AdmissionError::ConditionReference {
                    template,
                    node,
                    operand,
                },
            })?;
        }
        Ok(())
    }

    /// Owned atom descriptions still awaiting the objective's shared vocabulary.
    pub(crate) fn ingress_atoms(&self) -> impl Iterator<Item = AtomRef<'_>> {
        let nodes = match &self.0 {
            Source::Ingress(nodes) => nodes.as_slice(),
            Source::Canonical(_) => &[],
        };
        nodes.iter().filter_map(|node| match node {
            ConditionNode::Atom(atom) => Some(atom.into()),
            _ => None,
        })
    }

    pub(crate) fn canonical_catalog(&self) -> Option<&AtomCatalog> {
        match &self.0 {
            Source::Canonical(data) => Some(&data.atoms),
            Source::Ingress(_) => None,
        }
    }

    /// Named storage of canonical condition nodes, excluding their atom catalog.
    /// For a catalog-backed condition this is its shared condition envelope and
    /// actual node-buffer capacity, counted once across clones. Allocator
    /// bookkeeping and Arc counters are excluded. Owned construction input reports
    /// the proposed canonical envelope and one cell per node; this does not measure
    /// its still-owned atom payload or spare ingress capacity.
    #[must_use]
    pub fn node_storage_bytes(&self) -> u128 {
        match &self.0 {
            Source::Canonical(data) => node_bytes(data.nodes.capacity()),
            Source::Ingress(nodes) => node_bytes(nodes.len()),
        }
    }

    pub(crate) fn same_node_owner(&self, other: &Self) -> bool {
        matches!((&self.0, &other.0), (Source::Canonical(left), Source::Canonical(right)) if Arc::ptr_eq(left, right))
    }

    /// Import rows into the objective's shared tuple authority. The supplied
    /// owner allowance excludes all other live objective metadata; this operation
    /// additionally admits its own node buffer before allocation and row import.
    pub(crate) fn prepare_in(
        self,
        owner: &mut AtomInterner,
        limits: InternLimits,
        node_limit: usize,
    ) -> Result<PendingCondition, ConditionError> {
        match self.0 {
            Source::Canonical(data) => {
                storage_bound(node_bytes(data.nodes.capacity()), node_limit)?;
                Ok(PendingCondition::Existing(Self(Source::Canonical(data))))
            }
            Source::Ingress(nodes) => {
                storage_bound(node_bytes(nodes.len()), node_limit)?;
                storage_bound_u128(
                    owner.storage_bytes() + node_bytes(nodes.len()),
                    limits.max_bytes,
                )?;
                let mut indices = Vec::new();
                indices
                    .try_reserve_exact(nodes.len())
                    .map_err(|_| ConditionError::Storage(CatalogError::Allocation))?;
                let extra = node_bytes(indices.capacity());
                storage_bound(extra, node_limit)?;
                storage_bound_u128(owner.storage_bytes() + extra, limits.max_bytes)?;
                let limits = InternLimits {
                    max_bytes: limits.max_bytes - extra,
                    ..limits
                };
                for node in nodes {
                    indices.push(node.try_map(|atom| {
                        owner
                            .entry_atom_with(&atom, limits, || Ok::<_, Infallible>(()))
                            .map_err(intern_error)?
                            .insert_with(limits, || Ok::<_, Infallible>(()))
                            .map_err(intern_error)
                    })?);
                }
                Ok(PendingCondition::Nodes(indices))
            }
        }
    }
}

/// Private admission state: coordinates address the single writer used by
/// `prepare_in`, and finish receives that writer's discovery-order publication.
pub(crate) enum PendingCondition {
    Existing(Condition),
    Nodes(Vec<ConditionIndex>),
}
impl PendingCondition {
    pub(crate) fn node_storage_bytes(&self) -> u128 {
        match self {
            Self::Existing(condition) => condition.node_storage_bytes(),
            Self::Nodes(nodes) => node_bytes(nodes.capacity()),
        }
    }
    pub(crate) fn finish(self, atoms: &AtomCatalog) -> Result<Condition, ConditionError> {
        match self {
            Self::Existing(condition) => Ok(condition),
            Self::Nodes(nodes) => {
                for (index, node) in nodes.iter().enumerate() {
                    check_node(node.as_ref(), index, |id| {
                        if *id < atoms.atoms().len() {
                            Ok(())
                        } else {
                            Err(ConditionError::AtomReference {
                                node: index,
                                atom: *id,
                                atoms: atoms.atoms().len(),
                            })
                        }
                    })
                    .map_err(ConditionError::from)?;
                }
                Ok(Condition(Source::Canonical(Arc::new(Canonical {
                    atoms: atoms.clone(),
                    nodes,
                }))))
            }
        }
    }
}

// One reference check serves both owned input admission and catalog construction.
enum NodeError<E> {
    Atom(E),
    Reference { node: usize, operand: usize },
}
fn check_node<A, E>(
    node: ConditionNode<A>,
    index: usize,
    atom: impl FnOnce(A) -> Result<(), E>,
) -> Result<(), NodeError<E>> {
    let reference = |operand| {
        if operand < index {
            Ok(())
        } else {
            Err(NodeError::Reference {
                node: index,
                operand,
            })
        }
    };
    match node {
        ConditionNode::Boolean(_) => Ok(()),
        ConditionNode::Atom(value) => atom(value).map_err(NodeError::Atom),
        ConditionNode::Not(operand) => reference(operand),
        ConditionNode::And(left, right) | ConditionNode::Or(left, right) => {
            reference(left)?;
            reference(right)
        }
    }
}
impl<E> From<NodeError<E>> for ConditionError
where
    E: Into<ConditionError>,
{
    fn from(error: NodeError<E>) -> Self {
        match error {
            NodeError::Atom(error) => error.into(),
            NodeError::Reference { node, operand } => Self::Reference { node, operand },
        }
    }
}

fn node_bytes(capacity: usize) -> u128 {
    size_of::<Canonical>() as u128 + capacity as u128 * size_of::<ConditionIndex>() as u128
}
fn storage_bound(required: u128, limit: usize) -> Result<(), ConditionError> {
    if required > limit as u128 {
        Err(ConditionError::Storage(CatalogError::Storage {
            required,
            limit,
        }))
    } else {
        Ok(())
    }
}

fn storage_bound_u128(required: u128, limit: u128) -> Result<(), ConditionError> {
    if required > limit {
        Err(ConditionError::Storage(CatalogError::Storage {
            required,
            limit: usize::try_from(limit).unwrap_or(usize::MAX),
        }))
    } else {
        Ok(())
    }
}

fn intern_error(error: InternFailure<Infallible>) -> ConditionError {
    ConditionError::Storage(match error {
        InternFailure::Catalog(error) => error,
        InternFailure::Allocation(_) => CatalogError::Allocation,
        InternFailure::Bytes { required, limit } => CatalogError::Storage {
            required,
            limit: usize::try_from(limit).unwrap_or(usize::MAX),
        },
        InternFailure::Overflow | InternFailure::Atoms { .. } => CatalogError::Overflow,
        InternFailure::Stopped(never) => match never {},
    })
}

/// A borrowed logical node sequence, preserving the original operation order.
#[derive(Clone, Copy)]
pub struct ConditionNodes<'a>(&'a Source);
impl<'a> ConditionNodes<'a> {
    /// Number of query operations.
    #[must_use]
    pub fn len(self) -> usize {
        match self.0 {
            Source::Ingress(nodes) => nodes.len(),
            Source::Canonical(data) => data.nodes.len(),
        }
    }
    /// Whether the query is the empty, true query.
    #[must_use]
    pub fn is_empty(self) -> bool {
        self.len() == 0
    }
    /// Borrow one operation, or return absence outside the sequence.
    ///
    /// # Panics
    /// Panics if a canonical condition refers outside its retained atom catalog.
    /// Condition publication validates every occurrence before exposing a view.
    #[must_use]
    pub fn at(self, index: usize) -> Option<ConditionNodeRef<'a>> {
        let result = match self.0 {
            Source::Ingress(nodes) => nodes
                .get(index)?
                .as_ref()
                .try_map(|atom| Ok::<_, Infallible>(atom.into())),
            Source::Canonical(data) => data.nodes.get(index)?.as_ref().try_map(|id| {
                Ok::<_, Infallible>(
                    data.atoms
                        .atoms()
                        .at(*id)
                        .expect("validated condition occurrence"),
                )
            }),
        };
        Some(match result {
            Ok(node) => node,
            Err(never) => match never {},
        })
    }
    /// Traverse the sequence; cloning the cursor retains only its borrow.
    #[must_use]
    pub fn iter(self) -> ConditionIter<'a> {
        ConditionIter {
            nodes: self,
            index: 0,
        }
    }
}
impl<'a> IntoIterator for ConditionNodes<'a> {
    type Item = ConditionNodeRef<'a>;
    type IntoIter = ConditionIter<'a>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
/// Exact forward traversal of borrowed condition operations.
#[derive(Clone)]
pub struct ConditionIter<'a> {
    nodes: ConditionNodes<'a>,
    index: usize,
}
impl<'a> Iterator for ConditionIter<'a> {
    type Item = ConditionNodeRef<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        let node = self.nodes.at(self.index)?;
        self.index += 1;
        Some(node)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.nodes.len() - self.index;
        (remaining, Some(remaining))
    }
}
impl ExactSizeIterator for ConditionIter<'_> {}
impl FusedIterator for ConditionIter<'_> {}
impl fmt::Debug for Condition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.nodes()).finish()
    }
}
impl PartialEq for Condition {
    fn eq(&self, other: &Self) -> bool {
        self.nodes().len() == other.nodes().len() && self.nodes().iter().eq(other.nodes())
    }
}
impl Eq for Condition {}

#[cfg(test)]
mod tests;
