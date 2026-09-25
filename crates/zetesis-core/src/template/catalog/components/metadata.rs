//! One scope and capacity ledger for independent components and grouped rows.

use super::super::TemplateCatalogFailure as Failure;
use super::{FilterData, PatternData, TermData};
use crate::catalog::storage::{PredicateId, TermId, TextId, VocabularyScope};
use crate::catalog::{CatalogRead, ConstructorData, Error, ReadError};
use crate::{FilterRef, PatternRef, TemplateTerm};

#[derive(Debug)]
pub(in crate::template::catalog) struct Metadata {
    scope: VocabularyScope,
    pub(in crate::template::catalog) bytes: u128,
    pub(in crate::template::catalog) peak: u128,
    term_extent: Option<TermId>,
    predicate_extent: Option<PredicateId>,
    text_extent: Option<TextId>,
    failed: bool,
}
impl Metadata {
    pub(in crate::template::catalog) fn new(
        read: CatalogRead<'_>,
        header: usize,
        limit: usize,
    ) -> Result<Self, Failure> {
        check(header as u128, limit)?;
        Ok(Self {
            scope: read.storage().vocabulary_scope(),
            bytes: header as u128,
            peak: header as u128,
            term_extent: None,
            predicate_extent: None,
            text_extent: None,
            failed: false,
        })
    }

