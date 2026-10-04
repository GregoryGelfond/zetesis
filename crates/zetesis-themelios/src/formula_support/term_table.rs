//! Sparse term coordinates over one source vocabulary, independent of ASP order.

use crate::ProgramSite;
use crate::formula_support::Context;
use zetesis_core::catalog::{CatalogRead, TermKey, TermRef};

use super::{Computation, Counters, StorageLease, reserve};
use crate::formula::ceiling;
use crate::formula_binding::Binding;
use crate::{FormulaFailure, FormulaLimits, FormulaResource};

/// A first-occurrence coordinate and a sorted identity index. Payload belongs
/// to Computation; each cell is an integer and one frame retains the scope.
/// Sparse indexing avoids allocating by the largest unrelated source term ID.
pub(crate) struct TermTable {
    values: Binding<'static>,
    order: Vec<usize>,
    lease: StorageLease,
}
impl TermTable {
    pub(crate) fn new(
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Self, FormulaFailure> {
        let values = Binding::new(computation, limits, counters, location)?;
        let mut lease = computation.lease();
        let header = size_of::<Self>() - size_of::<Binding>();
        lease.observe(header, location)?;
        computation.storage_observed(&lease, 0, header, limits, counters, location)?;
        Ok(Self {
            values,
            order: Vec::new(),
            lease,
        })
    }
    pub(crate) fn value<'a>(
        &self,
        slot: usize,
        read: CatalogRead<'a>,
        location: ProgramSite,
    ) -> Result<TermRef<'a>, FormulaFailure> {
        self.values.read(slot, read, location)
    }
    pub(crate) fn key(
        &self,
        slot: usize,
        location: ProgramSite,
    ) -> Result<TermKey, FormulaFailure> {
        self.values.key(slot, location)
    }
    pub(crate) fn find(
        &self,
        key: &TermKey,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Option<usize>, FormulaFailure> {
        self.authenticate(key, computation, limits, counters, location)?;
        self.search(key, limits, counters, location)
            .map(|entry| entry.ok().map(|at| self.order[at]))
    }
    pub(crate) fn insert(
        &mut self,
        key: &TermKey,
        bound: Option<(FormulaResource, usize)>,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<usize, FormulaFailure> {
        self.authenticate(key, computation, limits, counters, location)?;
        let at = match self.search(key, limits, counters, location)? {
            Ok(at) => return Ok(self.order[at]),
            Err(at) => at,
        };
        let slot = self.values.len();
        if let Some(bound) = bound {
            ceiling(bound.0, slot as u128 + 1, bound.1 as u128, location)?;
        }
        reserve(
            &mut self.order,
            1,
            &mut self.lease,
            size_of::<Self>() - size_of::<Binding>(),
            Context::new(computation, limits, counters, location),
        )?;
        counters.charge_work((self.order.len() - at) as u128 + 1, limits, location)?;
        let prepared = self
            .values
            .extend_scope(slot + 1, computation, limits, counters, location)
            .and_then(|()| self.values.set(slot, key, limits, counters, location));
        if let Err(error) = prepared {
            self.values.discard_suffix(slot);
            return Err(error);
        }
        self.order.insert(at, slot);
        Ok(slot)
    }
    /// The empty prefix authenticates the table witness without scanning its
    /// coordinates. The selected key is separately checked against the exact
    /// accessible term prefix; unrelated later table entries need no payload read.
    fn authenticate(
        &self,
        key: &TermKey,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        self.values
            .slots()
            .prefix(0)
            .expect("the empty prefix always exists")
            .bind_with(computation.read(), || counters.work(limits, location))
            .map_err(|error| crate::formula_binding::failure(error, location))?;
        counters.work(limits, location)?;
        computation.read().term(key).map_err(|error| {
            crate::formula_binding::assignment(
                zetesis_core::catalog::AssignmentError::Read(error),
                location,
            )
        })?;
        Ok(())
    }
    fn search(
        &self,
        key: &TermKey,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Result<usize, usize>, FormulaFailure> {
        let mut start = 0;
        let mut end = self.order.len();
        while start < end {
            counters.work(limits, location)?;
            let middle = start + (end - start) / 2;
            match self
                .values
                .slots()
                .compare_key(self.order[middle], key)
                .map_err(|error| crate::formula_binding::assignment(error, location))?
            {
                std::cmp::Ordering::Less => start = middle + 1,
                std::cmp::Ordering::Greater => end = middle,
                std::cmp::Ordering::Equal => return Ok(Ok(middle)),
            }
        }
        Ok(Err(start))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formula_support::testing::Fixture;
    use themelios_base::{
        source::SourceId,
        span::{ByteOffset, Span},
    };
    use zetesis_core::{
        ValueNodeRef,
        catalog::{AssignmentError, ReadError},
    };
    fn location() -> ProgramSite {
        ProgramSite::source(themelios_base::span::Location {
            source: SourceId::new(93),
            span: Span::empty(ByteOffset::new(0)),
        })
    }

    fn foreign(error: &FormulaFailure) -> bool {
        matches!(
            error,
            FormulaFailure::TermAssignment {
                error: AssignmentError::Read(ReadError::ForeignCatalog),
                ..
            }
        )
    }
    #[test]
    fn empty_lookup_authenticates_the_table_vocabulary() {
        let mut first = Fixture::default();
        let table = first.with(location(), |_, computation, counters| {
            TermTable::new(computation, &FormulaLimits::default(), counters, location()).unwrap()
        });
        Fixture::default().with(location(), |_, computation, counters| {
            let limits = FormulaLimits::default();
            let key = computation
                .number(7, &limits, counters, location())
                .unwrap();
            assert!(foreign(
                &table
                    .find(&key, computation, &limits, counters, location())
                    .unwrap_err()
            ));
        });
    }
    #[test]
    fn empty_insertion_does_not_adopt_another_vocabulary() {
        let mut first = Fixture::default();
        let mut table = first.with(location(), |_, computation, counters| {
            TermTable::new(computation, &FormulaLimits::default(), counters, location()).unwrap()
        });
        Fixture::default().with(location(), |_, computation, counters| {
            let limits = FormulaLimits::default();
            let key = computation
                .number(7, &limits, counters, location())
                .unwrap();
            assert!(foreign(
                &table
                    .insert(&key, None, computation, &limits, counters, location())
                    .unwrap_err()
            ));
        });
        first.with(location(), |_, computation, counters| {
            let limits = FormulaLimits::default();
            let key = computation
                .number(7, &limits, counters, location())
                .unwrap();
            assert_eq!(
                table
                    .insert(&key, None, computation, &limits, counters, location())
                    .unwrap(),
                0
            );
        });
    }
    fn prepared(
        computation: &mut Computation<'_, '_>,
        counters: &mut Counters,
    ) -> (TermTable, [TermKey; 3]) {
        let limits = FormulaLimits::default();
        // Admission order, semantic number order and table occurrence order
        // intentionally differ, exercising movement within the identity index.
        let keys = [9, 3, 7].map(|value| {
            computation
                .number(value, &limits, counters, location())
                .unwrap()
        });
        let mut table = TermTable::new(computation, &limits, counters, location()).unwrap();
        assert_eq!(
            table
                .insert(&keys[2], None, computation, &limits, counters, location())
                .unwrap(),
            0
        );
        assert_eq!(
            table
                .insert(&keys[0], None, computation, &limits, counters, location())
                .unwrap(),
            1
        );
        (table, keys)
    }
    fn retry(
        mut table: TermTable,
        keys: &[TermKey; 3],
        computation: &Computation<'_, '_>,
        counters: &mut Counters,
    ) {
        let limits = FormulaLimits::default();
        assert_eq!(
            table
                .insert(&keys[1], None, computation, &limits, counters, location())
                .unwrap(),
            2
        );
        for (key, expected) in [(2, 0), (0, 1), (1, 2)] {
            assert_eq!(
                table
                    .find(&keys[key], computation, &limits, counters, location())
                    .unwrap(),
                Some(expected)
            );
        }
    }
    #[test]
    fn coordinates_follow_occurrence_not_term_order() {
        Fixture::default().with(location(), |_, computation, counters| {
            let (mut table, keys) = prepared(computation, counters);
            let limits = FormulaLimits::default();
            assert_eq!(
                table
                    .insert(&keys[1], None, computation, &limits, counters, location())
                    .unwrap(),
                2
            );
            for (slot, number) in [7, 9, 3].into_iter().enumerate() {
                assert_eq!(
                    table
                        .value(slot, computation.read(), location())
                        .unwrap()
                        .descriptor(),
                    ValueNodeRef::Number(number)
                );
            }
            assert_eq!(table.order, vec![1, 2, 0]);
        });
    }
    #[test]
    fn every_work_stop_preserves_coordinates_for_retry() {
        let needed = Fixture::default().with(location(), |_, computation, counters| {
            let (mut table, keys) = prepared(computation, counters);
            let before = counters.accounting.work;
            table
                .insert(
                    &keys[1],
                    None,
                    computation,
                    &FormulaLimits::default(),
                    counters,
                    location(),
                )
                .unwrap();
            counters.accounting.work - before
        });
        for cutoff in 0..needed {
            Fixture::default().with(location(), |_, computation, counters| {
                let (mut table, keys) = prepared(computation, counters);
                let limits = FormulaLimits {
                    max_work: counters.accounting.work + cutoff,
                    ..Default::default()
                };
                assert!(
                    table
                        .insert(&keys[1], None, computation, &limits, counters, location())
                        .is_err()
                );
                assert_eq!(table.values.len(), 2);
                assert_eq!(table.order.len(), 2);
                retry(table, &keys, computation, counters);
            });
        }
    }
    #[test]
    fn storage_refusal_preserves_coordinates_for_retry() {
        let mut refused = 0;
        for extra in 0..128 {
            Fixture::default().with(location(), |_, computation, counters| {
                let (mut table, keys) = prepared(computation, counters);
                let limits = FormulaLimits::default();
                let observer = computation.lease();
                let current = limits.max_support_bytes
                    - computation
                        .allowance(&observer, &limits, location())
                        .unwrap();
                let bounded = FormulaLimits {
                    max_support_bytes: current + extra,
                    ..limits
                };
                if table
                    .insert(&keys[1], None, computation, &bounded, counters, location())
                    .is_err()
                {
                    refused += 1;
                    assert_eq!(table.values.len(), 2);
                    assert_eq!(table.order.len(), 2);
                    retry(table, &keys, computation, counters);
                }
            });
        }
        assert!(refused > 0);
    }
}
