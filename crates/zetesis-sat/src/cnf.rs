use std::fmt;

use crate::{Clause, Clauses};

/// A signed variable reference; variable indices are zero based.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Literal {
    variable: usize,
    positive: bool,
}
impl Literal {
    /// Construct a reference. Its range is validated when a CNF is admitted.
    #[must_use]
    pub const fn new(variable: usize, positive: bool) -> Self {
        Self { variable, positive }
    }
    /// The referenced zero-based variable index.
    #[must_use]
    pub const fn variable(self) -> usize {
        self.variable
    }
    /// True for a positive occurrence, false for a negated occurrence.
    #[must_use]
    pub const fn positive(self) -> bool {
        self.positive
    }
    /// The complementary occurrence of the same variable.
    #[must_use]
    pub const fn negated(self) -> Self {
        Self::new(self.variable, !self.positive)
    }
    pub(crate) fn index(self) -> usize {
        self.variable * 2 + usize::from(self.positive)
    }
}

/// Independent finite storage dimensions, checked before canonicalization.
#[derive(Clone, Copy, Debug)]
pub struct AdmissionLimits {
    /// Maximum variables, including Tseitin variables on the formula path.
    pub max_variables: usize,
    /// Maximum submitted clauses, including actual candidate restrictions.
    /// Exact candidate exclusions use independent `ProjectionLimits`.
    pub max_clauses: usize,
    /// Maximum submitted literal occurrences, before canonicalization.
    /// This is not an allocated-byte limit.
    pub max_literals: usize,
}
impl Default for AdmissionLimits {
    fn default() -> Self {
        Self {
            max_variables: 1_048_576,
            max_clauses: 4_194_304,
            max_literals: 12_582_912,
        }
    }
}

/// The independently bounded CNF dimension.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resource {
    /// Semantic and auxiliary variables.
    Variables,
    /// Asserted disjunctions, including empty clauses.
    Clauses,
    /// Signed references across all submitted clauses.
    Literals,
}

/// A shape, representation, storage or explicit admission refusal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdmissionError {
    /// A caller-specified dimension ceiling was exceeded.
    Limit {
        /// Refused dimension.
        resource: Resource,
        /// Count proposed before canonicalization.
        observed: usize,
        /// Inclusive configured ceiling.
        limit: usize,
    },
    /// A literal references a variable outside the declared universe.
    Variable {
        /// Invalid index.
        variable: usize,
        /// Declared variable count.
        variables: usize,
    },
    /// Shape arithmetic or an internal index exceeds its representation.
    /// Projection exclusions use at most `u32::MAX` trie nodes; other indices
    /// retain their host-sized representation bounds.
    Overflow,
    /// Storage reservation was refused.
    Allocation,
}
impl fmt::Display for AdmissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Limit {
                resource,
                observed,
                limit,
            } => {
                write!(f, "SAT {resource:?} count {observed} exceeds {limit}")
            }
            Self::Variable {
                variable,
                variables,
            } => {
                write!(f, "SAT variable {variable} is outside universe {variables}")
            }
            Self::Overflow => f.write_str("SAT shape or index exceeds its representation"),
            Self::Allocation => f.write_str("SAT storage reservation failed"),
        }
    }
}
impl std::error::Error for AdmissionError {}

pub(crate) fn bound(
    resource: Resource,
    observed: usize,
    limit: usize,
) -> Result<(), AdmissionError> {
    if observed > limit {
        Err(AdmissionError::Limit {
            resource,
            observed,
            limit,
        })
    } else {
        Ok(())
    }
}

/// Finite classical CNF. Duplicate literals are coalesced and tautological
/// clauses discarded; an empty clause remains an explicit contradiction.
/// Admitted references occupy one word each in a single arena, with one end
/// offset per clause. Borrowed views decode public literals without allocating.
/// Admission bounds submitted shape; allocator capacity is not a byte ceiling.
#[derive(Debug)]
pub struct Cnf {
    variables: usize,
    // Admission checks 2 * variables before packing any reference. End offsets
    // are monotone; repeated offsets retain distinct empty clauses.
    literals: Vec<usize>,
    ends: Vec<usize>,
    submitted_clauses: usize,
    submitted_literals: usize,
    limits: AdmissionLimits,
}

