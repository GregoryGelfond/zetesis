//! Fresh-scope derived roots with borrowed input edges and shared typed lanes.
//! Registration hashes expanded preorder through the ordinary term cursor; a
//! repeated subtree can therefore be visited repeatedly. Every visit is metered.

use std::{fmt, mem::size_of};

use super::{
    Failure, Fault, Read, TermId, TextId, VocabularyScope,
    budget::Budget,
    control::Work,
    index::Index,
    nodes::{Kind, Measures, Nodes},
};
use crate::{
    ValueNodeRef,
    catalog::{
        AssignmentError, AssignmentFailure, CatalogRead, ConstructorData, DeclaredConstructor,
        ReadError, TermKey, TermRead, TermRef,
    },
};

mod admission;

/// Failure of a derived-term operation, preserving the caller's stop value.
#[derive(Debug)]
pub enum DerivedFailure<E> {
    /// Logical construction, named capacity, allocation, or shape refusal.
    Storage(Fault),
    /// An input or declaration is foreign or outside its registered prefix.
    Read(ReadError),
    /// A child assignment is foreign, unbound, or outside its slot extent.
    Assignment(AssignmentError),
    /// The caller stopped before the next operation.
    Stopped(E),
}
impl<E> From<Fault> for DerivedFailure<E> {
    fn from(error: Fault) -> Self {
        Self::Storage(error)
    }
}
impl<E> From<ReadError> for DerivedFailure<E> {
    fn from(error: ReadError) -> Self {
        Self::Read(error)
    }
}
impl<E> From<Failure<E>> for DerivedFailure<E> {
    fn from(error: Failure<E>) -> Self {
        match error {
            Failure::Storage(error) => Self::Storage(error),
            Failure::Stopped(error) => Self::Stopped(error),
        }
    }
}
impl<E> From<AssignmentFailure<E>> for DerivedFailure<E> {
    fn from(error: AssignmentFailure<E>) -> Self {
        match error {
            AssignmentFailure::Assignment(error) => Self::Assignment(error),
            AssignmentFailure::Stopped(error) => Self::Stopped(error),
        }
    }
}
impl<E: fmt::Display> fmt::Display for DerivedFailure<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(error) => error.fmt(f),
            Self::Read(error) => error.fmt(f),
            Self::Assignment(error) => error.fmt(f),
            Self::Stopped(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for DerivedFailure<E> {}

#[derive(Clone, Copy, Debug)]
struct InputText {
    input: usize,
    id: TextId,
}
#[derive(Clone, Copy, Debug)]
enum Root {
    Input { input: usize, id: TermId },
    Node(usize),
}

/// A bounded append arena over exact immutable input prefixes. Registered input
/// roots retain coordinates only; new compounds retain local child IDs and
/// borrowed name coordinates. No input payload is copied or recursively owned.
///
/// Its fresh identity scope canonicalizes all registered aliases and constructed
/// roots by exact typed content. Inputs remain alive for this arena's lifetime.
/// A read borrow prevents mutation; retained keys own identity witnesses only.
///
/// The physical ceiling counts this header and named buffer/index capacities,
/// including actual old/replacement overlap. It excludes borrowed input owners,
/// allocator overhead and fixed identity-allocation overhead. This is not RSS.
/// Logical limits apply separately to each new construction. Failed operations
/// can retain reserved capacity, but never a partial or unindexed root. Callback
/// panics have the same complete-prefix guarantee when caught by the caller.
#[derive(Debug)]
pub struct DerivedTerms<'inputs> {
    scope: VocabularyScope,
    inputs: Vec<Read<'inputs>>,
    roots: Vec<Root>,
    nodes: Nodes<TermId, InputText>,
    symbol_names: Vec<InputText>,
    index: Index,
    children: Vec<TermId>,
    budget: Budget,
}
impl<'inputs> DerivedTerms<'inputs> {
    /// Register immutable input prefixes without scanning or copying payload.
    /// Equal vocabulary lineages with different extents remain distinct inputs.
    /// # Errors
    /// Refuses named capacity or caller work before publishing the arena.
    pub fn new_with<E>(
        inputs: &[CatalogRead<'inputs>],
        max_storage_bytes: usize,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<Self, DerivedFailure<E>> {
        let mut work = Work::new(&mut before);
        work.step()?;
        let mut result = Self {
            scope: VocabularyScope::fresh(),
            inputs: Vec::new(),
            roots: Vec::new(),
            nodes: Nodes::new(),
            symbol_names: Vec::new(),
            index: Index::default(),
            children: Vec::new(),
            budget: Budget::new(max_storage_bytes, size_of::<Self>()),
        };
        result.budget.check()?;
        work.reserve(&mut result.inputs, inputs.len(), &mut result.budget)?;
        work.steps(inputs.len())?;
        result.inputs.extend(inputs.iter().map(|read| read.0));
        Ok(result)
    }
    /// Borrow this arena's exact term prefix. The read blocks further appends.
    #[must_use]
    pub fn read(&self) -> TermRead<'_> {
        TermRead::derived(self)
    }
    /// Current named retained bytes, excluding the borrowed input owners.
    #[must_use]
    pub fn storage_bytes(&self) -> u128 {
        self.budget.used
    }
    /// Largest actual named capacity/replacement overlap since the last restart.
    /// Rejected reservation requests do not count as allocations.
    #[must_use]
    pub fn storage_peak_bytes(&self) -> u128 {
        self.budget.peak
    }
    /// Begin a new actual-capacity peak interval at the current retained bytes.
    pub fn restart_storage_peak(&mut self) {
        self.budget.peak = self.budget.used;
    }
    /// Replace the named-storage ceiling; the new bound also applies on refusal.
    /// # Errors
    /// Refuses when current retained bytes already exceed the supplied ceiling.
    pub fn ceiling(&mut self, max_storage_bytes: usize) -> Result<(), Fault> {
        self.budget.limit = max_storage_bytes;
        self.budget.check()
    }
    pub(in crate::catalog) fn scope(&self) -> &VocabularyScope {
        &self.scope
    }
    pub(in crate::catalog) fn contains(&self, id: TermId) -> bool {
        id.position() < self.roots.len()
    }
    pub(in crate::catalog) fn term(&self, id: TermId) -> DerivedTerm<'_> {
        DerivedTerm { arena: self, id }
    }
    fn key(&self, id: TermId) -> TermKey {
        TermKey {
            scope: self.scope.clone(),
            id,
        }
    }
    fn text(&self, name: InputText) -> &'inputs str {
        self.inputs[name.input].text(name.id)
    }
    pub(in crate::catalog) fn constructor(&self, id: TermId) -> Option<DeclaredConstructor> {
        match self.roots[id.position()] {
            Root::Input { input, id } => {
                let read = self.inputs[input];
                read.term_constructor(id)
                    .map(|data| DeclaredConstructor::new(read, data))
            }
            Root::Node(node) => {
                let (name, sign, arity) = match self.nodes.kinds[node] {
                    Kind::Symbol => (
                        Some(self.symbol_names[self.nodes.payloads[node] as usize]),
                        crate::Sign::Positive,
                        0,
                    ),
                    Kind::Function | Kind::Tuple => {
                        let compound = self.nodes.compound(node)?;
                        (compound.name, compound.sign, compound.children.len())
                    }
                    _ => return None,
                };
                Some(match name {
                    Some(name) => DeclaredConstructor::new(
                        self.inputs[name.input],
                        ConstructorData::Function {
                            name: name.id,
                            sign,
                            arity,
                        },
                    ),
                    None => DeclaredConstructor {
                        scope: self.scope.clone(),
                        data: ConstructorData::Tuple { arity },
                    },
                })
            }
        }
    }
}

