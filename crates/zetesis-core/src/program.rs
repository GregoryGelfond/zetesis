//! Admission establishes a finite symbolic domain and safe dense variables.

use std::fmt;
use std::sync::Arc;

use crate::catalog::storage::{FrozenVocabulary, PredicateId, TermId};
use crate::catalog::{AtomRef, PredicateRef, TermRef};
use crate::template::catalog::RowData;
use crate::{AtomIter, Template, TemplateCatalog};
use std::ops::Range;

mod admission;
mod facts;
pub use facts::{
    FactIter, FactOccurrences, FactRef, FactRelation, FactSegment, FactTables, RuleRef, Rules,
};
mod positions;
mod views;
pub use positions::DomainPositions;
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
    /// Maximum number of source-normalized rules: templates and fact occurrences.
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
    /// Total template and table-fact occurrences.
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

/// The native input associated with a canonical admission failure.
/// Template and fact indices refer to their separate original input sequences;
/// this is an input identity, not a parsed source coordinate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdmissionOrigin {
    /// A whole-program operation with no individual occurrence.
    Program,
    /// An original template occurrence.
    Template(usize),
    /// An original table-fact occurrence.
    Fact(usize),
}

/// A typed admission refusal. Template and fact indices retain original input identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AdmissionError {
    /// A configured count bound failed at an original fact occurrence.
    FactLimitExceeded {
        /// Count whose inclusive bound was exceeded.
        resource: AdmissionResource,
        /// Configured maximum.
        limit: usize,
        /// Requested count.
        actual: usize,
        /// Original table-fact occurrence index.
        fact: usize,
    },
    /// Canonical storage or admitted metadata could not be constructed.
    Canonical {
        /// Typed storage, allocation or representation cause. Storage byte
        /// counts include the simultaneously live admitted metadata.
        error: crate::catalog::Error,
        /// Original input occurrence, or a whole-program operation. One shared
        /// cause representation reports allocation failures without allocating.
        origin: AdmissionOrigin,
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
    /// Original fact-occurrence position for parallel source evidence. Template
    /// positions and fact positions are distinct; neither is the whole rule order.
    #[must_use]
    pub const fn fact_index(&self) -> Option<usize> {
        match self {
            Self::Canonical {
                origin: AdmissionOrigin::Fact(fact),
                ..
            }
            | Self::FactLimitExceeded { fact, .. } => Some(*fact),
            _ => None,
        }
    }
    /// Locate a template-level refusal without parsing a message.
    #[must_use]
    pub fn template_index(&self) -> Option<usize> {
        match self {
            Self::Canonical {
                origin: AdmissionOrigin::Template(template),
                ..
            } => Some(*template),
            Self::Canonical {
                origin: AdmissionOrigin::Program | AdmissionOrigin::Fact(_),
                ..
            }
            | Self::FactLimitExceeded { .. } => None,
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
            Self::FactLimitExceeded {
                resource,
                limit,
                actual,
                fact,
            } => write!(
                f,
                "{resource:?} count {actual} exceeds {limit} (fact {fact})"
            ),
            Self::Canonical { error, origin } => match origin {
                AdmissionOrigin::Fact(fact) => {
                    write!(f, "program admission failed (fact {fact}): {error}")
                }
                AdmissionOrigin::Template(template) => write!(
                    f,
                    "program admission failed (template Some({template})): {error}"
                ),
                AdmissionOrigin::Program => {
                    write!(f, "program admission failed (template None): {error}")
                }
            },
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
    facts: Option<facts::Data>,
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
    /// Consume templates and explicit fact tables without enumerating variable
    /// substitutions. Fact admission interns their distinct atoms once and keeps
    /// original occurrences beside predicate-local relation columns.
    /// Variables must be dense and occur in at least one positive pattern;
    /// comparisons and candidate gates do not establish safety. Closed values
    /// already passed their constructors, so no implicit depth-128 limit is
    /// imposed again. The named storage allowance still bounds canonical import.
    ///
    /// Original template/body and fact occurrences survive as metadata. Facts
    /// supplied as templates remain templates; this constructor never reclassifies.
    /// Owned construction payloads are discarded; borrowed execution views use
    /// the frozen base.
    /// Buffer reservations are fallible. Arc envelopes retain stable Rust's
    /// infallible allocation boundary, not universal allocation recovery.
    ///
    /// # Cost and storage
    /// Imports each explicit scalar alternative into the shared vocabulary,
    /// then visits every fact occurrence and its argument columns once. Exact
    /// atom interning and relation indexing additionally perform their checked
    /// key lookups and reservations. Intervals avoid an expanded owned input
    /// vector, but this admission still enumerates their scalar values and rows.
    /// For `f` original occurrences and `u` distinct atoms, retained occurrence
    /// metadata is O(f); canonical atom payload and relation columns are O(u)
    /// at fixed arity. Template topology, explicit-domain entries and canonical
    /// vocabulary have their own retained capacities. Temporary imported-column
    /// IDs and old/replacement/publication overlap share the byte allowance.
    ///
    /// # Errors
    /// Refuses unsafe/non-dense variables, exceeded counts, unavailable named
    /// storage or canonical representation limits. No partial `Program` escapes.
    pub fn new(
        templates: Vec<Template>,
        facts: FactTables,
        limits: AdmissionLimits,
    ) -> Result<Self, AdmissionError> {
        admission::admit(templates, facts, limits).map(|data| Self(Arc::new(data)))
    }

    /// All template and fact occurrences in original admitted order. Facts retain
    /// duplicate occurrences while sharing each distinct atom's canonical payload.
    #[must_use]
    pub fn rules(&self) -> Rules<'_> {
        Rules::new(&self.0)
    }

    /// Whether neither templates nor facts occur in this program.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.templates.is_empty() && self.facts().is_empty()
    }

    /// Original fact occurrences, for traversal and parallel source evidence.
    #[must_use]
    pub fn facts(&self) -> FactOccurrences<'_> {
        FactOccurrences {
            data: self.0.facts.as_ref(),
        }
    }

    /// Distinct fact relations in signed predicate order, borrowing their closed
    /// authority, column dictionaries and prepared canonical runs.
    pub fn fact_relations(&self) -> impl Iterator<Item = FactRelation<'_>> {
        facts::relations(self.0.facts.as_ref())
    }

    /// One immutable fact authority for oracle descendants. Its storage presence
    /// is not discovery or truth; each descendant starts with empty discovery.
    /// A program with no table facts has no closed fact authority.
    #[must_use]
    pub fn fact_catalog(&self) -> Option<&crate::atom_interner::ClosedCatalog> {
        self.0.facts.as_ref().map(|facts| &facts.store)
    }

    /// Non-fact templates in original admitted order, including duplicates.
    /// Body-free templates explicitly supplied to `new` remain templates; only
    /// the separate `FactTables` input becomes table facts.
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
        self.0.catalog.storage_bytes()
            + self.0.metadata_bytes
            + self
                .0
                .facts
                .as_ref()
                .map_or(0, |facts| facts.bytes(self.shared_vocabulary_bytes()))
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
