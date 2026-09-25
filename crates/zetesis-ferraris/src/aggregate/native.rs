//! Retained finite aggregate operations over already-coalesced whole tuples.
//!
//! A group retains canonical keys, eligibility node indices and immutable
//! theory identity. `Group` owns its vocabulary; `GroupData` retains only metadata
//! and binds to a caller-owned canonical prefix through `GroupRef`. Admission checks shape and uniqueness; it does not assert
//! that a source program or theory contains this aggregate. Mask reduction is
//! separate from acquisition of actual original/frozen formula truth. Neither
//! operation decides stable-model membership or changes the original theory.

use std::fmt;

use zetesis_core::catalog::{CatalogRead, TermRef, Vocabulary};
use zetesis_core::{TemplateComponents, TemplateComponentsRef, TemplateTerm, Value as Term};
use zetesis_cpu::{Cancellation, Stop};

use crate::{AggregateComparison, Theory};

mod admission;
mod eligibility;
mod reduction;
pub use eligibility::{Eligibility, EligibilityLimits};
pub use reduction::{Evaluation, Reduction, ReductionLimits, Value};

/// Finite aggregate function; no floating-point arithmetic is used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Function {
    /// Number of selected complete tuple keys, including an empty tuple.
    Count,
    /// Checked signed sum of selected numeric first components.
    Sum,
    /// Checked sum of strictly positive numeric first components.
    SumPlus,
    /// Minimum selected first component in ASP term order; empty result is `#sup`.
    Min,
    /// Maximum selected first component in ASP term order; empty result is `#inf`.
    Max,
}

/// One complete tuple and its already OR-coalesced eligibility formula.
#[derive(Debug)]
pub struct Tuple<K = Vec<Term>> {
    /// Complete logical key. Equal weights or equal conditions do not identify
    /// equal keys. Empty keys remain present, even for a neutral contribution.
    pub key: K,
    /// Absolute node index in the group's original immutable theory.
    pub condition: usize,
}

/// An integer guard can exceed source i32 width without becoming a term sentinel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bound<T = Term> {
    /// Exact wide integer.
    Integer(i128),
    /// Ordered ASP term, including genuine `#inf` and `#sup` endpoints.
    Term(T),
}

/// One aggregate comparison. Several guards are combined by conjunction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Guard<T = Term> {
    /// Aggregate comparison, not default negation of another comparison.
    pub comparison: AggregateComparison,
    /// Right-hand value of the comparison.
    pub bound: Bound<T>,
}

/// Bounds on accepting transferred tuple/guard storage and duplicate checking.
#[derive(Clone, Copy, Debug)]
pub struct AdmissionLimits {
    /// Number of unique complete tuples, including neutral contributions.
    pub max_tuples: usize,
    /// Total scalar/structured value occurrences in complete keys.
    pub max_tuple_values: usize,
    /// Total preorder nodes across keys and term-valued guards; scalars count one.
    pub max_value_nodes: usize,
    /// Number of guards, including repeated guard occurrences.
    pub max_guards: usize,
    /// Logical transferred vector capacities, referenced term payload and
    /// duplicate-check scratch. Shared term payload is conservatively charged
    /// per occurrence; allocator overhead and extra string capacity are excluded.
    pub max_bytes: u64,
    /// Actual named canonical storage, component/tuple/guard metadata and
    /// simultaneous duplicate-check/publication scratch. Borrowed admission
    /// excludes its external catalog; owned admission includes its vocabulary.
    /// Caller ingress and allocator/Arc bookkeeping are excluded; this is not RSS.
    pub max_storage_bytes: u64,
    /// Charged shape visits, index moves and comparison-carrier operations.
    pub max_work: u64,
}

impl Default for AdmissionLimits {
    fn default() -> Self {
        Self {
            max_tuples: 65_536,
            max_tuple_values: 262_144,
            max_value_nodes: 1_048_576,
            max_guards: 64,
            max_bytes: 64 * 1024 * 1024,
            max_storage_bytes: 64 * 1024 * 1024,
            max_work: 100_000_000,
        }
    }
}

