//! Admission keeps candidate state private until every fallible step completes.

use super::super::{
    Failure, Fault, TermId, budget,
    control::Work,
    next_id,
    nodes::{self, Compound, Kind, Measures},
};
use super::{DerivedFailure, DerivedTerms, InputText, Root};
use crate::catalog::{ConstructorData, DeclaredConstructor, ReadError, TermKey, TermRef};
use crate::{
    ValueNodeRef, ValueResource,
    catalog::{AssignmentSlice, Limits},
};
use std::{cmp::Ordering, collections::hash_map::DefaultHasher, hash::Hasher};

impl DerivedTerms<'_> {
    /// Select an immediate child as a local registered root. Generated children
    /// reuse their IDs; a borrowed input child is registered through the same
    /// exact admission path as [`Self::borrow_with`]. Unselected siblings remain
    /// borrowed. An out-of-arity index returns `None` without registering a root.
    /// # Errors
    /// Refuses a foreign key, unavailable prefix, capacity, or caller work.
    pub fn child_with<E>(
        &mut self,
        key: &TermKey,
        index: usize,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<Option<TermKey>, DerivedFailure<E>> {
        before().map_err(DerivedFailure::Stopped)?;
        if !self.scope.same(&key.scope) {
            return Err(ReadError::ForeignCatalog.into());
        }
        before().map_err(DerivedFailure::Stopped)?;
        let root = *self
            .roots
            .get(key.id.position())
            .ok_or(ReadError::OutsidePrefix)?;
        before().map_err(DerivedFailure::Stopped)?;
        match root {
            Root::Node(node) => {
                let child = self.nodes.child(node, index);
                before().map_err(DerivedFailure::Stopped)?;
                Ok(child.map(|id| self.key(id)))
            }
            Root::Input { input, id } => {
                let term = TermRef::new(self.inputs[input], id).ok_or(ReadError::OutsidePrefix)?;
                before().map_err(DerivedFailure::Stopped)?;
                term.child(index)
                    .map(|child| self.borrow_with(child, before))
                    .transpose()
            }
        }
    }

    /// Register an ordinary canonical term from an exact declared input prefix.
    /// Registration does not copy payload or impose a new construction-depth
    /// limit. Expanded hashing and exact collision comparisons are metered.
    /// # Errors
    /// Refuses ingress/foreign terms, absent prefixes, capacity, or caller work.
    pub fn borrow_with<E>(
        &mut self,
        value: TermRef<'_>,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<TermKey, DerivedFailure<E>> {
        let mut work = Work::new(&mut before);
        work.step()?;
        self.budget.check()?;
        work.step()?;
        let (read, id) = value.canonical().ok_or(ReadError::Uninterned)?;
        let mut same_scope = false;
        let mut selected = None;
        for (input, registered) in self.inputs.iter().enumerate() {
            work.step()?;
            if registered.same_vocabulary(read) {
                same_scope = true;
                work.step()?;
                if registered.contains_term(id) {
                    selected = Some(input);
                    break;
                }
            }
        }
        let input = selected.ok_or(if same_scope {
            ReadError::OutsidePrefix
        } else {
            ReadError::ForeignCatalog
        })?;
        let hash = term_hash(value, &mut work)?;
        if let Some(found) = self.index.find_with(hash, &mut work, |candidate, work| {
            let term = TermRef::derived(self, TermId(candidate)).ok_or(Fault::Shape)?;
            term.compare_ref_with(value, || work.step())
                .map(std::cmp::Ordering::is_eq)
        })? {
            return Ok(self.key(TermId(found)));
        }
        let root = TermId(next_id(self.roots.len())?);
        work.reserve(&mut self.roots, 1, &mut self.budget)?;
        self.index.reserve_with(hash, &mut self.budget, &mut work)?;
        work.steps(2)?;
        self.roots.push(Root::Input { input, id });
        self.index.insert(hash, root.0);
        Ok(self.key(root))
    }

    /// Construct a numeric scalar or an extremum without any text owner.
    /// Strings and named values require registered canonical input payload.
    /// # Errors
    /// Refuses other descriptors, logical/capacity limits, or caller work.
    pub fn scalar_with<E>(
        &mut self,
        value: ValueNodeRef<'_>,
        logical: Limits,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<TermKey, DerivedFailure<E>> {
        self.children.clear();
        let mut work = Work::new(&mut before);
        work.step()?;
        if !matches!(
            value,
            ValueNodeRef::Number(_) | ValueNodeRef::Infimum | ValueNodeRef::Supremum
        ) {
            return Err(Fault::Shape.into());
        }
        self.intern(value, None, logical, &mut work)
            .map_err(Into::into)
    }

    /// Construct a function or tuple from declared input text and local ID slots.
    /// The declaration and children keep their distinct checked scope witnesses.
    /// No foreign subtree is imported; registered roots can be reused as children.
    /// # Errors
    /// Refuses undeclared text, foreign/unbound slots, shape, limits, or work.
    pub fn construct_with<E>(
        &mut self,
        shape: &DeclaredConstructor,
        values: AssignmentSlice<'_>,
        slots: &[usize],
        logical: Limits,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<TermKey, DerivedFailure<E>> {
        self.children.clear();
        values.validate_with(self.read(), slots, &mut before)?;
        let mut work = Work::new(&mut before);
        work.step()?;
        self.budget.check()?;
        let name = self.name(shape, &mut work)?;
        let descriptor = match shape.data {
            ConstructorData::Function { sign, arity, .. } => ValueNodeRef::Function {
                name: self.text(name.ok_or(Fault::Shape)?),
                sign,
                arity,
            },
            ConstructorData::Tuple { arity } => ValueNodeRef::Tuple { arity },
        };
        work.step()?;
        if nodes::arity(descriptor) != slots.len() {
            return Err(Fault::Shape.into());
        }
        nodes::ceiling(
            ValueResource::Bytes,
            budget::reservation_bytes(&self.children, slots.len())?,
            logical.max_bytes,
        )?;
        work.reserve(&mut self.children, slots.len(), &mut self.budget)?;
        for &slot in slots {
            work.step()?;
            // Validation above covered this immutable frame and arena prefix.
            self.children.push(values.slots[slot].ok_or(Fault::Shape)?);
        }
        self.intern(descriptor, name, logical, &mut work)
            .map_err(Into::into)
    }

    fn name<E>(
        &self,
        shape: &DeclaredConstructor,
        work: &mut Work<'_, E>,
    ) -> Result<Option<InputText>, DerivedFailure<E>> {
        work.step()?;
        if shape.data.text().is_none() && self.scope.same(&shape.scope) {
            return Ok(None);
        }
        let mut same_scope = false;
        for (input, read) in self.inputs.iter().enumerate() {
            work.step()?;
            if read.accepts_vocabulary_scope(&shape.scope) {
                same_scope = true;
                work.step()?;
                match shape.data.text() {
                    Some(id) if read.contains_text(id) => return Ok(Some(InputText { input, id })),
                    None => return Ok(None),
                    Some(_) => {}
                }
            }
        }
        Err(if same_scope {
            ReadError::OutsidePrefix
        } else {
            ReadError::ForeignCatalog
        }
        .into())
    }

    fn intern<E>(
        &mut self,
        descriptor: ValueNodeRef<'_>,
        name: Option<InputText>,
        logical: Limits,
        work: &mut Work<'_, E>,
    ) -> Result<TermKey, Failure<E>> {
        work.step()?;
        self.budget.check()?;
        let descriptor = nodes::normalized(descriptor)?;
        let measures = nodes::node_measures(
            descriptor,
            &self.children,
            |child| self.term(child).measures(),
            work,
        )?;
        measures.check(logical)?;
        nodes::ceiling(
            ValueResource::Bytes,
            measures.canonical as u128
                + measures.rendered as u128
                + budget::capacity(&self.children),
            logical.max_bytes,
        )?;
        let mut state = DefaultHasher::new();
        descriptor_hash(descriptor, &mut state, work)?;
        for child in &self.children {
            work.step()?;
            hash_nodes(
                TermRef::derived(self, *child).ok_or(Fault::Shape)?,
                &mut state,
                work,
            )?;
        }
        work.step()?;
        let hash = state.finish();
        if let Some(found) = self.index.find_with(hash, work, |candidate, work| {
            self.equal(TermId(candidate), descriptor, work)
        })? {
            return Ok(self.key(TermId(found)));
        }
        self.publish(descriptor, name, measures, hash, work)
    }

    fn equal<E>(
        &self,
        candidate: TermId,
        descriptor: ValueNodeRef<'_>,
        work: &mut Work<'_, E>,
    ) -> Result<bool, Failure<E>> {
        work.step()?;
        let candidate = TermRef::derived(self, candidate).ok_or(Fault::Shape)?;
        work.step()?;
        let order =
            crate::term_order::storage_with(candidate.descriptor(), descriptor, |left, right| {
                work.text_equal(left, right).map(|equal| {
                    if equal {
                        Ordering::Equal
                    } else {
                        Ordering::Less
                    }
                })
            })?;
        if !order.is_eq() {
            return Ok(false);
        }
        for (position, child) in self.children.iter().enumerate() {
            work.step()?;
            let existing = candidate.child(position).ok_or(Fault::Shape)?;
            let supplied = TermRef::derived(self, *child).ok_or(Fault::Shape)?;
            if !existing.compare_ref_with(supplied, || work.step())?.is_eq() {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn publish<E>(
        &mut self,
        descriptor: ValueNodeRef<'_>,
        name: Option<InputText>,
        measures: Measures,
        hash: u64,
        work: &mut Work<'_, E>,
    ) -> Result<TermKey, Failure<E>> {
        let root = TermId(next_id(self.roots.len())?);
        let (kind, payload, compound) = match descriptor {
            ValueNodeRef::Infimum => (Kind::Infimum, 0, false),
            ValueNodeRef::Supremum => (Kind::Supremum, 0, false),
            ValueNodeRef::Number(value) => {
                (Kind::Number, u32::from_ne_bytes(value.to_ne_bytes()), false)
            }
            ValueNodeRef::Symbol(_) => (Kind::Symbol, next_id(self.symbol_names.len())?, false),
            ValueNodeRef::Function { .. } => {
                (Kind::Function, next_id(self.nodes.compounds.len())?, true)
            }
            ValueNodeRef::Tuple { .. } => (Kind::Tuple, next_id(self.nodes.compounds.len())?, true),
            ValueNodeRef::String(_) => return Err(Fault::Shape.into()),
        };
        let start = self.nodes.children.len();
        let end = start
            .checked_add(self.children.len())
            .ok_or(Fault::Overflow)?;
        work.reserve(&mut self.roots, 1, &mut self.budget)?;
        work.reserve(&mut self.nodes.kinds, 1, &mut self.budget)?;
        work.reserve(&mut self.nodes.payloads, 1, &mut self.budget)?;
        if compound {
            work.reserve(&mut self.nodes.compounds, 1, &mut self.budget)?;
            work.reserve(
                &mut self.nodes.children,
                self.children.len(),
                &mut self.budget,
            )?;
            work.reserve(&mut self.nodes.ends, self.children.len(), &mut self.budget)?;
        }
        if kind == Kind::Symbol {
            work.reserve(&mut self.symbol_names, 1, &mut self.budget)?;
        }
        self.index.reserve_with(hash, &mut self.budget, work)?;
        // No callback, allocation, or fallible work occurs after these permits.
        // Each child read/end/copy plus fixed metadata publication is admitted.
        for _ in &self.children {
            work.steps(3)?;
        }
        work.steps(7)?;
        let node = self.nodes.kinds.len();
        if compound {
            let mut total = 0;
            for child in &self.children {
                total += self.term(*child).expanded_nodes();
                self.nodes.children.push(*child);
                self.nodes.ends.push(total);
            }
            self.nodes.compounds.push(Compound {
                name,
                sign: match descriptor {
                    ValueNodeRef::Function { sign, .. } => sign,
                    _ => crate::Sign::Positive,
                },
                children: start..end,
                measures,
            });
        }
        if kind == Kind::Symbol {
            self.symbol_names
                .push(name.expect("normalized declared function retains its name"));
        }
        self.nodes.kinds.push(kind);
        self.nodes.payloads.push(payload);
        self.roots.push(Root::Node(node));
        self.index.insert(hash, root.0);
        Ok(self.key(root))
    }
}

fn descriptor_hash<E>(
    descriptor: ValueNodeRef<'_>,
    state: &mut DefaultHasher,
    work: &mut Work<'_, E>,
) -> Result<(), Failure<E>> {
    work.step()?;
    let bytes = match descriptor {
        ValueNodeRef::Symbol(text) | ValueNodeRef::String(text) => text.len(),
        ValueNodeRef::Function { name, .. } => name.len(),
        _ => 0,
    };
    work.steps(bytes)?;
    crate::term_hash::descriptor(descriptor, state);
    Ok(())
}
fn hash_nodes<E>(
    term: TermRef<'_>,
    state: &mut DefaultHasher,
    work: &mut Work<'_, E>,
) -> Result<(), Failure<E>> {
    let mut nodes = term.nodes();
    while let Some(descriptor) = nodes.next_with(|| work.step())? {
        descriptor_hash(descriptor, state, work)?;
    }
    Ok(())
}
fn term_hash<E>(term: TermRef<'_>, work: &mut Work<'_, E>) -> Result<u64, Failure<E>> {
    let mut state = DefaultHasher::new();
    hash_nodes(term, &mut state, work)?;
    work.step()?;
    Ok(state.finish())
}

#[cfg(test)]
mod tests;
