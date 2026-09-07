use std::fmt;

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
    /// Maximum clauses, including later candidate blocking clauses.
    pub max_clauses: usize,
    /// Maximum literal occurrences across clauses.
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
    /// A count or internal watch index cannot be represented on this host.
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
            Self::Overflow => f.write_str("SAT shape arithmetic overflow"),
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
#[derive(Debug)]
pub struct Cnf {
    variables: usize,
    clauses: Vec<Vec<Literal>>,
    submitted_clauses: usize,
    submitted_literals: usize,
    limits: AdmissionLimits,
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
    pub(crate) fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            variables: self.variables,
            clauses: self.clauses.len(),
            submitted_clauses: self.submitted_clauses,
            submitted_literals: self.submitted_literals,
        }
    }
    pub(crate) fn rollback(&mut self, checkpoint: Checkpoint) {
        self.variables = checkpoint.variables;
        self.clauses.truncate(checkpoint.clauses);
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
            clauses: Vec::new(),
            submitted_clauses: 0,
            submitted_literals: 0,
            limits,
        })
    }
    pub(crate) fn reserve_clauses(&mut self, count: usize) -> Result<(), AdmissionError> {
        self.clauses
            .try_reserve_exact(count.min(self.limits.max_clauses))
            .map_err(|_| AdmissionError::Allocation)
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
    pub fn clauses(&self) -> &[Vec<Literal>] {
        &self.clauses
    }
    pub(crate) fn append(&mut self, mut clause: Vec<Literal>) -> Result<(), AdmissionError> {
        let clauses = self
            .submitted_clauses
            .checked_add(1)
            .ok_or(AdmissionError::Overflow)?;
        let literals = self
            .submitted_literals
            .checked_add(clause.len())
            .ok_or(AdmissionError::Overflow)?;
        bound(Resource::Clauses, clauses, self.limits.max_clauses)?;
        clauses.checked_mul(2).ok_or(AdmissionError::Overflow)?;
        bound(Resource::Literals, literals, self.limits.max_literals)?;
        for literal in &clause {
            if literal.variable >= self.variables {
                return Err(AdmissionError::Variable {
                    variable: literal.variable,
                    variables: self.variables,
                });
            }
        }
        clause.sort_unstable();
        clause.dedup();
        let tautology = clause
            .windows(2)
            .any(|pair| pair[0].variable == pair[1].variable);
        if !tautology {
            self.clauses
                .try_reserve(1)
                .map_err(|_| AdmissionError::Allocation)?;
            self.clauses.push(clause);
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