    pub(in crate::template::catalog) fn begin<E>(
        &mut self,
        read: CatalogRead<'_>,
        limit: usize,
        before: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<(), Failure<E>> {
        if self.failed {
            return Err(Failure::Incomplete);
        }
        // Set before even calling caller code: a caught unwind cannot expose a
        // partially appended sequence or an abandoned private allocation.
        self.failed = true;
        self.scope(read, before)?;
        check(self.bytes, limit)
    }

    pub(in crate::template::catalog) fn complete(&mut self) {
        self.failed = false;
    }

    fn scope<E>(
        &self,
        read: CatalogRead<'_>,
        before: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<(), Failure<E>> {
        before().map_err(Failure::Stopped)?;
        if read.storage().accepts_vocabulary_scope(&self.scope) {
            Ok(())
        } else {
            Err(ReadError::ForeignCatalog.into())
        }
    }

    pub(in crate::template::catalog) fn bind<E>(
        &self,
        read: CatalogRead<'_>,
        before: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<(), Failure<E>> {
        if self.failed {
            return Err(Failure::Incomplete);
        }
        self.scope(read, before)?;
        let read = read.storage();
        before().map_err(Failure::Stopped)?;
        if self.term_extent.is_some_and(|id| !read.contains_term(id)) {
            return Err(ReadError::OutsidePrefix.into());
        }
        before().map_err(Failure::Stopped)?;
        if self
            .predicate_extent
            .is_some_and(|id| !read.contains_predicate(id))
        {
            return Err(ReadError::OutsidePrefix.into());
        }
        before().map_err(Failure::Stopped)?;
        if self.text_extent.is_some_and(|id| !read.contains_text(id)) {
            return Err(ReadError::OutsidePrefix.into());
        }
        Ok(())
    }

    pub(in crate::template::catalog) fn terms<'a, E>(
        &mut self,
        output: &mut Vec<TermData>,
        read: CatalogRead<'_>,
        terms: impl IntoIterator<Item = TemplateTerm<'a>>,
        limit: usize,
        before: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<(), Failure<E>> {
        let mut terms = terms.into_iter();
        while let Some(term) = next(&mut terms, before)? {
            self.reserve(output, 1, limit, before)?;
            let term = self.term(read, term, before)?;
            before().map_err(Failure::Stopped)?;
            output.push(term);
        }
        Ok(())
    }

    pub(in crate::template::catalog) fn pattern<E>(
        &mut self,
        output: &mut Vec<PatternData>,
        read: CatalogRead<'_>,
        pattern: PatternRef<'_>,
        limit: usize,
        before: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<(), Failure<E>> {
        self.reserve(output, 1, limit, before)?;
        before().map_err(Failure::Stopped)?;
        let predicate = read.selected_predicate(pattern.predicate())?;
        self.predicate(predicate);
        before().map_err(Failure::Stopped)?;
        output.push(PatternData {
            predicate,
            terms: Vec::new(),
        });
        let output = &mut output.last_mut().expect("just appended pattern").terms;
        before().map_err(Failure::Stopped)?;
        let terms = pattern.terms();
        self.reserve(output, terms.len(), limit, before)?;
        self.terms(output, read, terms, limit, before)
    }

    pub(in crate::template::catalog) fn filter<E>(
        &mut self,
        output: &mut Vec<FilterData>,
        read: CatalogRead<'_>,
        filter: FilterRef<'_>,
        limit: usize,
        before: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<(), Failure<E>> {
        self.reserve(output, 1, limit, before)?;
        before().map_err(Failure::Stopped)?;
        let (left, right) = filter.terms();
        let equal = filter.is_equality();
        let left = self.term(read, left, before)?;
        let right = self.term(read, right, before)?;
        before().map_err(Failure::Stopped)?;
        output.push(FilterData { equal, left, right });
        Ok(())
    }

    pub(in crate::template::catalog) fn constructor(&mut self, data: ConstructorData) {
        if let Some(id) = data.text() {
            self.text_extent = Some(self.text_extent.map_or(id, |old| old.max(id)));
        }
    }

    pub(in crate::template::catalog) fn predicate(&mut self, id: PredicateId) {
        self.predicate_extent = Some(self.predicate_extent.map_or(id, |old| old.max(id)));
    }

    fn term<E>(
        &mut self,
        read: CatalogRead<'_>,
        term: TemplateTerm<'_>,
        before: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<TermData, Failure<E>> {
        before().map_err(Failure::Stopped)?;
        match term {
            TemplateTerm::Variable(variable) => Ok(TermData::Variable(variable)),
            TemplateTerm::Constant(value) => {
                let id = read.selected_term(value)?;
                self.term_extent = Some(self.term_extent.map_or(id, |old| old.max(id)));
                Ok(TermData::Constant(id))
            }
        }
    }

    pub(in crate::template::catalog) fn reserve<T, E>(
        &mut self,
        values: &mut Vec<T>,
        additional: usize,
        limit: usize,
        before: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<(), Failure<E>> {
        let required = values
            .len()
            .checked_add(additional)
            .ok_or(Error::Overflow)?;
        if required <= values.capacity() {
            return Ok(());
        }
        let preferred = values
            .capacity()
            .checked_mul(2)
            .unwrap_or(required)
            .max(required)
            .max(4);
        let cell = size_of::<T>() as u128;
        let target = if self.bytes + preferred as u128 * cell <= limit as u128 {
            preferred
        } else {
            required
        };
        check(self.bytes + target as u128 * cell, limit)?;
        for _ in 0..values.len() {
            before().map_err(Failure::Stopped)?;
        }
        before().map_err(Failure::Stopped)?;
        let old = values.capacity() as u128 * cell;
        values
            .try_reserve_exact(target - values.len())
            .map_err(|_| Error::Allocation)?;
        let actual = values.capacity() as u128 * cell;
        let overlap = self.bytes + actual;
        // Synchronize capacity before any post-allocation refusal.
        self.bytes += actual - old;
        self.peak = self.peak.max(overlap);
        check(overlap, limit)
    }
}

pub(in crate::template::catalog) fn next<T, E>(
    values: &mut impl Iterator<Item = T>,
    before: &mut impl FnMut() -> Result<(), E>,
) -> Result<Option<T>, Failure<E>> {
    before().map_err(Failure::Stopped)?;
    Ok(values.next())
}

pub(in crate::template::catalog) fn check<E>(
    required: u128,
    limit: usize,
) -> Result<(), Failure<E>> {
    if required > limit as u128 {
        Err(Error::Storage { required, limit }.into())
    } else {
        Ok(())
    }
}
