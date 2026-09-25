//! Normalized templates keep ordinary positive support separate from gates.

use crate::{Atom, ConstructionError, Predicate, Value};
use std::collections::BTreeSet;
use std::fmt;

pub(crate) mod catalog;
pub use catalog::{
    TemplateCatalog, TemplateCatalogBuilder, TemplateCatalogFailure, TemplateCatalogSelection,
    TemplateComponents, TemplateComponentsRef, TemplateRow,
};

/// A template argument. Variable IDs are local to one template.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Term {
    /// A zero-based variable ID, validated as dense at admission.
    Variable(usize),
    /// A closed value.
    Constant(Value),
}
impl Term {
    /// Resolve against a complete assignment.
    ///
    /// # Errors
    /// Returns the missing variable ID if the assignment is too short.
    pub fn resolve<'a>(&'a self, values: &'a [Value]) -> Result<&'a Value, InstantiationError> {
        match self {
            Self::Variable(variable) => values.get(*variable).ok_or(InstantiationError {
                variable: *variable,
            }),
            Self::Constant(value) => Ok(value),
        }
    }
}

/// An arity-checked relational pattern.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AtomPattern {
    predicate: Predicate,
    terms: Vec<Term>,
}
impl AtomPattern {
    /// Construct a pattern with exactly the signature's arity.
    ///
    /// # Errors
    /// Returns [`ConstructionError::ArityMismatch`] for the wrong term count.
    pub fn new(predicate: Predicate, terms: Vec<Term>) -> Result<Self, ConstructionError> {
        if predicate.arity() != terms.len() {
            return Err(ConstructionError::ArityMismatch {
                expected: predicate.arity(),
                actual: terms.len(),
            });
        }
        Ok(Self { predicate, terms })
    }
    /// The pattern's predicate signature.
    #[must_use]
    pub fn predicate(&self) -> &Predicate {
        &self.predicate
    }
    /// Arguments in normalized order.
    #[must_use]
    pub fn terms(&self) -> &[Term] {
        &self.terms
    }
    /// Substitute a complete assignment, preserving argument order.
    ///
    /// # Errors
    /// Returns the first missing variable; constants do not read the assignment.
    pub fn instantiate(&self, assignment: &[Value]) -> Result<Atom, InstantiationError> {
        self.key(assignment)?;
        let values = self
            .terms
            .iter()
            .map(|term| match term {
                Term::Constant(value) => value.clone(),
                Term::Variable(variable) => assignment[*variable].clone(),
            })
            .collect();
        Ok(Atom::from_valid_parts(self.predicate.clone(), values))
    }
}

/// An exact comparison on closed values after substitution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Filter {
    /// Both resolved values must be equal, including their value class.
    Eq(Term, Term),
    /// The resolved values must differ.
    Neq(Term, Term),
}
impl Filter {
    /// The compared terms.
    #[must_use]
    pub fn terms(&self) -> (&Term, &Term) {
        match self {
            Self::Eq(left, right) | Self::Neq(left, right) => (left, right),
        }
    }
    /// Evaluate with exact value equality.
    ///
    /// # Errors
    /// Returns the first variable not present in the assignment.
    pub fn evaluate(&self, assignment: &[Value]) -> Result<bool, InstantiationError> {
        crate::program::FilterRef::from(self).evaluate(assignment)
    }
}

/// A normalized rule. `None` is a constraint; gates never seed positive support.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Template {
    head: Option<AtomPattern>,
    positive: Vec<AtomPattern>,
    gate_true: Vec<AtomPattern>,
    gate_false: Vec<AtomPattern>,
    filters: Vec<Filter>,
    variable_count: usize,
}
impl Template {
    /// Assemble a template. Program admission checks safety, dense variable IDs,
    /// and budgets. Input template order and duplicate templates are preserved.
    #[must_use]
    pub fn new(
        head: Option<AtomPattern>,
        positive: Vec<AtomPattern>,
        gate_true: Vec<AtomPattern>,
        gate_false: Vec<AtomPattern>,
        filters: Vec<Filter>,
    ) -> Self {
        let mut template = Self {
            head,
            positive,
            gate_true,
            gate_false,
            filters,
            variable_count: 0,
        };
        template.variable_count = template.variables().len();
        template
    }
    /// The consequence pattern, or `None` for a constraint.
    #[must_use]
    pub fn head(&self) -> Option<&AtomPattern> {
        self.head.as_ref()
    }
    /// Ordinary positive antecedents, evaluated against growing consequences.
    #[must_use]
    pub fn positive(&self) -> &[AtomPattern] {
        &self.positive
    }
    /// Patterns required true in the frozen candidate.
    #[must_use]
    pub fn gate_true(&self) -> &[AtomPattern] {
        &self.gate_true
    }
    /// Patterns required false in the frozen candidate.
    #[must_use]
    pub fn gate_false(&self) -> &[AtomPattern] {
        &self.gate_false
    }
    /// Comparisons evaluated after their arguments are bound.
    #[must_use]
    pub fn filters(&self) -> &[Filter] {
        &self.filters
    }
    /// Distinct variables; admitted templates use exactly IDs `0..count`.
    #[must_use]
    pub fn variable_count(&self) -> usize {
        self.variable_count
    }
    pub(crate) fn patterns(&self) -> impl Iterator<Item = &AtomPattern> {
        self.head
            .iter()
            .chain(&self.positive)
            .chain(&self.gate_true)
            .chain(&self.gate_false)
    }
    pub(crate) fn all_terms(&self) -> impl Iterator<Item = &Term> {
        self.patterns()
            .flat_map(AtomPattern::terms)
            .chain(self.filters.iter().flat_map(|filter| {
                let (left, right) = filter.terms();
                [left, right]
            }))
    }
    pub(crate) fn variables(&self) -> BTreeSet<usize> {
        self.all_terms()
            .filter_map(|term| match term {
                Term::Variable(variable) => Some(*variable),
                Term::Constant(_) => None,
            })
            .collect()
    }
}

/// An assignment omitted a variable required by a pattern or comparison.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstantiationError {
    /// The absent zero-based variable ID.
    pub variable: usize,
}
impl fmt::Display for InstantiationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "assignment does not bind variable {}", self.variable)
    }
}
impl std::error::Error for InstantiationError {}
