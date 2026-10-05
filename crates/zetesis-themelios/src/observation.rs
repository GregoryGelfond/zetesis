//! Bounded observation queries over supplied full models, independent of any oracle.
//!
//! Term output is deduplicated within its own channel. Combining it with selected
//! original atoms retains cross-channel duplicates and never projects model identity.
//! The caller establishes stability separately; these queries create no support.

pub(crate) mod compile;
mod evaluate;
mod identity;
mod render;
pub mod view;
pub mod json;

pub use view::{ModelView, ViewError, ViewLimits};

use std::fmt;
use std::sync::Arc;

use crate::metadata::{Constructor, MetadataVocabulary, Predicate, Read, Scalar};
use themelios_base::span::Location;
use themelios_program::program::{AggregateFunction, DefaultNegation, Relation};
/// Shared logical symbol vocabulary, nameable without another pinned dependency.
pub use themelios_program::symbol::{Name, Sign as SymbolSign, Symbol};
/// Checked arithmetic causes from the pinned shared value vocabulary.
pub use themelios_program::term::EvalError as EvaluationError;
use themelios_program::term::{BinaryOp, UnaryOp};
use zetesis_core::Model;
use zetesis_cpu::{Cancellation, Stop};

pub(crate) use compile::compile;

/// Independent ceilings for source observation templates. Zero means zero.
#[derive(Clone, Copy, Debug)]
pub struct AdmissionLimits {
    /// Distinct directive templates.
    pub max_directives: u32,
    /// Cumulative template, condition, and ground-symbol nodes, including
    /// synthesized keys and guard references. Repeated bounded source walks
    /// also consume this allowance; it is not the final tree's node count.
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
    /// as 16 bytes per semantic node plus UTF-8 text. A wildcard occupies one
    /// key node; default negation is metadata, not an extra logical value.
    /// Borrowed model values,
    /// container capacity and allocator overhead are excluded; this is not RSS.
    /// Storage is released when its local query or key scope ends.
    pub max_local_bytes: usize,
    /// Combined named capacity of the derived term arena and retained wildcard
    /// key graph, including their replacement overlap. Borrowed input payload
    /// is excluded. Transient ID frames each use this ceiling independently;
    /// their combined capacities and allocator overhead are not this measure.
    /// Logical local and construction allowances remain independent.
    pub max_term_storage_bytes: usize,
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
            max_term_storage_bytes: 64 * 1024 * 1024,
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
    /// allowance covers constructed `Symbol` cells, the other bounds compact
    /// parent frames, whose reservation depends on nesting depth. Actual named
    /// capacities are checked against the same ceiling after each reservation.
    /// The extra text allowance covers the canonical name validator's temporary
    /// source text copy. The same conservative formula applies to all
    /// symbols and is checked before construction, including duplicate terms.
    /// Comparison operands share this logical construction preflight. Canonical
    /// comparison uses a borrowed cursor and does not copy either input payload.
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
    /// Named capacity of the derived term arena and retained wildcard keys.
    TermStorageBytes,
}

/// A source form outside this observation slice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Feature {
    /// External calls cannot be evaluated by a pure observation query.
    Term,
    /// Anonymous variables cannot construct ordinary output values.
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
    /// Compiled canonical metadata admission or publication failed.
    Metadata(crate::MetadataStorageError),
    /// Scoped term assignment, slot or prefix validation failed.
    TermAssignment(zetesis_core::catalog::AssignmentError),
    /// A non-ceiling canonical storage or shape operation failed.
    TermStorage(zetesis_core::catalog::Error),
}

/// Work completed before success or refusal.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Statistics {
    /// Charged operations including text/key payload comparisons.
    pub work: u64,
    /// Completed outer or local substitutions examined.
    pub bindings: u64,
    /// Current named capacity of derived terms and retained wildcard keys.
    pub term_storage_bytes: u128,
    /// Peak named term/key capacity envelope. Borrowed inputs, transient query
    /// frames and allocator bookkeeping are excluded; this is not resident memory.
    pub peak_term_storage_bytes: u128,
}