#[derive(Clone, Copy)]
pub(in crate::catalog) struct DerivedTerm<'a> {
    arena: &'a DerivedTerms<'a>,
    id: TermId,
}
impl<'a> DerivedTerm<'a> {
    fn input(self) -> Option<TermRef<'a>> {
        match self.arena.roots[self.id.position()] {
            Root::Input { input, id } => TermRef::new(self.arena.inputs[input], id),
            Root::Node(_) => None,
        }
    }
    fn node(self) -> usize {
        match self.arena.roots[self.id.position()] {
            Root::Node(node) => node,
            Root::Input { .. } => unreachable!("generated-node projection"),
        }
    }
    pub(in crate::catalog) fn descriptor(self) -> ValueNodeRef<'a> {
        self.input().map_or_else(
            || {
                self.arena.nodes.descriptor(
                    self.node(),
                    |id| self.arena.text(self.arena.symbol_names[id as usize]),
                    |name| self.arena.text(name),
                )
            },
            TermRef::descriptor,
        )
    }
    fn measures(self) -> Measures {
        self.input().map_or_else(
            || {
                self.arena.nodes.measures(self.node(), |id| {
                    let name = self.arena.symbol_names[id as usize];
                    self.arena.inputs[name.input].text_measures(name.id)
                })
            },
            |term| Measures {
                nodes: term.expanded_nodes(),
                depth: term.depth(),
                canonical: term.canonical_bytes(),
                rendered: term.rendered_bytes(),
            },
        )
    }
    pub(in crate::catalog) fn expanded_nodes(self) -> usize {
        self.measures().nodes
    }
    pub(in crate::catalog) fn depth(self) -> usize {
        self.measures().depth
    }
    pub(in crate::catalog) fn canonical_bytes(self) -> usize {
        self.measures().canonical
    }
    pub(in crate::catalog) fn rendered_bytes(self) -> usize {
        self.measures().rendered
    }
    pub(in crate::catalog) fn child(self, index: usize) -> Option<TermRef<'a>> {
        match self.input() {
            Some(term) => term.child(index),
            None => TermRef::derived(self.arena, self.arena.nodes.child(self.node(), index)?),
        }
    }
    pub(in crate::catalog) fn child_end(self, index: usize) -> Option<usize> {
        self.input().map_or_else(
            || self.arena.nodes.child_end(self.node(), index),
            |term| term.child_end(index),
        )
    }
}

#[cfg(test)]
mod tests;
