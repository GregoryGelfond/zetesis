//! Admission establishes a finite symbolic domain and safe dense variables.

use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;

use crate::{Atom, AtomIter, AtomPattern, Predicate, Template, Term, Value};

const DEFAULT_MAX_TEMPLATES: usize = 100_000;
const DEFAULT_MAX_PREDICATE_ARITY: usize = 32;
const DEFAULT_MAX_VARIABLES: usize = 64;
const DEFAULT_MAX_POSITIVE_BODY: usize = 1_024;
const DEFAULT_MAX_DOMAIN_VALUES: usize = 1_000_000;

/// Template-level admission budgets. Zero is a real limit, never unlimited.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdmissionLimits {
    /// Maximum number of source-normalized templates.
    pub max_templates: usize,
    /// Maximum predicate arity.
    pub max_predicate_arity: usize,
    /// Maximum distinct variables in any one template.
    pub max_variables_per_template: usize,
    /// Maximum ordinary positive antecedents per template.
    pub max_positive_body: usize,
    /// Maximum distinct values appearing anywhere, including filters/gates.
    pub max_domain_values: usize,
}
impl Default for AdmissionLimits {
    fn default() -> Self {
        Self {
            max_templates: DEFAULT_MAX_TEMPLATES,
            max_predicate_arity: DEFAULT_MAX_PREDICATE_ARITY,
            max_variables_per_template: DEFAULT_MAX_VARIABLES,
            max_positive_body: DEFAULT_MAX_POSITIVE_BODY,
            max_domain_values: DEFAULT_MAX_DOMAIN_VALUES,
        }
    }
}

/// A resource counted by source admission, before any carrier expansion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdmissionResource {
    /// Program template count.
    Templates,
    /// Predicate argument count.
    PredicateArity,
    /// Distinct variables within one template.
    Variables,
    /// Ordinary positive antecedent count.
    PositiveBody,
    /// Distinct closed values.
    DomainValues,
}

/// A typed admission refusal. Template indices refer to the original input order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AdmissionError {
    /// A configured bound was exceeded.
    LimitExceeded {
        /// Which count exceeded its bound.
        resource: AdmissionResource,
        /// Configured maximum.
        limit: usize,
        /// Observed count.
        actual: usize,
        /// Original template index, or `None` for a program-wide count.
        template: Option<usize>,
    },
    /// A variable is used without an ordinary positive binding antecedent.
    UnsafeVariable {
        /// Original template index.
        template: usize,
        /// Unbound variable ID.
        variable: usize,
    },
    /// Variable IDs do not form the dense range starting at zero.
    NonDenseVariable {
        /// Original template index.
        template: usize,
        /// Next required ID.
        expected: usize,
        /// Actual ID at that sorted position.
        actual: usize,
    },
}
impl AdmissionError {
    /// Locate a template-level refusal without parsing a message.
    #[must_use]
    pub fn template_index(&self) -> Option<usize> {
        match self {
            Self::LimitExceeded { template, .. } => *template,
            Self::UnsafeVariable { template, .. } | Self::NonDenseVariable { template, .. } => {
                Some(*template)
            }
        }
    }
}
impl fmt::Display for AdmissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LimitExceeded {
                resource,
                limit,
                actual,
                template,
            } => write!(
                f,
                "{resource:?} count {actual} exceeds {limit} (template {template:?})"
            ),
            Self::UnsafeVariable { template, variable } => write!(
                f,
                "template {template} variable {variable} has no positive binding"
            ),
            Self::NonDenseVariable {
                template,
                expected,
                actual,
            } => write!(
                f,
                "template {template} expected variable ID {expected}, found {actual}"
            ),
        }
    }
}
impl std::error::Error for AdmissionError {}

#[derive(Debug)]
struct ProgramData {
    templates: Vec<Template>,
    domain: Vec<Value>,
    predicates: Vec<Predicate>,
    gate_predicates: Vec<Predicate>,
}

/// An immutable admitted program. Clones share identity; independently admitted
/// equal syntax is a different instance and cannot silently reuse a seed.
#[derive(Clone, Debug)]
pub struct Program(Arc<ProgramData>);

