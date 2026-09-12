//! Borrowed logical views over one admitted literal arena.

use std::iter::FusedIterator;

use crate::Literal;

/// One canonical clause, borrowed from its CNF. Empty clauses remain visible.
/// Iteration decodes admitted references without allocating a literal copy.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Clause<'a>(pub(crate) &'a [usize]);

impl std::fmt::Debug for Clause<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_list().entries(self.iter()).finish()
    }
}

impl<'a> Clause<'a> {
    /// Number of distinct literals retained in this clause.
    #[must_use]
    pub const fn len(self) -> usize {
        self.0.len()
    }

    /// Whether the clause is the explicit empty contradiction.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0.is_empty()
    }

    /// Literal at a zero-based position, or `None` outside this clause.
    #[must_use]
    pub fn get(self, index: usize) -> Option<Literal> {
        self.0.get(index).copied().map(decode)
    }

    /// Canonical literal order: variable index first, then negative before positive.
    pub fn iter(self) -> impl ExactSizeIterator<Item = Literal> + DoubleEndedIterator + 'a {
        self.0.iter().copied().map(decode)
    }

    pub(crate) fn at(self, index: usize) -> Literal {
        decode(self.0[index])
    }
}

fn decode(value: usize) -> Literal {
    Literal::new(value >> 1, value & 1 != 0)
}

/// Ordered, exact-size traversal of canonical clauses. This view borrows the
/// CNF's single arena and retains no per-clause allocation or owned literals.
#[derive(Clone)]
pub struct Clauses<'a> {
    pub(crate) literals: &'a [usize],
    pub(crate) ends: &'a [usize],
    pub(crate) start: usize,
}

impl std::fmt::Debug for Clauses<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_list().entries(self.clone()).finish()
    }
}

impl Clauses<'_> {
    /// Whether this traversal has no remaining clauses.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ends.is_empty()
    }
}

impl<'a> Iterator for Clauses<'a> {
    type Item = Clause<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let (&end, rest) = self.ends.split_first()?;
        let clause = Clause(&self.literals[self.start..end]);
        self.start = end;
        self.ends = rest;
        Some(clause)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.ends.len(), Some(self.ends.len()))
    }
}

impl ExactSizeIterator for Clauses<'_> {}
impl FusedIterator for Clauses<'_> {}
