//! Immutable indexed lookup. Local ID scratch and its peak belong to this
//! capability, never to the shared writer or snapshot payload.

use super::control::Work;
use super::nodes::{Measures, arity, ceiling, descriptor_measures};
use super::{Failure, Fault, Read, Store, TermId, budget};
use crate::catalog::{Limits, TermRef};
use crate::{ValueNodeRef, ValueResource};

pub(crate) struct TermLookup<'a> {
    store: &'a Store,
    budget: budget::Budget,
}

impl<'a> TermLookup<'a> {
    pub(crate) fn new(store: &'a Store) -> Self {
        Self {
            store,
            budget: budget::Budget {
                used: store.current_bytes(),
                peak: store.current_bytes(),
                limit: usize::MAX,
            },
        }
    }
    pub(crate) fn read(&self) -> Read<'a> {
        Read::from(self.store)
    }
    pub(crate) fn peak_bytes(&self) -> u128 {
        self.budget.peak
    }
    pub(crate) fn restart_peak(&mut self) {
        self.budget.peak = self.budget.used;
    }
    pub(crate) fn ceiling(&mut self, limit: usize) -> Result<(), Fault> {
        self.budget.limit = limit;
        self.budget.check()
    }

    pub(crate) fn find_term_with<E>(
        &mut self,
        term: TermRef<'_>,
        limits: Limits,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<Option<TermId>, Failure<E>> {
        let mut work = Work::new(&mut before);
        work.step()?;
        self.budget.check()?;
        work.step()?;
        let nodes = term.expanded_nodes();
        ceiling(ValueResource::Nodes, nodes as u128, limits.max_nodes)?;
        work.step()?;
        let canonical = term.canonical();
        if let Some((read, id)) = canonical {
            work.step()?;
            let source = read.term(id).ok_or(Fault::Shape)?;
            work.steps(4)?;
            Measures {
                nodes,
                depth: source.depth(),
                canonical: source.canonical_bytes(),
                rendered: source.rendered_bytes(),
            }
            .check(limits)?;
            work.step()?;
            if read.vocabulary_belongs_to(self.store) {
                return Ok(Some(id));
            }
        }
        if nodes == 1 {
            work.step()?;
            return self
                .store
                .find_node_with(term.descriptor(), &[], 0, limits, &mut work);
        }
        let mut ids = Vec::new();
        let result = (|| {
            ceiling(
                ValueResource::Bytes,
                budget::reservation_bytes(&ids, nodes)?,
                limits.max_bytes,
            )?;
            work.reserve(&mut ids, nodes, &mut self.budget)?;
            let mut logical = 0u128;
            // Reverse preorder leaves each node's children at the top of this
            // stack in reverse argument order. Collapse them to one existing ID.
            // Canonical rank selection may revisit ancestors; no linear-time
            // claim is made for this constant-navigation-memory fallback.
            for position in (0..nodes).rev() {
                work.step()?;
                let descriptor = if term.is_canonical() {
                    let subtree = term
                        .subterm_with(position, || work.step())?
                        .ok_or(Fault::Shape)?;
                    work.step()?;
                    subtree.descriptor()
                } else {
                    term.flat_node(position).ok_or(Fault::Shape)?
                };
                ceiling(
                    ValueResource::Bytes,
                    logical
                        .checked_add(descriptor.canonical_bytes())
                        .and_then(|bytes| bytes.checked_add(budget::capacity(&ids)))
                        .ok_or(Fault::Overflow)?,
                    limits.max_bytes,
                )?;
                let head = descriptor_measures(descriptor, &mut work)?;
                logical = logical
                    .checked_add(head.canonical as u128 + head.rendered as u128)
                    .ok_or(Fault::Overflow)?;
                ceiling(
                    ValueResource::Bytes,
                    logical
                        .checked_add(budget::capacity(&ids))
                        .ok_or(Fault::Overflow)?,
                    limits.max_bytes,
                )?;
                let children = arity(descriptor);
                let start = ids.len().checked_sub(children).ok_or(Fault::Shape)?;
                work.steps(children)?;
                ids[start..].reverse();
                let Some(id) = self.store.find_node_with(
                    descriptor,
                    &ids[start..],
                    budget::capacity(&ids),
                    limits,
                    &mut work,
                )?
                else {
                    return Ok(None);
                };
                work.step()?;
                ids.truncate(start);
                ids.push(id);
            }
            if ids.len() != 1 {
                return Err(Fault::Shape.into());
            }
            Ok(Some(ids[0]))
        })();
        self.budget.used -= budget::capacity(&ids);
        result
    }

    pub(crate) fn find_constructed_with<E>(
        &mut self,
        descriptor: ValueNodeRef<'_>,
        values: &[Option<TermId>],
        slots: &[usize],
        limits: Limits,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<Option<TermId>, Failure<E>> {
        let mut work = Work::new(&mut before);
        let mut ids = Vec::new();
        let result = (|| {
            work.step()?;
            self.budget.check()?;
            ceiling(
                ValueResource::Bytes,
                budget::reservation_bytes(&ids, slots.len())?,
                limits.max_bytes,
            )?;
            work.reserve(&mut ids, slots.len(), &mut self.budget)?;
            for &slot in slots {
                work.step()?;
                let id = values.get(slot).copied().flatten().ok_or(Fault::Shape)?;
                work.step()?;
                ids.push(id);
            }
            self.store
                .find_node_with(descriptor, &ids, budget::capacity(&ids), limits, &mut work)
        })();
        self.budget.used -= budget::capacity(&ids);
        result
    }
}