/// Resource whose inclusive ceiling refused the operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resource {
    /// Complete tuple count.
    Tuples,
    /// Key value occurrences.
    TupleValues,
    /// Term preorder-node occurrences.
    ValueNodes,
    /// Guard occurrences.
    Guards,
    /// Logical storage payload.
    Bytes,
    /// Actual named canonical/metadata storage and scratch.
    StorageBytes,
    /// Charged operation budget.
    Work,
}

/// Which eligibility observation has an invalid occurrence count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Truth of the original element formulas in M.
    Original,
    /// Truth in J of those formulas frozen in M.
    Frozen,
}

/// Typed refusal; none denotes aggregate truth, stable membership or UNSAT.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    /// An inclusive finite resource ceiling was exceeded.
    Limit(Resource),
    /// Eligibility references a node outside the retained theory.
    Condition {
        /// Input tuple occurrence.
        tuple: usize,
        /// Invalid formula node index.
        node: usize,
    },
    /// Equal whole keys have not been OR-coalesced by the caller.
    Duplicate {
        /// First input occurrence in deterministic index order.
        first: usize,
        /// Second equal-key input occurrence.
        second: usize,
    },
    /// A supplied eligibility mask has the wrong tuple occurrence count.
    Mask {
        /// Original or frozen mask.
        phase: Phase,
        /// Exact required count.
        expected: usize,
        /// Supplied count.
        actual: usize,
    },
    /// Original/tested interpretation belongs to a different admitted theory.
    WrongTheory,
    /// Canonical input has a foreign vocabulary, inaccessible prefix or ingress payload.
    Canonical(zetesis_core::catalog::ReadError),
    /// Typed canonical representation failure not classified as a capacity/stop.
    Catalog(zetesis_core::catalog::Error),
    /// Checked integer, dimension or accounting overflow.
    Overflow,
    /// Cancellation, deadline or fallible allocation stopped the operation.
    Stopped(Stop),
}

/// Deterministic accounting local to one admission, acquisition or reduction.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Statistics {
    /// Charged operations, including the prefix before a refusal.
    pub work: u64,
    /// Planned retained logical vector/value payload for this operation.
    pub resident_bytes: u64,
    /// Planned simultaneous retained and temporary payload; allocation may fail.
    pub peak_bytes: u64,
    /// Named capacity retained at the last canonical admission observation;
    /// external borrowed catalogs are excluded. Acquisition and reduction leave
    /// this field zero and use their existing logical payload fields.
    pub storage_bytes: u64,
    /// Greatest observed canonical admission capacity, including simultaneous
    /// replacement, scratch and successful publication envelopes. A refused
    /// reservation is not reported as allocated capacity. Zero for acquisition
    /// and reduction.
    pub storage_peak_bytes: u64,
}

/// Incomplete operation retaining typed reason and accounted prefix.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    kind: ErrorKind,
    statistics: Statistics,
}

impl Error {
    /// Typed refusal, separate from a completed false aggregate guard.
    #[must_use]
    pub fn kind(&self) -> ErrorKind {
        self.kind.clone()
    }

    /// Local operation accounting before the refusal.
    #[must_use]
    pub const fn statistics(&self) -> Statistics {
        self.statistics
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            ErrorKind::Stopped(stop) => stop.fmt(f),
            ErrorKind::Limit(resource) => write!(f, "native aggregate {resource:?} limit reached"),
            ErrorKind::Condition { tuple, node } => {
                write!(f, "aggregate tuple {tuple} references absent node {node}")
            }
            ErrorKind::Duplicate { first, second } => write!(
                f,
                "aggregate tuples {first} and {second} have an uncoalesced equal key"
            ),
            ErrorKind::Mask {
                phase,
                expected,
                actual,
            } => write!(
                f,
                "aggregate {phase:?} mask has {actual} occurrences; expected {expected}"
            ),
            ErrorKind::WrongTheory => {
                f.write_str("aggregate interpretation belongs to another theory")
            }
            ErrorKind::Canonical(error) => error.fmt(f),
            ErrorKind::Catalog(error) => error.fmt(f),
            ErrorKind::Overflow => {
                f.write_str("native aggregate arithmetic or accounting overflow")
            }
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.kind {
            ErrorKind::Stopped(stop) => Some(stop),
            ErrorKind::Canonical(error) => Some(error),
            ErrorKind::Catalog(error) => Some(error),
            _ => None,
        }
    }
}

