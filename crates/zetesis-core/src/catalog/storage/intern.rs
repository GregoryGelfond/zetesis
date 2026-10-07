//! Collision-exact interning with all fallible work before row publication.

use crate::catalog::{ConstructorData, Limits, PredicateRef};
use crate::{Sign, ValueNodeRef, ValueResource};

use super::control::Work;
use super::nodes::{Compound, Kind, arity, ceiling, node_measures, normalized};
use super::segments::{Columns, Locator, Signature, Text};
use super::{AtomId, Failure, Fault, PredicateId, Store, TermId, TextId, budget, next_id};

/// Exact row result for one unchanged input under exclusive writer access.
/// It owns no tuple, bucket address or permission to use another authority.
pub(crate) struct PreparedAtom(RowPreparation);

enum RowPreparation {
    Present(AtomId),
    Vacant {
        predicate: PredicateId,
        arity: usize,
        hash: u64,
        column: Option<usize>,
    },
}

impl PreparedAtom {
    /// A location is only a hint: publication checks its signed predicate in
    /// the current tail before reuse. It carries no tuple-absence authority.
    pub(crate) fn with_column(mut self, column: usize) -> Self {
        if let RowPreparation::Vacant {
            column: location, ..
        } = &mut self.0
        {
            *location = Some(column);
        }
        self
    }

    /// Transfer the opaque single-use result into its publication state.
    fn into_preparation(self) -> RowPreparation {
        self.0
    }

    /// The caller has authenticated this identity in the same atom authority.
    pub(crate) const fn existing(id: AtomId) -> Self {
        Self(RowPreparation::Present(id))
    }

    pub(crate) const fn present(&self) -> Option<AtomId> {
        match self.0 {
            RowPreparation::Present(id) => Some(id),
            RowPreparation::Vacant { .. } => None,
        }
    }
}

#[derive(Clone, Copy, Hash)]
struct NodeKey {
    kind: Kind,
    payload: u32,
    name: Option<TextId>,
    sign: Sign,
}
impl NodeKey {
    fn compound(self) -> bool {
        matches!(self.kind, Kind::Function | Kind::Tuple)
    }
}

impl Store {
    pub(crate) fn declare_constructor_with<E>(
        &mut self,
        descriptor: ValueNodeRef<'_>,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<ConstructorData, Failure<E>> {
        let mut work = Work::new(&mut before);
        work.step()?;
        self.budget.check()?;
        let data = match descriptor {
            ValueNodeRef::Function { name, sign, arity } if !name.is_empty() => {
                ConstructorData::Function {
                    name: self.intern_text_with(name, &mut work)?,
                    sign,
                    arity,
                }
            }
            ValueNodeRef::Tuple { arity } => ConstructorData::Tuple { arity },
            _ => return Err(Fault::Shape.into()),
        };
        work.step()?;
        Ok(data)
    }

    fn intern_text_with<E>(
        &mut self,
        text: &str,
        work: &mut Work<'_, E>,
    ) -> Result<TextId, Failure<E>> {
        work.step()?;
        self.budget.check()?;
        let hash = work.text_hash(text)?;
        if let Some(id) = self.find_text_hash_with(text, hash, work)? {
            return Ok(id);
        }
        work.step()?;
        self.vocabulary.growing()?;
        let id = TextId(next_id(self.counts().texts)?);
        let growing = self.vocabulary.growing()?;
        let quoted_bytes = work.quoted_bytes(text)?;
        let start = growing.tail.text.len();
        let end = start.checked_add(text.len()).ok_or(Fault::Overflow)?;
        work.reserve_text(&mut growing.tail.text, text.len(), &mut self.budget)?;
        work.reserve(&mut growing.tail.texts, 1, &mut self.budget)?;
        growing
            .indexes
            .texts
            .reserve_with(hash, &mut self.budget, work)?;
        work.steps(text.len())?;
        work.steps(3)?;
        growing.tail.text.push_str(text);
        growing.tail.texts.push(Text {
            bytes: start..end,
            quoted_bytes,
        });
        growing.indexes.texts.insert(hash, id.0);
        Ok(id)
    }

    pub(super) fn intern_node_with<E>(
        &mut self,
        descriptor: ValueNodeRef<'_>,
        children: &[TermId],
        limits: Limits,
        work: &mut Work<'_, E>,
    ) -> Result<TermId, Failure<E>> {
        self.intern_node_scratch_with(descriptor, children, 0, limits, work)
    }

