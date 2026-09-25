//! Admission establishes a finite symbolic domain and safe dense variables.

use std::fmt;
use std::sync::Arc;

use crate::catalog::storage::{FrozenVocabulary, PredicateId, TermId};
use crate::catalog::{AtomRef, PredicateRef, TermRef};
use crate::template::catalog::RowData;
use crate::{AtomIter, Template, TemplateCatalog};
use std::ops::Range;

mod admission;
mod views;
pub use views::{
    Domain, FilterRef, Filters, PatternRef, PatternTerms, Patterns, Predicates, TemplateRef,
    TemplateTerm, Templates,
};

const DEFAULT_MAX_TEMPLATES: usize = 100_000;
const DEFAULT_MAX_PREDICATE_ARITY: usize = 32;
const DEFAULT_MAX_VARIABLES: usize = 64;
const DEFAULT_MAX_POSITIVE_BODY: usize = 1_024;
const DEFAULT_MAX_DOMAIN_VALUES: usize = 1_000_000;
// The same independent named-storage default as core relations and CPU closure.
const DEFAULT_MAX_BYTES: usize = 128 * 1024 * 1024;

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
    /// Inclusive canonical storage and admitted metadata allowance, including
    /// indexes, vector capacity, admission scratch and publication overlap.
    /// Caller-supplied construction descriptions, allocator bookkeeping and Arc
    /// reference counters are excluded. The default is 128 MiB, independently
    /// of the template/domain count limits; zero is a real allowance.
    pub max_bytes: usize,
}
impl Default for AdmissionLimits {
    fn default() -> Self {
        Self {
            max_templates: DEFAULT_MAX_TEMPLATES,
            max_predicate_arity: DEFAULT_MAX_PREDICATE_ARITY,
            max_variables_per_template: DEFAULT_MAX_VARIABLES,
            max_positive_body: DEFAULT_MAX_POSITIVE_BODY,
            max_domain_values: DEFAULT_MAX_DOMAIN_VALUES,
            max_bytes: DEFAULT_MAX_BYTES,
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
    /// Canonical storage or admitted metadata could not be constructed.
    Canonical {
        /// Typed storage, allocation or representation cause. Storage byte
        /// counts include the simultaneously live admitted metadata.
        error: crate::catalog::Error,
        /// Original input template, when construction was within that template.
        template: Option<usize>,
    },
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
            Self::Canonical { template, .. } | Self::LimitExceeded { template, .. } => *template,
            Self::UnsafeVariable { template, .. } | Self::NonDenseVariable { template, .. } => {
                Some(*template)
            }
        }
    }
}
impl fmt::Display for AdmissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Canonical { error, template } => {
                write!(
                    f,
                    "program admission failed (template {template:?}): {error}"
                )
            }
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
impl std::error::Error for AdmissionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Canonical { error, .. } => Some(error),
            _ => None,
        }
    }
}

#[derive(Debug)]
struct ProgramData {
    catalog: TemplateCatalog,
    templates: Vec<TemplateData>,
    domain: Vec<TermId>,
    predicates: Vec<PredicateId>,
    gate_predicates: Vec<PredicateId>,
    metadata_bytes: u128,
}

#[derive(Debug)]
struct TemplateData {
    row: usize,
    head: Option<usize>,
    positive: Range<usize>,
    gate_true: Range<usize>,
    gate_false: Range<usize>,
    variable_count: usize,
}

impl ProgramData {
    fn term(&self, id: TermId) -> TermRef<'_> {
        crate::template::catalog::term(self.catalog.read(), id)
    }
    fn predicate(&self, id: PredicateId) -> PredicateRef<'_> {
        crate::template::catalog::predicate(self.catalog.read(), id)
    }
    fn row(&self, template: &TemplateData) -> &RowData {
        self.catalog
            .data(template.row)
            .expect("rule topology names its admitted component row")
    }
    fn patterns(&self, template: &TemplateData, range: Range<usize>) -> Patterns<'_> {
        Patterns::admitted(self.catalog.read(), &self.row(template).patterns[range])
    }
}

/// An immutable admitted program with one canonical term/predicate authority.
/// Clones share identity and payload; equal independently admitted syntax is a
/// different instance and cannot silently reuse a seed. Stored templates contain
/// only variable slots and references into the frozen authority. Canonical
/// subterms do not become domain members unless explicitly present as constants.
#[derive(Clone, Debug)]
pub struct Program(Arc<ProgramData>);