/// Immutable finite operation owning one canonical vocabulary and ID-only metadata.
/// Owned descriptions are construction input only; execution borrows canonical terms.
#[derive(Debug)]
pub struct Group {
    vocabulary: Vocabulary,
    data: GroupData,
}

/// Complete tuple and guard occurrences over one externally retained vocabulary.
/// This owns only metadata, never the supplied catalog or a typed value payload.
/// Bind to a compatible exact prefix before execution; no atom or rule is invented.
#[derive(Debug)]
pub struct GroupData {
    theory: Theory,
    function: Function,
    components: TemplateComponents,
    tuples: Vec<TupleData>,
    guards: Vec<GuardData>,
    first_costs: Vec<u64>,
    guard_costs: Vec<u64>,
    statistics: Statistics,
}
#[derive(Debug)]
struct TupleData {
    key: std::ops::Range<usize>,
    condition: usize,
}
#[derive(Clone, Copy, Debug)]
enum BoundData {
    Integer(i128),
    Term(usize),
}
#[derive(Clone, Copy, Debug)]
struct GuardData {
    comparison: AggregateComparison,
    bound: BoundData,
}

/// One checked aggregate metadata owner and canonical read prefix. Copy borrows
/// the same operation; it does not duplicate keys, guards or vocabulary storage.
#[derive(Clone, Copy, Debug)]
pub struct GroupRef<'a> {
    data: &'a GroupData,
    components: TemplateComponentsRef<'a>,
}

/// Ordered complete key values, including a genuine empty key. This view owns
/// no payload and its coordinates are local occurrences, never canonical IDs.
#[derive(Clone, Copy, Debug)]
pub struct Key<'a>(zetesis_core::PatternTerms<'a>);
impl<'a> Key<'a> {
    /// Number of original key components.
    #[must_use]
    pub fn len(self) -> usize {
        self.0.len()
    }
    /// Whether this is an empty complete key.
    #[must_use]
    pub fn is_empty(self) -> bool {
        self.0.is_empty()
    }
    /// Borrow an original component, or None outside the key.
    /// # Panics
    /// Panics if internal admission produces a variable in this closed key.
    /// Public constructors admit constants only.
    #[must_use]
    pub fn at(self, index: usize) -> Option<TermRef<'a>> {
        self.0.at(index).map(closed_term)
    }
    /// First component; empty keys remain distinct from any term sentinel.
    #[must_use]
    pub fn first(self) -> Option<TermRef<'a>> {
        self.at(0)
    }
    /// Original component order, allocating no storage.
    /// # Panics
    /// Panics if internal admission produces a variable in this closed key.
    /// Public constructors admit constants only.
    #[must_use]
    pub fn iter(self) -> impl ExactSizeIterator<Item = TermRef<'a>> + DoubleEndedIterator {
        self.0.iter().map(closed_term)
    }
}

/// Aggregate keys and term guards share the admitted closed-term projection.
fn closed_term(term: TemplateTerm<'_>) -> TermRef<'_> {
    match term {
        TemplateTerm::Constant(value) => value,
        TemplateTerm::Variable(_) => unreachable!("aggregate components are admitted constants"),
    }
}

/// Original tuple occurrences and checked eligibility-node coordinates.
#[derive(Clone, Copy, Debug)]
pub struct Tuples<'a>(GroupRef<'a>);
impl<'a> Tuples<'a> {
    /// Complete tuple count, including neutral/empty tuples.
    #[must_use]
    pub fn len(self) -> usize {
        self.0.data.tuples.len()
    }
    /// Whether no tuples were supplied.
    #[must_use]
    pub fn is_empty(self) -> bool {
        self.0.data.tuples.is_empty()
    }
    /// Borrow one tuple by original occurrence position.
    /// # Panics
    /// Panics if an internally admitted key range is outside its components.
    /// Public admission establishes these ranges before publishing the group.
    #[must_use]
    pub fn at(self, index: usize) -> Option<Tuple<Key<'a>>> {
        self.0
            .data
            .tuples
            .get(index)
            .map(|tuple| self.0.tuple(tuple))
    }
    /// Original occurrence order; equal conditions remain separate occurrences.
    /// # Panics
    /// Panics if an internally admitted key range is outside its components,
    /// as described by [`Self::at`].
    #[must_use]
    pub fn iter(self) -> impl ExactSizeIterator<Item = Tuple<Key<'a>>> + DoubleEndedIterator {
        self.0
            .data
            .tuples
            .iter()
            .map(move |tuple| self.0.tuple(tuple))
    }
}