#[cfg(test)]
mod arena_tests {
    use super::*;

    #[test]
    fn rollback_retains_one_arena_without_old_clause_contents() {
        let mut cnf = Cnf::new(
            6,
            vec![vec![Literal::new(0, false)], vec![]],
            AdmissionLimits::default(),
        )
        .unwrap();
        let checkpoint = cnf.checkpoint();
        cnf.append(vec![Literal::new(5, true), Literal::new(2, false)])
            .unwrap();
        assert_eq!(cnf.literals, [0, 4, 11]);
        assert_eq!(cnf.ends, [1, 1, 3]);
        let capacities = (cnf.literals.capacity(), cnf.ends.capacity());
        cnf.rollback(checkpoint);
        assert_eq!(cnf.literals, [0]);
        assert_eq!(cnf.ends, [1, 1]);
        assert_eq!((cnf.literals.capacity(), cnf.ends.capacity()), capacities);
        cnf.append(vec![Literal::new(1, true)]).unwrap();
        assert_eq!(cnf.literals, [0, 3]);
        assert_eq!(cnf.ends, [1, 1, 2]);
    }
}

/// Only append-only encoding uses this logical checkpoint; existing clauses
/// never change, and retained allocation capacity is not rolled back.
#[derive(Clone, Copy)]
pub(crate) struct Checkpoint {
    variables: usize,
    clauses: usize,
    submitted_clauses: usize,
    submitted_literals: usize,
}
impl Cnf {
    fn submission(&self, width: usize) -> Result<(usize, usize), AdmissionError> {
        let clauses = self
            .submitted_clauses
            .checked_add(1)
            .ok_or(AdmissionError::Overflow)?;
        let literals = self
            .submitted_literals
            .checked_add(width)
            .ok_or(AdmissionError::Overflow)?;
        bound(Resource::Clauses, clauses, self.limits.max_clauses)?;
        clauses.checked_mul(2).ok_or(AdmissionError::Overflow)?;
        bound(Resource::Literals, literals, self.limits.max_literals)?;
        Ok((clauses, literals))
    }
    pub(crate) fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            variables: self.variables,
            clauses: self.ends.len(),
            submitted_clauses: self.submitted_clauses,
            submitted_literals: self.submitted_literals,
        }
    }
    pub(crate) fn rollback(&mut self, checkpoint: Checkpoint) {
        self.variables = checkpoint.variables;
        self.ends.truncate(checkpoint.clauses);
        self.literals
            .truncate(self.ends.last().copied().unwrap_or(0));
        self.submitted_clauses = checkpoint.submitted_clauses;
        self.submitted_literals = checkpoint.submitted_literals;
    }
    /// Admit owned clauses. Limits count submitted occurrences before duplicate
    /// or tautology elimination. Construction does not perform logical search.
    ///
    /// # Errors
    /// Refuses configured limits, invalid references, overflow or allocation.
    pub fn new(
        variables: usize,
        clauses: Vec<Vec<Literal>>,
        limits: AdmissionLimits,
    ) -> Result<Self, AdmissionError> {
        let mut result = Self::empty(variables, limits)?;
        bound(Resource::Clauses, clauses.len(), limits.max_clauses)?;
        for clause in clauses {
            result.append(clause)?;
        }
        Ok(result)
    }
    pub(crate) fn empty(variables: usize, limits: AdmissionLimits) -> Result<Self, AdmissionError> {
        bound(Resource::Variables, variables, limits.max_variables)?;
        variables.checked_mul(2).ok_or(AdmissionError::Overflow)?;
        Ok(Self {
            variables,
            literals: Vec::new(),
            ends: Vec::new(),
            submitted_clauses: 0,
            submitted_literals: 0,
            limits,
        })
    }
    pub(crate) fn reserve(
        &mut self,
        clauses: usize,
        literals: usize,
    ) -> Result<(), AdmissionError> {
        self.ends
            .try_reserve_exact(clauses.min(self.limits.max_clauses))
            .map_err(|_| AdmissionError::Allocation)?;
        self.literals
            .try_reserve_exact(literals.min(self.limits.max_literals))
            .map_err(|_| AdmissionError::Allocation)
    }

    pub(crate) fn reset(
        &mut self,
        variables: usize,
        limits: AdmissionLimits,
    ) -> Result<(), AdmissionError> {
        bound(Resource::Variables, variables, limits.max_variables)?;
        variables.checked_mul(2).ok_or(AdmissionError::Overflow)?;
        self.variables = variables;
        self.literals.clear();
        self.ends.clear();
        self.submitted_clauses = 0;
        self.submitted_literals = 0;
        self.limits = limits;
        Ok(())
    }

    pub(crate) const fn submitted_counts(&self) -> (usize, usize) {
        (self.submitted_clauses, self.submitted_literals)
    }

    pub(crate) fn retained_bytes(&self) -> u128 {
        (self.literals.capacity() as u128 + self.ends.capacity() as u128)
            * std::mem::size_of::<usize>() as u128
    }
    /// Declared variables, including variables absent from all clauses.
    #[must_use]
    pub const fn variables(&self) -> usize {
        self.variables
    }
    pub(crate) fn fresh(&mut self) -> Result<Literal, AdmissionError> {
        let variables = self
            .variables
            .checked_add(1)
            .ok_or(AdmissionError::Overflow)?;
        bound(Resource::Variables, variables, self.limits.max_variables)?;
        variables.checked_mul(2).ok_or(AdmissionError::Overflow)?;
        let literal = Literal::new(self.variables, true);
        self.variables = variables;
        Ok(literal)
    }
    /// Canonical clauses; clause order follows the input after tautology removal.
    #[must_use]
    pub fn clauses(&self) -> Clauses<'_> {
        Clauses {
            literals: &self.literals,
            ends: &self.ends,
            start: 0,
        }
    }

    /// Canonical clause at a zero-based position, or `None` outside this CNF.
    #[must_use]
    pub fn clause(&self, index: usize) -> Option<Clause<'_>> {
        let end = *self.ends.get(index)?;
        let start = if index == 0 { 0 } else { self.ends[index - 1] };
        Some(Clause(&self.literals[start..end]))
    }

    pub(crate) fn clause_at(&self, index: usize) -> Clause<'_> {
        self.clause(index).expect("admitted clause index")
    }

    pub(crate) fn append(&mut self, mut clause: Vec<Literal>) -> Result<(), AdmissionError> {
        self.append_slice(&mut clause)
    }

    pub(crate) fn append_slice(&mut self, clause: &mut [Literal]) -> Result<(), AdmissionError> {
        let (clauses, literals) = self.submission(clause.len())?;
        for literal in clause.iter() {
            if literal.variable >= self.variables {
                return Err(AdmissionError::Variable {
                    variable: literal.variable,
                    variables: self.variables,
                });
            }
        }
        clause.sort_unstable();
        let tautology = clause.windows(2).any(|pair| {
            pair[0].variable == pair[1].variable && pair[0].positive != pair[1].positive
        });
        if !tautology {
            self.ends
                .try_reserve(1)
                .map_err(|_| AdmissionError::Allocation)?;
            let count = clause
                .iter()
                .enumerate()
                .filter(|(index, literal)| *index == 0 || clause[*index - 1] != **literal)
                .count();
            self.literals
                .try_reserve(count)
                .map_err(|_| AdmissionError::Allocation)?;
            for (index, literal) in clause.iter().enumerate() {
                if index == 0 || clause[index - 1] != *literal {
                    self.literals.push(literal.index());
                }
            }
            self.ends.push(self.literals.len());
        }
        self.submitted_clauses = clauses;
        self.submitted_literals = literals;
        Ok(())
    }
}

/// A total classical assignment in the CNF's declared variable order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Assignment(pub(crate) Vec<bool>);
impl Assignment {
    /// Read one variable, or `None` for an out-of-universe index.
    #[must_use]
    pub fn value(&self, variable: usize) -> Option<bool> {
        self.0.get(variable).copied()
    }
    /// Number of assigned variables.
    #[must_use]
    pub fn variables(&self) -> usize {
        self.0.len()
    }
}
