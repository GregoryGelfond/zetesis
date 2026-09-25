//! Direct, metered borrowed ingress. Flat source descriptions and canonical
//! child graphs share one interner; neither route materializes Value or Atom.

use std::convert::Infallible;

use crate::ValueResource;
use crate::catalog::{AtomRef, Limits, PredicateRef, TermRef};

use super::control::{Work, uncontrolled};
use super::nodes::{Measures, arity, ceiling, descriptor_measures};
use super::{AtomId, Failure, Fault, Read, Store, TermId, budget};

#[derive(Clone, Copy)]
struct Frame {
    source: TermId,
    next: usize,
    start: usize,
}

impl Store {
    /// Owned descriptions use exactly the borrowed importer, with no caller
    /// stop. No source payload or alternate interning algorithm is retained.
    #[cfg(test)]
    pub(crate) fn import_value(
        &mut self,
        value: &crate::Value,
        limits: Limits,
    ) -> Result<TermId, Fault> {
        uncontrolled(self.import_term_with(
            TermRef::from(value),
            limits,
            || Ok::<_, Infallible>(()),
        ))
    }

    pub(crate) fn import_atom(
        &mut self,
        atom: &crate::Atom,
        limits: Limits,
    ) -> Result<AtomId, Fault> {
        uncontrolled(self.import_atom_with(AtomRef::from(atom), limits, || Ok::<_, Infallible>(())))
    }

    pub(crate) fn import_term_with<E>(
        &mut self,
        term: TermRef<'_>,
        limits: Limits,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<TermId, Failure<E>> {
        self.import_term(term, limits, &mut Work::new(&mut before))
    }

    pub(crate) fn import_predicate_with<E>(
        &mut self,
        predicate: PredicateRef<'_>,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<super::PredicateId, Failure<E>> {
        self.intern_predicate_with(predicate, &mut Work::new(&mut before))
    }

    pub(crate) fn import_atom_with<E>(
        &mut self,
        atom: AtomRef<'_>,
        limits: Limits,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<AtomId, Failure<E>> {
        let mut work = Work::new(&mut before);
        work.step()?;
        let existing = atom
            .canonical()
            .filter(|(read, _)| read.atoms_belong_to(self))
            .map(|(_, id)| id);
        work.step()?;
        let predicate = atom.predicate();
        work.step()?;
        let arguments = atom.arguments().iter();
        self.import_row(predicate, arguments, limits, existing, &mut work)
    }

    pub(crate) fn import_row_with<'a, E>(
        &mut self,
        predicate: PredicateRef<'a>,
        arguments: impl IntoIterator<Item = TermRef<'a>>,
        limits: Limits,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<AtomId, Failure<E>> {
        let mut work = Work::new(&mut before);
        work.step()?;
        self.import_row(predicate, arguments.into_iter(), limits, None, &mut work)
    }

    fn import_row<'a, E>(
        &mut self,
        predicate: PredicateRef<'a>,
        mut arguments: impl Iterator<Item = TermRef<'a>>,
        limits: Limits,
        existing: Option<AtomId>,
        work: &mut Work<'_, E>,
    ) -> Result<AtomId, Failure<E>> {
        work.step()?;
        let arity = predicate.arity();
        let mut ids = Vec::new();
        let result = (|| {
            work.reserve(&mut ids, arity, &mut self.budget)?;
            for _ in 0..arity {
                work.step()?;
                let term = arguments.next().ok_or(Fault::Shape)?;
                let id = self.import_term(term, limits, work)?;
                work.step()?;
                ids.push(id);
            }
            work.step()?;
            if arguments.next().is_some() {
                return Err(Fault::Shape.into());
            }
            if let Some(id) = existing {
                return Ok(id);
            }
            let predicate = self.intern_predicate_with(predicate, work)?;
            self.intern_atom_with(predicate, &ids, work)
        })();
        self.budget.used -= budget::capacity(&ids);
        result
    }

    fn import_term<E>(
        &mut self,
        term: TermRef<'_>,
        limits: Limits,
        work: &mut Work<'_, E>,
    ) -> Result<TermId, Failure<E>> {
        work.step()?;
        self.budget.check()?;
        work.step()?;
        if let Some((read, id)) = term.canonical() {
            work.step()?;
            let source = read.term(id).ok_or(Fault::Shape)?;
            work.step()?;
            let nodes = source.expanded_nodes();
            work.step()?;
            let depth = source.depth();
            work.step()?;
            let canonical = source.canonical_bytes();
            work.step()?;
            let rendered = source.rendered_bytes();
            let measures = Measures {
                nodes,
                depth,
                canonical,
                rendered,
            };
            measures.check(limits)?;
            work.step()?;
            if read.vocabulary_belongs_to(self) {
                return Ok(id);
            }
            if nodes == 1 {
                work.step()?;
                return self.intern_node_with(source.descriptor(), &[], limits, work);
            }
            self.import_canonical(read, id, measures, limits, work)
        } else {
            if term.is_canonical() {
                work.steps(4)?;
                Measures {
                    nodes: term.expanded_nodes(),
                    depth: term.depth(),
                    canonical: term.canonical_bytes(),
                    rendered: term.rendered_bytes(),
                }
                .check(limits)?;
            }
            self.import_preorder(term, limits, work)
        }
    }

    fn import_preorder<E>(
        &mut self,
        term: TermRef<'_>,
        limits: Limits,
        work: &mut Work<'_, E>,
    ) -> Result<TermId, Failure<E>> {
        work.step()?;
        let nodes = term.expanded_nodes();
        ceiling(ValueResource::Nodes, nodes as u128, limits.max_nodes)?;
        if nodes == 1 {
            work.step()?;
            let descriptor = term.descriptor();
            ceiling(
                ValueResource::Bytes,
                descriptor.canonical_bytes(),
                limits.max_bytes,
            )?;
            return self.intern_node_with(descriptor, &[], limits, work);
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
            for index in (0..nodes).rev() {
                work.step()?;
                let descriptor = if term.is_canonical() {
                    // Derived graphs share this reverse-preorder collapse. Rank
                    // selection uses the common checked cursor; it may revisit
                    // ancestors without retaining a second traversal schema.
                    term.subterm_with(index, || work.step())?
                        .ok_or(Fault::Shape)?
                        .descriptor()
                } else {
                    term.flat_node(index).ok_or(Fault::Shape)?
                };
                let minimum = logical
                    .checked_add(descriptor.canonical_bytes())
                    .and_then(|bytes| bytes.checked_add(budget::capacity(&ids)))
                    .ok_or(Fault::Overflow)?;
                ceiling(ValueResource::Bytes, minimum, limits.max_bytes)?;
                let head = descriptor_measures(descriptor, work)?;
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
                let id = self.intern_node_with(descriptor, &ids[start..], limits, work)?;
                work.step()?;
                ids.truncate(start);
                ids.push(id);
            }
            if ids.len() != 1 {
                return Err(Fault::Shape.into());
            }
            Ok(ids[0])
        })();
        self.budget.used -= budget::capacity(&ids);
        result
    }