impl Program {
    /// Consume construction templates without enumerating atoms or substitutions.
    /// Variables must be dense and occur in at least one positive pattern;
    /// comparisons and candidate gates do not establish safety. Closed values
    /// already passed their constructors, so no implicit depth-128 limit is
    /// imposed again. The named storage allowance still bounds canonical import.
    ///
    /// Original template/body occurrences survive as metadata. Owned construction
    /// payloads are discarded; borrowed execution views use the frozen base.
    /// Buffer reservations are fallible. Arc envelopes retain stable Rust's
    /// infallible allocation boundary, not universal allocation recovery.
    ///
    /// # Errors
    /// Refuses unsafe/non-dense variables, exceeded counts, unavailable named
    /// storage or canonical representation limits. No partial Program escapes.
    pub fn new(templates: Vec<Template>, limits: AdmissionLimits) -> Result<Self, AdmissionError> {
        admission::admit(templates, limits).map(|data| Self(Arc::new(data)))
    }

    /// Templates in original admitted order, including duplicates.
    #[must_use]
    pub fn templates(&self) -> Templates<'_> {
        Templates { program: &self.0 }
    }

    /// Distinct explicitly occurring closed constants in semantic storage order.
    /// The view excludes merely interned subterms and later execution identities.
    #[must_use]
    pub fn domain(&self) -> Domain<'_> {
        Domain { program: &self.0 }
    }

    /// Distinct signed signatures in name, arity and sign order.
    #[must_use]
    pub fn predicates(&self) -> Predicates<'_> {
        Predicates {
            program: &self.0,
            ids: &self.0.predicates,
        }
    }

    /// Signatures consulted by any gate, including constraint-only gates.
    #[must_use]
    pub fn gate_predicates(&self) -> Predicates<'_> {
        Predicates {
            program: &self.0,
            ids: &self.0.gate_predicates,
        }
    }

    pub(crate) fn vocabulary(&self) -> &FrozenVocabulary {
        self.0
            .catalog
            .vocabulary()
            .expect("Program admission constructs an indexed closed vocabulary")
    }

    /// Named canonical-base and admitted-metadata retention. Shared clones name
    /// the same allocations; adding their byte counts would count them twice.
    /// This excludes caller input, allocator overhead and future tuple storage.
    #[must_use]
    pub fn storage_bytes(&self) -> u128 {
        self.0.catalog.storage_bytes() + self.0.metadata_bytes
    }

    /// Immutable vocabulary payload and lookup indexes shared by tuple owners
    /// created with [`crate::atom_interner::AtomInterner::for_program`]. This is
    /// included in [`Self::storage_bytes`]; it excludes Program metadata and the
    /// tuple owner's separate rows, indexes, scratch and ownership envelopes.
    /// A caller whose allowance excludes the input Program may subtract this
    /// exact shared subtotal once from that tuple owner's complete receipt.
    #[must_use]
    pub fn shared_vocabulary_bytes(&self) -> u128 {
        self.0.catalog.shared_vocabulary_bytes()
    }

    /// Whether two handles share precisely the same admitted instance.
    #[must_use]
    pub fn same_instance(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    /// Test symbolic atom-carrier membership without enumerating its tuples.
    #[must_use]
    pub fn contains_atom<'a>(&self, atom: impl Into<AtomRef<'a>>) -> bool {
        let atom = atom.into();
        self.predicates().binary_search(atom.predicate()).is_ok()
            && atom
                .values()
                .iter()
                .all(|value| self.domain().binary_search(value).is_ok())
    }

    /// Test the conservative gate carrier: gate predicates over the whole domain.
    #[must_use]
    pub fn contains_gate_atom<'a>(&self, atom: impl Into<AtomRef<'a>>) -> bool {
        let atom = atom.into();
        self.gate_predicates()
            .binary_search(atom.predicate())
            .is_ok()
            && self.contains_atom(atom)
    }

    /// Iterate the gate carrier lazily, without pre-expansion.
    #[must_use]
    pub fn gate_atoms(&self) -> AtomIter<'_> {
        AtomIter::new(self, self.gate_predicates())
    }

    /// Iterate the full conceptual atom carrier, without pre-expansion.
    #[must_use]
    pub fn carrier_atoms(&self) -> AtomIter<'_> {
        AtomIter::new(self, self.predicates())
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