impl Program {
    /// Admit templates without enumerating atoms or ground substitutions.
    /// Variables must be dense and occur in at least one positive pattern;
    /// comparisons and candidate gates do not establish safety.
    ///
    /// # Errors
    /// Returns [`AdmissionError`] for unsafe/non-dense variables or exceeded
    /// budgets. The original template order is preserved in the admitted value.
    pub fn new(templates: Vec<Template>, limits: AdmissionLimits) -> Result<Self, AdmissionError> {
        check_limit(
            AdmissionResource::Templates,
            templates.len(),
            limits.max_templates,
            None,
        )?;
        let mut domain = BTreeSet::new();
        let mut predicates = BTreeSet::new();
        let mut gate_predicates = BTreeSet::new();
        for (index, template) in templates.iter().enumerate() {
            check_limit(
                AdmissionResource::PositiveBody,
                template.positive().len(),
                limits.max_positive_body,
                Some(index),
            )?;
            check_limit(
                AdmissionResource::Variables,
                template.variable_count(),
                limits.max_variables_per_template,
                Some(index),
            )?;
            let bound: BTreeSet<usize> = template
                .positive()
                .iter()
                .flat_map(AtomPattern::terms)
                .filter_map(|term| match term {
                    Term::Variable(v) => Some(*v),
                    Term::Constant(_) => None,
                })
                .collect();
            for (expected, variable) in template.variables().into_iter().enumerate() {
                if expected != variable {
                    return Err(AdmissionError::NonDenseVariable {
                        template: index,
                        expected,
                        actual: variable,
                    });
                }
                if !bound.contains(&variable) {
                    return Err(AdmissionError::UnsafeVariable {
                        template: index,
                        variable,
                    });
                }
            }
            for pattern in template.patterns() {
                check_limit(
                    AdmissionResource::PredicateArity,
                    pattern.predicate().arity(),
                    limits.max_predicate_arity,
                    Some(index),
                )?;
                predicates.insert(pattern.predicate().clone());
            }
            for pattern in template.gate_true().iter().chain(template.gate_false()) {
                gate_predicates.insert(pattern.predicate().clone());
            }
            for term in template.all_terms() {
                if let Term::Constant(value) = term {
                    domain.insert(value.clone());
                    check_limit(
                        AdmissionResource::DomainValues,
                        domain.len(),
                        limits.max_domain_values,
                        None,
                    )?;
                }
            }
        }
        Ok(Self(Arc::new(ProgramData {
            templates,
            domain: domain.into_iter().collect(),
            predicates: predicates.into_iter().collect(),
            gate_predicates: gate_predicates.into_iter().collect(),
        })))
    }
    /// Templates in original admitted order, including duplicates.
    #[must_use]
    pub fn templates(&self) -> &[Template] {
        &self.0.templates
    }
    /// Distinct closed values in canonical order.
    #[must_use]
    pub fn domain(&self) -> &[Value] {
        &self.0.domain
    }
    /// Distinct signatures in canonical order.
    #[must_use]
    pub fn predicates(&self) -> &[Predicate] {
        &self.0.predicates
    }
    /// Signatures consulted by any gate, including constraint-only gates.
    #[must_use]
    pub fn gate_predicates(&self) -> &[Predicate] {
        &self.0.gate_predicates
    }
    /// Whether two handles share precisely the same admitted instance.
    #[must_use]
    pub fn same_instance(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
    /// Test symbolic atom-carrier membership without enumerating its tuples.
    #[must_use]
    pub fn contains_atom(&self, atom: &Atom) -> bool {
        self.predicates().binary_search(atom.predicate()).is_ok()
            && atom
                .values()
                .iter()
                .all(|value| self.domain().binary_search(value).is_ok())
    }
    /// Test the conservative gate carrier: gate predicates over the whole domain.
    #[must_use]
    pub fn contains_gate_atom(&self, atom: &Atom) -> bool {
        self.gate_predicates()
            .binary_search(atom.predicate())
            .is_ok()
            && self.contains_atom(atom)
    }
    /// Iterate the gate carrier lazily. Construction allocates no tuple state;
    /// each requested item is one canonical tuple or a terminal capacity error.
    #[must_use]
    pub fn gate_atoms(&self) -> AtomIter<'_> {
        AtomIter::new(self.gate_predicates(), self.domain())
    }
    /// Iterate the full conceptual atom carrier, without pre-expansion.
    #[must_use]
    pub fn carrier_atoms(&self) -> AtomIter<'_> {
        AtomIter::new(self.predicates(), self.domain())
    }
}

fn check_limit(
    resource: AdmissionResource,
    actual: usize,
    limit: usize,
    template: Option<usize>,
) -> Result<(), AdmissionError> {
    if actual > limit {
        Err(AdmissionError::LimitExceeded {
            resource,
            limit,
            actual,
            template,
        })
    } else {
        Ok(())
    }
}