    pub(super) fn intern_node_scratch_with<E>(
        &mut self,
        descriptor: ValueNodeRef<'_>,
        children: &[TermId],
        scratch: u128,
        limits: Limits,
        work: &mut Work<'_, E>,
    ) -> Result<TermId, Failure<E>> {
        work.step()?;
        self.budget.check()?;
        if arity(descriptor) != children.len() {
            return Err(Fault::Shape.into());
        }
        let descriptor = normalized(descriptor)?;
        let measures = node_measures(
            descriptor,
            children,
            |child| self.term_measures(child),
            work,
        )?;
        measures.check(limits)?;
        ceiling(
            ValueResource::Bytes,
            measures.canonical as u128 + measures.rendered as u128 + scratch,
            limits.max_bytes,
        )?;
        let key = node_key(descriptor, work, |text, work| {
            self.intern_text_with(text, work).map(Some)
        })?
        .ok_or(Fault::Shape)?;
        let hash = work.key_hash(key, children)?;
        if let Some(id) = self
            .vocabulary
            .indexes()
            .terms
            .find_with(hash, work, |id, work| {
                self.equal_node(TermId(id), key, children, work)
            })?
        {
            return Ok(TermId(id));
        }
        work.step()?;
        self.vocabulary.growing()?;
        let id = TermId(next_id(self.counts().terms)?);
        let growing = self.vocabulary.growing()?;
        let payload = if key.compound() {
            next_id(growing.tail.terms.compounds.len())?
        } else {
            key.payload
        };
        let start = growing.tail.terms.children.len();
        let end = start.checked_add(children.len()).ok_or(Fault::Overflow)?;
        self.reserve_node(key, children.len(), hash, work)?;
        // Permit all child measure reads and writes before the first commit.
        for _ in children {
            work.steps(3)?;
        }
        work.steps(5)?;
        if key.compound() {
            let mut expanded = 0;
            for child in children {
                expanded += self.term_measures(*child).nodes;
                let tail = &mut self.vocabulary.admitted_growing().tail;
                tail.terms.children.push(*child);
                tail.terms.ends.push(expanded);
            }
            self.vocabulary
                .admitted_growing()
                .tail
                .terms
                .compounds
                .push(Compound {
                    name: key.name,
                    sign: key.sign,
                    children: start..end,
                    measures,
                });
        }
        let growing = self.vocabulary.admitted_growing();
        growing.tail.terms.kinds.push(key.kind);
        growing.tail.terms.payloads.push(payload);
        growing.indexes.terms.insert(hash, id.0);
        Ok(id)
    }

    fn find_text_with<E>(
        &self,
        text: &str,
        work: &mut Work<'_, E>,
    ) -> Result<Option<TextId>, Failure<E>> {
        let hash = work.text_hash(text)?;
        self.find_text_hash_with(text, hash, work)
    }

    fn find_text_hash_with<E>(
        &self,
        text: &str,
        hash: u64,
        work: &mut Work<'_, E>,
    ) -> Result<Option<TextId>, Failure<E>> {
        self.vocabulary
            .indexes()
            .texts
            .find_with(hash, work, |id, work| {
                work.step()?;
                work.text_equal(self.text(TextId(id)), text)
            })
            .map(|id| id.map(TextId))
    }

    pub(super) fn find_node_with<E>(
        &self,
        descriptor: ValueNodeRef<'_>,
        children: &[TermId],
        scratch: u128,
        limits: Limits,
        work: &mut Work<'_, E>,
    ) -> Result<Option<TermId>, Failure<E>> {
        work.step()?;
        if arity(descriptor) != children.len() {
            return Err(Fault::Shape.into());
        }
        let descriptor = normalized(descriptor)?;
        let measures = node_measures(
            descriptor,
            children,
            |child| self.term_measures(child),
            work,
        )?;
        measures.check(limits)?;
        ceiling(
            ValueResource::Bytes,
            measures.canonical as u128 + measures.rendered as u128 + scratch,
            limits.max_bytes,
        )?;
        let Some(key) = node_key(descriptor, work, |text, work| {
            self.find_text_with(text, work)
        })?
        else {
            return Ok(None);
        };
        let hash = work.key_hash(key, children)?;
        self.vocabulary
            .indexes()
            .terms
            .find_with(hash, work, |id, work| {
                self.equal_node(TermId(id), key, children, work)
            })
            .map(|id| id.map(TermId))
    }

