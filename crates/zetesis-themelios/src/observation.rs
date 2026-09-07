//! Bounded observation queries over supplied full models, independent of any oracle.
//!
//! Term output is deduplicated within its own channel. Combining it with selected
//! original atoms retains cross-channel duplicates and never projects model identity.
//! The caller establishes stability separately; these queries create no support.

mod compile;
mod evaluate;
mod render;

use std::fmt;

use themelios_base::span::Location;
use themelios_program::program::{DefaultNegation, Relation};
use themelios_program::symbol::{Name, Symbol};
use zetesis_core::{Model, Predicate, Value};
use zetesis_cpu::{Control, Stop};

pub(crate) use compile::compile;

/// Independent ceilings for source observation templates. Zero means zero.
#[derive(Clone, Copy, Debug)]
pub struct AdmissionLimits {
    /// Distinct directive templates.
    pub max_directives: u32,
    /// Cumulative template, condition, and ground-symbol nodes.
    pub max_nodes: u32,
    /// Maximum constructor nesting, checked before descending; capped at 64.
    pub max_depth: u32,
    /// Cumulative owned text payload.
    pub max_bytes: u32,
    /// Named variables in each directive-local scope.
    pub max_variables: u32,
    /// Body conditions per directive.
    pub max_body_elements: u32,
    /// Arguments in a constructor or predicate.
    pub max_arity: u32,
    /// Original directive locations.
    pub max_origins: u32,
}
impl Default for AdmissionLimits {
    fn default() -> Self {
        Self {
            max_directives: 1_024,
            max_nodes: 65_536,
            max_depth: 64,
            max_bytes: 1_048_576,
            max_variables: 64,
            max_body_elements: 128,
            max_arity: 64,
            max_origins: 16_384,
        }
    }
}

/// Independent limits for one complete observation/rendering operation.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Charged scalar/key comparisons, scans, construction, and rendering work.
    pub max_work: u64,
    /// Complete positive relational bindings inspected.
    pub max_bindings: u64,
    /// Distinct enabled shown terms.
    pub max_terms: usize,
    /// Nodes per constructed symbol, before cloning or ordering.
    pub max_symbol_nodes: usize,
    /// Maximum constructed symbol depth.
    pub max_symbol_depth: usize,
    /// Bytes per constructed symbol.
    pub max_symbol_bytes: usize,
    /// Retained term payload (16 bytes/node plus UTF-8 text; excluding allocator
    /// overhead) and complete rendered line bytes, each independently.
    pub max_output_bytes: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            max_work: 1_000_000,
            max_bindings: 100_000,
            max_terms: 65_536,
            max_symbol_nodes: 1_024,
            max_symbol_depth: 64,
            max_symbol_bytes: 1_048_576,
            max_output_bytes: 8_388_608,
        }
    }
}

/// A counted observation resource.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resource {
    /// Source directive count.
    Directives,
    /// Template or constructed symbol node count.
    Nodes,
    /// Constructor depth.
    Depth,
    /// Source or constructed symbol text payload.
    Bytes,
    /// Directive-local variables.
    Variables,
    /// Body elements.
    BodyElements,
    /// Constructor/predicate argument count.
    Arity,
    /// Original source locations.
    Origins,
    /// Runtime charged work.
    Work,
    /// Complete positive bindings.
    Bindings,
    /// Unique enabled output terms.
    Terms,
    /// Total retained/rendered payload.
    OutputBytes,
}

/// A source form outside this observation slice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Feature {
    /// General arithmetic, intervals, pools or external calls.
    Term,
    /// Anonymous variables cannot occur in the output term.
    AnonymousOutput,
    /// Aggregate, conditional, or Boolean body literal.
    Body,
    /// Pooled predicate arguments.
    Atom,
    /// Chained, negated, or non-scalar comparisons.
    Comparison,
    /// A named variable lacks an ordinary positive binding, or an anonymous
    /// variable occurs under default negation of a signed predicate.
    UnsafeVariable,
}