impl Group {
    /// Import already-coalesced owned keys once into canonical storage.
    /// Original tuple/guard order is preserved and equal complete keys are refused.
    /// Existing logical transferred-storage limits remain separate from the new
    /// named canonical-storage ceiling. Prior input allocation remains the caller's cost.
    /// # Errors
    /// Refuses duplicate keys, condition IDs, resources, control or allocation.
    pub fn new(
        theory: &Theory,
        function: Function,
        tuples: Vec<Tuple>,
        guards: Vec<Guard>,
        limits: AdmissionLimits,
        cancellation: &Cancellation,
    ) -> Result<Self, Error> {
        admission::build(theory, function, tuples, guards, limits, cancellation)
    }
    /// Borrow the admitted operation without copying logical payload.
    /// # Panics
    /// Panics if internal group metadata no longer belongs to the stored
    /// vocabulary prefix. [`Self::new`] establishes that immutable pairing.
    #[must_use]
    pub fn view(&self) -> GroupRef<'_> {
        self.data
            .bind_with(self.vocabulary.read(), || {
                Ok::<_, std::convert::Infallible>(())
            })
            .expect("owned aggregate metadata belongs to its sealed vocabulary")
    }
    /// Immutable formula subject defining eligibility-node identities.
    #[must_use]
    pub fn theory(&self) -> &Theory {
        &self.data.theory
    }
    /// Original complete keys and condition IDs in input order.
    #[must_use]
    pub fn tuples(&self) -> Tuples<'_> {
        self.view().tuples()
    }
    /// Guard occurrences in input order, borrowing term-valued bounds.
    /// # Panics
    /// Panics if the internal vocabulary pairing or closed guard coordinates
    /// violate the invariants established by admission.
    #[must_use]
    pub fn guards(&self) -> impl ExactSizeIterator<Item = Guard<TermRef<'_>>> {
        self.view().guards()
    }
    /// Aggregate function used by every reduction.
    #[must_use]
    pub const fn function(&self) -> Function {
        self.data.function
    }
    /// Completed admission accounting with logical and named capacity dimensions.
    #[must_use]
    pub const fn statistics(&self) -> Statistics {
        self.data.statistics
    }
}
impl GroupData {
    /// Admit canonical keys over an existing authority without importing payload.
    /// `max_bytes` measures a flat logical carrier (exact input key capacities,
    /// minimal expanded term/spelling lengths); `max_storage_bytes` covers only
    /// this metadata and duplicate scratch. The caller retains/accounts the catalog.
    /// # Errors
    /// Refuses foreign/uninterned terms, prefixes, shape, duplicates or resources.
    pub fn new<'a>(
        theory: &Theory,
        function: Function,
        read: CatalogRead<'a>,
        tuples: Vec<Tuple<Vec<TermRef<'a>>>>,
        guards: Vec<Guard<TermRef<'a>>>,
        limits: AdmissionLimits,
        cancellation: &Cancellation,
    ) -> Result<Self, Error> {
        admission::borrowed(theory, function, read, tuples, guards, limits, cancellation)
    }
    /// Bind scope and exact required prefix before borrowing any values.
    /// # Errors
    /// Refuses a foreign/older catalog or preserves the caller's stop value.
    pub fn bind_with<'a, E>(
        &'a self,
        read: CatalogRead<'a>,
        before: impl FnMut() -> Result<(), E>,
    ) -> Result<GroupRef<'a>, zetesis_core::TemplateCatalogFailure<E>> {
        self.components
            .bind_with(read, before)
            .map(|components| GroupRef {
                data: self,
                components,
            })
    }
    /// Completed admission accounting; borrowed canonical owner is excluded.
    #[must_use]
    pub const fn statistics(&self) -> Statistics {
        self.statistics
    }
}
impl<'a> From<&'a Group> for GroupRef<'a> {
    fn from(group: &'a Group) -> Self {
        group.view()
    }
}
impl<'a> GroupRef<'a> {
    /// Whether two views denote the same complete aggregate occurrence owner.
    /// Binding already authenticates vocabulary scope and required prefix.
    #[must_use]
    pub fn same_group(self, other: Self) -> bool {
        std::ptr::eq(self.data, other.data)
    }
    /// Original immutable theory, not a source-completeness certificate.
    #[must_use]
    pub fn theory(self) -> &'a Theory {
        &self.data.theory
    }
    /// Ordered original tuple occurrences.
    #[must_use]
    pub fn tuples(self) -> Tuples<'a> {
        Tuples(self)
    }
    /// Borrow one guard occurrence, without an owned term reconstruction.
    /// # Panics
    /// Panics if an internally admitted term guard does not name a constant in
    /// the bound component prefix. Public admission establishes that invariant.
    #[must_use]
    pub fn guard(self, index: usize) -> Option<Guard<TermRef<'a>>> {
        self.data
            .guards
            .get(index)
            .map(|guard| self.guard_data(*guard))
    }
    /// Ordered guards, with no owned term reconstruction.
    /// # Panics
    /// Panics if an internally admitted term guard does not name a constant,
    /// as described by [`Self::guard`].
    #[must_use]
    pub fn guards(self) -> impl ExactSizeIterator<Item = Guard<TermRef<'a>>> {
        self.data
            .guards
            .iter()
            .map(move |guard| self.guard_data(*guard))
    }
    /// Decode one admitted tuple occurrence for both indexed and sequential reads.
    fn tuple(self, tuple: &TupleData) -> Tuple<Key<'a>> {
        Tuple {
            key: Key(self
                .components
                .terms()
                .slice(tuple.key.clone())
                .expect("admitted key range")),
            condition: tuple.condition,
        }
    }
    /// Decode one admitted guard occurrence for both indexed and sequential reads.
    fn guard_data(self, guard: GuardData) -> Guard<TermRef<'a>> {
        Guard {
            comparison: guard.comparison,
            bound: match guard.bound {
                BoundData::Integer(value) => Bound::Integer(value),
                BoundData::Term(index) => Bound::Term(closed_term(
                    self.components
                        .term(index)
                        .expect("admitted guard coordinate"),
                )),
            },
        }
    }
    /// Aggregate function shared by every physical execution.
    #[must_use]
    pub const fn function(self) -> Function {
        self.data.function
    }
}

