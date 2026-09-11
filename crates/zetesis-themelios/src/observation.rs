//! Bounded observation queries over supplied full models, independent of any oracle.
//!
//! Term output is deduplicated within its own channel. Combining it with selected
//! original atoms retains cross-channel duplicates and never projects model identity.
//! The caller establishes stability separately; these queries create no support.

mod compile;
mod evaluate;
mod render;
pub mod view;
pub mod json;

pub use view::{ModelView, ViewError, ViewLimits};

use std::fmt;

use themelios_base::span::Location;
use themelios_program::program::{AggregateFunction, DefaultNegation, Relation};
/// Shared logical symbol vocabulary, nameable without another pinned dependency.
pub use themelios_program::symbol::{Name, Sign as SymbolSign, Symbol};
/// Checked arithmetic causes from the pinned shared value vocabulary.
pub use themelios_program::term::EvalError as EvaluationError;
use themelios_program::term::{BinaryOp, UnaryOp};
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
    /// Named and generated binding slots in each directive-local scope.
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
    /// Completed substitutions inspected in outer directives and local queries.
    pub max_bindings: u64,
    /// Distinct enabled shown terms.
    pub max_terms: usize,
    /// Nodes per constructed symbol, before cloning or ordering.
    pub max_symbol_nodes: usize,
    /// Maximum constructed symbol depth.
    pub max_symbol_depth: usize,
    /// UTF-8 string and constructor-name bytes per constructed symbol.
    pub max_symbol_bytes: usize,
    /// Retained term payload (16 bytes/node plus UTF-8 text; excluding allocator
    /// overhead) and complete rendered line bytes, each independently.
    pub max_output_bytes: usize,
    /// Simultaneously retained generated expression alternatives, owned bindings,
    /// and aggregate tuple keys, measured
    /// as 16 bytes per semantic node plus UTF-8 text. Borrowed model values,
    /// container capacity and allocator overhead are excluded; this is not RSS.
    /// Storage is released when its local query or key scope ends.
    pub max_local_bytes: usize,
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
            max_local_bytes: 8_388_608,
        }
    }
}

