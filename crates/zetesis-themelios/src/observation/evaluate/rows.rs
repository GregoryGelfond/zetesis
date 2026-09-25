//! Borrowed model rows and predicate-local observation alternatives.
//!
//! Canonical atom order groups equal predicates contiguously. Binary bounds
//! select exactly that group; tuple matching still checks the complete row.
//! A query retains ranges, never copied atoms or materialized matching positions.

use std::cmp::Ordering;
use std::ops::Range;

use zetesis_core::{
    Model, Sign,
    catalog::{AtomRef, PredicateRef},
};

use super::{Error, Pattern, Work};

pub(super) struct ModelRows<'a> {
    model: &'a Model,
}

impl<'a> ModelRows<'a> {
    pub fn new(model: &'a Model, work: &mut Work<'_>) -> Result<Self, Error> {
        work.step(1)?;
        Ok(Self { model })
    }

    pub fn get(&self, row: usize) -> AtomRef<'a> {
        self.model.atoms().at(row).expect("admitted model row")
    }

    fn bound(
        &self,
        key: (&str, usize, Sign),
        after_equal: bool,
        work: &mut Work<'_>,
    ) -> Result<usize, Error> {
        let mut lower = 0;
        let mut upper = self.model.atoms().len();
        while lower < upper {
            let middle = lower + (upper - lower) / 2;
            work.step(1)?;
            let predicate = self.get(middle).predicate();
            work.step(1 + key.0.len() as u128 + predicate.name().len() as u128)?;
            // Predicate::Ord is name, arity, sign. No owned key is constructed.
            let order = (predicate.name(), predicate.arity(), predicate.sign()).cmp(&key);
            if order == Ordering::Less || (after_equal && order == Ordering::Equal) {
                lower = middle + 1;
            } else {
                upper = middle;
            }
        }
        Ok(lower)
    }

    fn range(&self, key: (&str, usize, Sign), work: &mut Work<'_>) -> Result<Range<usize>, Error> {
        let lower = self.bound(key, false, work)?;
        let upper = self.bound(key, true, work)?;
        Ok(lower..upper)
    }

    pub fn predicate<'predicate>(
        &self,
        predicate: impl Into<PredicateRef<'predicate>>,
        work: &mut Work<'_>,
    ) -> Result<Range<usize>, Error> {
        let predicate = predicate.into();
        self.range(
            (predicate.name(), predicate.arity(), predicate.sign()),
            work,
        )
    }
}

/// Original alternative order followed by canonical row order within each.
/// Exhaustion restores the initial cursor for the next enclosing binding.
pub(super) struct AtomChoices {
    ranges: Vec<Range<usize>>,
    alternative: usize,
    position: usize,
}

impl AtomChoices {
    pub fn new(
        patterns: &[Pattern],
        rows: &ModelRows<'_>,
        metadata: crate::metadata::Read<'_>,
        work: &mut Work<'_>,
    ) -> Result<Self, Error> {
        work.step(patterns.len() as u128)?;
        let mut ranges = work.reserve(patterns.len())?;
        for pattern in patterns {
            ranges.push(
                rows.predicate(
                    metadata
                        .predicate(pattern.predicate)
                        .expect("compiled predicate"),
                    work,
                )?,
            );
        }
        Ok(Self {
            ranges,
            alternative: 0,
            position: 0,
        })
    }

    pub fn next(&mut self, work: &mut Work<'_>) -> Result<Option<(usize, usize)>, Error> {
        while let Some(range) = self.ranges.get(self.alternative) {
            work.step(1)?;
            if self.position < range.len() {
                let row = range.start + self.position;
                self.position += 1;
                return Ok(Some((self.alternative, row)));
            }
            self.alternative += 1;
            self.position = 0;
        }
        self.alternative = 0;
        self.position = 0;
        Ok(None)
    }
}
