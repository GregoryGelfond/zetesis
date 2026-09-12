//! Borrowed model rows and predicate-local observation alternatives.
//!
//! Canonical atom order groups equal predicates contiguously. Binary bounds
//! select exactly that group; tuple matching still checks the complete row.
//! A query retains ranges, never copied atoms or materialized matching positions.

use std::cmp::Ordering;
use std::ops::Range;

use zetesis_core::{Atom, Model, Predicate, Sign};

use super::{Error, Pattern, Symbol, Work};

pub(super) struct ModelRows<'a> {
    atoms: Vec<&'a Atom>,
}

impl<'a> ModelRows<'a> {
    pub fn new(model: &'a Model, work: &mut Work<'_>) -> Result<Self, Error> {
        work.step(model.atoms().len() as u128)?;
        let mut atoms = work.reserve(model.atoms().len())?;
        atoms.extend(model.atoms());
        Ok(Self { atoms })
    }

    pub fn get(&self, row: usize) -> &'a Atom {
        self.atoms[row]
    }

    fn bound(
        &self,
        key: (&str, usize, Sign),
        after_equal: bool,
        work: &mut Work<'_>,
    ) -> Result<usize, Error> {
        let mut lower = 0;
        let mut upper = self.atoms.len();
        while lower < upper {
            let middle = lower + (upper - lower) / 2;
            let predicate = self.atoms[middle].predicate();
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

    pub fn predicate(
        &self,
        predicate: &Predicate,
        work: &mut Work<'_>,
    ) -> Result<Range<usize>, Error> {
        self.range(
            (predicate.name(), predicate.arity(), predicate.sign()),
            work,
        )
    }

    pub fn symbol(&self, value: &Symbol, work: &mut Work<'_>) -> Result<Range<usize>, Error> {
        let Symbol::Function {
            name,
            sign,
            arguments,
        } = value
        else {
            unreachable!("an atom key is a signed function")
        };
        self.range(
            (
                name.as_str(),
                arguments.len(),
                crate::coherence::core_sign(*sign),
            ),
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
        work: &mut Work<'_>,
    ) -> Result<Self, Error> {
        work.step(patterns.len() as u128)?;
        let mut ranges = work.reserve(patterns.len())?;
        for pattern in patterns {
            ranges.push(rows.predicate(&pattern.predicate, work)?);
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