/// Independent storage preflight for simultaneously constructing observation symbols.
/// Existing evaluation/rendering methods use the default; explicit variants allow
/// callers to set this ceiling independently of logical text/output limits.
#[derive(Clone, Copy, Debug)]
pub struct ConstructionLimits {
    /// Inclusive conservative bound: twice the semantic node count times
    /// `size_of::<Symbol>()`, plus twice the UTF-8 string/name bytes. One node
    /// allowance covers constructed Symbol cells, the other the reverse-conversion
    /// stack. The extra text allowance covers the canonical name validator's
    /// temporary Source text copy. The same conservative formula applies to all
    /// symbols and is checked before construction, including duplicate terms.
    /// Comparison operands share this ceiling while both are live.
    /// Borrowed input capacity/cached spelling, allocator overhead, reference-count
    /// headers and join/result-container bookkeeping are excluded. Logical output
    /// and node limits separately bound retained results. This is not an RSS cap.
    pub max_bytes: usize,
}
impl Default for ConstructionLimits {
    fn default() -> Self {
        Self {
            max_bytes: 8_388_608,
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
    /// Variables in the current scope, including inherited outer slots.
    Variables,
    /// Body elements.
    BodyElements,
    /// Constructor/predicate argument count.
    Arity,
    /// Original source locations.
    Origins,
    /// Runtime charged work.
    Work,
    /// Completed outer or local substitutions.
    Bindings,
    /// Unique enabled output terms.
    Terms,
    /// Total retained/rendered payload.
    OutputBytes,
    /// Conservative per-symbol construction cells, text and conversion stack.
    ConstructionBytes,
    /// Live generated alternatives, owned bindings and aggregate tuple payload.
    LocalBytes,
}

/// A source form outside this observation slice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Feature {
    /// External calls cannot be evaluated by a pure observation query.
    Term,
    /// Anonymous variables cannot construct output terms or negative atom keys.
    AnonymousOutput,
    /// Theory atoms or a future body form without a pure finite interpretation.
    Body,
    /// Set-cardinality elements without an atomic literal.
    Atom,
    /// An extremum element lacks its required first tuple component.
    AggregateMeasure,
    /// A comparison form without a finite checked interpretation.
    Comparison,
    /// A named variable lacks a finite established binding.
    UnsafeVariable,
}

/// Refusal, never a complete prefix or semantic UNSAT.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    /// A source construct is unsupported.
    Unsupported(Feature),
    /// A ground expression is undefined or exceeds the pinned integer range.
    Evaluation(EvaluationError),
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
    /// Completed outer or local substitutions examined.
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
    Unary(UnaryOp, Box<Self>),
    Binary(BinaryOp, Box<Self>, Box<Self>),
    Absolute(Box<Self>),
    Pool(Vec<Self>),
    Interval(Box<Self>, Box<Self>),
}
impl Template {
    fn multiple(&self) -> bool {
        match self {
            Self::Pool(_) | Self::Interval(_, _) => true,
            Self::Unary(_, argument) | Self::Absolute(argument) => argument.multiple(),
            Self::Binary(_, left, right) => left.multiple() || right.multiple(),
            Self::Function(_, _, arguments) | Self::Tuple(arguments) => {
                arguments.iter().any(Self::multiple)
            }
            Self::Value(_) | Self::Variable(_) => false,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
enum Operand {
    Value(Value),
    Variable(usize),
    Any,
    Function(SymbolSign, Name, Vec<Self>),
    Tuple(Vec<Self>),
    Expression(Template),
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Pattern {
    predicate: Predicate,
    terms: Vec<Operand>,
    evaluated: bool,
    key: Option<usize>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct AtomTest {
    pattern: Pattern,
    expansion: Query,
}
#[derive(Clone, Debug, PartialEq, Eq)]
enum Condition {
    Atom(DefaultNegation, Vec<AtomTest>),
    AtomValue(DefaultNegation, usize),
    Compare(DefaultNegation, Template, Vec<(Relation, Template)>),
    Boolean(bool),
    Conditional(Query, Box<Self>),
    Aggregate(DefaultNegation, AggregateQuery, Vec<Guard>),
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Guard {
    relation: Relation,
    bound: Template,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct AggregateElement {
    tuple: Template,
    query: Query,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct AggregateQuery {
    function: AggregateFunction,
    elements: Vec<AggregateElement>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
enum Binder {
    Atom(Vec<Pattern>),
    Assign(usize, Template),
    Aggregate(usize, AggregateQuery),
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Query {
    binders: Vec<Binder>,
    conditions: Vec<Condition>,
    variables: usize,
    inputs: Vec<usize>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Directive {
    term: Template,
    query: Query,
    origins: Vec<Location>,
}

/// Immutable display templates with original source evidence. No solver or
/// candidate carrier is retained; the query is meaningful for any supplied model.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ObservationProgram {
    directives: Vec<Directive>,
}
impl ObservationProgram {
    /// Compile only this channel through the validated shared-program metadata door.
    /// Constants and observation safety are checked; logical execution is not admitted.
    ///
    /// # Errors
    /// Returns the same located refusals as [`crate::SourceMetadata::compile`].
    pub fn compile(
        program: &themelios_program::program::Program,
        limits: crate::MetadataLimits,
        fallback: Location,
    ) -> Result<Self, crate::MetadataError> {
        crate::SourceMetadata::compile(program, limits, fallback)
            .map(|metadata| metadata.observations)
    }

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
    /// Returns a located evaluation, support, limit or control error without a partial term set.
    pub fn evaluate(
        &self,
        model: &Model,
        limits: Limits,
        control: &Control,
    ) -> Result<Evaluation, Error> {
        self.evaluate_with_construction_limits(
            model,
            limits,
            ConstructionLimits::default(),
            control,
        )
    }
    /// Evaluate with an explicit independent symbol-construction storage ceiling.
    ///
    /// # Errors
    /// Returns a typed refusal and charged work, without a partial term set.
    pub fn evaluate_with_construction_limits(
        &self,
        model: &Model,
        limits: Limits,
        construction: ConstructionLimits,
        control: &Control,
    ) -> Result<Evaluation, Error> {
        evaluate::evaluate(self, model, limits, construction, control)
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
        self.render_with_construction_limits(
            model,
            selection,
            limits,
            ConstructionLimits::default(),
            control,
        )
    }
    /// Render with an independent construction ceiling for observed terms.
    /// Borrowed atom spelling does not construct new Symbol values.
    ///
    /// # Errors
    /// Returns a typed refusal without a partial rendered line.
    pub fn render_with_construction_limits(
        &self,
        model: &Model,
        selection: &crate::OutputSelection,
        limits: Limits,
        construction: ConstructionLimits,
        control: &Control,
    ) -> Result<Rendered, Error> {
        render::render(self, model, selection, limits, construction, control)
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