    fn import_canonical<E>(
        &mut self,
        read: Read<'_>,
        root: TermId,
        measures: Measures,
        limits: Limits,
        work: &mut Work<'_, E>,
    ) -> Result<TermId, Failure<E>> {
        let mut frames = Vec::new();
        let mut ids = Vec::new();
        let allowance = ImportBudget {
            logical: measures.canonical as u128 + measures.rendered as u128,
            limits,
        };
        let result = (|| {
            allowance.reserve(
                &mut frames,
                1,
                budget::capacity(&ids),
                &mut self.budget,
                work,
            )?;
            work.step()?;
            frames.push(Frame {
                source: root,
                next: 0,
                start: 0,
            });
            loop {
                work.step()?;
                let frame = *frames.last().ok_or(Fault::Shape)?;
                work.step()?;
                let term = read.term(frame.source).ok_or(Fault::Shape)?;
                work.step()?;
                let descriptor = term.descriptor();
                if frame.next < arity(descriptor) {
                    work.step()?;
                    let child = term.child(frame.next).ok_or(Fault::Shape)?;
                    allowance.reserve(
                        &mut frames,
                        1,
                        budget::capacity(&ids),
                        &mut self.budget,
                        work,
                    )?;
                    work.steps(2)?;
                    frames.last_mut().ok_or(Fault::Shape)?.next += 1;
                    frames.push(Frame {
                        source: child,
                        next: 0,
                        start: ids.len(),
                    });
                } else {
                    let id =
                        self.intern_node_with(descriptor, &ids[frame.start..], limits, work)?;
                    work.step()?;
                    ids.truncate(frame.start);
                    frames.pop();
                    if frames.is_empty() {
                        return Ok(id);
                    }
                    allowance.reserve(
                        &mut ids,
                        1,
                        budget::capacity(&frames),
                        &mut self.budget,
                        work,
                    )?;
                    work.step()?;
                    ids.push(id);
                }
            }
        })();
        self.budget.used -= budget::capacity(&frames) + budget::capacity(&ids);
        result
    }
}

/// Expanded logical bytes remain fixed while iterative traversal scratch grows.
struct ImportBudget {
    logical: u128,
    limits: Limits,
}

impl ImportBudget {
    fn reserve<T, E>(
        &self,
        values: &mut Vec<T>,
        additional: usize,
        other: u128,
        budget: &mut super::budget::Budget,
        work: &mut Work<'_, E>,
    ) -> Result<(), Failure<E>> {
        work.step()?;
        let base = self.logical.checked_add(other).ok_or(Fault::Overflow)?;
        let requested = base
            .checked_add(super::budget::reservation_bytes(values, additional)?)
            .ok_or(Fault::Overflow)?;
        ceiling(ValueResource::Bytes, requested, self.limits.max_bytes)?;
        work.reserve(values, additional, budget)?;
        let observed = base
            .checked_add(super::budget::capacity(values))
            .ok_or(Fault::Overflow)?;
        ceiling(ValueResource::Bytes, observed, self.limits.max_bytes)?;
        Ok(())
    }
}
