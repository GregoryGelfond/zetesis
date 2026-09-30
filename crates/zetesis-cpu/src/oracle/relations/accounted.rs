//! Relations in predicate order with the exact sum of their retained capacity.
//!
//! Accounting reads the sum on every storage decision, and a candidate can hold
//! thousands of relations, so the sum is maintained rather than recomputed. The
//! fields are private to this module: a relation's capacity can change only
//! through [`RelationMut`], whose release records the change.

use std::{
    mem::size_of,
    ops::{Deref, DerefMut},
};

use super::{Relation, Stop, Work, storage};

/// The candidate's relations in semantic predicate order and the sum of their
/// retained capacities, exact at every read.
#[derive(Default)]
pub(in crate::oracle) struct AccountedRelations {
    relations: Vec<Relation>,
    retained: u128,
}

impl AccountedRelations {
    pub(super) fn as_slice(&self) -> &[Relation] {
        &self.relations
    }

    pub(super) fn len(&self) -> usize {
        self.relations.len()
    }

    /// Capacity of the directory itself: one relation header per slot.
    pub(super) fn directory_bytes(&self) -> u128 {
        self.relations.capacity() as u128 * size_of::<Relation>() as u128
    }

    /// Summed retained capacity of the relations, read without visiting them.
    /// Debug builds recompute the sum and check it.
    pub(super) fn retained_bytes(&self) -> u128 {
        debug_assert_eq!(
            self.retained,
            self.relations
                .iter()
                .map(Relation::retained_bytes)
                .sum::<u128>(),
            "every relation capacity change is recorded"
        );
        self.retained
    }

    /// Reserve directory slots under the closure storage ceiling, as
    /// [`storage::reserve`] does; relation contents are unchanged.
    pub(super) fn reserve(
        &mut self,
        additional: usize,
        live: u128,
        work: &mut Work<'_>,
    ) -> Result<u128, Stop> {
        storage::reserve(&mut self.relations, additional, live, work)
    }

    /// Place a new relation at its predicate-order position and add its
    /// capacity. The caller has reserved the slot.
    pub(super) fn insert(&mut self, position: usize, relation: Relation) {
        self.retained += relation.retained_bytes();
        self.relations.insert(position, relation);
    }

    /// Borrow one relation for a change. Its capacity is measured now and again
    /// when the borrow ends, whether by completion, an early return or
    /// unwinding, and the sum takes the difference.
    pub(super) fn get_mut(&mut self, index: usize) -> Option<RelationMut<'_>> {
        let relation = self.relations.get_mut(index)?;
        let before = relation.retained_bytes();
        Some(RelationMut {
            relation,
            retained: &mut self.retained,
            before,
        })
    }
}

/// One relation borrowed for a change, with the sum it contributes to.
pub(in crate::oracle) struct RelationMut<'a> {
    relation: &'a mut Relation,
    retained: &'a mut u128,
    before: u128,
}

impl Deref for RelationMut<'_> {
    type Target = Relation;
    fn deref(&self) -> &Relation {
        self.relation
    }
}

impl DerefMut for RelationMut<'_> {
    fn deref_mut(&mut self) -> &mut Relation {
        self.relation
    }
}

impl Drop for RelationMut<'_> {
    // The sum contains `before`, so replacing it with the current capacity
    // cannot underflow.
    fn drop(&mut self) {
        *self.retained = *self.retained - self.before + self.relation.retained_bytes();
    }
}