/// Logical or source failure, or runtime refusal with partial accounting.
/// The default human view contains the cause and any known source identity and
/// byte span. [`Self::retain_source`]
/// adds an original source excerpt through themelios's canonical plain view;
/// terminal styling and publication remain the consumer's responsibility.
/// The cause uses a fixed boxed diagnostic envelope, allocated only on refusal.
/// Its allocation follows the standard allocator's failure policy; successful
/// observation operations allocate no diagnostic envelope.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    kind: Box<ErrorKind>,
    site: crate::ProgramSite,
    statistics: Statistics,
    source: Option<Box<crate::source_diagnostics::RetainedSource>>,
}
impl Error {
    fn new(kind: ErrorKind, site: crate::ProgramSite, statistics: Statistics) -> Self {
        Self {
            kind: Box::new(kind),
            site,
            statistics,
            source: None,
        }
    }
    /// Typed cause.
    #[must_use]
    pub fn kind(&self) -> &ErrorKind {
        &self.kind
    }
    /// Real source coordinate of the original directive, when available.
    #[must_use]
    pub fn location(&self) -> Option<Location> {
        self.site.location()
    }
    /// Logical statement identity, even when no parsed source evidence exists.
    /// Resolve its ID through the formula receipt that owns this observation.
    #[must_use]
    pub fn site(&self) -> crate::ProgramSite {
        self.site
    }
    /// Attach the matching original source for a later human diagnostic.
    ///
    /// Only a source with the error's retained identity is copied; an unlocated
    /// error or a different identity leaves the current context unchanged. The
    /// caller supplies the original bytes and display name, without rereading or
    /// reminting their identity. This copies one source and name in linear time
    /// and space, independently of observation evaluation budgets. Input loading
    /// ceilings bound those bytes. No entire include graph is retained.
    pub fn retain_source(&mut self, name: &str, source: &themelios_base::source::Source) {
        if self
            .site
            .location()
            .is_some_and(|location| location.source == source.id())
        {
            self.source = Some(Box::new(crate::source_diagnostics::RetainedSource {
                name: name.to_owned(),
                source: source.clone(),
            }));
        }
    }
    /// Original bytes attached for diagnostic rendering, if any.
    #[must_use]
    pub fn diagnostic_source(&self) -> Option<&themelios_base::source::Source> {
        self.source.as_ref().map(|context| &context.source)
    }
    /// Display name attached with the original source, if any.
    #[must_use]
    pub fn diagnostic_source_name(&self) -> Option<&str> {
        self.source.as_ref().map(|context| context.name.as_str())
    }
    /// Typed located diagnostic, independent of any attached source or styling.
    /// Allocates the cause's diagnostic message; unlocated errors return `None`.
    #[must_use]
    pub fn diagnostic(&self) -> Option<themelios_base::diagnostic::Diagnostic> {
        self.site.location().map(|location| {
            crate::diagnostic::diagnostic(
                "observation",
                format!("observation refused: {:?}", self.kind),
                location,
            )
        })
    }
    /// Work actually charged before refusal.
    #[must_use]
    pub fn statistics(&self) -> Statistics {
        self.statistics
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observation refused: {:?}", self.kind)?;
        if let Some(context) = &self.source
            && let Some(diagnostic) = self.diagnostic()
        {
            context.write(f, &diagnostic)?;
        } else if let Some(location) = self.site.location() {
            write!(
                f,
                " at source {}, bytes {}..{}",
                location.source.get(),
                location.span.start().get(),
                location.span.end().get(),
            )?;
        }
        Ok(())
    }
}
impl std::error::Error for Error {}

