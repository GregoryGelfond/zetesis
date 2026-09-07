//! A fallible streaming Cartesian carrier with O(arity) tuple-index state.

use crate::{Atom, Predicate, Value};
use std::fmt;
use std::iter::FusedIterator;

/// Carrier enumeration could not reserve the next tuple's index/value buffers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CarrierError;
impl fmt::Display for CarrierError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("carrier tuple storage could not be reserved")
    }
}
impl std::error::Error for CarrierError {}

/// Canonical predicate/tuple enumeration. Creating this iterator allocates no
/// tuple state. Each `next` reserves only O(arity) indexes and output values;
/// a capacity error is returned once and terminates the iterator. As with Rust
/// string cloning generally, process-wide allocator failure remains possible.
#[derive(Clone)]
pub struct AtomIter<'a> {
    predicates: &'a [Predicate],
    domain: &'a [Value],
    current: usize,
    coordinates: Vec<usize>,
    started: bool,
    finished: bool,
}
impl<'a> AtomIter<'a> {
    pub(crate) fn new(predicates: &'a [Predicate], domain: &'a [Value]) -> Self {
        Self {
            predicates,
            domain,
            current: 0,
            coordinates: Vec::new(),
            started: false,
            finished: false,
        }
    }
    fn next_atom(&mut self) -> Result<Option<Atom>, CarrierError> {
        loop {
            let Some(predicate) = self.predicates.get(self.current) else {
                return Ok(None);
            };
            if !self.started {
                if self.domain.is_empty() && predicate.arity() != 0 {
                    self.current += 1;
                    continue;
                }
                self.coordinates.clear();
                self.coordinates
                    .try_reserve_exact(predicate.arity())
                    .map_err(|_| CarrierError)?;
                self.coordinates.resize(predicate.arity(), 0);
                self.started = true;
            } else if !advance(&mut self.coordinates, self.domain.len()) {
                self.current += 1;
                self.started = false;
                continue;
            }
            let mut values = Vec::new();
            values
                .try_reserve_exact(predicate.arity())
                .map_err(|_| CarrierError)?;
            for index in &self.coordinates {
                values.push(self.domain[*index].clone());
            }
            return Ok(Some(Atom::from_valid_parts(predicate.clone(), values)));
        }
    }
}
impl Iterator for AtomIter<'_> {
    type Item = Result<Atom, CarrierError>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }
        match self.next_atom() {
            Ok(Some(atom)) => Some(Ok(atom)),
            Ok(None) => {
                self.finished = true;
                None
            }
            Err(error) => {
                self.finished = true;
                Some(Err(error))
            }
        }
    }
}
impl FusedIterator for AtomIter<'_> {}

/// Advance a fixed-length tuple. Empty tuples have exactly one assignment.
pub(crate) fn advance(coordinates: &mut [usize], radix: usize) -> bool {
    for index in coordinates.iter_mut().rev() {
        if *index < radix.saturating_sub(1) {
            *index += 1;
            return true;
        }
        *index = 0;
    }
    false
}
