//! Whole-argument unification over borrowed terms and caller-owned scratch.
//!
//! Predicate selection and join traversal remain with the caller. This shared
//! operation owns no payload and uses the canonical checked term comparator.

use std::fmt;

use crate::{PatternTerms, TemplateTerm, catalog::TermRef};

/// Malformed input to an argument unification, rather than a logical mismatch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnificationError {
    /// The row ended before the requested argument could be read.
    RowTooShort {
        /// Complete pattern arity.
        expected: usize,
        /// Number of row arguments consumed before its end.
        observed: usize,
    },
    /// A row argument remained after the complete pattern matched.
    RowTooLong {
        /// Complete pattern arity; at least one more row argument was present.
        expected: usize,
    },
    /// A pattern variable is outside the supplied assignment.
    Variable {
        /// Requested source-variable coordinate.
        variable: usize,
        /// Number of assignment slots.
        slots: usize,
    },
    /// A fresh capture would exceed the caller's already reserved trail.
    TrailCapacity {
        /// Existing trail capacity, in variable coordinates.
        capacity: usize,
    },
}

impl fmt::Display for UnificationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RowTooShort { expected, observed } => write!(
                formatter,
                "argument row ended after {observed} values; expected {expected}"
            ),
            Self::RowTooLong { expected } => {
                write!(formatter, "argument row exceeds arity {expected}")
            }
            Self::Variable { variable, slots } => {
                write!(
                    formatter,
                    "variable {variable} is outside {slots} assignment slots"
                )
            }
            Self::TrailCapacity { capacity } => {
                write!(
                    formatter,
                    "capture trail exceeds its reserved {capacity} cells"
                )
            }
        }
    }
}

impl std::error::Error for UnificationError {}

/// An invalid argument frame or the original caller's refused permit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UnificationFailure<E> {
    /// Row shape, variable extent or prepared trail capacity is invalid.
    Input(UnificationError),
    /// Work or cancellation stopped before the next operation.
    Stopped(E),
}

impl<E: fmt::Display> fmt::Display for UnificationFailure<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Input(error) => error.fmt(formatter),
            Self::Stopped(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for UnificationFailure<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Input(error) => Some(error),
            Self::Stopped(error) => Some(error),
        }
    }
}

impl PatternTerms<'_> {
    /// Unify a row's complete arguments with these constants and variables.
    /// Predicates are not inspected; the caller selects a compatible signature.
    /// Captures borrow `row`'s terms, and constants use checked structural
    /// equality even across independent canonical owners.
    ///
    /// One permit precedes each argument's metadata/row read and possible
    /// capture. Typed equality requests its own navigation and text permits.
    /// After every argument matches, one final permit precedes checking the
    /// row's end, including for a nullary pattern. At most arity + 1 row values
    /// are requested. A mismatch returns immediately, so its unused suffix is
    /// not inspected or shape-validated. Iterator-internal work remains the
    /// caller's responsibility.
    ///
    /// No allocation occurs. A fresh variable is appended to `changes` only
    /// when its existing capacity suffices, then its assignment slot is set.
    /// Repeated variables see earlier captures from the same row. The caller
    /// reserves/accountably owns the assignment and trail before this call.
    /// Existing trail entries are preserved, and only previously absent slots
    /// are appended. Work is linear in the inspected argument prefix plus
    /// checked equality work; additional storage is constant.
    ///
    /// On mismatch, error or unwind, successful prefix captures remain in the
    /// assignment and trail. The caller must undo that trail suffix before
    /// trying another row, or discard the private frame on failure. `true`
    /// alone establishes a complete argument match, never semantic membership.
    ///
    /// ```
    /// use zetesis_core::{AtomPattern, PatternRef, Predicate, Term, Value};
    /// use zetesis_core::catalog::TermRef;
    /// let pattern = AtomPattern::new(Predicate::new("edge", 2)?,
    ///     vec![Term::Variable(0), Term::Variable(0)])?;
    /// let values = [Value::Number(7), Value::Number(7)];
    /// let mut slots = [None];
    /// let mut changes = Vec::with_capacity(1);
    /// assert!(PatternRef::from(&pattern).terms().unify_with(
    ///     values.iter().map(TermRef::from), &mut slots, &mut changes,
    ///     || Ok::<_, std::convert::Infallible>(()),
    /// )?);
    /// assert_eq!(changes, [0]);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Errors
    /// Returns the first observed row/slot/trail defect or caller refusal. A
    /// refused final end probe is an error, not a successful match. There is no
    /// implicit trail growth or fallback allocation.
    ///
    /// # Panics
    /// Panics if the immutable pattern view violates its own reported length.
    pub fn unify_with<'row, E>(
        self,
        row: impl IntoIterator<Item = TermRef<'row>>,
        assignment: &mut [Option<TermRef<'row>>],
        changes: &mut Vec<usize>,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<bool, UnificationFailure<E>> {
        let mut row = row.into_iter();
        let arity = self.len();
        for column in 0..arity {
            before().map_err(UnificationFailure::Stopped)?;
            let term = self
                .at(column)
                .expect("column is within immutable pattern arity");
            let value =
                row.next()
                    .ok_or(UnificationFailure::Input(UnificationError::RowTooShort {
                        expected: arity,
                        observed: column,
                    }))?;
            let expected = match term {
                TemplateTerm::Constant(expected) => Some(expected),
                TemplateTerm::Variable(variable) => {
                    let slots = assignment.len();
                    let target = assignment
                        .get_mut(variable)
                        .ok_or(UnificationFailure::Input(UnificationError::Variable {
                            variable,
                            slots,
                        }))?;
                    if let Some(expected) = *target {
                        Some(expected)
                    } else {
                        if changes.len() == changes.capacity() {
                            return Err(UnificationFailure::Input(
                                UnificationError::TrailCapacity {
                                    capacity: changes.capacity(),
                                },
                            ));
                        }
                        changes.push(variable);
                        *target = Some(value);
                        None
                    }
                }
            };
            if let Some(expected) = expected
                && !expected
                    .equals_ref_with(value, &mut before)
                    .map_err(UnificationFailure::Stopped)?
            {
                return Ok(false);
            }
        }
        before().map_err(UnificationFailure::Stopped)?;
        if row.next().is_some() {
            return Err(UnificationFailure::Input(UnificationError::RowTooLong {
                expected: arity,
            }));
        }
        Ok(true)
    }
}

#[cfg(test)]
mod tests;