    fn equal_node<E>(
        &self,
        id: TermId,
        key: NodeKey,
        children: &[TermId],
        work: &mut Work<'_, E>,
    ) -> Result<bool, Failure<E>> {
        work.step()?;
        let (segment, local) = self.term_segment(id);
        work.step()?;
        if segment.terms.kinds[local] != key.kind {
            return Ok(false);
        }
        let payload = segment.terms.payloads[local];
        if !key.compound() {
            return Ok(payload == key.payload);
        }
        work.step()?;
        let compound = &segment.terms.compounds[payload as usize];
        if compound.name != key.name
            || compound.sign != key.sign
            || compound.children.len() != children.len()
        {
            return Ok(false);
        }
        for (index, child) in children.iter().enumerate() {
            work.step()?;
            if segment.terms.children[compound.children.start + index] != *child {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn reserve_node<E>(
        &mut self,
        key: NodeKey,
        children: usize,
        hash: u64,
        work: &mut Work<'_, E>,
    ) -> Result<(), Failure<E>> {
        let growing = self.vocabulary.growing()?;
        work.reserve(&mut growing.tail.terms.kinds, 1, &mut self.budget)?;
        work.reserve(&mut growing.tail.terms.payloads, 1, &mut self.budget)?;
        if key.compound() {
            work.reserve(&mut growing.tail.terms.compounds, 1, &mut self.budget)?;
            work.reserve(&mut growing.tail.terms.children, children, &mut self.budget)?;
            work.reserve(&mut growing.tail.terms.ends, children, &mut self.budget)?;
        }
        growing
            .indexes
            .terms
            .reserve_with(hash, &mut self.budget, work)
    }

    pub(super) fn intern_predicate_with<E>(
        &mut self,
        predicate: PredicateRef<'_>,
        work: &mut Work<'_, E>,
    ) -> Result<PredicateId, Failure<E>> {
        work.step()?;
        self.budget.check()?;
        work.step()?;
        if let Some((read, id)) = predicate.canonical() {
            work.step()?;
            if read.vocabulary_belongs_to(self) {
                return Ok(id);
            }
        }
        work.step()?;
        let name = predicate.name();
        work.step()?;
        let sign = predicate.sign();
        work.step()?;
        let arity = predicate.arity();
        let signature = Signature {
            name: self.intern_text_with(name, work)?,
            sign,
            arity,
        };
        let hash = work.key_hash(signature, &[])?;
        if let Some(id) =
            self.vocabulary
                .indexes()
                .predicates
                .find_with(hash, work, |id, work| {
                    work.step()?;
                    Ok(self.signature(PredicateId(id)) == signature)
                })?
        {
            return Ok(PredicateId(id));
        }
        work.step()?;
        self.vocabulary.growing()?;
        let id = PredicateId(next_id(self.counts().predicates)?);
        let growing = self.vocabulary.growing()?;
        work.reserve(&mut growing.tail.predicates, 1, &mut self.budget)?;
        growing
            .indexes
            .predicates
            .reserve_with(hash, &mut self.budget, work)?;
        work.steps(3)?;
        growing.tail.predicates.push(signature);
        growing.indexes.predicates.insert(hash, id.0);
        Ok(id)
    }

    pub(super) fn intern_atom_with<E>(
        &mut self,
        predicate: PredicateId,
        arguments: &[TermId],
        work: &mut Work<'_, E>,
    ) -> Result<AtomId, Failure<E>> {
        work.step()?;
        self.budget.check()?;
        let prepared =
            self.prepare_atom(predicate, arguments.len(), |index| arguments[index], work)?;
        self.publish_atom(prepared, |index| arguments[index], work)
    }

    /// Consume an exact result for the same authenticated tuple under one
    /// exclusive writer. No logical atom insertion may intervene. Reservations
    /// may rehash: only the hash survives preparation, never a bucket position.
    /// `argument` is the same already authenticated immutable internal source:
    /// it performs only infallible coordinate reads, with no caller callback or
    /// deferred validation. All permits precede row writes. An arbitrary stateful
    /// or panicking accessor would violate this private publication contract.
    pub(crate) fn publish_prepared_atom_with<E>(
        &mut self,
        prepared: PreparedAtom,
        argument: impl FnMut(usize) -> TermId,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<AtomId, Failure<E>> {
        let mut work = Work::new(&mut before);
        work.step()?;
        self.budget.check()?;
        self.publish_atom(prepared, argument, &mut work)
    }

    fn publish_atom<E>(
        &mut self,
        prepared: PreparedAtom,
        mut argument: impl FnMut(usize) -> TermId,
        work: &mut Work<'_, E>,
    ) -> Result<AtomId, Failure<E>> {
        let (predicate, arity, hash, column) = match prepared.into_preparation() {
            RowPreparation::Present(id) => return Ok(id),
            RowPreparation::Vacant {
                predicate,
                arity,
                hash,
                column,
            } => (predicate, arity, hash, column),
        };
        work.step()?;
        let id = AtomId(next_id(self.counts().atoms)?);
        let local = u32::try_from(
            (id.0 as usize)
                .checked_sub(self.base_count())
                .ok_or(Fault::Shape)?,
        )
        .map_err(|_| Fault::IdExhausted)?;
        let block = self.column_block(predicate, arity, column, work)?;
        for column in &mut self.tail.columns[block].arguments {
            work.reserve(column, 1, &mut self.budget)?;
        }
        work.reserve(&mut self.tail.atoms, 1, &mut self.budget)?;
        self.atoms.reserve_with(hash, &mut self.budget, work)?;
        let columns = &mut self.tail.columns[block];
        let row = columns.rows;
        let rows = row.checked_add(1).ok_or(Fault::Overflow)?;
        // As with relocation, one coordinate-copy permit covers its immutable
        // source read and destination write. Admit the complete copy first.
        work.steps(arity.checked_add(4).ok_or(Fault::Overflow)?)?;
        // The private accessor only reads the validated, unchanged coordinate
        // source; no caller code, validation or allocation occurs among writes.
        for (index, column) in columns.arguments.iter_mut().enumerate() {
            column.push(argument(index));
        }
        columns.rows = rows;
        self.tail.atoms.push(Locator {
            predicate,
            block,
            row,
        });
        self.atoms.insert(hash, local);
        Ok(id)
    }

    /// Exact immutable lookup over an already authenticated, immutable tuple.
    /// Retaining this result requires exclusive access to the same writer until
    /// publication. The input source must stay identical throughout that interval.
    pub(crate) fn prepare_atom_by_with<E>(
        &self,
        predicate: PredicateId,
        arity: usize,
        argument: impl FnMut(usize) -> TermId,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<PreparedAtom, Failure<E>> {
        let mut work = Work::new(&mut before);
        self.prepare_atom(predicate, arity, argument, &mut work)
    }

    fn prepare_atom<E>(
        &self,
        predicate: PredicateId,
        arity: usize,
        mut argument: impl FnMut(usize) -> TermId,
        work: &mut Work<'_, E>,
    ) -> Result<PreparedAtom, Failure<E>> {
        work.step()?;
        if self.signature(predicate).arity != arity {
            return Err(Fault::Shape.into());
        }
        let hash = work.key_hash_by(predicate, arity, &mut argument)?;
        self.prepare_atom_hash(predicate, arity, argument, hash, work)
    }

    fn prepare_atom_hash<E>(
        &self,
        predicate: PredicateId,
        arity: usize,
        argument: impl FnMut(usize) -> TermId,
        hash: u64,
        work: &mut Work<'_, E>,
    ) -> Result<PreparedAtom, Failure<E>> {
        let found = self.find_atom_hash_with(predicate, arity, argument, hash, work)?;
        Ok(PreparedAtom(found.map_or(
            RowPreparation::Vacant {
                predicate,
                arity,
                hash,
                column: None,
            },
            RowPreparation::Present,
        )))
    }

    fn find_atom_hash_with<E>(
        &self,
        predicate: PredicateId,
        arity: usize,
        mut argument: impl FnMut(usize) -> TermId,
        hash: u64,
        work: &mut Work<'_, E>,
    ) -> Result<Option<AtomId>, Failure<E>> {
        if let Some(base) = &self.base
            && let Some(id) = base.index.find_with(hash, work, |id, work| {
                self.atom_matches(AtomId(id), predicate, arity, &mut argument, work)
            })?
        {
            return Ok(Some(AtomId(id)));
        }
        let offset = self.base_count();
        self.atoms
            .find_with(hash, work, |local, work| {
                let id = atom_offset(offset, local)?;
                self.atom_matches(id, predicate, arity, &mut argument, work)
            })?
            .map(|local| atom_offset(offset, local))
            .transpose()
            .map_err(Failure::Storage)
    }

    fn atom_matches<E>(
        &self,
        id: AtomId,
        predicate: PredicateId,
        arity: usize,
        argument: &mut impl FnMut(usize) -> TermId,
        work: &mut Work<'_, E>,
    ) -> Result<bool, Failure<E>> {
        work.step()?;
        let (columns, locator) = self.atom_columns(id);
        if locator.predicate != predicate {
            return Ok(false);
        }
        for index in 0..arity {
            work.step()?;
            if columns.arguments[index][locator.row] != argument(index) {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Candidate location in the current tail, or its next insertion position.
    /// No absence result survives this immutable operation. The consuming
    /// writer must validate the actual block predicate before reusing it.
    pub(crate) fn atom_column_location_with<E>(
        &self,
        predicate: PredicateId,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<usize, E> {
        for (index, column) in self.tail.columns.iter().enumerate() {
            before()?;
            if column.predicate == predicate {
                return Ok(index);
            }
        }
        Ok(self.tail.columns.len())
    }

    fn column_block<E>(
        &mut self,
        predicate: PredicateId,
        arity: usize,
        location: Option<usize>,
        work: &mut Work<'_, E>,
    ) -> Result<usize, Failure<E>> {
        if let Some(index) = location {
            work.step()?;
            if self
                .tail
                .columns
                .get(index)
                .is_some_and(|column| column.predicate == predicate)
            {
                return Ok(index);
            }
        }
        let index = self.atom_column_location_with(predicate, || work.step())?;
        if index < self.tail.columns.len() {
            return Ok(index);
        }
        work.reserve(&mut self.tail.columns, 1, &mut self.budget)?;
        let mut arguments = Vec::new();
        let reserved = (|| {
            work.reserve(&mut arguments, arity, &mut self.budget)?;
            work.steps(arity.checked_add(1).ok_or(Fault::Overflow)?)
        })();
        if let Err(error) = reserved {
            self.budget.used -= budget::capacity(&arguments);
            return Err(error);
        }
        arguments.resize_with(arity, Vec::new);
        let block = self.tail.columns.len();
        self.tail.columns.push(Columns {
            predicate,
            arguments,
            rows: 0,
        });
        Ok(block)
    }

    #[cfg(test)]
    pub(super) fn intern_node(
        &mut self,
        descriptor: ValueNodeRef<'_>,
        children: &[TermId],
        limits: Limits,
    ) -> Result<TermId, Fault> {
        let mut before = || Ok::<_, std::convert::Infallible>(());
        super::control::uncontrolled(self.intern_node_with(
            descriptor,
            children,
            limits,
            &mut Work::new(&mut before),
        ))
    }

    #[cfg(test)]
    pub(super) fn intern_atom(
        &mut self,
        predicate: PredicateId,
        arguments: &[TermId],
    ) -> Result<AtomId, Fault> {
        let mut before = || Ok::<_, std::convert::Infallible>(());
        super::control::uncontrolled(self.intern_atom_with(
            predicate,
            arguments,
            &mut Work::new(&mut before),
        ))
    }
}

fn atom_offset(offset: usize, local: u32) -> Result<AtomId, Fault> {
    let global = offset.checked_add(local as usize).ok_or(Fault::Overflow)?;
    u32::try_from(global)
        .map(AtomId)
        .map_err(|_| Fault::IdExhausted)
}

/// Writer and immutable lookup build exactly the same typed hash key. Only the
/// authority to add a missing spelling differs at their text boundary.
fn node_key<E>(
    descriptor: ValueNodeRef<'_>,
    work: &mut Work<'_, E>,
    mut text: impl FnMut(&str, &mut Work<'_, E>) -> Result<Option<TextId>, Failure<E>>,
) -> Result<Option<NodeKey>, Failure<E>> {
    work.step()?;
    let mut key = NodeKey {
        kind: Kind::Infimum,
        payload: 0,
        name: None,
        sign: Sign::Positive,
    };
    match descriptor {
        ValueNodeRef::Infimum => {}
        ValueNodeRef::Supremum => key.kind = Kind::Supremum,
        ValueNodeRef::Number(number) => {
            key.kind = Kind::Number;
            key.payload = u32::from_ne_bytes(number.to_ne_bytes());
        }
        ValueNodeRef::String(value) | ValueNodeRef::Symbol(value) => {
            key.kind = if matches!(descriptor, ValueNodeRef::String(_)) {
                Kind::String
            } else {
                Kind::Symbol
            };
            let Some(id) = text(value, work)? else {
                return Ok(None);
            };
            key.payload = id.0;
        }
        ValueNodeRef::Function { name, sign, .. } => {
            key.kind = Kind::Function;
            let Some(id) = text(name, work)? else {
                return Ok(None);
            };
            key.name = Some(id);
            key.sign = sign;
        }
        ValueNodeRef::Tuple { .. } => key.kind = Kind::Tuple,
    }
    Ok(Some(key))
}

#[cfg(test)]
mod tests;