/// Refusal, never a complete prefix or semantic UNSAT.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    /// A source construct is unsupported.
    Unsupported(Feature),
    /// An inclusive ceiling would be exceeded.
    Limit {
        /// Resource being counted.
        resource: Resource,
        /// Inclusive configured ceiling.
        limit: u128,
        /// Count required to continue.
        observed: u128,
    },
    /// Shared cancellation or deadline.
    Stopped(Stop),
    /// A fallible allocation failed.
    Allocation,
    /// A public supplied model contains a symbol name that cannot be represented.
    InvalidSymbol,
}

/// Work completed before success or refusal.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Statistics {
    /// Charged operations including text/key payload comparisons.
    pub work: u64,
    /// Complete positive relational substitutions examined.
    pub bindings: u64,
}

/// Located source failure or runtime refusal with partial accounting.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    kind: ErrorKind,
    location: Option<Location>,
    statistics: Statistics,
}
impl Error {
    /// Typed cause.
    #[must_use]
    pub fn kind(&self) -> &ErrorKind {
        &self.kind
    }
    /// Original directive when one is being compiled/evaluated.
    #[must_use]
    pub fn location(&self) -> Option<Location> {
        self.location
    }
    /// Work actually charged before refusal.
    #[must_use]
    pub fn statistics(&self) -> Statistics {
        self.statistics
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observation refused: {:?}", self.kind)
    }
}
impl std::error::Error for Error {}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Template {
    Value(Symbol),
    Variable(usize),
    Function(themelios_program::symbol::Sign, Name, Vec<Self>),
    Tuple(Vec<Self>),
}
#[derive(Clone, Debug, PartialEq, Eq)]
enum Operand {
    Value(Value),
    Variable(usize),
    Any,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Pattern {
    predicate: Predicate,
    terms: Vec<Operand>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
enum Condition {
    Atom(DefaultNegation, Pattern),
    Compare(Operand, Relation, Operand),
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Directive {
    term: Template,
    positive: Vec<Pattern>,
    conditions: Vec<Condition>,
    variables: usize,
    origins: Vec<Location>,
}

/// Immutable display templates with original source evidence. No solver or
/// candidate carrier is retained; the query is meaningful for any supplied model.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ObservationProgram {
    directives: Vec<Directive>,
}
impl ObservationProgram {
    /// Whether the term channel contains no source templates.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.directives.is_empty()
    }
    /// Original locations per distinct source template, in compilation order.
    pub fn origins(&self) -> impl Iterator<Item = &[Location]> {
        self.directives
            .iter()
            .map(|directive| directive.origins.as_slice())
    }
    /// Evaluate the distinct term channel over a supplied complete model.
    ///
    /// # Errors
    /// Returns a typed limit/control refusal without a partial term set.
    pub fn evaluate(
        &self,
        model: &Model,
        limits: Limits,
        control: &Control,
    ) -> Result<Evaluation, Error> {
        evaluate::evaluate(self, model, limits, control)
    }
    /// Render selected original atoms plus distinct terms as one complete line.
    /// Equal symbols from the two channels remain repeated. No newline is included.
    ///
    /// # Errors
    /// No partial line escapes a resource, cancellation, or symbol failure.
    pub fn render(
        &self,
        model: &Model,
        selection: &crate::OutputSelection,
        limits: Limits,
        control: &Control,
    ) -> Result<Rendered, Error> {
        render::render(self, model, selection, limits, control)
    }
}

/// The complete distinct term channel; never a replacement for the original model.
#[derive(Debug)]
pub struct Evaluation {
    symbols: Vec<Symbol>,
    statistics: Statistics,
}
impl Evaluation {
    /// Distinct enabled shared symbols in deterministic term order.
    #[must_use]
    pub fn symbols(&self) -> &[Symbol] {
        &self.symbols
    }
    /// Completed work accounting.
    #[must_use]
    pub fn statistics(&self) -> Statistics {
        self.statistics
    }
}
/// A complete rendered model line, buffered before external output.
#[derive(Debug)]
pub struct Rendered {
    text: String,
    statistics: Statistics,
}
impl Rendered {
    /// Complete line, excluding the final newline.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }
    /// Completed evaluation and rendering work.
    #[must_use]
    pub fn statistics(&self) -> Statistics {
        self.statistics
    }
}