#[derive(Clone, Debug)]
enum Template {
    Constant(Scalar),
    Variable(usize),
    Construct(Constructor, Vec<Self>),
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
            Self::Construct(_, arguments) => arguments.iter().any(Self::multiple),
            Self::Constant(_) | Self::Variable(_) => false,
        }
    }
}
#[derive(Clone, Debug)]
enum Operand {
    Constant(Scalar),
    Variable(usize),
    Any,
    Construct(Constructor, Vec<Self>),
    Expression(Template),
    Inverse { slot: usize, expression: Template },
}
#[derive(Clone, Debug)]
struct Pattern {
    predicate: Predicate,
    terms: Vec<Operand>,
    evaluated: bool,
    key: Option<usize>,
}
#[derive(Clone, Debug)]
struct AtomTest {
    pattern: Pattern,
    expansion: Query,
}
#[derive(Clone, Debug)]
enum Condition {
    Atom(DefaultNegation, Vec<AtomTest>),
    AtomPatternValue(DefaultNegation, usize),
    Compare(DefaultNegation, Template, Vec<(Relation, Template)>),
    Boolean(bool),
    Conditional(Query, Box<Self>),
    Aggregate(DefaultNegation, AggregateQuery, Vec<Guard>),
}
#[derive(Clone, Debug)]
struct Guard {
    relation: Relation,
    bound: Template,
}
#[derive(Clone, Debug)]
enum AggregateKey {
    Tuple(Template),
    Atom {
        negation: DefaultNegation,
        slot: usize,
    },
}
#[derive(Clone, Debug)]
struct AggregateElement {
    key: AggregateKey,
    query: Query,
}
#[derive(Clone, Debug)]
struct AggregateQuery {
    function: AggregateFunction,
    elements: Vec<AggregateElement>,
}
#[derive(Clone, Debug)]
enum KeyTemplate {
    Value(Template),
    Any,
    Construct(Constructor, Vec<Self>),
    Pool(Vec<Self>),
}
#[derive(Clone, Debug)]
struct AtomKeyTemplate {
    predicate: Predicate,
    arguments: Vec<KeyTemplate>,
}
#[derive(Clone, Debug)]
enum Binder {
    Atom(Vec<Pattern>),
    Assign(usize, Template),
    AtomKey(usize, AtomKeyTemplate),
    Match {
        patterns: Vec<Operand>,
        value: Template,
        complete: usize,
    },
    Aggregate(usize, AggregateQuery),
    NumericMismatch(AggregateQuery),
}
#[derive(Clone, Debug)]
struct Query {
    binders: Vec<Binder>,
    conditions: Vec<Condition>,
    variables: usize,
    inputs: Vec<usize>,
}
#[derive(Clone, Debug)]
pub(crate) struct Directive {
    term: Template,
    query: Query,
    origins: Vec<Location>,
    site: crate::ProgramSite,
}

/// Prepare an output selection for repeated answers over one catalog's
/// vocabulary, charging its decisions against `limits.max_work` as one
/// observation would. A preparation that would exceed that ceiling keeps every
/// atom's search, which is the unprepared behaviour.
///
/// # Errors
/// Returns the cancellation stop, without a prepared selection.
pub fn prepare_selection<'a>(
    selection: &'a crate::OutputSelection,
    read: zetesis_core::catalog::CatalogRead<'_>,
    limits: Limits,
    cancellation: &Cancellation,
) -> Result<crate::PreparedSelection<'a>, Stop> {
    enum Refusal {
        Work,
        Stopped(Stop),
    }
    let mut work = 0u128;
    let prepared = selection.prepare_with(read, |units| {
        cancellation.poll().map_err(Refusal::Stopped)?;
        work += units;
        if work > u128::from(limits.max_work) {
            Err(Refusal::Work)
        } else {
            Ok(())
        }
    });
    match prepared {
        Ok(prepared) => Ok(prepared),
        Err(Refusal::Work) => Ok(crate::PreparedSelection::from(selection)),
        Err(Refusal::Stopped(stop)) => Err(stop),
    }
}