struct Work<'a> {
    maximum: u64,
    cancellation: &'a Cancellation,
    statistics: Statistics,
}
impl Work<'_> {
    fn poll(&self) -> Result<(), ErrorKind> {
        self.cancellation.poll().map_err(ErrorKind::Stopped)
    }

    fn charge(&mut self, amount: u64) -> Result<(), ErrorKind> {
        self.poll()?;
        let remaining = self.maximum - self.statistics.work;
        if amount > remaining {
            return Err(ErrorKind::Limit(Resource::Work));
        }
        self.statistics.work += amount;
        Ok(())
    }

    fn failure(&self, kind: ErrorKind) -> Error {
        Error {
            kind,
            statistics: self.statistics,
        }
    }
}

fn storage<T>(count: usize) -> Result<Vec<T>, ErrorKind> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| ErrorKind::Stopped(Stop::Allocation))?;
    Ok(values)
}

fn bytes<T>(count: usize) -> Result<u64, ErrorKind> {
    let bytes = count
        .checked_mul(size_of::<T>())
        .ok_or(ErrorKind::Overflow)?;
    u64::try_from(bytes).map_err(|_| ErrorKind::Overflow)
}

fn add(left: u64, right: u64) -> Result<u64, ErrorKind> {
    left.checked_add(right).ok_or(ErrorKind::Overflow)
}