/// Immutable display templates with logical identity and any original source
/// evidence. No solver or candidate carrier is retained; the query is meaningful
/// for any supplied model.
#[derive(Clone, Debug, Default)]
pub struct ObservationProgram {
    data: Option<Arc<ObservationData>>,
}
#[derive(Debug)]
struct ObservationData {
    vocabulary: Arc<MetadataVocabulary>,
    directives: Vec<Directive>,
}
#[derive(Clone, Copy)]
struct ObservationRead<'a> {
    metadata: Read<'a>,
    directives: &'a [Directive],
}
impl ObservationProgram {
    pub(crate) fn publish(vocabulary: Arc<MetadataVocabulary>, directives: Vec<Directive>) -> Self {
        if directives.is_empty() {
            Self::default()
        } else {
            Self {
                data: Some(Arc::new(ObservationData {
                    vocabulary,
                    directives,
                })),
            }
        }
    }
    fn read_with<E>(
        &self,
        before: impl FnMut() -> Result<(), E>,
    ) -> Result<Option<ObservationRead<'_>>, zetesis_core::TemplateCatalogFailure<E>> {
        self.data
            .as_ref()
            .map(|data| {
                data.vocabulary
                    .read_with(before)
                    .map(|metadata| ObservationRead {
                        metadata,
                        directives: &data.directives,
                    })
            })
            .transpose()
    }
    /// Compile only this channel through the validated shared-program metadata door.
    /// Constants and observation safety are checked; logical execution is not admitted.
    ///
    /// # Errors
    /// Returns the same typed refusals as [`crate::SourceMetadata::compile`].
    pub fn compile(
        program: &themelios_program::program::Program,
        limits: crate::MetadataLimits,
        fallback: impl Into<crate::ProgramSite>,
    ) -> Result<Self, crate::MetadataError> {
        crate::SourceMetadata::compile(program, limits, fallback)
            .map(crate::SourceMetadata::into_observations)
    }

    /// Whether the term channel contains no source templates.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.data.is_none()
    }
    /// Real parsed locations per distinct template, in compilation order.
    /// Constructed templates have an empty slice; [`Self::sites`] retains identity.
    pub fn origins(&self) -> impl Iterator<Item = &[Location]> {
        self.data
            .iter()
            .flat_map(|data| data.directives.iter())
            .map(|directive| directive.origins.as_slice())
    }
    /// Statement identities per directive, in compilation order. IDs belong to
    /// the original canonical program supplied to compilation or formula preparation.
    pub fn sites(&self) -> impl Iterator<Item = crate::ProgramSite> + '_ {
        self.data
            .iter()
            .flat_map(|data| data.directives.iter())
            .map(|directive| directive.site)
    }
    /// Evaluate the distinct term channel over a supplied complete model.
    ///
    /// # Errors
    /// Returns a typed evaluation, support, limit or control error without a
    /// partial term set. Source coordinates are present only when available.
    pub fn evaluate(
        &self,
        model: &Model,
        limits: Limits,
        cancellation: &Cancellation,
    ) -> Result<Evaluation, Error> {
        self.evaluate_with_construction_limits(
            model,
            limits,
            ConstructionLimits::default(),
            cancellation,
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
        cancellation: &Cancellation,
    ) -> Result<Evaluation, Error> {
        evaluate::evaluate(self, model, limits, construction, cancellation)
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
        cancellation: &Cancellation,
    ) -> Result<Rendered, Error> {
        self.render_with_construction_limits(
            model,
            selection,
            limits,
            ConstructionLimits::default(),
            cancellation,
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
        cancellation: &Cancellation,
    ) -> Result<Rendered, Error> {
        render::render(self, model, selection, limits, construction, cancellation)
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
    /// Transfer the complete distinct term channel without copying symbols or
    /// reallocating its vector. The evaluation receipt is consumed; read its
    /// statistics first when the caller needs to retain them. O(1).
    #[must_use]
    pub fn into_symbols(self) -> Vec<Symbol> {
        self.symbols
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
